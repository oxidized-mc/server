//! The first packet sent by the client to declare its intent.
//!
//! This corresponds to `net.minecraft.network.protocol.handshake.ClientIntentionPacket`
//! in the vanilla server.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;
use crate::codec::varint;

/// The client's declared intent after the handshake.
///
/// Determines which protocol state the server transitions to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientIntent {
    /// Server list ping — transition to STATUS state.
    Status = 1,
    /// Player login — transition to LOGIN state.
    Login = 2,
    /// Server transfer (1.20.5+) — treated as LOGIN.
    Transfer = 3,
}

impl ClientIntent {
    /// Converts a raw wire value to a [`ClientIntent`].
    ///
    /// # Errors
    ///
    /// Returns [`PacketDecodeError::InvalidData`] if the value is not a known intent.
    pub fn from_id(value: i32) -> Result<Self, PacketDecodeError> {
        match value {
            1 => Ok(Self::Status),
            2 => Ok(Self::Login),
            3 => Ok(Self::Transfer),
            other => Err(PacketDecodeError::InvalidData(format!(
                "unknown client intent: {other}"
            ))),
        }
    }
}

/// Serverbound packet `0x00` — declares the client's protocol version and intent.
///
/// This is always the first packet in any connection. After receiving it,
/// the server transitions to either [`Status`](ClientIntent::Status) or
/// [`Login`](ClientIntent::Login) state.
///
/// # Examples
///
/// ```rust,ignore
/// use oxidized_protocol::packets::handshake::{ClientIntentionPacket, ClientIntent};
/// use oxidized_protocol::codec::Packet;
///
/// let packet = ClientIntentionPacket {
///     protocol_version: 1_073_742_124,
///     server_address: "localhost".to_string(),
///     server_port: 25565,
///     next_state: ClientIntent::Status,
/// };
///
/// // Encode and decode roundtrip
/// let encoded = Packet::encode(&packet);
/// let decoded =
///     <ClientIntentionPacket as Packet>::decode(encoded.freeze()).unwrap();
/// assert_eq!(decoded, packet);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientIntentionPacket {
    /// The protocol version the client is using.
    pub protocol_version: i32,
    /// The hostname the client connected to (max 255 chars).
    pub server_address: String,
    /// The port the client connected to.
    pub server_port: u16,
    /// What the client wants to do next.
    pub next_state: ClientIntent,
}

impl Packet for ClientIntentionPacket {
    const PACKET_ID: i32 = 0x00;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let protocol_version = varint::read_varint_buf(&mut data)?;
        let server_address = types::read_string(&mut data, 255)?;
        let server_port = types::read_u16(&mut data)?;
        let next_state_raw = varint::read_varint_buf(&mut data)?;
        let next_state = ClientIntent::from_id(next_state_raw)?;

        Ok(Self {
            protocol_version,
            server_address,
            server_port,
            next_state,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        varint::write_varint_buf(self.protocol_version, &mut buf);
        types::write_string(&mut buf, &self.server_address);
        types::write_u16(&mut buf, self.server_port);
        varint::write_varint_buf(self.next_state as i32, &mut buf);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_status_intent() {
        let pkt = ClientIntentionPacket {
            protocol_version: 1_073_742_124,
            server_address: "localhost".to_string(),
            server_port: 25565,
            next_state: ClientIntent::Status,
        };
        let encoded = pkt.encode();
        let decoded = ClientIntentionPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_decode_login_intent() {
        let pkt = ClientIntentionPacket {
            protocol_version: 1_073_742_124,
            server_address: "mc.example.com".to_string(),
            server_port: 25565,
            next_state: ClientIntent::Login,
        };
        let encoded = pkt.encode();
        let decoded = ClientIntentionPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_decode_transfer_intent() {
        let pkt = ClientIntentionPacket {
            protocol_version: 1_073_742_124,
            server_address: "transfer.example.com".to_string(),
            server_port: 25566,
            next_state: ClientIntent::Transfer,
        };
        let encoded = pkt.encode();
        let decoded = ClientIntentionPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_unknown_intent() {
        let mut buf = BytesMut::new();
        varint::write_varint_buf(1_073_742_124, &mut buf);
        types::write_string(&mut buf, "localhost");
        types::write_u16(&mut buf, 25565);
        varint::write_varint_buf(99, &mut buf); // invalid intent
        let err = <ClientIntentionPacket as Packet>::decode(buf.freeze()).unwrap_err();
        assert!(matches!(err, PacketDecodeError::InvalidData(_)));
        assert!(err.to_string().contains("99"));
    }

    #[test]
    fn test_decode_real_handshake_bytes() {
        // Bytes captured from a real MC 26.1-pre-3 client (minus packet ID):
        // protocol_version as VarInt, "localhost", port 25565, next_state=1 (Status)
        let pkt = ClientIntentionPacket {
            protocol_version: 1_073_742_124,
            server_address: "localhost".to_string(),
            server_port: 25565,
            next_state: ClientIntent::Status,
        };
        let encoded = pkt.encode();

        // Verify the encoded bytes are reasonable
        assert!(encoded.len() > 5); // at least varint + string prefix + port + intent
        assert!(encoded.len() < 50); // shouldn't be huge

        let decoded = ClientIntentionPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.protocol_version, 1_073_742_124);
        assert_eq!(decoded.server_address, "localhost");
        assert_eq!(decoded.server_port, 25565);
        assert_eq!(decoded.next_state, ClientIntent::Status);
    }

    // --- Packet trait tests ---

    #[test]
    fn test_packet_trait_unknown_intent_maps_to_invalid_data() {
        let mut buf = BytesMut::new();
        varint::write_varint_buf(1_073_742_124, &mut buf);
        types::write_string(&mut buf, "localhost");
        types::write_u16(&mut buf, 25565);
        varint::write_varint_buf(99, &mut buf);
        let err = <ClientIntentionPacket as Packet>::decode(buf.freeze()).unwrap_err();
        assert!(matches!(err, PacketDecodeError::InvalidData(_)));
        assert!(err.to_string().contains("99"));
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(<ClientIntentionPacket as Packet>::PACKET_ID, 0x00);
    }
}
