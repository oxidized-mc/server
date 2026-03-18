//! Serverbound client information — the client sends its display settings
//! and preferences during configuration.
//!
//! Corresponds to `net.minecraft.network.protocol.common.ServerboundClientInformationPacket`
//! wrapping `net.minecraft.server.level.ClientInformation`.

use bytes::{Buf, BufMut, Bytes, BytesMut};
use thiserror::Error;

use crate::codec::types::{self, TypeError};
use crate::codec::varint;
use crate::types::{ChatVisibility, HumanoidArm, ParticleStatus};

/// Maximum length of the language string (in characters).
const MAX_LANGUAGE_LENGTH: usize = 16;

/// Errors from decoding a [`ServerboundClientInformationPacket`].
#[derive(Debug, Error)]
pub enum ClientInformationError {
    /// A wire type could not be decoded.
    #[error("{0}")]
    Type(#[from] TypeError),

    /// VarInt decoding failed.
    #[error("varint error: {0}")]
    VarInt(#[from] varint::VarIntError),

    /// Invalid chat visibility value.
    #[error("invalid chat visibility: {0}")]
    InvalidChatVisibility(i32),

    /// Invalid main hand value.
    #[error("invalid main hand: {0}")]
    InvalidMainHand(i32),

    /// Invalid particle status value.
    #[error("invalid particle status: {0}")]
    InvalidParticleStatus(i32),

    /// Not enough bytes remaining in the buffer.
    #[error("unexpected end of buffer (need {need}, have {have})")]
    UnexpectedEof {
        /// Bytes needed.
        need: usize,
        /// Bytes remaining.
        have: usize,
    },
}

/// The client's display settings and preferences, sent during configuration.
///
/// This data is stored per-player and influences view distance, chat
/// filtering, particle rendering, and skin part visibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientInformation {
    /// The client's language (e.g. `"en_us"`). Max 16 characters.
    pub language: String,
    /// Render distance in chunks (2–32). Sent as a signed byte on the wire.
    pub view_distance: i8,
    /// Which chat messages the client wants to see.
    pub chat_visibility: ChatVisibility,
    /// Whether the client wants coloured chat messages.
    pub chat_colors: bool,
    /// Bitmask of displayed skin parts (cape, jacket, sleeves, pants, hat).
    pub model_customisation: u8,
    /// Which hand the player uses as their main hand.
    pub main_hand: HumanoidArm,
    /// Whether the client has text filtering enabled.
    pub text_filtering: bool,
    /// Whether the player allows being listed in the server's player list.
    pub allows_listing: bool,
    /// The client's particle rendering level.
    pub particle_status: ParticleStatus,
}

impl ClientInformation {
    /// Creates a [`ClientInformation`] with vanilla default values.
    ///
    /// Matches `ClientInformation.createDefault()` in the Java reference.
    pub fn create_default() -> Self {
        Self {
            language: "en_us".to_string(),
            view_distance: 2,
            chat_visibility: ChatVisibility::Full,
            chat_colors: true,
            model_customisation: 0,
            main_hand: HumanoidArm::DEFAULT,
            text_filtering: false,
            allows_listing: false,
            particle_status: ParticleStatus::All,
        }
    }

    /// Decodes a [`ClientInformation`] from the wire format.
    ///
    /// # Errors
    ///
    /// Returns [`ClientInformationError`] if the buffer is truncated or
    /// contains invalid enum values.
    pub fn read(buf: &mut Bytes) -> Result<Self, ClientInformationError> {
        let language = types::read_string(buf, MAX_LANGUAGE_LENGTH)?;

        if buf.remaining() < 1 {
            return Err(ClientInformationError::UnexpectedEof { need: 1, have: 0 });
        }
        let view_distance = buf.get_i8();

        let chat_vis_id = varint::read_varint_buf(buf)?;
        let chat_visibility = ChatVisibility::by_id(chat_vis_id)
            .ok_or(ClientInformationError::InvalidChatVisibility(chat_vis_id))?;

        let chat_colors = types::read_bool(buf)?;

        if buf.remaining() < 1 {
            return Err(ClientInformationError::UnexpectedEof { need: 1, have: 0 });
        }
        let model_customisation = buf.get_u8();

        let main_hand_id = varint::read_varint_buf(buf)?;
        let main_hand = HumanoidArm::by_id(main_hand_id)
            .ok_or(ClientInformationError::InvalidMainHand(main_hand_id))?;

        let text_filtering = types::read_bool(buf)?;
        let allows_listing = types::read_bool(buf)?;

        let particle_id = varint::read_varint_buf(buf)?;
        let particle_status = ParticleStatus::by_id(particle_id)
            .ok_or(ClientInformationError::InvalidParticleStatus(particle_id))?;

        Ok(Self {
            language,
            view_distance,
            chat_visibility,
            chat_colors,
            model_customisation,
            main_hand,
            text_filtering,
            allows_listing,
            particle_status,
        })
    }

