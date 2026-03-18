//! [`Vec3i`] — an immutable integer 3D vector.
//!
//! Used for block positions and other integer coordinates before they
//! are wrapped in higher-level newtypes like `BlockPos` or `ChunkPos`.

use std::fmt;
use std::ops::{Add, Sub};

use bytes::{Bytes, BytesMut};

use crate::codec::types::TypeError;
use crate::codec::varint;

use super::direction::{Axis, Direction};

/// An immutable integer 3D vector.
///
/// # Wire format
///
/// Three consecutive VarInts (x, y, z).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Vec3i {
    /// The X component.
    pub x: i32,
    /// The Y component.
    pub y: i32,
    /// The Z component.
    pub z: i32,
}

impl Vec3i {
    /// The zero vector `(0, 0, 0)`.
    pub const ZERO: Vec3i = Vec3i { x: 0, y: 0, z: 0 };

    /// Creates a new `Vec3i`.
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Returns a new vector offset by `(dx, dy, dz)`.
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
        self.offset(0, 1, 0)
    }

    /// Returns the position one block below (negative Y).
    pub const fn below(self) -> Self {
        self.offset(0, -1, 0)
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

    /// Returns the position offset by one step in the given direction.
    pub fn relative(self, dir: Direction) -> Self {
        self.offset(dir.step_x(), dir.step_y(), dir.step_z())
    }

    /// Returns the position offset by `steps` in the given direction.
    pub fn relative_steps(self, dir: Direction, steps: i32) -> Self {
        self.offset(
            dir.step_x().wrapping_mul(steps),
            dir.step_y().wrapping_mul(steps),
            dir.step_z().wrapping_mul(steps),
        )
    }

    /// Computes the cross product of two vectors.
    ///
    /// Uses wrapping arithmetic. In practice, cross products are only used
    /// with small direction/normal vectors where overflow cannot occur.
    pub const fn cross(self, other: Vec3i) -> Self {
        Self::new(
            self.y.wrapping_mul(other.z).wrapping_sub(self.z.wrapping_mul(other.y)),
            self.z.wrapping_mul(other.x).wrapping_sub(self.x.wrapping_mul(other.z)),
            self.x.wrapping_mul(other.y).wrapping_sub(self.y.wrapping_mul(other.x)),
        )
    }

    /// Returns the squared Euclidean distance to `other`.
    ///
    /// Uses `i64` arithmetic to avoid overflow for large coordinates.
    pub fn dist_sqr(self, other: Vec3i) -> i64 {
        let dx = i64::from(self.x) - i64::from(other.x);
        let dy = i64::from(self.y) - i64::from(other.y);
        let dz = i64::from(self.z) - i64::from(other.z);
        dx * dx + dy * dy + dz * dz
    }

    /// Returns the Manhattan (L1) distance to `other`.
    ///
    /// Uses `i64` arithmetic internally to avoid overflow with extreme coordinates.
    pub fn dist_manhattan(self, other: Vec3i) -> i64 {
        let dx = (i64::from(self.x) - i64::from(other.x)).abs();
        let dy = (i64::from(self.y) - i64::from(other.y)).abs();
        let dz = (i64::from(self.z) - i64::from(other.z)).abs();
        dx + dy + dz
    }

    /// Returns the Chebyshev (chessboard / L∞) distance to `other`.
    ///
    /// Uses `i64` arithmetic internally to avoid overflow with extreme coordinates.
    pub fn dist_chessboard(self, other: Vec3i) -> i64 {
        let dx = (i64::from(self.x) - i64::from(other.x)).abs();
        let dy = (i64::from(self.y) - i64::from(other.y)).abs();
        let dz = (i64::from(self.z) - i64::from(other.z)).abs();
        dx.max(dy).max(dz)
    }

    /// Selects the component on the given axis.
    pub fn get(self, axis: Axis) -> i32 {
        match axis {
            Axis::X => self.x,
            Axis::Y => self.y,
            Axis::Z => self.z,
        }
    }

    /// Returns a new vector with all components multiplied by `scale`.
    ///
    /// Uses wrapping arithmetic to match vanilla Java behavior.
    pub const fn multiply(self, scale: i32) -> Self {
        Self::new(
            self.x.wrapping_mul(scale),
            self.y.wrapping_mul(scale),
            self.z.wrapping_mul(scale),
        )
    }

    /// Reads a `Vec3i` from a wire buffer (3× VarInt).
    ///
    /// # Errors
    ///
    /// Returns [`TypeError`] if the buffer is truncated.
    pub fn read(buf: &mut Bytes) -> Result<Self, TypeError> {
        let x = varint::read_varint_buf(buf)?;
        let y = varint::read_varint_buf(buf)?;
        let z = varint::read_varint_buf(buf)?;
        Ok(Self { x, y, z })
    }

    /// Writes this `Vec3i` to a wire buffer (3× VarInt).
    pub fn write(&self, buf: &mut BytesMut) {
        varint::write_varint_buf(self.x, buf);
        varint::write_varint_buf(self.y, buf);
        varint::write_varint_buf(self.z, buf);
    }
}

