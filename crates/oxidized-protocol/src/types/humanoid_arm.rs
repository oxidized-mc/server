//! [`HumanoidArm`] — which hand the player uses as their main hand.
//!
//! Maps to the vanilla `HumanoidArm` enum.
//! Used in [`ServerboundClientInformationPacket`](crate::packets::configuration::ServerboundClientInformationPacket) during configuration.

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

impl_protocol_enum! {
    HumanoidArm {
        Left  = 0 => "left",
        Right = 1 => "right",
    }
}

impl HumanoidArm {
    /// The default main hand (right).
    pub const DEFAULT: HumanoidArm = HumanoidArm::Right;

    /// Returns the opposite arm.
    pub const fn opposite(self) -> HumanoidArm {
        match self {
            HumanoidArm::Left => HumanoidArm::Right,
            HumanoidArm::Right => HumanoidArm::Left,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use bytes::{Bytes, BytesMut};

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
