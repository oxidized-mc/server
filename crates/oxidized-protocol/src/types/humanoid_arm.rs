//! [`HumanoidArm`] — which hand the player uses as their main hand.
//!
//! Maps to the vanilla `HumanoidArm` enum.
//! Used in [`ServerboundClientInformationPacket`] during configuration.

use std::fmt;

use bytes::{Bytes, BytesMut};

use crate::codec::types::TypeError;
use crate::codec::varint;

/// Which hand the player uses as their main hand.
///
/// # Wire format
///
/// Encoded as a VarInt (0–1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum HumanoidArm {
    /// Left hand.
    Left = 0,
    /// Right hand.
    Right = 1,
}

impl HumanoidArm {
    /// The default main hand (right).
    pub const DEFAULT: HumanoidArm = HumanoidArm::Right;

    /// Returns the numeric ID of this arm.
    pub const fn id(self) -> i32 {
        self as i32
    }

    /// Returns the lowercase name of this arm.
    pub const fn name(self) -> &'static str {
        match self {
            HumanoidArm::Left => "left",
            HumanoidArm::Right => "right",
        }
    }

    /// Looks up an arm by numeric ID.
    ///
    /// Returns `None` if `id` is not in 0–1.
    pub const fn by_id(id: i32) -> Option<HumanoidArm> {
        match id {
            0 => Some(HumanoidArm::Left),
            1 => Some(HumanoidArm::Right),
            _ => None,
        }
    }

    /// Returns the opposite arm.
    pub const fn opposite(self) -> HumanoidArm {
        match self {
            HumanoidArm::Left => HumanoidArm::Right,
            HumanoidArm::Right => HumanoidArm::Left,
        }
    }

    /// Reads a [`HumanoidArm`] from a wire buffer as a VarInt.
    ///
    /// # Errors
    ///
    /// Returns [`TypeError`] if the buffer is truncated or the value is
    /// out of range.
    pub fn read(buf: &mut Bytes) -> Result<Self, TypeError> {
        let id = varint::read_varint_buf(buf)?;
        HumanoidArm::by_id(id).ok_or(TypeError::UnexpectedEof { need: 1, have: 0 })
    }

    /// Writes this [`HumanoidArm`] to a wire buffer as a VarInt.
    pub fn write(&self, buf: &mut BytesMut) {
        varint::write_varint_buf(self.id(), buf);
    }
}

impl fmt::Display for HumanoidArm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ── by_id ───────────────────────────────────────────────────────

    #[test]
    fn test_humanoid_arm_by_id_valid() {
        assert_eq!(HumanoidArm::by_id(0), Some(HumanoidArm::Left));
        assert_eq!(HumanoidArm::by_id(1), Some(HumanoidArm::Right));
    }

    #[test]
    fn test_humanoid_arm_by_id_invalid() {
        assert_eq!(HumanoidArm::by_id(-1), None);
        assert_eq!(HumanoidArm::by_id(2), None);
        assert_eq!(HumanoidArm::by_id(100), None);
    }

    // ── opposite ────────────────────────────────────────────────────

    #[test]
    fn test_humanoid_arm_opposite() {
        assert_eq!(HumanoidArm::Left.opposite(), HumanoidArm::Right);
        assert_eq!(HumanoidArm::Right.opposite(), HumanoidArm::Left);
    }

    // ── Display ─────────────────────────────────────────────────────

    #[test]
    fn test_humanoid_arm_display() {
        assert_eq!(format!("{}", HumanoidArm::Left), "left");
        assert_eq!(format!("{}", HumanoidArm::Right), "right");
    }

    // ── Roundtrip id ↔ enum ─────────────────────────────────────────

    #[test]
    fn test_humanoid_arm_id_roundtrip() {
        for id in 0..=1 {
            let arm = HumanoidArm::by_id(id).unwrap();
            assert_eq!(arm.id(), id);
        }
    }

    // ── Wire roundtrip ──────────────────────────────────────────────

    #[test]
    fn test_humanoid_arm_wire_roundtrip() {
        for id in 0..=1 {
            let arm = HumanoidArm::by_id(id).unwrap();
            let mut buf = BytesMut::new();
            arm.write(&mut buf);
            let mut data = buf.freeze();
            let decoded = HumanoidArm::read(&mut data).unwrap();
            assert_eq!(decoded, arm);
        }
    }

    // ── Default ─────────────────────────────────────────────────────

    #[test]
    fn test_humanoid_arm_default_is_right() {
        assert_eq!(HumanoidArm::DEFAULT, HumanoidArm::Right);
    }

    #[test]
    fn test_humanoid_arm_read_empty_buffer() {
        let mut data = Bytes::new();
        assert!(HumanoidArm::read(&mut data).is_err());
    }
}
