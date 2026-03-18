//! [`BlockPos`] — an immutable block position in the world.
//!
//! Block positions are packed into a 64-bit integer for the wire format.
//! Bit layout: X\[26 bits, 38–63\] | Z\[26 bits, 12–37\] | Y\[12 bits, 0–11\].

use std::fmt;

use bytes::{Bytes, BytesMut};

use crate::codec::types::{self, TypeError};

use super::direction::Direction;
use super::vec3::Vec3;
use super::vec3i::Vec3i;

/// Number of bits used to pack the X coordinate.
const PACKED_X_LENGTH: u32 = 26;

/// Number of bits used to pack the Z coordinate.
const PACKED_Z_LENGTH: u32 = 26;

/// Number of bits used to pack the Y coordinate.
const PACKED_Y_LENGTH: u32 = 12;

/// Bit mask for a packed X coordinate.
const PACKED_X_MASK: i64 = (1_i64 << PACKED_X_LENGTH) - 1;

/// Bit mask for a packed Z coordinate.
const PACKED_Z_MASK: i64 = (1_i64 << PACKED_Z_LENGTH) - 1;

/// Bit mask for a packed Y coordinate.
const PACKED_Y_MASK: i64 = (1_i64 << PACKED_Y_LENGTH) - 1;

/// Bit offset of the Z coordinate in the packed representation.
const Z_OFFSET: u32 = PACKED_Y_LENGTH;

/// Bit offset of the X coordinate in the packed representation.
const X_OFFSET: u32 = Z_OFFSET + PACKED_Z_LENGTH;

/// A block position in the world, packed into a 64-bit integer for wire format.
///
/// # Wire format
///
/// A single big-endian `i64` (8 bytes) with the following bit layout:
///
/// | Field | Bits | Width |
/// |-------|------|-------|
/// | X | 38–63 | 26 |
/// | Z | 12–37 | 26 |
/// | Y | 0–11 | 12 |
///
/// Sign extension is applied when unpacking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockPos {
    /// The X coordinate (block space).
    pub x: i32,
    /// The Y coordinate (block space / height).
    pub y: i32,
    /// The Z coordinate (block space).
    pub z: i32,
}

impl BlockPos {
    /// The origin `(0, 0, 0)`.
    pub const ZERO: BlockPos = BlockPos { x: 0, y: 0, z: 0 };

