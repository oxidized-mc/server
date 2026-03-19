//! Entity movement delta encoding.
//!
//! Provides helpers for encoding entity position changes as delta shorts
//! (1/4096 block precision) and packing rotation degrees into bytes
//! (0–255 → 0–360°). Used to build [`ClientboundMoveEntityPacket`]
//! variants and decide when to fall back to full position sync.

/// Scale factor for delta position encoding (1 block = 4096 units).
pub const DELTA_SCALE: f64 = 4096.0;

/// Maximum delta representable as an `i16` in blocks (~7.999).
pub const MAX_DELTA_BLOCKS: f64 = 7.999;

/// Encodes a position delta as a short.
///
/// Returns `None` if the delta exceeds `i16` range (~8 blocks),
/// indicating that a full position sync packet should be used instead.
///
/// # Formula
///
/// `delta = (new * 4096) as i64 - (old * 4096) as i64`
///
/// This matches the vanilla encoding in `ClientboundMoveEntityPacket`.
///
/// # Examples
///
/// ```
/// use oxidized_game::net::entity_movement::encode_delta;
///
/// // One block = 4096 units
/// assert_eq!(encode_delta(0.0, 1.0), Some(4096));
///
/// // Too far for delta encoding
/// assert_eq!(encode_delta(0.0, 9.0), None);
/// ```
pub fn encode_delta(old: f64, new: f64) -> Option<i16> {
    let raw = (new * DELTA_SCALE) as i64 - (old * DELTA_SCALE) as i64;
    if raw < i64::from(i16::MIN) || raw > i64::from(i16::MAX) {
        None
    } else {
        Some(raw as i16)
    }
}

/// Packs a rotation angle (degrees) into a byte (0–255 → 0–360°).
///
/// Matches `Mth.packDegrees()` in vanilla.
///
/// # Examples
///
/// ```
/// use oxidized_game::net::entity_movement::pack_degrees;
///
/// assert_eq!(pack_degrees(0.0), 0);
/// assert_eq!(pack_degrees(180.0), 128);
/// assert_eq!(pack_degrees(360.0), 0); // wraps
/// ```
pub fn pack_degrees(angle: f32) -> u8 {
    ((angle * 256.0 / 360.0) as i32 & 0xFF) as u8
}

/// Unpacks a byte (0–255) back to degrees (0–360°).
///
/// Inverse of [`pack_degrees`].
///
/// # Examples
///
/// ```
/// use oxidized_game::net::entity_movement::unpack_degrees;
///
/// assert!((unpack_degrees(128) - 180.0).abs() < 0.01);
/// assert_eq!(unpack_degrees(0), 0.0);
/// ```
pub fn unpack_degrees(byte: u8) -> f32 {
    byte as f32 * 360.0 / 256.0
}

/// Describes how to send an entity's movement to clients.
#[derive(Debug, Clone, PartialEq)]
pub enum EntityMoveKind {
    /// Small delta — use delta-encoded shorts for position.
    Delta {
        /// Delta X in 1/4096 block units.
        dx: i16,
        /// Delta Y in 1/4096 block units.
        dy: i16,
        /// Delta Z in 1/4096 block units.
        dz: i16,
    },
    /// Large teleport (> 8 blocks) — use absolute coordinates.
    Sync {
        /// Absolute X position.
        x: f64,
        /// Absolute Y position.
        y: f64,
        /// Absolute Z position.
        z: f64,
    },
}

