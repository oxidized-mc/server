//! Serverbound player movement packets.
//!
//! Four variants share the same struct — the difference is which fields
//! the client populates. Packet IDs correspond to the four Java inner
//! classes: `Pos`, `Rot`, `PosRot`, `StatusOnly`.

use bytes::{Buf, Bytes};

use crate::codec::types::{TypeError, read_f32, read_f64};

/// Serverbound movement packet — decoded from one of four wire formats.
///
/// # Wire Formats
///
/// | Variant | ID | Fields |
/// |---------|----|--------|
/// | Pos | 0x1E | f64 x, y, z + u8 flags |
/// | PosRot | 0x1F | f64 x, y, z + f32 yaw, pitch + u8 flags |
/// | Rot | 0x20 | f32 yaw, pitch + u8 flags |
/// | StatusOnly | 0x21 | u8 flags |
///
/// Flags byte: bit 0 = on_ground, bit 1 = horizontal_collision.
#[derive(Debug, Clone)]
pub struct ServerboundMovePlayerPacket {
    /// World X coordinate (present in Pos and PosRot).
    pub x: Option<f64>,
    /// World Y coordinate (present in Pos and PosRot).
    pub y: Option<f64>,
    /// World Z coordinate (present in Pos and PosRot).
    pub z: Option<f64>,
    /// Yaw rotation in degrees (present in Rot and PosRot).
    pub yaw: Option<f32>,
    /// Pitch rotation in degrees (present in Rot and PosRot).
    pub pitch: Option<f32>,
    /// Whether the player is on the ground.
    pub on_ground: bool,
    /// Whether the player is colliding horizontally.
    pub horizontal_collision: bool,
}

impl ServerboundMovePlayerPacket {
    /// Packet ID for position-only variant.
    pub const PACKET_ID_POS: i32 = 0x1E;
    /// Packet ID for position + rotation variant.
    pub const PACKET_ID_POS_ROT: i32 = 0x1F;
    /// Packet ID for rotation-only variant.
    pub const PACKET_ID_ROT: i32 = 0x20;
    /// Packet ID for status-only variant (on_ground flag only).
    pub const PACKET_ID_STATUS_ONLY: i32 = 0x21;

    const FLAG_ON_GROUND: u8 = 0x01;
    const FLAG_HORIZONTAL_COLLISION: u8 = 0x02;

    /// Reads the one-byte flags field, returning `(on_ground, horizontal_collision)`.
    fn read_flags(buf: &mut Bytes) -> Result<(bool, bool), TypeError> {
        if !buf.has_remaining() {
            return Err(TypeError::UnexpectedEof { need: 1, have: 0 });
        }
        let flags = buf.get_u8();
        Ok((
            flags & Self::FLAG_ON_GROUND != 0,
            flags & Self::FLAG_HORIZONTAL_COLLISION != 0,
        ))
    }

    /// Decodes the position-only variant (0x1E).
    pub fn decode_pos(mut data: Bytes) -> Result<Self, TypeError> {
        let x = read_f64(&mut data)?;
        let y = read_f64(&mut data)?;
        let z = read_f64(&mut data)?;
        let (on_ground, horizontal_collision) = Self::read_flags(&mut data)?;
        Ok(Self {
            x: Some(x),
            y: Some(y),
            z: Some(z),
            yaw: None,
            pitch: None,
            on_ground,
            horizontal_collision,
        })
    }

    /// Decodes the position + rotation variant (0x1F).
    pub fn decode_pos_rot(mut data: Bytes) -> Result<Self, TypeError> {
        let x = read_f64(&mut data)?;
        let y = read_f64(&mut data)?;
        let z = read_f64(&mut data)?;
        let yaw = read_f32(&mut data)?;
        let pitch = read_f32(&mut data)?;
        let (on_ground, horizontal_collision) = Self::read_flags(&mut data)?;
        Ok(Self {
            x: Some(x),
            y: Some(y),
            z: Some(z),
            yaw: Some(yaw),
            pitch: Some(pitch),
            on_ground,
            horizontal_collision,
        })
    }

    /// Decodes the rotation-only variant (0x20).
    pub fn decode_rot(mut data: Bytes) -> Result<Self, TypeError> {
        let yaw = read_f32(&mut data)?;
        let pitch = read_f32(&mut data)?;
        let (on_ground, horizontal_collision) = Self::read_flags(&mut data)?;
        Ok(Self {
            x: None,
            y: None,
            z: None,
            yaw: Some(yaw),
            pitch: Some(pitch),
            on_ground,
            horizontal_collision,
        })
    }

    /// Decodes the status-only variant (0x21).
    pub fn decode_status_only(mut data: Bytes) -> Result<Self, TypeError> {
        let (on_ground, horizontal_collision) = Self::read_flags(&mut data)?;
        Ok(Self {
            x: None,
            y: None,
            z: None,
            yaw: None,
            pitch: None,
            on_ground,
            horizontal_collision,
        })
    }

    /// Returns `true` if this packet includes position data.
    pub fn has_pos(&self) -> bool {
        self.x.is_some()
    }

    /// Returns `true` if this packet includes rotation data.
    pub fn has_rot(&self) -> bool {
        self.yaw.is_some()
    }