    /// Creates a new [`BlockPos`].
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Creates a [`BlockPos`] from a [`Vec3i`].
    pub const fn from_vec3i(v: &Vec3i) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }

    /// Packs this position into a 64-bit integer.
    pub const fn as_long(&self) -> i64 {
        ((self.x as i64 & PACKED_X_MASK) << X_OFFSET)
            | ((self.z as i64 & PACKED_Z_MASK) << Z_OFFSET)
            | (self.y as i64 & PACKED_Y_MASK)
    }

    /// Unpacks a block position from a 64-bit integer.
    ///
    /// Sign extension is applied to recover negative coordinates.
    pub const fn from_long(packed: i64) -> Self {
        // Arithmetic right shift on i64 sign-extends.
        let x = (packed >> X_OFFSET) as i32;
        let z = ((packed << PACKED_X_LENGTH as i64) >> (PACKED_X_LENGTH + Z_OFFSET) as i64) as i32;
        let y = ((packed << (64 - PACKED_Y_LENGTH) as i64) >> (64 - PACKED_Y_LENGTH) as i64) as i32;
        Self { x, y, z }
    }

    /// Returns the block position containing the given floating-point coordinates.
    ///
    /// Each component is floored to the nearest integer.
    pub fn containing(x: f64, y: f64, z: f64) -> Self {
        Self::new(x.floor() as i32, y.floor() as i32, z.floor() as i32)
    }

    /// Returns a new position offset by `(dx, dy, dz)`.
    ///
    /// Uses wrapping arithmetic to match vanilla Java behavior.
    /// In practice, Minecraft coordinates are bounded by the world border (±30M)
    /// and overflow cannot occur with valid game coordinates.
    pub const fn offset(self, dx: i32, dy: i32, dz: i32) -> Self {
        Self::new(
            self.x.wrapping_add(dx),
            self.y.wrapping_add(dy),
            self.z.wrapping_add(dz),
        )
    }

    /// Returns the position one block above (positive Y).
    pub const fn above(self) -> Self {
        self.above_n(1)
    }

    /// Returns the position `n` blocks above (positive Y).
    pub const fn above_n(self, n: i32) -> Self {
        self.offset(0, n, 0)
    }

    /// Returns the position one block below (negative Y).
    pub const fn below(self) -> Self {
        self.below_n(1)
    }

    /// Returns the position `n` blocks below (negative Y).
    pub const fn below_n(self, n: i32) -> Self {
        self.offset(0, -n, 0)
    }

    /// Returns the position one block to the north (negative Z).
    pub const fn north(self) -> Self {
        self.offset(0, 0, -1)
    }

    /// Returns the position one block to the south (positive Z).
    pub const fn south(self) -> Self {
        self.offset(0, 0, 1)
    }

    /// Returns the position one block to the east (positive X).
    pub const fn east(self) -> Self {
        self.offset(1, 0, 0)
    }

    /// Returns the position one block to the west (negative X).
    pub const fn west(self) -> Self {
        self.offset(-1, 0, 0)
    }

    /// Returns the position offset by one step in the given [`Direction`].
    pub fn relative(self, dir: Direction) -> Self {
        self.offset(dir.step_x(), dir.step_y(), dir.step_z())
    }

    /// Returns the position offset by `n` steps in the given [`Direction`].
    pub fn relative_steps(self, dir: Direction, n: i32) -> Self {
        self.offset(
            dir.step_x().wrapping_mul(n),
            dir.step_y().wrapping_mul(n),
            dir.step_z().wrapping_mul(n),
        )
    }

    /// Converts this position to a [`Vec3i`].
    pub const fn as_vec3i(&self) -> Vec3i {
        Vec3i::new(self.x, self.y, self.z)
    }

    /// Returns the squared Euclidean distance to `other`.
    ///
    /// Uses `i64` arithmetic to avoid overflow for large coordinates.
    pub fn dist_sqr(&self, other: &BlockPos) -> i64 {
        let dx = i64::from(self.x) - i64::from(other.x);
        let dy = i64::from(self.y) - i64::from(other.y);
        let dz = i64::from(self.z) - i64::from(other.z);
        dx * dx + dy * dy + dz * dz
    }

    /// Returns the Manhattan (L1) distance to `other`.
    ///
    /// Uses `i64` arithmetic internally to avoid overflow with extreme coordinates.
    pub fn dist_manhattan(&self, other: &BlockPos) -> i64 {
        let dx = (i64::from(self.x) - i64::from(other.x)).abs();
        let dy = (i64::from(self.y) - i64::from(other.y)).abs();
        let dz = (i64::from(self.z) - i64::from(other.z)).abs();
        dx + dy + dz
    }

    /// Returns the center of this block as a [`Vec3`] (each component + 0.5).
    pub fn get_center(&self) -> Vec3 {
        Vec3::new(
            f64::from(self.x) + 0.5,
            f64::from(self.y) + 0.5,
            f64::from(self.z) + 0.5,
        )
    }

    /// Reads a [`BlockPos`] from a wire buffer (packed `i64`).
    ///
    /// # Errors
    ///
    /// Returns [`TypeError`] if fewer than 8 bytes remain.
    pub fn read(buf: &mut Bytes) -> Result<Self, TypeError> {
        let packed = types::read_i64(buf)?;
        Ok(Self::from_long(packed))
    }

    /// Writes this [`BlockPos`] to a wire buffer (packed `i64`).
    pub fn write(&self, buf: &mut BytesMut) {
        types::write_i64(buf, self.as_long());
    }
}

impl From<Vec3i> for BlockPos {
    fn from(v: Vec3i) -> Self {
        Self::new(v.x, v.y, v.z)
    }
}

impl From<BlockPos> for Vec3i {
    fn from(pos: BlockPos) -> Self {
        Self::new(pos.x, pos.y, pos.z)
    }
}