impl Add for Vec3i {
    type Output = Vec3i;

    fn add(self, rhs: Vec3i) -> Vec3i {
        Vec3i::new(
            self.x.wrapping_add(rhs.x),
            self.y.wrapping_add(rhs.y),
            self.z.wrapping_add(rhs.z),
        )
    }
}

impl Sub for Vec3i {
    type Output = Vec3i;

    fn sub(self, rhs: Vec3i) -> Vec3i {
        Vec3i::new(
            self.x.wrapping_sub(rhs.x),
            self.y.wrapping_sub(rhs.y),
            self.z.wrapping_sub(rhs.z),
        )
    }
}

impl fmt::Display for Vec3i {
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
    fn test_vec3i_new() {
        let v = Vec3i::new(1, 2, 3);
        assert_eq!(v.x, 1);
        assert_eq!(v.y, 2);
        assert_eq!(v.z, 3);
    }

    #[test]
    fn test_vec3i_zero() {
        assert_eq!(Vec3i::ZERO, Vec3i::new(0, 0, 0));
    }

    // ── Offset ──────────────────────────────────────────────────────

    #[test]
    fn test_vec3i_offset() {
        let v = Vec3i::new(1, 2, 3).offset(10, 20, 30);
        assert_eq!(v, Vec3i::new(11, 22, 33));
    }

    // ── Directional stepping ────────────────────────────────────────

    #[test]
    fn test_vec3i_above() {
        assert_eq!(Vec3i::new(0, 5, 0).above(), Vec3i::new(0, 6, 0));
    }

    #[test]
    fn test_vec3i_below() {
        assert_eq!(Vec3i::new(0, 5, 0).below(), Vec3i::new(0, 4, 0));
    }

    #[test]
    fn test_vec3i_north() {
        assert_eq!(Vec3i::new(0, 0, 5).north(), Vec3i::new(0, 0, 4));
    }

    #[test]
    fn test_vec3i_south() {
        assert_eq!(Vec3i::new(0, 0, 5).south(), Vec3i::new(0, 0, 6));
    }

    #[test]
    fn test_vec3i_east() {
        assert_eq!(Vec3i::new(5, 0, 0).east(), Vec3i::new(6, 0, 0));
    }

    #[test]
    fn test_vec3i_west() {
        assert_eq!(Vec3i::new(5, 0, 0).west(), Vec3i::new(4, 0, 0));
    }

    // ── Relative ────────────────────────────────────────────────────

    #[test]
    fn test_vec3i_relative_all_directions() {
        let origin = Vec3i::ZERO;
        assert_eq!(origin.relative(Direction::Down), Vec3i::new(0, -1, 0));
        assert_eq!(origin.relative(Direction::Up), Vec3i::new(0, 1, 0));
        assert_eq!(origin.relative(Direction::North), Vec3i::new(0, 0, -1));
        assert_eq!(origin.relative(Direction::South), Vec3i::new(0, 0, 1));
        assert_eq!(origin.relative(Direction::West), Vec3i::new(-1, 0, 0));
        assert_eq!(origin.relative(Direction::East), Vec3i::new(1, 0, 0));
    }

    #[test]
    fn test_vec3i_relative_steps() {
        let v = Vec3i::new(0, 0, 0);
        assert_eq!(v.relative_steps(Direction::East, 5), Vec3i::new(5, 0, 0));
        assert_eq!(v.relative_steps(Direction::Up, 3), Vec3i::new(0, 3, 0));
    }

    // ── Cross product ───────────────────────────────────────────────

    #[test]
    fn test_vec3i_cross_product() {
        // i × j = k
        let i = Vec3i::new(1, 0, 0);
        let j = Vec3i::new(0, 1, 0);
        let k = Vec3i::new(0, 0, 1);
        assert_eq!(i.cross(j), k);
        assert_eq!(j.cross(k), i);
        assert_eq!(k.cross(i), j);
    }

