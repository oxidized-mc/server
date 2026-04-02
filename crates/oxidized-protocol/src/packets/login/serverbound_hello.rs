//! Serverbound login start — the client sends its username and profile UUID.
//!
//! Corresponds to `net.minecraft.network.protocol.login.ServerboundHelloPacket`.

use bytes::{Bytes, BytesMut};

use oxidized_codec::Packet;
use oxidized_codec::packet::PacketDecodeError;
use oxidized_codec::types;

/// Serverbound packet `0x00` in the LOGIN state — login start.
///
/// Sent by the client immediately after the handshake transitions to the LOGIN
/// state. Contains the player's chosen name and their Mojang profile UUID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundHelloPacket {
    /// The player's username (max 16 characters).
    pub name: String,
    /// The player's Mojang profile UUID.
    pub profile_id: uuid::Uuid,
}

impl Packet for ServerboundHelloPacket {
    const PACKET_ID: i32 = 0x00;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let name = types::read_string(&mut data, 16)?;
        let profile_id = types::read_uuid(&mut data)?;
        Ok(Self { name, profile_id })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        types::write_string(&mut buf, &self.name);
        types::write_uuid(&mut buf, &self.profile_id);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ServerboundHelloPacket {
            name: "Notch".to_string(),
            profile_id: uuid::Uuid::from_u128(0x1234_5678_9abc_def0_1234_5678_9abc_def0),
        };
        let encoded = pkt.encode();
        let decoded = ServerboundHelloPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_max_length_name() {
        let pkt = ServerboundHelloPacket {
            name: "A".repeat(16),
            profile_id: uuid::Uuid::nil(),
        };
        let encoded = pkt.encode();
        let decoded = ServerboundHelloPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ServerboundHelloPacket, 0x00);
    }
}