/// Determines the best encoding for an entity position change.
///
/// Returns [`EntityMoveKind::Delta`] if all three axis deltas fit in `i16`,
/// or [`EntityMoveKind::Sync`] if any delta exceeds the range.
///
/// # Examples
///
/// ```
/// use oxidized_game::net::entity_movement::{classify_move, EntityMoveKind};
///
/// // Small movement — delta-encoded
/// let kind = classify_move(0.0, 64.0, 0.0, 1.0, 64.5, -0.5);
/// assert!(matches!(kind, EntityMoveKind::Delta { .. }));
///
/// // Large teleport — full position sync
/// let kind = classify_move(0.0, 64.0, 0.0, 100.0, 64.0, 0.0);
/// assert!(matches!(kind, EntityMoveKind::Sync { .. }));
/// ```
pub fn classify_move(
    old_x: f64,
    old_y: f64,
    old_z: f64,
    new_x: f64,
    new_y: f64,
    new_z: f64,
) -> EntityMoveKind {
    match (
        encode_delta(old_x, new_x),
        encode_delta(old_y, new_y),
        encode_delta(old_z, new_z),
    ) {
        (Some(dx), Some(dy), Some(dz)) => EntityMoveKind::Delta { dx, dy, dz },
        _ => EntityMoveKind::Sync {
            x: new_x,
            y: new_y,
            z: new_z,
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_one_block() {
        assert_eq!(encode_delta(0.0, 1.0), Some(4096));
        assert_eq!(encode_delta(0.0, -1.0), Some(-4096));
    }

    #[test]
    fn test_delta_zero() {
        assert_eq!(encode_delta(5.0, 5.0), Some(0));
    }

    #[test]
    fn test_delta_small_fraction() {
        // 0.5 blocks = 2048 units
        assert_eq!(encode_delta(0.0, 0.5), Some(2048));
    }

    #[test]
    fn test_delta_too_large_positive() {
        assert_eq!(encode_delta(0.0, 8.001), None);
    }

    #[test]
    fn test_delta_too_large_negative() {
        assert_eq!(encode_delta(0.0, -8.001), None);
    }

    #[test]
    fn test_delta_max_valid() {
        let result = encode_delta(0.0, 7.999);
        assert!(result.is_some());
        let val = i64::from(result.unwrap());
        assert!(val <= i64::from(i16::MAX), "delta {val} exceeds i16::MAX");
    }

    #[test]
    fn test_delta_offset_positions() {
        // Moving from 100.0 to 101.0 should produce same delta as 0.0 to 1.0
        assert_eq!(encode_delta(100.0, 101.0), Some(4096));
    }

    #[test]
    fn test_delta_large_offset_too_big() {
        assert_eq!(encode_delta(100.0, 108.5), None);
    }

    #[test]
    fn test_pack_degrees_zero() {
        assert_eq!(pack_degrees(0.0), 0);
    }

    #[test]
    fn test_pack_degrees_180() {
        assert_eq!(pack_degrees(180.0), 128);
    }

    #[test]
    fn test_pack_degrees_360_wraps() {
        assert_eq!(pack_degrees(360.0), 0);
    }

    #[test]
    fn test_pack_degrees_90() {
        assert_eq!(pack_degrees(90.0), 64);
    }

    #[test]
    fn test_unpack_degrees_roundtrip() {
        for deg in [0.0, 45.0, 90.0, 135.0, 180.0, 270.0] {
            let packed = pack_degrees(deg);
            let unpacked = unpack_degrees(packed);
            assert!(
                (unpacked - deg).abs() < 2.0,
                "roundtrip failed for {deg}°: got {unpacked}°"
            );
        }
    }

    #[test]
    fn test_classify_move_small_delta() {
        let kind = classify_move(0.0, 64.0, 0.0, 1.0, 64.5, -0.5);
        match kind {
            EntityMoveKind::Delta { dx, dy, dz } => {
                assert_eq!(dx, 4096);
                assert_eq!(dy, 2048);
                assert_eq!(dz, -2048);
            },
            _ => panic!("Expected Delta variant"),
        }
    }

    #[test]
    fn test_classify_move_large_teleport() {
        let kind = classify_move(0.0, 64.0, 0.0, 100.0, 64.0, 0.0);
        match kind {
            EntityMoveKind::Sync { x, y, z } => {
                assert!((x - 100.0).abs() < f64::EPSILON);
                assert!((y - 64.0).abs() < f64::EPSILON);
                assert!((z - 0.0).abs() < f64::EPSILON);
            },
            _ => panic!("Expected Sync variant"),
        }
    }
}