    /// Encodes this [`ClientInformation`] to the wire format.
    pub fn write(&self, buf: &mut BytesMut) {
        types::write_string(buf, &self.language);
        buf.put_i8(self.view_distance);
        self.chat_visibility.write(buf);
        types::write_bool(buf, self.chat_colors);
        buf.put_u8(self.model_customisation);
        self.main_hand.write(buf);
        types::write_bool(buf, self.text_filtering);
        types::write_bool(buf, self.allows_listing);
        self.particle_status.write(buf);
    }
}

/// Serverbound packet `0x00` in the CONFIGURATION state — client information.
///
/// The client sends its display settings and preferences during configuration.
/// This packet may be sent at any point during the configuration state, and
/// can be sent again if the player changes settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundClientInformationPacket {
    /// The client's information.
    pub information: ClientInformation,
}

impl ServerboundClientInformationPacket {
    /// Packet ID in the CONFIGURATION state.
    pub const PACKET_ID: i32 = 0x00;

    /// Decodes from the raw packet body.
    ///
    /// # Errors
    ///
    /// Returns [`ClientInformationError`] if the buffer is truncated or
    /// contains invalid values.
    pub fn decode(mut data: Bytes) -> Result<Self, ClientInformationError> {
        let information = ClientInformation::read(&mut data)?;
        Ok(Self { information })
    }

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        self.information.write(&mut buf);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Builds a default `ClientInformation` for tests.
    fn default_info() -> ClientInformation {
        ClientInformation::create_default()
    }

    /// Builds a fully customised `ClientInformation` for tests.
    fn custom_info() -> ClientInformation {
        ClientInformation {
            language: "de_de".to_string(),
            view_distance: 16,
            chat_visibility: ChatVisibility::System,
            chat_colors: false,
            model_customisation: 0x7F, // all skin parts
            main_hand: HumanoidArm::Left,
            text_filtering: true,
            allows_listing: true,
            particle_status: ParticleStatus::Minimal,
        }
    }

    // ── ClientInformation roundtrip ─────────────────────────────────

    #[test]
    fn test_client_information_roundtrip_default() {
        let info = default_info();
        let mut buf = BytesMut::new();
        info.write(&mut buf);
        let mut data = buf.freeze();
        let decoded = ClientInformation::read(&mut data).unwrap();
        assert_eq!(decoded.language, "en_us");
        assert_eq!(decoded.view_distance, 2);
        assert_eq!(decoded.chat_visibility, ChatVisibility::Full);
        assert!(decoded.chat_colors);
        assert_eq!(decoded.model_customisation, 0);
        assert_eq!(decoded.main_hand, HumanoidArm::Right);
        assert!(!decoded.text_filtering);
        assert!(!decoded.allows_listing);
        assert_eq!(decoded.particle_status, ParticleStatus::All);
    }

    #[test]
    fn test_client_information_roundtrip_custom() {
        let info = custom_info();
        let mut buf = BytesMut::new();
        info.write(&mut buf);
        let mut data = buf.freeze();
        let decoded = ClientInformation::read(&mut data).unwrap();
        assert_eq!(decoded, info);
    }

    // ── Packet roundtrip ────────────────────────────────────────────

