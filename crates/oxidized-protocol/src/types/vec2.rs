//! [`Vec2`] — an immutable 2D float vector.
//!
//! Typically used for yaw/pitch angles and other 2D quantities in the
//! Minecraft protocol.

use std::fmt;

use bytes::{Bytes, BytesMut};

use crate::codec::types::{self, TypeError};

/// An immutable 2D float vector, typically used for yaw/pitch angles.
///
/// # Wire format
///
/// Two consecutive big-endian `f32` values (8 bytes total).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    /// The X component (often yaw).
    pub x: f32,
    /// The Y component (often pitch).
    pub y: f32,
}

impl Vec2 {
    /// The zero vector `(0, 0)`.
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    /// The one vector `(1, 1)`.
    pub const ONE: Vec2 = Vec2 { x: 1.0, y: 1.0 };

    /// Unit vector along the X axis.
    pub const UNIT_X: Vec2 = Vec2 { x: 1.0, y: 0.0 };

    /// Negative unit vector along the X axis.
    pub const NEG_UNIT_X: Vec2 = Vec2 { x: -1.0, y: 0.0 };

    /// Unit vector along the Y axis.
    pub const UNIT_Y: Vec2 = Vec2 { x: 0.0, y: 1.0 };

    /// Negative unit vector along the Y axis.
    pub const NEG_UNIT_Y: Vec2 = Vec2 { x: 0.0, y: -1.0 };

    /// Maximum representable vector.
    pub const MAX: Vec2 = Vec2 {
        x: f32::MAX,
        y: f32::MAX,
    };

    /// Minimum representable vector.
    pub const MIN: Vec2 = Vec2 {
        x: f32::MIN,
        y: f32::MIN,
    };

    /// Creates a new `Vec2`.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Returns a new vector scaled by `factor`.
    pub fn scale(self, factor: f32) -> Self {
        Self::new(self.x * factor, self.y * factor)
    }

    /// Returns the sum of this vector and `other`.
    #[allow(clippy::should_implement_trait)]
    pub fn add(self, other: Vec2) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }

    /// Returns a new vector with `v` added to both components.
    pub fn add_scalar(self, v: f32) -> Self {
        Self::new(self.x + v, self.y + v)
    }

    /// Returns the dot product of this vector and `other`.
    pub fn dot(self, other: Vec2) -> f32 {
        self.x * other.x + self.y * other.y
    }

    /// Returns the negated vector.
    pub fn negated(self) -> Self {
        Self::new(-self.x, -self.y)
    }

    /// Returns the normalized (unit-length) vector.
    ///
    /// Returns [`Vec2::ZERO`] if the length is less than `1e-4`.
    pub fn normalized(self) -> Self {
        let len = self.length();
        if len < 1e-4 {
            return Vec2::ZERO;
        }
        self.scale(1.0 / len)
    }

    /// Returns the Euclidean length of this vector.
    pub fn length(self) -> f32 {
        self.length_sqr().sqrt()
    }

    /// Returns the squared Euclidean length of this vector.
    pub fn length_sqr(self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    /// Returns the squared Euclidean distance to `other`.
    pub fn distance_to_sqr(self, other: Vec2) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    /// Reads a `Vec2` from a wire buffer (2× big-endian `f32`).
    ///
    /// # Errors
    ///
    /// Returns [`TypeError`] if fewer than 8 bytes remain.
    pub fn read(buf: &mut Bytes) -> Result<Self, TypeError> {
        let x = types::read_f32(buf)?;
        let y = types::read_f32(buf)?;
        Ok(Self { x, y })
    }

    /// Writes this `Vec2` to a wire buffer (2× big-endian `f32`).
    pub fn write(&self, buf: &mut BytesMut) {
        types::write_f32(buf, self.x);
        types::write_f32(buf, self.y);
    }
}

