//! Clientbound player position packet.
//!
//! Synchronizes the player's position and rotation from the server.
//! Client must respond with `ServerboundAcceptTeleportationPacket`.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundPlayerPositionPacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::types;
use crate::codec::varint;

use super::clientbound_login::PlayPacketError;

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;

/// Bitflags indicating which position/rotation fields are relative.
///
/// Mirrors `net.minecraft.world.entity.Relative`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelativeFlags(pub i32);

impl RelativeFlags {
    /// X position is relative.
    pub const X: i32 = 1 << 0;
    /// Y position is relative.
    pub const Y: i32 = 1 << 1;
    /// Z position is relative.
    pub const Z: i32 = 1 << 2;
    /// Y rotation (yaw) is relative.
    pub const Y_ROT: i32 = 1 << 3;
    /// X rotation (pitch) is relative.
    pub const X_ROT: i32 = 1 << 4;
    /// Delta X is relative.
    pub const DELTA_X: i32 = 1 << 5;
    /// Delta Y is relative.
    pub const DELTA_Y: i32 = 1 << 6;
    /// Delta Z is relative.
    pub const DELTA_Z: i32 = 1 << 7;
    /// Rotate delta flag.
    pub const ROTATE_DELTA: i32 = 1 << 8;

    /// Creates empty flags (all absolute).
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Returns `true` if the given flag bit is set.
    pub fn contains(self, flag: i32) -> bool {
        self.0 & flag != 0
    }
}

/// Server-initiated player position synchronization.
///
/// Wire format: `teleport_id: VarInt | x: f64 | y: f64 | z: f64 |
/// dx: f64 | dy: f64 | dz: f64 | yaw: f32 | pitch: f32 | flags: i32`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundPlayerPositionPacket {
    /// Unique teleport ID for confirmation.
    pub teleport_id: i32,
    /// X position.
    pub x: f64,
    /// Y position.
    pub y: f64,
    /// Z position.
    pub z: f64,
    /// Delta X velocity.
    pub dx: f64,
    /// Delta Y velocity.
    pub dy: f64,
    /// Delta Z velocity.
    pub dz: f64,
    /// Y rotation (yaw) in degrees.
    pub yaw: f32,
    /// X rotation (pitch) in degrees.
    pub pitch: f32,
    /// Relative position/rotation flags.
    pub relative_flags: RelativeFlags,
}

impl ClientboundPlayerPositionPacket {
    /// Packet ID in the PLAY state.
    pub const PACKET_ID: i32 = 0x48; // 72

    /// Decodes from the raw packet body.
    pub fn decode(mut data: Bytes) -> Result<Self, PlayPacketError> {
        let teleport_id = varint::read_varint_buf(&mut data)?;
        let x = types::read_f64(&mut data)?;
        let y = types::read_f64(&mut data)?;
        let z = types::read_f64(&mut data)?;
        let dx = types::read_f64(&mut data)?;
        let dy = types::read_f64(&mut data)?;
        let dz = types::read_f64(&mut data)?;
        if data.remaining() < 8 {
            return Err(PlayPacketError::UnexpectedEof);
        }
        let yaw = data.get_f32();
        let pitch = data.get_f32();
        let flags = types::read_i32(&mut data)?;

        Ok(Self {
            teleport_id,
            x,
            y,
            z,
            dx,
            dy,
            dz,
            yaw,
            pitch,
            relative_flags: RelativeFlags(flags),
        })
    }

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(64);
        varint::write_varint_buf(self.teleport_id, &mut buf);
        types::write_f64(&mut buf, self.x);
        types::write_f64(&mut buf, self.y);
        types::write_f64(&mut buf, self.z);
        types::write_f64(&mut buf, self.dx);
        types::write_f64(&mut buf, self.dy);
        types::write_f64(&mut buf, self.dz);
        buf.put_f32(self.yaw);
        buf.put_f32(self.pitch);
        types::write_i32(&mut buf, self.relative_flags.0);
        buf
    }
}

