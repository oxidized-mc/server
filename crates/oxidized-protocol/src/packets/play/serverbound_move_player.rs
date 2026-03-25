//! Serverbound player movement packets.
//!
//! Four wire packets map to four Rust structs, each implementing
//! [`Packet`]. The packet IDs correspond to the four
//! Java inner classes: `Pos`, `PosRot`, `Rot`, `StatusOnly`.
//!
//! All four variants convert into [`ServerboundMovePlayerPacket`] via `From`,
//! which unifies them into a single type with optional fields for convenient
//! handling in the server layer.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ServerboundMovePlayerPacket`
//! and its inner classes.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types::{ensure_remaining, read_f32, read_f64, write_f32, write_f64};

const FLAG_ON_GROUND: u8 = 0x01;
const FLAG_HORIZONTAL_COLLISION: u8 = 0x02;

/// Reads the one-byte flags field, returning `(is_on_ground, has_horizontal_collision)`.
fn read_flags(buf: &mut Bytes) -> Result<(bool, bool), PacketDecodeError> {
    ensure_remaining(buf, 1, "MovePlayer flags")?;
    let flags = buf.get_u8();
    Ok((
        flags & FLAG_ON_GROUND != 0,
        flags & FLAG_HORIZONTAL_COLLISION != 0,
    ))
}

/// Encodes the flags byte from on_ground and horizontal_collision.
fn encode_flags(buf: &mut BytesMut, is_on_ground: bool, has_horizontal_collision: bool) {
    let flags = if is_on_ground { FLAG_ON_GROUND } else { 0 }
        | if has_horizontal_collision {
            FLAG_HORIZONTAL_COLLISION
        } else {
            0
        };
    buf.put_u8(flags);
}

// ---------------------------------------------------------------------------
// Position-only variant (0x1E)
// ---------------------------------------------------------------------------

/// Position-only movement packet (0x1E).
///
/// # Wire Format
///
/// | Field | Type |
/// |-------|------|
/// | x | f64 |
/// | y | f64 |
/// | z | f64 |
/// | flags | u8 |
#[derive(Debug, Clone, PartialEq)]
pub struct ServerboundMovePlayerPosPacket {
    /// World X coordinate.
    pub x: f64,
    /// World Y coordinate.
    pub y: f64,
    /// World Z coordinate.
    pub z: f64,
    /// Whether the player is on the ground.
    pub is_on_ground: bool,
    /// Whether the player is colliding horizontally.
    pub has_horizontal_collision: bool,
}

