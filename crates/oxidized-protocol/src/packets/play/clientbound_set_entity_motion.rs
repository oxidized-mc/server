//! Clientbound set-entity-motion packet.
//!
//! Sent when the server changes an entity's velocity (e.g. knockback,
//! explosions, water currents). The velocity is encoded using the
//! compact [`LpVec3`](crate::codec::lp_vec3) format.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundSetEntityMotionPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::lp_vec3;
use crate::codec::varint;

use super::clientbound_login::PlayPacketError;

/// Set entity motion packet (0x64).
///
/// # Wire Format
///
/// | Field | Type | Description |
/// |-------|------|-------------|
/// | entity_id | VarInt | Network entity ID |
/// | movement | LpVec3 | Velocity (low-precision packed) |
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundSetEntityMotionPacket {
    /// Network entity ID.
    pub entity_id: i32,
    /// Velocity X component (blocks/tick).
    pub vx: f64,
    /// Velocity Y component (blocks/tick).
    pub vy: f64,
    /// Velocity Z component (blocks/tick).
    pub vz: f64,
}

impl ClientboundSetEntityMotionPacket {
    /// Packet ID for `ClientboundSetEntityMotionPacket` in the PLAY state.
    pub const PACKET_ID: i32 = 0x64;

    /// Creates a new set-entity-motion packet.
    pub fn new(entity_id: i32, vx: f64, vy: f64, vz: f64) -> Self {
        Self {
            entity_id,
            vx,
            vy,
            vz,
        }
    }

    /// Encodes this packet to its wire format.
    ///
    /// # Errors
    ///
    /// Returns [`PlayPacketError`] if encoding fails.
    pub fn encode(&self) -> Result<Bytes, PlayPacketError> {
        let mut buf = BytesMut::with_capacity(16);
        varint::write_varint_buf(self.entity_id, &mut buf);
        lp_vec3::write(&mut buf, self.vx, self.vy, self.vz);
        Ok(buf.freeze())
    }

    /// Decodes a set-entity-motion packet from wire bytes.
    ///
    /// # Errors
    ///
    /// Returns [`PlayPacketError`] if the data is malformed.
    pub fn decode(mut data: Bytes) -> Result<Self, PlayPacketError> {
        let entity_id = varint::read_varint_buf(&mut data)?;
        let (vx, vy, vz) = lp_vec3::read(&mut data)
            .map_err(|e| PlayPacketError::InvalidData(format!("movement: {e}")))?;
        Ok(Self {
            entity_id,
            vx,
            vy,
            vz,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_packet_id() {
        assert_eq!(ClientboundSetEntityMotionPacket::PACKET_ID, 0x64);
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let packet = ClientboundSetEntityMotionPacket::new(42, 1.5, -0.08, 0.0);
        let encoded = packet.encode().unwrap();
        let decoded = ClientboundSetEntityMotionPacket::decode(encoded).unwrap();
        assert_eq!(decoded.entity_id, 42);
        // LpVec3 has limited precision, so compare with tolerance.
        assert!(
            (decoded.vx - 1.5).abs() < 0.01,
            "vx: {}",
            decoded.vx
        );
        assert!(
            (decoded.vy - (-0.08)).abs() < 0.01,
            "vy: {}",
            decoded.vy
        );
        assert!(
            (decoded.vz - 0.0).abs() < 0.01,
            "vz: {}",
            decoded.vz
        );
    }

    #[test]
    fn test_encode_decode_zero_velocity() {
        let packet = ClientboundSetEntityMotionPacket::new(1, 0.0, 0.0, 0.0);
        let encoded = packet.encode().unwrap();
        let decoded = ClientboundSetEntityMotionPacket::decode(encoded).unwrap();
        assert_eq!(decoded.entity_id, 1);
        assert!(decoded.vx.abs() < 1e-5);
        assert!(decoded.vy.abs() < 1e-5);
        assert!(decoded.vz.abs() < 1e-5);
    }
}
