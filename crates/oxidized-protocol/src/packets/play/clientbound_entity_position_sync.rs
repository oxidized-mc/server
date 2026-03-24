//! Clientbound full entity position synchronization.
//!
//! Sent for large teleports or periodic position corrections where
//! delta encoding is insufficient (> 8 blocks).
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundEntityPositionSyncPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::types;
use crate::codec::varint;

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;

/// Full position sync for an entity (0x23).
///
/// # Wire Format
///
/// | Field | Type |
/// |-------|------|
/// | entity_id | VarInt |
/// | x | f64 |
/// | y | f64 |
/// | z | f64 |
/// | vx | f64 |
/// | vy | f64 |
/// | vz | f64 |
/// | yaw | f32 |
/// | pitch | f32 |
/// | on_ground | bool |
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundEntityPositionSyncPacket {
    /// Entity network ID.
    pub entity_id: i32,
    /// Absolute X position.
    pub x: f64,
    /// Absolute Y position.
    pub y: f64,
    /// Absolute Z position.
    pub z: f64,
    /// Velocity X component.
    pub vx: f64,
    /// Velocity Y component.
    pub vy: f64,
    /// Velocity Z component.
    pub vz: f64,
    /// Yaw rotation in degrees.
    pub yaw: f32,
    /// Pitch rotation in degrees.
    pub pitch: f32,
    /// Whether the entity is on the ground.
    pub is_on_ground: bool,
}

impl Packet for ClientboundEntityPositionSyncPacket {
    const PACKET_ID: i32 = 0x23;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let entity_id = varint::read_varint_buf(&mut data)?;
        let x = types::read_f64(&mut data)?;
        let y = types::read_f64(&mut data)?;
        let z = types::read_f64(&mut data)?;
        let vx = types::read_f64(&mut data)?;
        let vy = types::read_f64(&mut data)?;
        let vz = types::read_f64(&mut data)?;
        let yaw = types::read_f32(&mut data)?;
        let pitch = types::read_f32(&mut data)?;
        let is_on_ground = types::read_bool(&mut data)?;
        Ok(Self {
            entity_id,
            x,
            y,
            z,
            vx,
            vy,
            vz,
            yaw,
            pitch,
            is_on_ground,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(58);
        varint::write_varint_buf(self.entity_id, &mut buf);
        types::write_f64(&mut buf, self.x);
        types::write_f64(&mut buf, self.y);
        types::write_f64(&mut buf, self.z);
        types::write_f64(&mut buf, self.vx);
        types::write_f64(&mut buf, self.vy);
        types::write_f64(&mut buf, self.vz);
        types::write_f32(&mut buf, self.yaw);
        types::write_f32(&mut buf, self.pitch);
        types::write_bool(&mut buf, self.is_on_ground);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundEntityPositionSyncPacket {
            entity_id: 1,
            x: 100.5,
            y: 64.0,
            z: -200.25,
            vx: 0.0,
            vy: -0.08,
            vz: 0.0,
            yaw: 90.0,
            pitch: -15.0,
            is_on_ground: false,
        };
        let encoded = pkt.encode();
        // VarInt(1)=1 byte + 6×f64=48 bytes + 2×f32=8 bytes + bool=1 byte = 58 bytes
        assert_eq!(encoded.len(), 58);
        let decoded = ClientboundEntityPositionSyncPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_id() {
        assert_eq!(
            <ClientboundEntityPositionSyncPacket as Packet>::PACKET_ID,
            0x23
        );
    }

    #[test]
    fn test_velocity_fields() {
        let pkt = ClientboundEntityPositionSyncPacket {
            entity_id: 5,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            vx: 1.5,
            vy: -9.8,
            vz: 0.3,
            yaw: 0.0,
            pitch: 0.0,
            is_on_ground: true,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundEntityPositionSyncPacket::decode(encoded.freeze()).unwrap();
        assert!((decoded.vx - 1.5).abs() < f64::EPSILON);
        assert!((decoded.vy + 9.8).abs() < f64::EPSILON);
        assert!((decoded.vz - 0.3).abs() < f64::EPSILON);
    }
}
