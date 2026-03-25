//! Serverbound client information — the client sends its display settings
//! and preferences during configuration.
//!
//! Corresponds to `net.minecraft.network.protocol.common.ServerboundClientInformationPacket`
//! wrapping `net.minecraft.server.level.ClientInformation`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;
use crate::codec::varint;
use crate::types::{ChatVisibility, HumanoidArm, ParticleStatus};

/// Maximum length of the language string (in characters).
const MAX_LANGUAGE_LENGTH: usize = 16;

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
    pub has_chat_colors: bool,
    /// Bitmask of displayed skin parts (cape, jacket, sleeves, pants, hat).
    pub model_customisation: u8,
    /// Which hand the player uses as their main hand.
    pub main_hand: HumanoidArm,
    /// Whether the client has text filtering enabled.
    pub has_text_filtering: bool,
    /// Whether the player allows being listed in the server's player list.
    pub is_listing_allowed: bool,
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
            has_chat_colors: true,
            model_customisation: 0,
            main_hand: HumanoidArm::DEFAULT,
            has_text_filtering: false,
            is_listing_allowed: false,
            particle_status: ParticleStatus::All,
        }
    }

    /// Decodes a [`ClientInformation`] from the wire format.
    ///
    /// # Errors
    ///
    /// Returns [`PacketDecodeError`] if the buffer is truncated or
    /// contains invalid enum values.
    pub fn read(buf: &mut Bytes) -> Result<Self, PacketDecodeError> {
        let language = types::read_string(buf, MAX_LANGUAGE_LENGTH)?;

        if buf.remaining() < 1 {
            return Err(PacketDecodeError::InvalidData(
                "unexpected eof: need 1, have 0".to_string(),
            ));
        }
        let view_distance = buf.get_i8();

        let chat_vis_id = varint::read_varint_buf(buf)?;
        let chat_visibility = ChatVisibility::by_id(chat_vis_id).ok_or_else(|| {
            PacketDecodeError::InvalidData(format!("invalid chat visibility: {chat_vis_id}"))
        })?;

        let has_chat_colors = types::read_bool(buf)?;

        if buf.remaining() < 1 {
            return Err(PacketDecodeError::InvalidData(
                "unexpected eof: need 1, have 0".to_string(),
            ));
        }
        let model_customisation = buf.get_u8();

        let main_hand_id = varint::read_varint_buf(buf)?;
        let main_hand = HumanoidArm::by_id(main_hand_id).ok_or_else(|| {
            PacketDecodeError::InvalidData(format!("invalid main hand: {main_hand_id}"))
        })?;

        let has_text_filtering = types::read_bool(buf)?;
        let is_listing_allowed = types::read_bool(buf)?;

        let particle_id = varint::read_varint_buf(buf)?;
        let particle_status = ParticleStatus::by_id(particle_id).ok_or_else(|| {
            PacketDecodeError::InvalidData(format!("invalid particle status: {particle_id}"))
        })?;

        Ok(Self {
            language,
            view_distance,
            chat_visibility,
            has_chat_colors,
            model_customisation,
            main_hand,
            has_text_filtering,
            is_listing_allowed,
            particle_status,
        })
    }

    /// Encodes this [`ClientInformation`] to the wire format.
    pub fn write(&self, buf: &mut BytesMut) {
        types::write_string(buf, &self.language);
        buf.put_i8(self.view_distance);
        self.chat_visibility.write(buf);
        types::write_bool(buf, self.has_chat_colors);
        buf.put_u8(self.model_customisation);
        self.main_hand.write(buf);
        types::write_bool(buf, self.has_text_filtering);
        types::write_bool(buf, self.is_listing_allowed);
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

impl Packet for ServerboundClientInformationPacket {
    const PACKET_ID: i32 = 0x00;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let information = ClientInformation::read(&mut data)?;
        Ok(Self { information })
    }

    fn encode(&self) -> BytesMut {
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
            has_chat_colors: false,
            model_customisation: 0x7F, // all skin parts
            main_hand: HumanoidArm::Left,
            has_text_filtering: true,
            is_listing_allowed: true,
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
        assert!(decoded.has_chat_colors);
        assert_eq!(decoded.model_customisation, 0);
        assert_eq!(decoded.main_hand, HumanoidArm::Right);
        assert!(!decoded.has_text_filtering);
        assert!(!decoded.is_listing_allowed);
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
        assert_packet_roundtrip!(ServerboundClientInformationPacket {
            information: default_info(),
        });
    }

    #[test]
    fn test_packet_roundtrip_custom() {
        assert_packet_roundtrip!(ServerboundClientInformationPacket {
            information: custom_info(),
        });
    }

    // ── Field-level assertions ──────────────────────────────────────

    #[test]
    fn test_packet_all_fields_custom() {
        let pkt = ServerboundClientInformationPacket {
            information: custom_info(),
        };
        let encoded = Packet::encode(&pkt);
        let decoded =
            <ServerboundClientInformationPacket as Packet>::decode(encoded.freeze()).unwrap();
        let info = &decoded.information;
        assert_eq!(info.language, "de_de");
        assert_eq!(info.view_distance, 16);
        assert_eq!(info.chat_visibility, ChatVisibility::System);
        assert!(!info.has_chat_colors);
        assert_eq!(info.model_customisation, 0x7F);
        assert_eq!(info.main_hand, HumanoidArm::Left);
        assert!(info.has_text_filtering);
        assert!(info.is_listing_allowed);
        assert_eq!(info.particle_status, ParticleStatus::Minimal);
    }

    // ── create_default ──────────────────────────────────────────────

    #[test]
    fn test_create_default_matches_vanilla() {
        let info = ClientInformation::create_default();
        assert_eq!(info.language, "en_us");
        assert_eq!(info.view_distance, 2);
        assert_eq!(info.chat_visibility, ChatVisibility::Full);
        assert!(info.has_chat_colors);
        assert_eq!(info.model_customisation, 0);
        assert_eq!(info.main_hand, HumanoidArm::Right);
        assert!(!info.has_text_filtering);
        assert!(!info.is_listing_allowed);
        assert_eq!(info.particle_status, ParticleStatus::All);
    }

    // ── Packet ID ───────────────────────────────────────────────────

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ServerboundClientInformationPacket, 0x00);
    }

    // ── Error cases ─────────────────────────────────────────────────

    #[test]
    fn test_decode_empty_buffer() {
        let data = Bytes::new();
        assert!(<ServerboundClientInformationPacket as Packet>::decode(data).is_err());
    }

    #[test]
    fn test_decode_truncated_after_language() {
        let info = default_info();
        let mut buf = BytesMut::new();
        types::write_string(&mut buf, &info.language);
        // Missing everything after language
        let data = buf.freeze();
        assert!(<ServerboundClientInformationPacket as Packet>::decode(data).is_err());
    }

    #[test]
    fn test_decode_invalid_chat_visibility() {
        let mut buf = BytesMut::new();
        types::write_string(&mut buf, "en_us");
        buf.put_i8(2); // view_distance
        varint::write_varint_buf(99, &mut buf); // invalid chat visibility
        types::write_bool(&mut buf, true); // has_chat_colors
        buf.put_u8(0); // model_customisation
        varint::write_varint_buf(1, &mut buf); // main_hand
        types::write_bool(&mut buf, false); // has_text_filtering
        types::write_bool(&mut buf, false); // is_listing_allowed
        varint::write_varint_buf(0, &mut buf); // particle_status
        let err = <ServerboundClientInformationPacket as Packet>::decode(buf.freeze()).unwrap_err();
        assert!(
            matches!(err, PacketDecodeError::InvalidData(ref msg) if msg.contains("invalid chat visibility: 99"))
        );
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
        let err = <ServerboundClientInformationPacket as Packet>::decode(buf.freeze()).unwrap_err();
        assert!(
            matches!(err, PacketDecodeError::InvalidData(ref msg) if msg.contains("invalid main hand: 42"))
        );
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
        let err = <ServerboundClientInformationPacket as Packet>::decode(buf.freeze()).unwrap_err();
        assert!(
            matches!(err, PacketDecodeError::InvalidData(ref msg) if msg.contains("invalid particle status: 77"))
        );
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