    #[test]
    fn test_vec3i_cross_product_anticommutative() {
        let a = Vec3i::new(1, 2, 3);
        let b = Vec3i::new(4, 5, 6);
        let ab = a.cross(b);
        let ba = b.cross(a);
        assert_eq!(ab, Vec3i::new(-ba.x, -ba.y, -ba.z));
    }

    // ── Distances ───────────────────────────────────────────────────

    #[test]
    fn test_vec3i_dist_sqr() {
        let a = Vec3i::new(1, 2, 3);
        let b = Vec3i::new(4, 6, 3);
        // (3² + 4² + 0²) = 25
        assert_eq!(a.dist_sqr(b), 25);
    }

    #[test]
    fn test_vec3i_dist_sqr_large_coords() {
        // i32::MAX - 0 = 2_147_483_647, squared fits in i64
        let a = Vec3i::new(i32::MAX, 0, 0);
        let b = Vec3i::ZERO;
        let expected = (i32::MAX as i64) * (i32::MAX as i64);
        assert_eq!(a.dist_sqr(b), expected);
    }

    #[test]
    fn test_vec3i_dist_manhattan() {
        let a = Vec3i::new(1, 2, 3);
        let b = Vec3i::new(4, 6, 3);
        assert_eq!(a.dist_manhattan(b), 7); // 3 + 4 + 0
    }

    #[test]
    fn test_vec3i_dist_chessboard() {
        let a = Vec3i::new(1, 2, 3);
        let b = Vec3i::new(4, 6, 3);
        assert_eq!(a.dist_chessboard(b), 4); // max(3, 4, 0)
    }

    #[test]
    fn test_vec3i_dist_manhattan_no_overflow() {
        // This would panic with i32 arithmetic: i32::MAX - i32::MIN overflows.
        let a = Vec3i::new(i32::MAX, 0, 0);
        let b = Vec3i::new(i32::MIN, 0, 0);
        let expected = i64::from(i32::MAX) - i64::from(i32::MIN);
        assert_eq!(a.dist_manhattan(b), expected);
    }

    #[test]
    fn test_vec3i_dist_chessboard_no_overflow() {
        let a = Vec3i::new(i32::MAX, 0, 0);
        let b = Vec3i::new(i32::MIN, 0, 0);
        let expected = i64::from(i32::MAX) - i64::from(i32::MIN);
        assert_eq!(a.dist_chessboard(b), expected);
    }

    // ── Axis get ────────────────────────────────────────────────────

    #[test]
    fn test_vec3i_get_axis() {
        let v = Vec3i::new(10, 20, 30);
        assert_eq!(v.get(Axis::X), 10);
        assert_eq!(v.get(Axis::Y), 20);
        assert_eq!(v.get(Axis::Z), 30);
    }

    // ── Multiply ────────────────────────────────────────────────────

    #[test]
    fn test_vec3i_multiply() {
        let v = Vec3i::new(1, 2, 3).multiply(3);
        assert_eq!(v, Vec3i::new(3, 6, 9));
    }

    // ── Add / Sub traits ────────────────────────────────────────────

    #[test]
    fn test_vec3i_add() {
        let a = Vec3i::new(1, 2, 3);
        let b = Vec3i::new(4, 5, 6);
        assert_eq!(a + b, Vec3i::new(5, 7, 9));
    }

    #[test]
    fn test_vec3i_sub() {
        let a = Vec3i::new(4, 5, 6);
        let b = Vec3i::new(1, 2, 3);
        assert_eq!(a - b, Vec3i::new(3, 3, 3));
    }

    // ── Display ─────────────────────────────────────────────────────

    #[test]
    fn test_vec3i_display() {
        let v = Vec3i::new(1, -2, 3);
        assert_eq!(format!("{v}"), "(1, -2, 3)");
    }

    // ── Wire roundtrip ──────────────────────────────────────────────

    #[test]
    fn test_vec3i_wire_roundtrip() {
        let v = Vec3i::new(100, -200, 300);
        let mut buf = BytesMut::new();
        v.write(&mut buf);
        let mut data = buf.freeze();
        let decoded = Vec3i::read(&mut data).unwrap();
        assert_eq!(decoded, v);
    }

    #[test]
    fn test_vec3i_wire_roundtrip_zero() {
        let v = Vec3i::ZERO;
        let mut buf = BytesMut::new();
        v.write(&mut buf);
        let mut data = buf.freeze();
        let decoded = Vec3i::read(&mut data).unwrap();
        assert_eq!(decoded, v);
    }
}