    #[test]
    fn test_packet_roundtrip_default() {
        let pkt = ServerboundClientInformationPacket {
            information: default_info(),
        };
        let encoded = pkt.encode();
        let decoded = ServerboundClientInformationPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_roundtrip_custom() {
        let pkt = ServerboundClientInformationPacket {
            information: custom_info(),
        };
        let encoded = pkt.encode();
        let decoded = ServerboundClientInformationPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    // ── Field-level assertions ──────────────────────────────────────

    #[test]
    fn test_packet_all_fields_custom() {
        let pkt = ServerboundClientInformationPacket {
            information: custom_info(),
        };
        let encoded = pkt.encode();
        let decoded = ServerboundClientInformationPacket::decode(encoded.freeze()).unwrap();
        let info = &decoded.information;
        assert_eq!(info.language, "de_de");
        assert_eq!(info.view_distance, 16);
        assert_eq!(info.chat_visibility, ChatVisibility::System);
        assert!(!info.chat_colors);
        assert_eq!(info.model_customisation, 0x7F);
        assert_eq!(info.main_hand, HumanoidArm::Left);
        assert!(info.text_filtering);
        assert!(info.allows_listing);
        assert_eq!(info.particle_status, ParticleStatus::Minimal);
    }

    // ── create_default ──────────────────────────────────────────────

    #[test]
    fn test_create_default_matches_vanilla() {
        let info = ClientInformation::create_default();
        assert_eq!(info.language, "en_us");
        assert_eq!(info.view_distance, 2);
        assert_eq!(info.chat_visibility, ChatVisibility::Full);
        assert!(info.chat_colors);
        assert_eq!(info.model_customisation, 0);
        assert_eq!(info.main_hand, HumanoidArm::Right);
        assert!(!info.text_filtering);
        assert!(!info.allows_listing);
        assert_eq!(info.particle_status, ParticleStatus::All);
    }

    // ── Packet ID ───────────────────────────────────────────────────

    #[test]
    fn test_packet_id() {
        assert_eq!(ServerboundClientInformationPacket::PACKET_ID, 0x00);
    }

    // ── Error cases ─────────────────────────────────────────────────

    #[test]
    fn test_decode_empty_buffer() {
        let data = Bytes::new();
        assert!(ServerboundClientInformationPacket::decode(data).is_err());
    }

    #[test]
    fn test_decode_truncated_after_language() {
        let info = default_info();
        let mut buf = BytesMut::new();
        types::write_string(&mut buf, &info.language);
        // Missing everything after language
        let data = buf.freeze();
        assert!(ServerboundClientInformationPacket::decode(data).is_err());
    }

    #[test]
    fn test_decode_invalid_chat_visibility() {
        let mut buf = BytesMut::new();
        types::write_string(&mut buf, "en_us");
        buf.put_i8(2); // view_distance
        varint::write_varint_buf(99, &mut buf); // invalid chat visibility
        types::write_bool(&mut buf, true); // chat_colors
        buf.put_u8(0); // model_customisation
        varint::write_varint_buf(1, &mut buf); // main_hand
        types::write_bool(&mut buf, false); // text_filtering
        types::write_bool(&mut buf, false); // allows_listing
        varint::write_varint_buf(0, &mut buf); // particle_status
        let err = ServerboundClientInformationPacket::decode(buf.freeze()).unwrap_err();
        assert!(matches!(
            err,
            ClientInformationError::InvalidChatVisibility(99)
        ));
    }

    #[test]
    fn test_decode_invalid_main_hand() {
        let mut buf = BytesMut::new();
        types::write_string(&mut buf, "en_us");
        buf.put_i8(2);
        varint::write_varint_buf(0, &mut buf); // chat visibility
        types::write_bool(&mut buf, true);
        buf.put_u8(0);
        varint::write_varint_buf(42, &mut buf); // invalid main hand
        types::write_bool(&mut buf, false);
        types::write_bool(&mut buf, false);
        varint::write_varint_buf(0, &mut buf);
        let err = ServerboundClientInformationPacket::decode(buf.freeze()).unwrap_err();
        assert!(matches!(
            err,
            ClientInformationError::InvalidMainHand(42)
        ));
    }

    #[test]
    fn test_decode_invalid_particle_status() {
        let mut buf = BytesMut::new();
        types::write_string(&mut buf, "en_us");
        buf.put_i8(2);
        varint::write_varint_buf(0, &mut buf);
        types::write_bool(&mut buf, true);
        buf.put_u8(0);
        varint::write_varint_buf(1, &mut buf);
        types::write_bool(&mut buf, false);
        types::write_bool(&mut buf, false);
        varint::write_varint_buf(77, &mut buf); // invalid particle status
        let err = ServerboundClientInformationPacket::decode(buf.freeze()).unwrap_err();
        assert!(matches!(
            err,
            ClientInformationError::InvalidParticleStatus(77)
        ));
    }

    // ── Negative view distance ──────────────────────────────────────

    #[test]
    fn test_negative_view_distance_roundtrip() {
        let info = ClientInformation {
            view_distance: -1,
            ..default_info()
        };
        let mut buf = BytesMut::new();
        info.write(&mut buf);
        let mut data = buf.freeze();
        let decoded = ClientInformation::read(&mut data).unwrap();
        assert_eq!(decoded.view_distance, -1);
    }

    // ── Max view distance ───────────────────────────────────────────

    #[test]
    fn test_max_view_distance_roundtrip() {
        let info = ClientInformation {
            view_distance: 32,
            ..default_info()
        };
        let mut buf = BytesMut::new();
        info.write(&mut buf);
        let mut data = buf.freeze();
        let decoded = ClientInformation::read(&mut data).unwrap();
        assert_eq!(decoded.view_distance, 32);
    }

    // ── Model customisation bitmask ─────────────────────────────────

    #[test]
    fn test_model_customisation_all_bits_roundtrip() {
        let info = ClientInformation {
            model_customisation: 0xFF,
            ..default_info()
        };
        let mut buf = BytesMut::new();
        info.write(&mut buf);
        let mut data = buf.freeze();
        let decoded = ClientInformation::read(&mut data).unwrap();
        assert_eq!(decoded.model_customisation, 0xFF);
    }
}
