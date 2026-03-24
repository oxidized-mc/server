//! Clientbound delta-encoded entity movement packets.
//!
//! Three variants: position-only, rotation-only, and both. Position deltas
//! are encoded as `i16` values scaled by 4096 (1/4096 of a block). Rotation
//! is packed as bytes (0–255 maps to 0–360°).
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundMoveEntityPacket`
//! inner classes (`Pos`, `PosRot`, `Rot`).

use bytes::{Bytes, BytesMut};

use crate::codec::types;
use crate::codec::varint;

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;

/// Position-only delta movement (0x35).
///
/// # Wire Format
///
/// | Field | Type |
/// |-------|------|
/// | entity_id | VarInt |
/// | dx | i16 |
/// | dy | i16 |
/// | dz | i16 |
/// | on_ground | bool |
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundMoveEntityPosPacket {
    /// Entity network ID.
    pub entity_id: i32,
    /// Delta X in 1/4096 block units.
    pub dx: i16,
    /// Delta Y in 1/4096 block units.
    pub dy: i16,
    /// Delta Z in 1/4096 block units.
    pub dz: i16,
    /// Whether the entity is on the ground.
    pub is_on_ground: bool,
}

impl Packet for ClientboundMoveEntityPosPacket {
    const PACKET_ID: i32 = 0x35;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let entity_id = varint::read_varint_buf(&mut data)?;
        let dx = types::read_i16(&mut data)?;
        let dy = types::read_i16(&mut data)?;
        let dz = types::read_i16(&mut data)?;
        let is_on_ground = types::read_bool(&mut data)?;
        Ok(Self {
            entity_id,
            dx,
            dy,
            dz,
            is_on_ground,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(8);
        varint::write_varint_buf(self.entity_id, &mut buf);
        types::write_i16(&mut buf, self.dx);
        types::write_i16(&mut buf, self.dy);
        types::write_i16(&mut buf, self.dz);
        types::write_bool(&mut buf, self.is_on_ground);
        buf
    }
}

/// Position + rotation delta movement (0x36).
///
/// # Wire Format
///
/// | Field | Type |
/// |-------|------|
/// | entity_id | VarInt |
/// | dx | i16 |
/// | dy | i16 |
/// | dz | i16 |
/// | yaw | u8 (packed) |
/// | pitch | u8 (packed) |
/// | on_ground | bool |
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundMoveEntityPosRotPacket {
    /// Entity network ID.
    pub entity_id: i32,
    /// Delta X in 1/4096 block units.
    pub dx: i16,
    /// Delta Y in 1/4096 block units.
    pub dy: i16,
    /// Delta Z in 1/4096 block units.
    pub dz: i16,
    /// Packed yaw (0–255 → 0–360°).
    pub yaw: u8,
    /// Packed pitch (0–255 → 0–360°).
    pub pitch: u8,
    /// Whether the entity is on the ground.
    pub is_on_ground: bool,
}

impl Packet for ClientboundMoveEntityPosRotPacket {
    const PACKET_ID: i32 = 0x36;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let entity_id = varint::read_varint_buf(&mut data)?;
        let dx = types::read_i16(&mut data)?;
        let dy = types::read_i16(&mut data)?;
        let dz = types::read_i16(&mut data)?;
        let yaw = types::read_u8(&mut data)?;
        let pitch = types::read_u8(&mut data)?;
        let is_on_ground = types::read_bool(&mut data)?;
        Ok(Self {
            entity_id,
            dx,
            dy,
            dz,
            yaw,
            pitch,
            is_on_ground,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(10);
        varint::write_varint_buf(self.entity_id, &mut buf);
        types::write_i16(&mut buf, self.dx);
        types::write_i16(&mut buf, self.dy);
        types::write_i16(&mut buf, self.dz);
        types::write_u8(&mut buf, self.yaw);
        types::write_u8(&mut buf, self.pitch);
        types::write_bool(&mut buf, self.is_on_ground);
        buf
    }
}

/// Rotation-only delta movement (0x38).
///
/// # Wire Format
///
/// | Field | Type |
/// |-------|------|
/// | entity_id | VarInt |
/// | yaw | u8 (packed) |
/// | pitch | u8 (packed) |
/// | on_ground | bool |
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundMoveEntityRotPacket {
    /// Entity network ID.
    pub entity_id: i32,
    /// Packed yaw (0–255 → 0–360°).
    pub yaw: u8,
    /// Packed pitch (0–255 → 0–360°).
    pub pitch: u8,
    /// Whether the entity is on the ground.
    pub is_on_ground: bool,
}

impl Packet for ClientboundMoveEntityRotPacket {
    const PACKET_ID: i32 = 0x38;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let entity_id = varint::read_varint_buf(&mut data)?;
        let yaw = types::read_u8(&mut data)?;
        let pitch = types::read_u8(&mut data)?;
        let is_on_ground = types::read_bool(&mut data)?;
        Ok(Self {
            entity_id,
            yaw,
            pitch,
            is_on_ground,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(4);
        varint::write_varint_buf(self.entity_id, &mut buf);
        types::write_u8(&mut buf, self.yaw);
        types::write_u8(&mut buf, self.pitch);
        types::write_bool(&mut buf, self.is_on_ground);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_pos_roundtrip() {
        let pkt = ClientboundMoveEntityPosPacket {
            entity_id: 1,
            dx: 4096,
            dy: 0,
            dz: -4096,
            is_on_ground: true,
        };
        let encoded = pkt.encode();
        // entity_id=1 (1 byte varint) + 3×i16 (6 bytes) + bool (1 byte) = 8 bytes
        assert_eq!(encoded.len(), 8);
        let decoded = ClientboundMoveEntityPosPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_pos_rot_roundtrip() {
        let pkt = ClientboundMoveEntityPosRotPacket {
            entity_id: 1,
            dx: 100,
            dy: 200,
            dz: -100,
            yaw: 128,
            pitch: 64,
            is_on_ground: false,
        };
        let encoded = pkt.encode();
        // entity_id=1 (1 byte) + 3×i16 (6 bytes) + 2×u8 (2 bytes) + bool (1 byte) = 10 bytes
        assert_eq!(encoded.len(), 10);
        let decoded = ClientboundMoveEntityPosRotPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_rot_roundtrip() {
        let pkt = ClientboundMoveEntityRotPacket {
            entity_id: 42,
            yaw: 0,
            pitch: 255,
            is_on_ground: true,
        };
        let encoded = pkt.encode();
        // entity_id=42 (1 byte) + 2×u8 (2 bytes) + bool (1 byte) = 4 bytes
        assert_eq!(encoded.len(), 4);
        let decoded = ClientboundMoveEntityRotPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_ids() {
        assert_eq!(<ClientboundMoveEntityPosPacket as Packet>::PACKET_ID, 0x35);
        assert_eq!(
            <ClientboundMoveEntityPosRotPacket as Packet>::PACKET_ID,
            0x36
        );
        assert_eq!(<ClientboundMoveEntityRotPacket as Packet>::PACKET_ID, 0x38);
    }
}
