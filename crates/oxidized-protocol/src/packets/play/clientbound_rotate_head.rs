//! Clientbound head rotation packet.
//!
//! Sent to update an entity's head yaw independently of their body yaw.
//! Used for player and mob head-tracking behavior.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundRotateHeadPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;
use crate::codec::varint;

use super::clientbound_login::PlayPacketError;

/// Head rotation for a specific entity (0x53).
///
/// # Wire Format
///
/// | Field | Type |
/// |-------|------|
/// | entity_id | VarInt |
/// | head_yaw | u8 (packed rotation) |
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundRotateHeadPacket {
    /// Entity network ID.
    pub entity_id: i32,
    /// Packed head yaw (0–255 → 0–360°).
    pub head_yaw: u8,
}

impl ClientboundRotateHeadPacket {
    /// Packet ID in the PLAY state clientbound registry.
    pub const PACKET_ID: i32 = 0x53;

    /// Decodes from the raw packet body.
    pub fn decode(mut data: Bytes) -> Result<Self, PlayPacketError> {
        let entity_id = varint::read_varint_buf(&mut data)?;
        let head_yaw = types::read_u8(&mut data)?;
        Ok(Self {
            entity_id,
            head_yaw,
        })
    }

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(2);
        varint::write_varint_buf(self.entity_id, &mut buf);
        types::write_u8(&mut buf, self.head_yaw);
        buf
    }
}

impl Packet for ClientboundRotateHeadPacket {
    const PACKET_ID: i32 = 0x53;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let entity_id = varint::read_varint_buf(&mut data)?;
        let head_yaw = types::read_u8(&mut data)?;
        Ok(Self {
            entity_id,
            head_yaw,
        })
    }

    fn encode(&self) -> BytesMut {
        self.encode()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundRotateHeadPacket {
            entity_id: 42,
            head_yaw: 128, // ~180°
        };
        let encoded = pkt.encode();
        // VarInt(42)=1 byte + u8=1 byte = 2 bytes
        assert_eq!(encoded.len(), 2);
        let decoded = ClientboundRotateHeadPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_id() {
        assert_eq!(ClientboundRotateHeadPacket::PACKET_ID, 0x53);
    }

    #[test]
    fn test_large_entity_id() {
        let pkt = ClientboundRotateHeadPacket {
            entity_id: 300,
            head_yaw: 0,
        };
        let encoded = pkt.encode();
        // VarInt(300) = 2 bytes + u8 = 1 byte = 3 bytes
        assert_eq!(encoded.len(), 3);
        let decoded = ClientboundRotateHeadPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = ClientboundRotateHeadPacket {
            entity_id: 42,
            head_yaw: 128,
        };
        let encoded = Packet::encode(&pkt);
        let decoded = <ClientboundRotateHeadPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(<ClientboundRotateHeadPacket as Packet>::PACKET_ID, 0x53);
    }
}