    /// Returns `true` if any coordinate is NaN or infinite.
    pub fn contains_invalid_values(&self) -> bool {
        self.x.is_some_and(|v| !v.is_finite())
            || self.y.is_some_and(|v| !v.is_finite())
            || self.z.is_some_and(|v| !v.is_finite())
            || self.yaw.is_some_and(|v| !v.is_finite())
            || self.pitch.is_some_and(|v| !v.is_finite())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use bytes::{BufMut, BytesMut};

    use super::*;

    fn encode_pos(x: f64, y: f64, z: f64, on_ground: bool, horiz: bool) -> Bytes {
        let mut buf = BytesMut::new();
        buf.put_f64(x);
        buf.put_f64(y);
        buf.put_f64(z);
        let flags = if on_ground { 0x01u8 } else { 0 } | if horiz { 0x02 } else { 0 };
        buf.put_u8(flags);
        buf.freeze()
    }

    fn encode_pos_rot(x: f64, y: f64, z: f64, yaw: f32, pitch: f32, on_ground: bool) -> Bytes {
        let mut buf = BytesMut::new();
        buf.put_f64(x);
        buf.put_f64(y);
        buf.put_f64(z);
        buf.put_f32(yaw);
        buf.put_f32(pitch);
        let flags = if on_ground { 0x01u8 } else { 0 };
        buf.put_u8(flags);
        buf.freeze()
    }

    fn encode_rot(yaw: f32, pitch: f32, on_ground: bool) -> Bytes {
        let mut buf = BytesMut::new();
        buf.put_f32(yaw);
        buf.put_f32(pitch);
        let flags = if on_ground { 0x01u8 } else { 0 };
        buf.put_u8(flags);
        buf.freeze()
    }

    #[test]
    fn test_decode_pos() {
        let data = encode_pos(1.5, 64.0, -3.25, true, false);
        let pkt = ServerboundMovePlayerPacket::decode_pos(data).unwrap();
        assert!((pkt.x.unwrap() - 1.5).abs() < f64::EPSILON);
        assert!((pkt.y.unwrap() - 64.0).abs() < f64::EPSILON);
        assert!((pkt.z.unwrap() + 3.25).abs() < f64::EPSILON);
        assert!(pkt.yaw.is_none());
        assert!(pkt.pitch.is_none());
        assert!(pkt.on_ground);
        assert!(!pkt.horizontal_collision);
        assert!(pkt.has_pos());
        assert!(!pkt.has_rot());
    }

    #[test]
    fn test_decode_pos_rot() {
        let data = encode_pos_rot(10.0, 70.0, 20.0, 90.0, -15.0, false);
        let pkt = ServerboundMovePlayerPacket::decode_pos_rot(data).unwrap();
        assert!((pkt.x.unwrap() - 10.0).abs() < f64::EPSILON);
        assert!((pkt.yaw.unwrap() - 90.0).abs() < f32::EPSILON);
        assert!((pkt.pitch.unwrap() + 15.0).abs() < f32::EPSILON);
        assert!(!pkt.on_ground);
        assert!(pkt.has_pos());
        assert!(pkt.has_rot());
    }

    #[test]
    fn test_decode_rot() {
        let data = encode_rot(180.0, 45.0, true);
        let pkt = ServerboundMovePlayerPacket::decode_rot(data).unwrap();
        assert!(pkt.x.is_none());
        assert!((pkt.yaw.unwrap() - 180.0).abs() < f32::EPSILON);
        assert!(pkt.on_ground);
        assert!(!pkt.has_pos());
        assert!(pkt.has_rot());
    }

    #[test]
    fn test_decode_status_only() {
        let data = Bytes::from_static(&[0x03]); // both flags set
        let pkt = ServerboundMovePlayerPacket::decode_status_only(data).unwrap();
        assert!(pkt.x.is_none());
        assert!(pkt.yaw.is_none());
        assert!(pkt.on_ground);
        assert!(pkt.horizontal_collision);
        assert!(!pkt.has_pos());
        assert!(!pkt.has_rot());
    }

    #[test]
    fn test_contains_invalid_values_nan() {
        let pkt = ServerboundMovePlayerPacket {
            x: Some(f64::NAN),
            y: Some(0.0),
            z: Some(0.0),
            yaw: None,
            pitch: None,
            on_ground: false,
            horizontal_collision: false,
        };
        assert!(pkt.contains_invalid_values());
    }

    #[test]
    fn test_contains_invalid_values_infinity() {
        let pkt = ServerboundMovePlayerPacket {
            x: Some(0.0),
            y: Some(f64::INFINITY),
            z: Some(0.0),
            yaw: None,
            pitch: None,
            on_ground: false,
            horizontal_collision: false,
        };
        assert!(pkt.contains_invalid_values());
    }

    #[test]
    fn test_contains_invalid_values_valid() {
        let pkt = ServerboundMovePlayerPacket {
            x: Some(100.5),
            y: Some(64.0),
            z: Some(-200.0),
            yaw: Some(90.0),
            pitch: Some(-15.0),
            on_ground: true,
            horizontal_collision: false,
        };
        assert!(!pkt.contains_invalid_values());
    }

    #[test]
    fn test_horizontal_collision_flag() {
        let data = encode_pos(0.0, 0.0, 0.0, false, true);
        let pkt = ServerboundMovePlayerPacket::decode_pos(data).unwrap();
        assert!(!pkt.on_ground);
        assert!(pkt.horizontal_collision);
    }
}
