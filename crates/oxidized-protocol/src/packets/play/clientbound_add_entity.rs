//! Clientbound add-entity packet.
//!
//! Sent when an entity enters a player's tracking range. Contains the
//! entity's initial position, velocity, rotation, and type.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundAddEntityPacket`.

use bytes::{BufMut, Bytes, BytesMut};
use uuid::Uuid;

use crate::codec::lp_vec3;
use crate::codec::types;
use crate::codec::varint;

use super::clientbound_login::PlayPacketError;

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;

/// Spawn entity packet (0x01).
///
/// # Wire Format
///
/// | Field | Type | Description |
/// |-------|------|-------------|
/// | entity_id | VarInt | Network entity ID |
/// | uuid | UUID | 16-byte entity UUID |
/// | entity_type | VarInt | Registry ID from `minecraft:entity_type` |
/// | x | f64 | X position |
/// | y | f64 | Y position |
/// | z | f64 | Z position |
/// | movement | LpVec3 | Velocity (low-precision packed) |
/// | x_rot | u8 | Pitch (packed degrees) |
/// | y_rot | u8 | Yaw (packed degrees) |
/// | y_head_rot | u8 | Head yaw (packed degrees) |
/// | data | VarInt | Entity-type-specific data |
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundAddEntityPacket {
    /// Network entity ID.
    pub entity_id: i32,
    /// Entity UUID.
    pub uuid: Uuid,
    /// Entity type registry ID.
    pub entity_type: i32,
    /// X position.
    pub x: f64,
    /// Y position.
    pub y: f64,
    /// Z position.
    pub z: f64,
    /// X velocity (blocks/tick).
    pub vx: f64,
    /// Y velocity (blocks/tick).
    pub vy: f64,
    /// Z velocity (blocks/tick).
    pub vz: f64,
    /// Pitch in packed degrees (0–255 → 0–360°).
    pub x_rot: u8,
    /// Yaw in packed degrees (0–255 → 0–360°).
    pub y_rot: u8,
    /// Head yaw in packed degrees (0–255 → 0–360°).
    pub y_head_rot: u8,
    /// Entity-type-specific data (e.g., direction for paintings).
    pub data: i32,
}

impl ClientboundAddEntityPacket {
    /// Packet ID in the PLAY state clientbound registry.
    pub const PACKET_ID: i32 = 0x01;

    /// Decodes from the raw packet body.
    pub fn decode(mut data: Bytes) -> Result<Self, PlayPacketError> {
        let entity_id = varint::read_varint_buf(&mut data)?;
        let uuid = types::read_uuid(&mut data)?;
        let entity_type = varint::read_varint_buf(&mut data)?;
        let x = types::read_f64(&mut data)?;
        let y = types::read_f64(&mut data)?;
        let z = types::read_f64(&mut data)?;
        let (vx, vy, vz) = lp_vec3::read(&mut data)?;
        let x_rot = types::read_u8(&mut data)?;
        let y_rot = types::read_u8(&mut data)?;
        let y_head_rot = types::read_u8(&mut data)?;
        let extra_data = varint::read_varint_buf(&mut data)?;

        Ok(Self {
            entity_id,
            uuid,
            entity_type,
            x,
            y,
            z,
            vx,
            vy,
            vz,
            x_rot,
            y_rot,
            y_head_rot,
            data: extra_data,
        })
    }

    /// Encodes the packet body into `buf`.
    pub fn encode(&self, buf: &mut BytesMut) {
        varint::write_varint_buf(self.entity_id, buf);
        types::write_uuid(buf, &self.uuid);
        varint::write_varint_buf(self.entity_type, buf);
        buf.put_f64(self.x);
        buf.put_f64(self.y);
        buf.put_f64(self.z);
        lp_vec3::write(buf, self.vx, self.vy, self.vz);
        buf.put_u8(self.x_rot);
        buf.put_u8(self.y_rot);
        buf.put_u8(self.y_head_rot);
        varint::write_varint_buf(self.data, buf);
    }
}

impl Packet for ClientboundAddEntityPacket {
    const PACKET_ID: i32 = 0x01;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let entity_id = varint::read_varint_buf(&mut data)?;
        let uuid = types::read_uuid(&mut data)?;
        let entity_type = varint::read_varint_buf(&mut data)?;
        let x = types::read_f64(&mut data)?;
        let y = types::read_f64(&mut data)?;
        let z = types::read_f64(&mut data)?;
        let (vx, vy, vz) = lp_vec3::read(&mut data)?;
        let x_rot = types::read_u8(&mut data)?;
        let y_rot = types::read_u8(&mut data)?;
        let y_head_rot = types::read_u8(&mut data)?;
        let extra_data = varint::read_varint_buf(&mut data)?;

        Ok(Self {
            entity_id,
            uuid,
            entity_type,
            x,
            y,
            z,
            vx,
            vy,
            vz,
            x_rot,
            y_rot,
            y_head_rot,
            data: extra_data,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        self.encode(&mut buf);
        buf
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_packet_id() {
        assert_eq!(ClientboundAddEntityPacket::PACKET_ID, 0x01);
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let uuid = Uuid::new_v4();
        let pkt = ClientboundAddEntityPacket {
            entity_id: 42,
            uuid,
            entity_type: 7,
            x: 100.5,
            y: 64.0,
            z: -200.25,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            x_rot: 128,
            y_rot: 64,
            y_head_rot: 32,
            data: 0,
        };

        let mut buf = BytesMut::new();
        pkt.encode(&mut buf);
        let decoded = ClientboundAddEntityPacket::decode(buf.freeze()).unwrap();

        assert_eq!(decoded.entity_id, 42);
        assert_eq!(decoded.uuid, uuid);
        assert_eq!(decoded.entity_type, 7);
        assert!((decoded.x - 100.5).abs() < 1e-10);
        assert!((decoded.y - 64.0).abs() < 1e-10);
        assert!((decoded.z - (-200.25)).abs() < 1e-10);
        assert_eq!(decoded.x_rot, 128);
        assert_eq!(decoded.y_rot, 64);
        assert_eq!(decoded.y_head_rot, 32);
        assert_eq!(decoded.data, 0);
    }

    #[test]
    fn test_roundtrip_with_velocity() {
        let uuid = Uuid::new_v4();
        let pkt = ClientboundAddEntityPacket {
            entity_id: 1,
            uuid,
            entity_type: 0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            vx: 1.5,
            vy: -0.5,
            vz: 0.3,
            x_rot: 0,
            y_rot: 0,
            y_head_rot: 0,
            data: 0,
        };

        let mut buf = BytesMut::new();
        pkt.encode(&mut buf);
        let decoded = ClientboundAddEntityPacket::decode(buf.freeze()).unwrap();

        // LpVec3 is lossy — check within tolerance
        assert!((decoded.vx - 1.5).abs() < 0.1);
        assert!((decoded.vy - (-0.5)).abs() < 0.1);
        assert!((decoded.vz - 0.3).abs() < 0.1);
    }

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = ClientboundAddEntityPacket {
            entity_id: 42,
            uuid: Uuid::nil(),
            entity_type: 7,
            x: 100.5,
            y: 64.0,
            z: -200.25,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            x_rot: 128,
            y_rot: 64,
            y_head_rot: 32,
            data: 0,
        };
        let encoded = Packet::encode(&pkt);
        let decoded = <ClientboundAddEntityPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(<ClientboundAddEntityPacket as Packet>::PACKET_ID, 0x01);
    }
}
