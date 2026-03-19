//! [`Vec3`] — an immutable double-precision 3D vector.
//!
//! Used for entity positions, velocities, and other continuous spatial
//! values in the Minecraft protocol.

use std::fmt;

use bytes::{Bytes, BytesMut};

use crate::codec::types::{self, TypeError};

use super::direction::Axis;
use super::vec3i::Vec3i;

/// An immutable double-precision 3D vector for positions and velocities.
///
/// # Wire format
///
/// Three consecutive big-endian `f64` values (24 bytes total).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    /// The X component.
    pub x: f64,
    /// The Y component.
    pub y: f64,
    /// The Z component.
    pub z: f64,
}

impl Vec3 {
    /// The zero vector `(0, 0, 0)`.
    pub const ZERO: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// Unit vector along the X axis.
    pub const X_AXIS: Vec3 = Vec3 {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    };

    /// Unit vector along the Y axis.
    pub const Y_AXIS: Vec3 = Vec3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };

    /// Unit vector along the Z axis.
    pub const Z_AXIS: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    };

    /// Creates a new `Vec3`.
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Returns a new vector with the given offsets added.
    pub fn add(self, dx: f64, dy: f64, dz: f64) -> Self {
        Self::new(self.x + dx, self.y + dy, self.z + dz)
    }

    /// Returns the sum of this vector and `other`.
    pub fn add_vec(self, other: Vec3) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    /// Returns a new vector with the given offsets subtracted.
    pub fn subtract(self, dx: f64, dy: f64, dz: f64) -> Self {
        Self::new(self.x - dx, self.y - dy, self.z - dz)
    }

    /// Returns the difference of this vector and `other`.
    pub fn subtract_vec(self, other: Vec3) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    /// Returns the negation of this vector.
    pub fn negate(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }

    /// Returns a new vector scaled by `factor`.
    pub fn scale(self, factor: f64) -> Self {
        Self::new(self.x * factor, self.y * factor, self.z * factor)
    }

    /// Returns a new vector with each component multiplied independently.
    pub fn multiply(self, x: f64, y: f64, z: f64) -> Self {
        Self::new(self.x * x, self.y * y, self.z * z)
    }

    /// Returns the dot product of this vector and `other`.
    pub fn dot(self, other: Vec3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Returns the cross product of this vector and `other`.
    pub fn cross(self, other: Vec3) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    /// Returns the normalized (unit-length) vector.
    ///
    /// Returns [`Vec3::ZERO`] if the length is less than `1e-8`.
    pub fn normalize(self) -> Self {
        let len = self.length();
        if len < 1e-8 {
            return Vec3::ZERO;
        }
        self.scale(1.0 / len)
    }

    /// Returns the Euclidean length of this vector.
    pub fn length(self) -> f64 {
        self.length_sqr().sqrt()
    }

    /// Returns the squared Euclidean length of this vector.
    pub fn length_sqr(self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    /// Returns the Euclidean distance to `other`.
    pub fn distance_to(self, other: Vec3) -> f64 {
        self.distance_to_sqr(other).sqrt()
    }

    /// Returns the squared Euclidean distance to `other`.
    pub fn distance_to_sqr(self, other: Vec3) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        dx * dx + dy * dy + dz * dz
    }

    /// Returns the horizontal distance (in the XZ plane).
    pub fn horizontal_distance(self) -> f64 {
        self.horizontal_distance_sqr().sqrt()
    }

    /// Returns the squared horizontal distance (in the XZ plane).
    pub fn horizontal_distance_sqr(self) -> f64 {
        self.x * self.x + self.z * self.z
    }

    /// Returns `true` if the distance to `other` is less than `distance`.
    pub fn closer_than(self, other: Vec3, distance: f64) -> bool {
        self.distance_to_sqr(other) < distance * distance
    }

    /// Rotates this vector around the X axis by `radians`.
    pub fn x_rot(self, radians: f64) -> Self {
        let cos = radians.cos();
        let sin = radians.sin();
        Self::new(
            self.x,
            self.y * cos + self.z * sin,
            self.z * cos - self.y * sin,
        )
    }

    /// Rotates this vector around the Y axis by `radians`.
    pub fn y_rot(self, radians: f64) -> Self {
        let cos = radians.cos();
        let sin = radians.sin();
        Self::new(
            self.x * cos + self.z * sin,
            self.y,
            self.z * cos - self.x * sin,
        )
    }

    /// Rotates this vector around the Z axis by `radians`.
    pub fn z_rot(self, radians: f64) -> Self {
        let cos = radians.cos();
        let sin = radians.sin();
        Self::new(
            self.x * cos + self.y * sin,
            self.y * cos - self.x * sin,
            self.z,
        )
    }

    /// Converts an integer position to its lower corner in double coordinates.
    pub fn at_lower_corner_of(pos: &Vec3i) -> Self {
        Self::new(f64::from(pos.x), f64::from(pos.y), f64::from(pos.z))
    }

    /// Converts an integer position to its center (each component + 0.5).
    pub fn at_center_of(pos: &Vec3i) -> Self {
        Self::new(
            f64::from(pos.x) + 0.5,
            f64::from(pos.y) + 0.5,
            f64::from(pos.z) + 0.5,
        )
    }

    /// Reads a `Vec3` from a wire buffer (3× big-endian `f64`).
    ///
    /// # Errors
    ///
    /// Returns [`TypeError`] if fewer than 24 bytes remain.
    pub fn read(buf: &mut Bytes) -> Result<Self, TypeError> {
        let x = types::read_f64(buf)?;
        let y = types::read_f64(buf)?;
        let z = types::read_f64(buf)?;
        Ok(Self { x, y, z })
    }

    /// Writes this `Vec3` to a wire buffer (3× big-endian `f64`).
    pub fn write(&self, buf: &mut BytesMut) {
        types::write_f64(buf, self.x);
        types::write_f64(buf, self.y);
        types::write_f64(buf, self.z);
    }
}