impl fmt::Display for Vec2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-6;

    // ── Construction ─────────────────────────────────────────────────

    #[test]
    fn test_vec2_new() {
        let v = Vec2::new(1.0, 2.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 2.0);
    }

    #[test]
    fn test_vec2_constants() {
        assert_eq!(Vec2::ZERO, Vec2::new(0.0, 0.0));
        assert_eq!(Vec2::ONE, Vec2::new(1.0, 1.0));
        assert_eq!(Vec2::UNIT_X, Vec2::new(1.0, 0.0));
        assert_eq!(Vec2::NEG_UNIT_X, Vec2::new(-1.0, 0.0));
        assert_eq!(Vec2::UNIT_Y, Vec2::new(0.0, 1.0));
        assert_eq!(Vec2::NEG_UNIT_Y, Vec2::new(0.0, -1.0));
        assert_eq!(Vec2::MAX, Vec2::new(f32::MAX, f32::MAX));
        assert_eq!(Vec2::MIN, Vec2::new(f32::MIN, f32::MIN));
    }

    // ── Scale ───────────────────────────────────────────────────────

    #[test]
    fn test_vec2_scale() {
        let v = Vec2::new(2.0, 3.0).scale(4.0);
        assert_eq!(v, Vec2::new(8.0, 12.0));
    }

    // ── Add ─────────────────────────────────────────────────────────

    #[test]
    fn test_vec2_add() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(3.0, 4.0);
        assert_eq!(a.add(b), Vec2::new(4.0, 6.0));
    }

    #[test]
    fn test_vec2_add_scalar() {
        let v = Vec2::new(1.0, 2.0).add_scalar(5.0);
        assert_eq!(v, Vec2::new(6.0, 7.0));
    }

    // ── Dot product ─────────────────────────────────────────────────

    #[test]
    fn test_vec2_dot_orthogonal() {
        assert_eq!(Vec2::UNIT_X.dot(Vec2::UNIT_Y), 0.0);
    }

    #[test]
    fn test_vec2_dot_parallel() {
        let v = Vec2::new(3.0, 4.0);
        assert!((v.dot(v) - v.length_sqr()).abs() < EPSILON);
    }

    // ── Negate ──────────────────────────────────────────────────────

    #[test]
    fn test_vec2_negated() {
        let v = Vec2::new(1.0, -2.0).negated();
        assert_eq!(v, Vec2::new(-1.0, 2.0));
    }

    // ── Normalize ───────────────────────────────────────────────────

    #[test]
    fn test_vec2_normalized() {
        let v = Vec2::new(3.0, 4.0).normalized();
        assert!((v.length() - 1.0).abs() < EPSILON);
        assert!((v.x - 0.6).abs() < EPSILON);
        assert!((v.y - 0.8).abs() < EPSILON);
    }

    #[test]
    fn test_vec2_normalized_zero_returns_zero() {
        assert_eq!(Vec2::ZERO.normalized(), Vec2::ZERO);
    }

    #[test]
    fn test_vec2_normalized_tiny_returns_zero() {
        let v = Vec2::new(1e-5, 0.0).normalized();
        assert_eq!(v, Vec2::ZERO);
    }

    // ── Length ───────────────────────────────────────────────────────

    #[test]
    fn test_vec2_length() {
        let v = Vec2::new(3.0, 4.0);
        assert!((v.length() - 5.0).abs() < EPSILON);
    }

    #[test]
    fn test_vec2_length_sqr() {
        let v = Vec2::new(3.0, 4.0);
        assert!((v.length_sqr() - 25.0).abs() < EPSILON);
    }

    // ── Distance ────────────────────────────────────────────────────

    #[test]
    fn test_vec2_distance_to_sqr() {
        let a = Vec2::new(1.0, 0.0);
        let b = Vec2::new(4.0, 4.0);
        assert!((a.distance_to_sqr(b) - 25.0).abs() < EPSILON);
    }

    // ── Display ─────────────────────────────────────────────────────

    #[test]
    fn test_vec2_display() {
        let v = Vec2::new(1.5, -2.0);
        assert_eq!(format!("{v}"), "(1.5, -2)");
    }

    // ── Wire roundtrip ──────────────────────────────────────────────

    #[test]
    fn test_vec2_wire_roundtrip() {
        let v = Vec2::new(1.5, -2.5);
        let mut buf = BytesMut::new();
        v.write(&mut buf);
        assert_eq!(buf.len(), 8);
        let mut data = buf.freeze();
        let decoded = Vec2::read(&mut data).unwrap();
        assert_eq!(decoded, v);
    }

    #[test]
    fn test_vec2_wire_roundtrip_zero() {
        let v = Vec2::ZERO;
        let mut buf = BytesMut::new();
        v.write(&mut buf);
        let mut data = buf.freeze();
        let decoded = Vec2::read(&mut data).unwrap();
        assert_eq!(decoded, v);
    }
}