impl fmt::Display for BlockPos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ── Construction ─────────────────────────────────────────────────

    #[test]
    fn test_block_pos_new() {
        let pos = BlockPos::new(1, 2, 3);
        assert_eq!(pos.x, 1);
        assert_eq!(pos.y, 2);
        assert_eq!(pos.z, 3);
    }

    #[test]
    fn test_block_pos_zero() {
        assert_eq!(BlockPos::ZERO, BlockPos::new(0, 0, 0));
    }

    #[test]
    fn test_block_pos_from_vec3i() {
        let v = Vec3i::new(10, 20, 30);
        assert_eq!(BlockPos::from_vec3i(&v), BlockPos::new(10, 20, 30));
    }

    #[test]
    fn test_block_pos_as_vec3i() {
        let pos = BlockPos::new(10, 20, 30);
        assert_eq!(pos.as_vec3i(), Vec3i::new(10, 20, 30));
    }

    // ── Pack / unpack roundtrip ─────────────────────────────────────

    #[test]
    fn test_block_pos_pack_unpack_zero() {
        let pos = BlockPos::ZERO;
        assert_eq!(BlockPos::from_long(pos.as_long()), pos);
    }

    #[test]
    fn test_block_pos_pack_unpack_positive() {
        let pos = BlockPos::new(100, 64, 200);
        assert_eq!(BlockPos::from_long(pos.as_long()), pos);
    }

    #[test]
    fn test_block_pos_pack_unpack_negative() {
        let pos = BlockPos::new(-100, -64, -200);
        assert_eq!(BlockPos::from_long(pos.as_long()), pos);
    }

    #[test]
    fn test_block_pos_pack_known_values() {
        // Verify the bit layout manually.
        // X=1, Y=0, Z=0 → only X bit set at offset 38
        let pos = BlockPos::new(1, 0, 0);
        assert_eq!(pos.as_long(), 1_i64 << 38);

        // X=0, Y=0, Z=1 → only Z bit set at offset 12
        let pos = BlockPos::new(0, 0, 1);
        assert_eq!(pos.as_long(), 1_i64 << 12);

        // X=0, Y=1, Z=0 → only Y bit 0 set
        let pos = BlockPos::new(0, 1, 0);
        assert_eq!(pos.as_long(), 1_i64);
    }

    #[test]
    fn test_block_pos_pack_unpack_edge_max() {
        // Max values that fit in the packed format:
        // X/Z: 26-bit signed → -(2^25) to (2^25 - 1) = -33554432 to 33554431
        // Y: 12-bit signed → -(2^11) to (2^11 - 1) = -2048 to 2047
        let max = BlockPos::new(33_554_431, 2047, 33_554_431);
        assert_eq!(BlockPos::from_long(max.as_long()), max);

        let min = BlockPos::new(-33_554_432, -2048, -33_554_432);
        assert_eq!(BlockPos::from_long(min.as_long()), min);
    }

    // ── containing ──────────────────────────────────────────────────

    #[test]
    fn test_block_pos_containing_positive() {
        let pos = BlockPos::containing(1.7, 2.3, 3.9);
        assert_eq!(pos, BlockPos::new(1, 2, 3));
    }

    #[test]
    fn test_block_pos_containing_negative() {
        let pos = BlockPos::containing(-0.1, -1.9, -3.0);
        assert_eq!(pos, BlockPos::new(-1, -2, -3));
    }

    #[test]
    fn test_block_pos_containing_exact() {
        let pos = BlockPos::containing(5.0, 10.0, 15.0);
        assert_eq!(pos, BlockPos::new(5, 10, 15));
    }

    // ── Directional methods ─────────────────────────────────────────

    #[test]
    fn test_block_pos_above() {
        assert_eq!(BlockPos::new(1, 2, 3).above(), BlockPos::new(1, 3, 3));
    }

    #[test]
    fn test_block_pos_below() {
        assert_eq!(BlockPos::new(1, 2, 3).below(), BlockPos::new(1, 1, 3));
    }

    #[test]
    fn test_block_pos_above_n() {
        assert_eq!(BlockPos::new(1, 2, 3).above_n(5), BlockPos::new(1, 7, 3));
    }

    #[test]
    fn test_block_pos_below_n() {
        assert_eq!(BlockPos::new(1, 2, 3).below_n(5), BlockPos::new(1, -3, 3));
    }

    #[test]
    fn test_block_pos_north() {
        assert_eq!(BlockPos::new(1, 2, 3).north(), BlockPos::new(1, 2, 2));
    }

    #[test]
    fn test_block_pos_south() {
        assert_eq!(BlockPos::new(1, 2, 3).south(), BlockPos::new(1, 2, 4));
    }

    #[test]
    fn test_block_pos_east() {
        assert_eq!(BlockPos::new(1, 2, 3).east(), BlockPos::new(2, 2, 3));
    }

    #[test]
    fn test_block_pos_west() {
        assert_eq!(BlockPos::new(1, 2, 3).west(), BlockPos::new(0, 2, 3));
    }

    // ── Relative ────────────────────────────────────────────────────

    #[test]
    fn test_block_pos_relative_all_directions() {
        let origin = BlockPos::ZERO;
        assert_eq!(origin.relative(Direction::Down), BlockPos::new(0, -1, 0));
        assert_eq!(origin.relative(Direction::Up), BlockPos::new(0, 1, 0));
        assert_eq!(origin.relative(Direction::North), BlockPos::new(0, 0, -1));
        assert_eq!(origin.relative(Direction::South), BlockPos::new(0, 0, 1));
        assert_eq!(origin.relative(Direction::West), BlockPos::new(-1, 0, 0));
        assert_eq!(origin.relative(Direction::East), BlockPos::new(1, 0, 0));
    }

    #[test]
    fn test_block_pos_relative_steps() {
        let pos = BlockPos::new(5, 10, 15);
        assert_eq!(
            pos.relative_steps(Direction::East, 3),
            BlockPos::new(8, 10, 15)
        );
        assert_eq!(
            pos.relative_steps(Direction::Down, 2),
            BlockPos::new(5, 8, 15)
        );
    }

    // ── get_center ──────────────────────────────────────────────────

    #[test]
    fn test_block_pos_get_center() {
        let pos = BlockPos::new(10, 20, 30);
        let center = pos.get_center();
        assert_eq!(center, Vec3::new(10.5, 20.5, 30.5));
    }

    #[test]
    fn test_block_pos_get_center_negative() {
        let pos = BlockPos::new(-1, -1, -1);
        let center = pos.get_center();
        assert_eq!(center, Vec3::new(-0.5, -0.5, -0.5));
    }

    // ── Distances ───────────────────────────────────────────────────

    #[test]
    fn test_block_pos_dist_sqr() {
        let a = BlockPos::new(0, 0, 0);
        let b = BlockPos::new(3, 4, 0);
        assert_eq!(a.dist_sqr(&b), 25);
    }

    #[test]
    fn test_block_pos_dist_manhattan() {
        let a = BlockPos::new(0, 0, 0);
        let b = BlockPos::new(3, 4, 5);
        assert_eq!(a.dist_manhattan(&b), 12);
    }

    // ── From/Into conversions ───────────────────────────────────────

    #[test]
    fn test_block_pos_from_vec3i_trait() {
        let v = Vec3i::new(1, 2, 3);
        let pos: BlockPos = v.into();
        assert_eq!(pos, BlockPos::new(1, 2, 3));
    }

    #[test]
    fn test_vec3i_from_block_pos_trait() {
        let pos = BlockPos::new(1, 2, 3);
        let v: Vec3i = pos.into();
        assert_eq!(v, Vec3i::new(1, 2, 3));
    }

    // ── Display ─────────────────────────────────────────────────────

    #[test]
    fn test_block_pos_display() {
        let pos = BlockPos::new(1, -2, 3);
        assert_eq!(format!("{pos}"), "(1, -2, 3)");
    }

    // ── Wire roundtrip ──────────────────────────────────────────────

    #[test]
    fn test_block_pos_wire_roundtrip() {
        let pos = BlockPos::new(100, 64, -200);
        let mut buf = BytesMut::new();
        pos.write(&mut buf);
        assert_eq!(buf.len(), 8);
        let mut data = buf.freeze();
        let decoded = BlockPos::read(&mut data).unwrap();
        assert_eq!(decoded, pos);
    }

    #[test]
    fn test_block_pos_wire_roundtrip_zero() {
        let pos = BlockPos::ZERO;
        let mut buf = BytesMut::new();
        pos.write(&mut buf);
        let mut data = buf.freeze();
        let decoded = BlockPos::read(&mut data).unwrap();
        assert_eq!(decoded, pos);
    }

    #[test]
    fn test_block_pos_wire_roundtrip_negative() {
        let pos = BlockPos::new(-50, -64, -100);
        let mut buf = BytesMut::new();
        pos.write(&mut buf);
        let mut data = buf.freeze();
        let decoded = BlockPos::read(&mut data).unwrap();
        assert_eq!(decoded, pos);
    }
}