impl_vector_ops!(Vec3);
impl_axis_accessor!(Vec3, f64);

impl fmt::Display for Vec3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use super::*;

    const EPSILON: f64 = 1e-10;

    fn approx_eq(a: Vec3, b: Vec3) -> bool {
        (a.x - b.x).abs() < EPSILON && (a.y - b.y).abs() < EPSILON && (a.z - b.z).abs() < EPSILON
    }

    // ── Construction ─────────────────────────────────────────────────

    #[test]
    fn test_vec3_new() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 2.0);
        assert_eq!(v.z, 3.0);
    }

    #[test]
    fn test_vec3_constants() {
        assert_eq!(Vec3::ZERO, Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(Vec3::X_AXIS, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(Vec3::Y_AXIS, Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(Vec3::Z_AXIS, Vec3::new(0.0, 0.0, 1.0));
    }

    // ── Arithmetic ──────────────────────────────────────────────────

    #[test]
    fn test_vec3_add() {
        let v = Vec3::new(1.0, 2.0, 3.0).add(4.0, 5.0, 6.0);
        assert_eq!(v, Vec3::new(5.0, 7.0, 9.0));
    }

    #[test]
    fn test_vec3_add_vec() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        assert_eq!(a.add_vec(b), Vec3::new(5.0, 7.0, 9.0));
    }

    #[test]
    fn test_vec3_subtract() {
        let v = Vec3::new(5.0, 7.0, 9.0).subtract(4.0, 5.0, 6.0);
        assert_eq!(v, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_vec3_subtract_vec() {
        let a = Vec3::new(5.0, 7.0, 9.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        assert_eq!(a.subtract_vec(b), Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_vec3_scale() {
        let v = Vec3::new(1.0, 2.0, 3.0).scale(2.0);
        assert_eq!(v, Vec3::new(2.0, 4.0, 6.0));
    }

    #[test]
    fn test_vec3_multiply() {
        let v = Vec3::new(2.0, 3.0, 4.0).multiply(1.0, 2.0, 3.0);
        assert_eq!(v, Vec3::new(2.0, 6.0, 12.0));
    }

    // ── Dot product ─────────────────────────────────────────────────

    #[test]
    fn test_vec3_dot_orthogonal() {
        assert_eq!(Vec3::X_AXIS.dot(Vec3::Y_AXIS), 0.0);
        assert_eq!(Vec3::X_AXIS.dot(Vec3::Z_AXIS), 0.0);
        assert_eq!(Vec3::Y_AXIS.dot(Vec3::Z_AXIS), 0.0);
    }

    #[test]
    fn test_vec3_dot_parallel() {
        let v = Vec3::new(3.0, 4.0, 0.0);
        assert!((v.dot(v) - v.length_sqr()).abs() < EPSILON);
    }

    // ── Cross product ───────────────────────────────────────────────

    #[test]
    fn test_vec3_cross_basis() {
        // i × j = k
        assert!(approx_eq(Vec3::X_AXIS.cross(Vec3::Y_AXIS), Vec3::Z_AXIS));
        // j × k = i
        assert!(approx_eq(Vec3::Y_AXIS.cross(Vec3::Z_AXIS), Vec3::X_AXIS));
        // k × i = j
        assert!(approx_eq(Vec3::Z_AXIS.cross(Vec3::X_AXIS), Vec3::Y_AXIS));
    }

    // ── Normalize ───────────────────────────────────────────────────

    #[test]
    fn test_vec3_normalize_unit() {
        let v = Vec3::new(3.0, 4.0, 0.0).normalize();
        assert!((v.length() - 1.0).abs() < EPSILON);
        assert!((v.x - 0.6).abs() < EPSILON);
        assert!((v.y - 0.8).abs() < EPSILON);
    }

    #[test]
    fn test_vec3_normalize_zero_returns_zero() {
        assert_eq!(Vec3::ZERO.normalize(), Vec3::ZERO);
    }

    // ── Length ───────────────────────────────────────────────────────

    #[test]
    fn test_vec3_length() {
        let v = Vec3::new(3.0, 4.0, 0.0);
        assert!((v.length() - 5.0).abs() < EPSILON);
    }

    #[test]
    fn test_vec3_length_sqr() {
        let v = Vec3::new(3.0, 4.0, 0.0);
        assert!((v.length_sqr() - 25.0).abs() < EPSILON);
    }

    // ── Distances ───────────────────────────────────────────────────

    #[test]
    fn test_vec3_distance_to() {
        let a = Vec3::new(1.0, 0.0, 0.0);
        let b = Vec3::new(4.0, 4.0, 0.0);
        assert!((a.distance_to(b) - 5.0).abs() < EPSILON);
    }

    #[test]
    fn test_vec3_distance_to_sqr() {
        let a = Vec3::new(1.0, 0.0, 0.0);
        let b = Vec3::new(4.0, 4.0, 0.0);
        assert!((a.distance_to_sqr(b) - 25.0).abs() < EPSILON);
    }

    #[test]
    fn test_vec3_horizontal_distance() {
        let v = Vec3::new(3.0, 100.0, 4.0);
        assert!((v.horizontal_distance() - 5.0).abs() < EPSILON);
    }

    #[test]
    fn test_vec3_horizontal_distance_sqr() {
        let v = Vec3::new(3.0, 100.0, 4.0);
        assert!((v.horizontal_distance_sqr() - 25.0).abs() < EPSILON);
    }

    #[test]
    fn test_vec3_closer_than() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(1.0, 0.0, 0.0);
        assert!(a.closer_than(b, 1.5));
        assert!(!a.closer_than(b, 0.5));
    }

    // ── Axis access ─────────────────────────────────────────────────

    #[test]
    fn test_vec3_get_axis() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(v.get_axis(Axis::X), 1.0);
        assert_eq!(v.get_axis(Axis::Y), 2.0);
        assert_eq!(v.get_axis(Axis::Z), 3.0);
    }

    #[test]
    fn test_vec3_with_axis() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(v.with_axis(Axis::X, 10.0), Vec3::new(10.0, 2.0, 3.0));
        assert_eq!(v.with_axis(Axis::Y, 10.0), Vec3::new(1.0, 10.0, 3.0));
        assert_eq!(v.with_axis(Axis::Z, 10.0), Vec3::new(1.0, 2.0, 10.0));
    }

    // ── Rotations ───────────────────────────────────────────────────

    #[test]
    fn test_vec3_x_rot_90() {
        // Rotating Y_AXIS 90° around X → Z_AXIS (with sign from convention)
        let v = Vec3::Y_AXIS.x_rot(FRAC_PI_2);
        assert!(approx_eq(v, Vec3::new(0.0, 0.0, -1.0)));
    }

    #[test]
    fn test_vec3_y_rot_90() {
        // Rotating X_AXIS 90° around Y → Z_AXIS direction
        let v = Vec3::X_AXIS.y_rot(FRAC_PI_2);
        assert!(approx_eq(v, Vec3::new(0.0, 0.0, -1.0)));
    }

    #[test]
    fn test_vec3_z_rot_90() {
        // Rotating X_AXIS 90° around Z → -Y_AXIS
        let v = Vec3::X_AXIS.z_rot(FRAC_PI_2);
        assert!(approx_eq(v, Vec3::new(0.0, -1.0, 0.0)));
    }

    // ── Conversion from Vec3i ───────────────────────────────────────

    #[test]
    fn test_vec3_at_lower_corner_of() {
        let pos = Vec3i::new(10, 20, 30);
        assert_eq!(Vec3::at_lower_corner_of(&pos), Vec3::new(10.0, 20.0, 30.0));
    }

    #[test]
    fn test_vec3_at_center_of() {
        let pos = Vec3i::new(10, 20, 30);
        assert_eq!(Vec3::at_center_of(&pos), Vec3::new(10.5, 20.5, 30.5));
    }

    // ── Operator traits ─────────────────────────────────────────────

    #[test]
    fn test_vec3_add_trait() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        assert_eq!(a + b, Vec3::new(5.0, 7.0, 9.0));
    }

    #[test]
    fn test_vec3_sub_trait() {
        let a = Vec3::new(5.0, 7.0, 9.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        assert_eq!(a - b, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_vec3_neg_trait() {
        let v = Vec3::new(1.0, -2.0, 3.0);
        assert_eq!(-v, Vec3::new(-1.0, 2.0, -3.0));
    }

    // ── Display ─────────────────────────────────────────────────────

    #[test]
    fn test_vec3_display() {
        let v = Vec3::new(1.5, -2.0, 3.25);
        assert_eq!(format!("{v}"), "(1.5, -2, 3.25)");
    }

    // ── Wire roundtrip ──────────────────────────────────────────────

    #[test]
    fn test_vec3_wire_roundtrip() {
        let v = Vec3::new(1.5, -2.5, 3.75);
        let mut buf = BytesMut::new();
        v.write(&mut buf);
        assert_eq!(buf.len(), 24);
        let mut data = buf.freeze();
        let decoded = Vec3::read(&mut data).unwrap();
        assert_eq!(decoded, v);
    }

    #[test]
    fn test_vec3_wire_roundtrip_zero() {
        let v = Vec3::ZERO;
        let mut buf = BytesMut::new();
        v.write(&mut buf);
        let mut data = buf.freeze();
        let decoded = Vec3::read(&mut data).unwrap();
        assert_eq!(decoded, v);
    }
}