impl Packet for ServerboundMovePlayerPosPacket {
    const PACKET_ID: i32 = 0x1E;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let x = read_f64(&mut data)?;
        let y = read_f64(&mut data)?;
        let z = read_f64(&mut data)?;
        let (is_on_ground, has_horizontal_collision) = read_flags(&mut data)?;
        Ok(Self {
            x,
            y,
            z,
            is_on_ground,
            has_horizontal_collision,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(25);
        write_f64(&mut buf, self.x);
        write_f64(&mut buf, self.y);
        write_f64(&mut buf, self.z);
        encode_flags(&mut buf, self.is_on_ground, self.has_horizontal_collision);
        buf
    }
}

// ---------------------------------------------------------------------------
// Position + rotation variant (0x1F)
// ---------------------------------------------------------------------------

/// Position + rotation movement packet (0x1F).
///
/// # Wire Format
///
/// | Field | Type |
/// |-------|------|
/// | x | f64 |
/// | y | f64 |
/// | z | f64 |
/// | yaw | f32 |
/// | pitch | f32 |
/// | flags | u8 |
#[derive(Debug, Clone, PartialEq)]
pub struct ServerboundMovePlayerPosRotPacket {
    /// World X coordinate.
    pub x: f64,
    /// World Y coordinate.
    pub y: f64,
    /// World Z coordinate.
    pub z: f64,
    /// Yaw rotation in degrees.
    pub yaw: f32,
    /// Pitch rotation in degrees.
    pub pitch: f32,
    /// Whether the player is on the ground.
    pub is_on_ground: bool,
    /// Whether the player is colliding horizontally.
    pub has_horizontal_collision: bool,
}

impl Packet for ServerboundMovePlayerPosRotPacket {
    const PACKET_ID: i32 = 0x1F;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let x = read_f64(&mut data)?;
        let y = read_f64(&mut data)?;
        let z = read_f64(&mut data)?;
        let yaw = read_f32(&mut data)?;
        let pitch = read_f32(&mut data)?;
        let (is_on_ground, has_horizontal_collision) = read_flags(&mut data)?;
        Ok(Self {
            x,
            y,
            z,
            yaw,
            pitch,
            is_on_ground,
            has_horizontal_collision,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(33);
        write_f64(&mut buf, self.x);
        write_f64(&mut buf, self.y);
        write_f64(&mut buf, self.z);
        write_f32(&mut buf, self.yaw);
        write_f32(&mut buf, self.pitch);
        encode_flags(&mut buf, self.is_on_ground, self.has_horizontal_collision);
        buf
    }
}

// ---------------------------------------------------------------------------
// Rotation-only variant (0x20)
// ---------------------------------------------------------------------------

/// Rotation-only movement packet (0x20).
///
/// # Wire Format
///
/// | Field | Type |
/// |-------|------|
/// | yaw | f32 |
/// | pitch | f32 |
/// | flags | u8 |
#[derive(Debug, Clone, PartialEq)]
pub struct ServerboundMovePlayerRotPacket {
    /// Yaw rotation in degrees.
    pub yaw: f32,
    /// Pitch rotation in degrees.
    pub pitch: f32,
    /// Whether the player is on the ground.
    pub is_on_ground: bool,
    /// Whether the player is colliding horizontally.
    pub has_horizontal_collision: bool,
}

impl Packet for ServerboundMovePlayerRotPacket {
    const PACKET_ID: i32 = 0x20;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let yaw = read_f32(&mut data)?;
        let pitch = read_f32(&mut data)?;
        let (is_on_ground, has_horizontal_collision) = read_flags(&mut data)?;
        Ok(Self {
            yaw,
            pitch,
            is_on_ground,
            has_horizontal_collision,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(9);
        write_f32(&mut buf, self.yaw);
        write_f32(&mut buf, self.pitch);
        encode_flags(&mut buf, self.is_on_ground, self.has_horizontal_collision);
        buf
    }
}

// ---------------------------------------------------------------------------
// Status-only variant (0x21)
// ---------------------------------------------------------------------------

/// Status-only movement packet (0x21) — on_ground flag only.
///
/// # Wire Format
///
/// | Field | Type |
/// |-------|------|
/// | flags | u8 |
#[derive(Debug, Clone, PartialEq)]
pub struct ServerboundMovePlayerStatusOnlyPacket {
    /// Whether the player is on the ground.
    pub is_on_ground: bool,
    /// Whether the player is colliding horizontally.
    pub has_horizontal_collision: bool,
}

impl Packet for ServerboundMovePlayerStatusOnlyPacket {
    const PACKET_ID: i32 = 0x21;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let (is_on_ground, has_horizontal_collision) = read_flags(&mut data)?;
        Ok(Self {
            is_on_ground,
            has_horizontal_collision,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(1);
        encode_flags(&mut buf, self.is_on_ground, self.has_horizontal_collision);
        buf
    }
}

// ---------------------------------------------------------------------------
// Unified movement data
// ---------------------------------------------------------------------------

/// Unified serverbound movement data with optional fields.
///
/// All four wire variants ([`ServerboundMovePlayerPosPacket`],
/// [`ServerboundMovePlayerPosRotPacket`], [`ServerboundMovePlayerRotPacket`],
/// [`ServerboundMovePlayerStatusOnlyPacket`]) convert into this type via
/// `From`. The server handler uses this unified type after decoding.
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
    pub is_on_ground: bool,
    /// Whether the player is colliding horizontally.
    pub has_horizontal_collision: bool,
}

impl ServerboundMovePlayerPacket {
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

impl From<ServerboundMovePlayerPosPacket> for ServerboundMovePlayerPacket {
    fn from(p: ServerboundMovePlayerPosPacket) -> Self {
        Self {
            x: Some(p.x),
            y: Some(p.y),
            z: Some(p.z),
            yaw: None,
            pitch: None,
            is_on_ground: p.is_on_ground,
            has_horizontal_collision: p.has_horizontal_collision,
        }
    }
}

impl From<ServerboundMovePlayerPosRotPacket> for ServerboundMovePlayerPacket {
    fn from(p: ServerboundMovePlayerPosRotPacket) -> Self {
        Self {
            x: Some(p.x),
            y: Some(p.y),
            z: Some(p.z),
            yaw: Some(p.yaw),
            pitch: Some(p.pitch),
            is_on_ground: p.is_on_ground,
            has_horizontal_collision: p.has_horizontal_collision,
        }
    }
}

impl From<ServerboundMovePlayerRotPacket> for ServerboundMovePlayerPacket {
    fn from(p: ServerboundMovePlayerRotPacket) -> Self {
        Self {
            x: None,
            y: None,
            z: None,
            yaw: Some(p.yaw),
            pitch: Some(p.pitch),
            is_on_ground: p.is_on_ground,
            has_horizontal_collision: p.has_horizontal_collision,
        }
    }
}

impl From<ServerboundMovePlayerStatusOnlyPacket> for ServerboundMovePlayerPacket {
    fn from(p: ServerboundMovePlayerStatusOnlyPacket) -> Self {
        Self {
            x: None,
            y: None,
            z: None,
            yaw: None,
            pitch: None,
            is_on_ground: p.is_on_ground,
            has_horizontal_collision: p.has_horizontal_collision,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_pos_roundtrip() {
        let pkt = ServerboundMovePlayerPosPacket {
            x: 1.5,
            y: 64.0,
            z: -3.25,
            is_on_ground: true,
            has_horizontal_collision: false,
        };
        let encoded = pkt.encode();
        assert_eq!(encoded.len(), 25); // 3×f64 (24) + flags (1)
        let decoded = ServerboundMovePlayerPosPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_pos_to_unified() {
        let pkt = ServerboundMovePlayerPosPacket {
            x: 1.5,
            y: 64.0,
            z: -3.25,
            is_on_ground: true,
            has_horizontal_collision: false,
        };
        let unified: ServerboundMovePlayerPacket = pkt.into();
        assert!((unified.x.unwrap() - 1.5).abs() < f64::EPSILON);
        assert!((unified.y.unwrap() - 64.0).abs() < f64::EPSILON);
        assert!((unified.z.unwrap() + 3.25).abs() < f64::EPSILON);
        assert!(unified.yaw.is_none());
        assert!(unified.pitch.is_none());
        assert!(unified.is_on_ground);
        assert!(!unified.has_horizontal_collision);
        assert!(unified.has_pos());
        assert!(!unified.has_rot());
    }

    #[test]
    fn test_pos_rot_roundtrip() {
        let pkt = ServerboundMovePlayerPosRotPacket {
            x: 10.0,
            y: 70.0,
            z: 20.0,
            yaw: 90.0,
            pitch: -15.0,
            is_on_ground: false,
            has_horizontal_collision: false,
        };
        let encoded = pkt.encode();
        assert_eq!(encoded.len(), 33); // 3×f64 (24) + 2×f32 (8) + flags (1)
        let decoded = ServerboundMovePlayerPosRotPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_pos_rot_to_unified() {
        let pkt = ServerboundMovePlayerPosRotPacket {
            x: 10.0,
            y: 70.0,
            z: 20.0,
            yaw: 90.0,
            pitch: -15.0,
            is_on_ground: false,
            has_horizontal_collision: false,
        };
        let unified: ServerboundMovePlayerPacket = pkt.into();
        assert!((unified.x.unwrap() - 10.0).abs() < f64::EPSILON);
        assert!((unified.yaw.unwrap() - 90.0).abs() < f32::EPSILON);
        assert!((unified.pitch.unwrap() + 15.0).abs() < f32::EPSILON);
        assert!(!unified.is_on_ground);
        assert!(unified.has_pos());
        assert!(unified.has_rot());
    }

    #[test]
    fn test_rot_roundtrip() {
        let pkt = ServerboundMovePlayerRotPacket {
            yaw: 180.0,
            pitch: 45.0,
            is_on_ground: true,
            has_horizontal_collision: false,
        };
        let encoded = pkt.encode();
        assert_eq!(encoded.len(), 9); // 2×f32 (8) + flags (1)
        let decoded = ServerboundMovePlayerRotPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_rot_to_unified() {
        let pkt = ServerboundMovePlayerRotPacket {
            yaw: 180.0,
            pitch: 45.0,
            is_on_ground: true,
            has_horizontal_collision: false,
        };
        let unified: ServerboundMovePlayerPacket = pkt.into();
        assert!(unified.x.is_none());
        assert!((unified.yaw.unwrap() - 180.0).abs() < f32::EPSILON);
        assert!(unified.is_on_ground);
        assert!(!unified.has_pos());
        assert!(unified.has_rot());
    }

    #[test]
    fn test_status_only_roundtrip() {
        let pkt = ServerboundMovePlayerStatusOnlyPacket {
            is_on_ground: true,
            has_horizontal_collision: true,
        };
        let encoded = pkt.encode();
        assert_eq!(encoded.len(), 1); // flags only
        let decoded = ServerboundMovePlayerStatusOnlyPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_status_only_to_unified() {
        let pkt = ServerboundMovePlayerStatusOnlyPacket {
            is_on_ground: true,
            has_horizontal_collision: true,
        };
        let unified: ServerboundMovePlayerPacket = pkt.into();
        assert!(unified.x.is_none());
        assert!(unified.yaw.is_none());
        assert!(unified.is_on_ground);
        assert!(unified.has_horizontal_collision);
        assert!(!unified.has_pos());
        assert!(!unified.has_rot());
    }

    #[test]
    fn test_contains_invalid_values_nan() {
        let pkt = ServerboundMovePlayerPacket {
            x: Some(f64::NAN),
            y: Some(0.0),
            z: Some(0.0),
            yaw: None,
            pitch: None,
            is_on_ground: false,
            has_horizontal_collision: false,
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
            is_on_ground: false,
            has_horizontal_collision: false,
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
            is_on_ground: true,
            has_horizontal_collision: false,
        };
        assert!(!pkt.contains_invalid_values());
    }

    #[test]
    fn test_horizontal_collision_flag() {
        let pkt = ServerboundMovePlayerPosPacket {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            is_on_ground: false,
            has_horizontal_collision: true,
        };
        let encoded = pkt.encode();
        let decoded = ServerboundMovePlayerPosPacket::decode(encoded.freeze()).unwrap();
        assert!(!decoded.is_on_ground);
        assert!(decoded.has_horizontal_collision);
    }

    #[test]
    fn test_packet_ids() {
        assert_eq!(<ServerboundMovePlayerPosPacket as Packet>::PACKET_ID, 0x1E);
        assert_eq!(
            <ServerboundMovePlayerPosRotPacket as Packet>::PACKET_ID,
            0x1F
        );
        assert_eq!(<ServerboundMovePlayerRotPacket as Packet>::PACKET_ID, 0x20);
        assert_eq!(
            <ServerboundMovePlayerStatusOnlyPacket as Packet>::PACKET_ID,
            0x21
        );
    }
}