impl Packet for ClientboundPlayerPositionPacket {
    const PACKET_ID: i32 = 0x48;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let teleport_id = varint::read_varint_buf(&mut data)?;
        let x = types::read_f64(&mut data)?;
        let y = types::read_f64(&mut data)?;
        let z = types::read_f64(&mut data)?;
        let dx = types::read_f64(&mut data)?;
        let dy = types::read_f64(&mut data)?;
        let dz = types::read_f64(&mut data)?;
        if data.remaining() < 8 {
            return Err(PacketDecodeError::InvalidData(
                "unexpected end of packet data".into(),
            ));
        }
        let yaw = data.get_f32();
        let pitch = data.get_f32();
        let flags = types::read_i32(&mut data)?;

        Ok(Self {
            teleport_id,
            x,
            y,
            z,
            dx,
            dy,
            dz,
            yaw,
            pitch,
            relative_flags: RelativeFlags(flags),
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
    fn test_roundtrip_absolute() {
        let pkt = ClientboundPlayerPositionPacket {
            teleport_id: 1,
            x: 100.5,
            y: 64.0,
            z: -200.25,
            dx: 0.0,
            dy: 0.0,
            dz: 0.0,
            yaw: 90.0,
            pitch: -15.0,
            relative_flags: RelativeFlags::empty(),
        };
        let encoded = pkt.encode();
        let decoded = ClientboundPlayerPositionPacket::decode(encoded.freeze()).unwrap();

        assert_eq!(decoded.teleport_id, 1);
        assert!((decoded.x - 100.5).abs() < 0.001);
        assert!((decoded.y - 64.0).abs() < 0.001);
        assert!((decoded.z + 200.25).abs() < 0.001);
        assert!((decoded.yaw - 90.0).abs() < 0.001);
        assert!((decoded.pitch + 15.0).abs() < 0.001);
        assert_eq!(decoded.relative_flags, RelativeFlags::empty());
    }

    #[test]
    fn test_roundtrip_with_relative_flags() {
        let pkt = ClientboundPlayerPositionPacket {
            teleport_id: 42,
            x: 5.0,
            y: 3.0,
            z: -2.0,
            dx: 0.1,
            dy: -0.5,
            dz: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            relative_flags: RelativeFlags(RelativeFlags::X | RelativeFlags::Y | RelativeFlags::Z),
        };
        let encoded = pkt.encode();
        let decoded = ClientboundPlayerPositionPacket::decode(encoded.freeze()).unwrap();

        assert_eq!(decoded.teleport_id, 42);
        assert!(decoded.relative_flags.contains(RelativeFlags::X));
        assert!(decoded.relative_flags.contains(RelativeFlags::Y));
        assert!(decoded.relative_flags.contains(RelativeFlags::Z));
        assert!(!decoded.relative_flags.contains(RelativeFlags::Y_ROT));
    }

    #[test]
    fn test_velocity_fields() {
        let pkt = ClientboundPlayerPositionPacket {
            teleport_id: 1,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            dx: 1.5,
            dy: -0.08,
            dz: 0.3,
            yaw: 0.0,
            pitch: 0.0,
            relative_flags: RelativeFlags::empty(),
        };
        let encoded = pkt.encode();
        let decoded = ClientboundPlayerPositionPacket::decode(encoded.freeze()).unwrap();
        assert!((decoded.dx - 1.5).abs() < 0.001);
        assert!((decoded.dy + 0.08).abs() < 0.001);
        assert!((decoded.dz - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = ClientboundPlayerPositionPacket {
            teleport_id: 1,
            x: 100.5,
            y: 64.0,
            z: -200.25,
            dx: 0.0,
            dy: 0.0,
            dz: 0.0,
            yaw: 90.0,
            pitch: -15.0,
            relative_flags: RelativeFlags::empty(),
        };
        let encoded = Packet::encode(&pkt);
        let decoded =
            <ClientboundPlayerPositionPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(<ClientboundPlayerPositionPacket as Packet>::PACKET_ID, 0x48);
    }
}
