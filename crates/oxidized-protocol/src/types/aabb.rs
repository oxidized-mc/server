//! [`Aabb`] — an immutable axis-aligned bounding box.
//!
//! Used for entity collision detection, hit testing, and spatial queries.
//! Auto-corrects on construction so `min <= max` for all axes.

use std::fmt;

use super::block_pos::BlockPos;
use super::direction::Axis;
use super::vec3::Vec3;

/// An axis-aligned bounding box defined by minimum and maximum coordinates.
///
/// Auto-corrects on construction so `min <= max` for all axes.
/// This type is not sent over the network directly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    /// The minimum X coordinate.
    pub min_x: f64,
    /// The minimum Y coordinate.
    pub min_y: f64,
    /// The minimum Z coordinate.
    pub min_z: f64,
    /// The maximum X coordinate.
    pub max_x: f64,
    /// The maximum Y coordinate.
    pub max_y: f64,
    /// The maximum Z coordinate.
    pub max_z: f64,
}

impl Aabb {
    /// Creates a new [`Aabb`], auto-correcting so `min <= max` for each axis.
    pub fn new(x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64) -> Self {
        Self {
            min_x: x1.min(x2),
            min_y: y1.min(y2),
            min_z: z1.min(z2),
            max_x: x1.max(x2),
            max_y: y1.max(y2),
            max_z: z1.max(z2),
        }
    }

    /// Creates a unit cube (1×1×1) at the given [`BlockPos`].
    pub fn from_block_pos(pos: &BlockPos) -> Self {
        Self {
            min_x: f64::from(pos.x),
            min_y: f64::from(pos.y),
            min_z: f64::from(pos.z),
            max_x: f64::from(pos.x) + 1.0,
            max_y: f64::from(pos.y) + 1.0,
            max_z: f64::from(pos.z) + 1.0,
        }
    }

    /// Creates an [`Aabb`] spanning between two [`Vec3`] corners.
    pub fn from_vec3(a: Vec3, b: Vec3) -> Self {
        Self::new(a.x, a.y, a.z, b.x, b.y, b.z)
    }

    /// Creates a unit cube (1×1×1) at the given coordinates.
    pub fn unit_cube_at(x: f64, y: f64, z: f64) -> Self {
        Self {
            min_x: x,
            min_y: y,
            min_z: z,
            max_x: x + 1.0,
            max_y: y + 1.0,
            max_z: z + 1.0,
        }
    }

    /// Returns the size along the X axis.
    pub fn x_size(&self) -> f64 {
        self.max_x - self.min_x
    }

    /// Returns the size along the Y axis.
    pub fn y_size(&self) -> f64 {
        self.max_y - self.min_y
    }

    /// Returns the size along the Z axis.
    pub fn z_size(&self) -> f64 {
        self.max_z - self.min_z
    }

    /// Returns the average of the three axis sizes (as in vanilla).
    pub fn size(&self) -> f64 {
        (self.x_size() + self.y_size() + self.z_size()) / 3.0
    }

    /// Returns the center of this bounding box.
    pub fn get_center(&self) -> Vec3 {
        Vec3::new(
            (self.min_x + self.max_x) / 2.0,
            (self.min_y + self.max_y) / 2.0,
            (self.min_z + self.max_z) / 2.0,
        )
    }

    /// Returns the bottom center of this bounding box (center X/Z, min Y).
    pub fn get_bottom_center(&self) -> Vec3 {
        Vec3::new(
            (self.min_x + self.max_x) / 2.0,
            self.min_y,
            (self.min_z + self.max_z) / 2.0,
        )
    }

    /// Returns the minimum corner as a [`Vec3`].
    pub fn get_min_position(&self) -> Vec3 {
        Vec3::new(self.min_x, self.min_y, self.min_z)
    }

    /// Returns the maximum corner as a [`Vec3`].
    pub fn get_max_position(&self) -> Vec3 {
        Vec3::new(self.max_x, self.max_y, self.max_z)
    }

    /// Returns the minimum value on the given axis.
    pub fn min_axis(&self, axis: Axis) -> f64 {
        match axis {
            Axis::X => self.min_x,
            Axis::Y => self.min_y,
            Axis::Z => self.min_z,
        }
    }

    /// Returns the maximum value on the given axis.
    pub fn max_axis(&self, axis: Axis) -> f64 {
        match axis {
            Axis::X => self.max_x,
            Axis::Y => self.max_y,
            Axis::Z => self.max_z,
        }
    }

    /// Returns a new [`Aabb`] expanded equally on all sides by `amount`.
    pub fn inflate(&self, amount: f64) -> Self {
        self.inflate_xyz(amount, amount, amount)
    }

    /// Returns a new [`Aabb`] expanded by the given amounts on each axis.
    pub fn inflate_xyz(&self, x: f64, y: f64, z: f64) -> Self {
        Self {
            min_x: self.min_x - x,
            min_y: self.min_y - y,
            min_z: self.min_z - z,
            max_x: self.max_x + x,
            max_y: self.max_y + y,
            max_z: self.max_z + z,
        }
    }

    /// Returns a new [`Aabb`] shrunk equally on all sides by `amount`.
    pub fn deflate(&self, amount: f64) -> Self {
        self.inflate(-amount)
    }

    /// Returns a new [`Aabb`] extended in the direction of a velocity vector.
    ///
    /// For each axis, if the delta is negative the min is extended; if positive
    /// the max is extended.
    pub fn expand_towards(&self, dx: f64, dy: f64, dz: f64) -> Self {
        let mut min_x = self.min_x;
        let mut max_x = self.max_x;
        let mut min_y = self.min_y;
        let mut max_y = self.max_y;
        let mut min_z = self.min_z;
        let mut max_z = self.max_z;

        if dx < 0.0 {
            min_x += dx;
        } else if dx > 0.0 {
            max_x += dx;
        }

        if dy < 0.0 {
            min_y += dy;
        } else if dy > 0.0 {
            max_y += dy;
        }

        if dz < 0.0 {
            min_z += dz;
        } else if dz > 0.0 {
            max_z += dz;
        }

        Self {
            min_x,
            min_y,
            min_z,
            max_x,
            max_y,
            max_z,
        }
    }

    /// Returns a new [`Aabb`] contracted by the given amounts.
    ///
    /// For each axis, if the delta is negative the min side shrinks inward
    /// (min increases); if positive the max side shrinks inward (max decreases).
    pub fn contract(&self, dx: f64, dy: f64, dz: f64) -> Self {
        let mut min_x = self.min_x;
        let mut max_x = self.max_x;
        let mut min_y = self.min_y;
        let mut max_y = self.max_y;
        let mut min_z = self.min_z;
        let mut max_z = self.max_z;

        if dx < 0.0 {
            min_x -= dx;
        } else if dx > 0.0 {
            max_x -= dx;
        }

        if dy < 0.0 {
            min_y -= dy;
        } else if dy > 0.0 {
            max_y -= dy;
        }

        if dz < 0.0 {
            min_z -= dz;
        } else if dz > 0.0 {
            max_z -= dz;
        }

        Self {
            min_x,
            min_y,
            min_z,
            max_x,
            max_y,
            max_z,
        }
    }

    /// Returns a new [`Aabb`] translated by `(dx, dy, dz)`.
    pub fn move_by(&self, dx: f64, dy: f64, dz: f64) -> Self {
        Self {
            min_x: self.min_x + dx,
            min_y: self.min_y + dy,
            min_z: self.min_z + dz,
            max_x: self.max_x + dx,
            max_y: self.max_y + dy,
            max_z: self.max_z + dz,
        }
    }

    /// Returns a new [`Aabb`] translated by the given [`Vec3`].
    pub fn move_vec(&self, delta: Vec3) -> Self {
        self.move_by(delta.x, delta.y, delta.z)
    }

    /// Returns `true` if this bounding box overlaps with `other`.
    ///
    /// Touching (sharing an edge or face) is **not** considered overlapping.
    pub fn intersects(&self, other: &Aabb) -> bool {
        self.intersects_range(
            other.min_x,
            other.min_y,
            other.min_z,
            other.max_x,
            other.max_y,
            other.max_z,
        )
    }

    /// Returns `true` if this bounding box overlaps with the given range.
    pub fn intersects_range(
        &self,
        min_x: f64,
        min_y: f64,
        min_z: f64,
        max_x: f64,
        max_y: f64,
        max_z: f64,
    ) -> bool {
        self.min_x < max_x
            && self.max_x > min_x
            && self.min_y < max_y
            && self.max_y > min_y
            && self.min_z < max_z
            && self.max_z > min_z
    }

    /// Returns `true` if the point `(x, y, z)` is inside this bounding box.
    ///
    /// Points on the boundary are considered inside.
    pub fn contains(&self, x: f64, y: f64, z: f64) -> bool {
        x >= self.min_x
            && x <= self.max_x
            && y >= self.min_y
            && y <= self.max_y
            && z >= self.min_z
            && z <= self.max_z
    }

    /// Returns `true` if the given [`Vec3`] is inside this bounding box.
    pub fn contains_vec(&self, pos: Vec3) -> bool {
        self.contains(pos.x, pos.y, pos.z)
    }

    /// Returns the intersection (overlap region) of this bounding box and `other`.
    pub fn intersect(&self, other: &Aabb) -> Self {
        Self {
            min_x: self.min_x.max(other.min_x),
            min_y: self.min_y.max(other.min_y),
            min_z: self.min_z.max(other.min_z),
            max_x: self.max_x.min(other.max_x),
            max_y: self.max_y.min(other.max_y),
            max_z: self.max_z.min(other.max_z),
        }
    }

    /// Returns the smallest bounding box that contains both `self` and `other`.
    pub fn minmax(&self, other: &Aabb) -> Self {
        Self {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            min_z: self.min_z.min(other.min_z),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
            max_z: self.max_z.max(other.max_z),
        }
    }

    /// Returns the squared distance from the given point to the nearest
    /// surface of this bounding box.
    ///
    /// Returns `0.0` if the point is inside the box.
    pub fn distance_to_sqr(&self, pos: Vec3) -> f64 {
        let dx = 0.0_f64.max(self.min_x - pos.x).max(pos.x - self.max_x);
        let dy = 0.0_f64.max(self.min_y - pos.y).max(pos.y - self.max_y);
        let dz = 0.0_f64.max(self.min_z - pos.z).max(pos.z - self.max_z);
        dx * dx + dy * dy + dz * dz
    }
}

impl fmt::Display for Aabb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AABB[{}, {}, {} -> {}, {}, {}]",
            self.min_x, self.min_y, self.min_z, self.max_x, self.max_y, self.max_z
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-10;

    // ── Construction ─────────────────────────────────────────────────

    #[test]
    fn test_aabb_new_ordered() {
        let bb = Aabb::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
        assert_eq!(bb.min_x, 1.0);
        assert_eq!(bb.min_y, 2.0);
        assert_eq!(bb.min_z, 3.0);
        assert_eq!(bb.max_x, 4.0);
        assert_eq!(bb.max_y, 5.0);
        assert_eq!(bb.max_z, 6.0);
    }

    #[test]
    fn test_aabb_new_auto_correct() {
        let bb = Aabb::new(4.0, 5.0, 6.0, 1.0, 2.0, 3.0);
        assert_eq!(bb.min_x, 1.0);
        assert_eq!(bb.min_y, 2.0);
        assert_eq!(bb.min_z, 3.0);
        assert_eq!(bb.max_x, 4.0);
        assert_eq!(bb.max_y, 5.0);
        assert_eq!(bb.max_z, 6.0);
    }

    #[test]
    fn test_aabb_from_block_pos() {
        let bb = Aabb::from_block_pos(&BlockPos::new(3, 64, -5));
        assert_eq!(bb.min_x, 3.0);
        assert_eq!(bb.min_y, 64.0);
        assert_eq!(bb.min_z, -5.0);
        assert_eq!(bb.max_x, 4.0);
        assert_eq!(bb.max_y, 65.0);
        assert_eq!(bb.max_z, -4.0);
    }

    #[test]
    fn test_aabb_from_vec3() {
        let bb = Aabb::from_vec3(Vec3::new(5.0, 3.0, 1.0), Vec3::new(1.0, 5.0, 3.0));
        assert_eq!(bb.min_x, 1.0);
        assert_eq!(bb.min_y, 3.0);
        assert_eq!(bb.min_z, 1.0);
        assert_eq!(bb.max_x, 5.0);
        assert_eq!(bb.max_y, 5.0);
        assert_eq!(bb.max_z, 3.0);
    }

    #[test]
    fn test_aabb_unit_cube_at() {
        let bb = Aabb::unit_cube_at(2.0, 3.0, 4.0);
        assert_eq!(bb.min_x, 2.0);
        assert_eq!(bb.min_y, 3.0);
        assert_eq!(bb.min_z, 4.0);
        assert_eq!(bb.max_x, 3.0);
        assert_eq!(bb.max_y, 4.0);
        assert_eq!(bb.max_z, 5.0);
    }

    // ── Sizes ───────────────────────────────────────────────────────

    #[test]
    fn test_aabb_sizes() {
        let bb = Aabb::new(0.0, 0.0, 0.0, 2.0, 4.0, 6.0);
        assert!((bb.x_size() - 2.0).abs() < EPSILON);
        assert!((bb.y_size() - 4.0).abs() < EPSILON);
        assert!((bb.z_size() - 6.0).abs() < EPSILON);
        assert!((bb.size() - 4.0).abs() < EPSILON); // (2+4+6)/3 = 4
    }

    // ── Centers ─────────────────────────────────────────────────────

    #[test]
    fn test_aabb_get_center() {
        let bb = Aabb::new(0.0, 0.0, 0.0, 4.0, 6.0, 8.0);
        assert_eq!(bb.get_center(), Vec3::new(2.0, 3.0, 4.0));
    }

    #[test]
    fn test_aabb_get_bottom_center() {
        let bb = Aabb::new(0.0, 2.0, 0.0, 4.0, 6.0, 8.0);
        assert_eq!(bb.get_bottom_center(), Vec3::new(2.0, 2.0, 4.0));
    }

    #[test]
    fn test_aabb_get_min_max_position() {
        let bb = Aabb::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
        assert_eq!(bb.get_min_position(), Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(bb.get_max_position(), Vec3::new(4.0, 5.0, 6.0));
    }

    // ── Axis access ─────────────────────────────────────────────────

    #[test]
    fn test_aabb_min_max_axis() {
        let bb = Aabb::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
        assert_eq!(bb.min_axis(Axis::X), 1.0);
        assert_eq!(bb.min_axis(Axis::Y), 2.0);
        assert_eq!(bb.min_axis(Axis::Z), 3.0);
        assert_eq!(bb.max_axis(Axis::X), 4.0);
        assert_eq!(bb.max_axis(Axis::Y), 5.0);
        assert_eq!(bb.max_axis(Axis::Z), 6.0);
    }

    // ── Inflate / deflate ───────────────────────────────────────────

    #[test]
    fn test_aabb_inflate() {
        let bb = Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        let inflated = bb.inflate(0.5);
        assert!((inflated.min_x - (-0.5)).abs() < EPSILON);
        assert!((inflated.max_x - 1.5).abs() < EPSILON);
        assert!((inflated.min_y - (-0.5)).abs() < EPSILON);
        assert!((inflated.max_y - 1.5).abs() < EPSILON);
    }

    #[test]
    fn test_aabb_inflate_xyz() {
        let bb = Aabb::new(0.0, 0.0, 0.0, 2.0, 2.0, 2.0);
        let inflated = bb.inflate_xyz(1.0, 2.0, 3.0);
        assert!((inflated.min_x - (-1.0)).abs() < EPSILON);
        assert!((inflated.max_x - 3.0).abs() < EPSILON);
        assert!((inflated.min_y - (-2.0)).abs() < EPSILON);
        assert!((inflated.max_y - 4.0).abs() < EPSILON);
        assert!((inflated.min_z - (-3.0)).abs() < EPSILON);
        assert!((inflated.max_z - 5.0).abs() < EPSILON);
    }

    #[test]
    fn test_aabb_deflate() {
        let bb = Aabb::new(0.0, 0.0, 0.0, 4.0, 4.0, 4.0);
        let deflated = bb.deflate(1.0);
        assert!((deflated.min_x - 1.0).abs() < EPSILON);
        assert!((deflated.max_x - 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_aabb_inflate_deflate_symmetry() {
        let bb = Aabb::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
        let result = bb.inflate(2.0).deflate(2.0);
        assert!((result.min_x - bb.min_x).abs() < EPSILON);
        assert!((result.min_y - bb.min_y).abs() < EPSILON);
        assert!((result.min_z - bb.min_z).abs() < EPSILON);
        assert!((result.max_x - bb.max_x).abs() < EPSILON);
        assert!((result.max_y - bb.max_y).abs() < EPSILON);
        assert!((result.max_z - bb.max_z).abs() < EPSILON);
    }

    // ── expand_towards ──────────────────────────────────────────────

    #[test]
    fn test_aabb_expand_towards_positive() {
        let bb = Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        let expanded = bb.expand_towards(2.0, 0.0, 0.0);
        assert!((expanded.min_x - 0.0).abs() < EPSILON);
        assert!((expanded.max_x - 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_aabb_expand_towards_negative() {
        let bb = Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        let expanded = bb.expand_towards(-2.0, 0.0, 0.0);
        assert!((expanded.min_x - (-2.0)).abs() < EPSILON);
        assert!((expanded.max_x - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_aabb_expand_towards_zero() {
        let bb = Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        let expanded = bb.expand_towards(0.0, 0.0, 0.0);
        assert_eq!(expanded, bb);
    }

    // ── contract ────────────────────────────────────────────────────

    #[test]
    fn test_aabb_contract_positive() {
        let bb = Aabb::new(0.0, 0.0, 0.0, 4.0, 4.0, 4.0);
        let contracted = bb.contract(1.0, 0.0, 0.0);
        assert!((contracted.min_x - 0.0).abs() < EPSILON);
        assert!((contracted.max_x - 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_aabb_contract_negative() {
        let bb = Aabb::new(0.0, 0.0, 0.0, 4.0, 4.0, 4.0);
        let contracted = bb.contract(-1.0, 0.0, 0.0);
        assert!((contracted.min_x - 1.0).abs() < EPSILON);
        assert!((contracted.max_x - 4.0).abs() < EPSILON);
    }

    // ── move ────────────────────────────────────────────────────────

    #[test]
    fn test_aabb_move_by() {
        let bb = Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        let moved = bb.move_by(5.0, 10.0, 15.0);
        assert_eq!(moved, Aabb::new(5.0, 10.0, 15.0, 6.0, 11.0, 16.0));
    }

    #[test]
    fn test_aabb_move_vec() {
        let bb = Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        let moved = bb.move_vec(Vec3::new(5.0, 10.0, 15.0));
        assert_eq!(moved, Aabb::new(5.0, 10.0, 15.0, 6.0, 11.0, 16.0));
    }

    // ── intersects ──────────────────────────────────────────────────

    #[test]
    fn test_aabb_intersects_overlapping() {
        let a = Aabb::new(0.0, 0.0, 0.0, 2.0, 2.0, 2.0);
        let b = Aabb::new(1.0, 1.0, 1.0, 3.0, 3.0, 3.0);
        assert!(a.intersects(&b));
        assert!(b.intersects(&a));
    }

    #[test]
    fn test_aabb_intersects_touching() {
        let a = Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        let b = Aabb::new(1.0, 0.0, 0.0, 2.0, 1.0, 1.0);
        // Touching is NOT overlapping (strict inequality).
        assert!(!a.intersects(&b));
    }

    #[test]
    fn test_aabb_intersects_separate() {
        let a = Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        let b = Aabb::new(5.0, 5.0, 5.0, 6.0, 6.0, 6.0);
        assert!(!a.intersects(&b));
    }

    // ── contains ────────────────────────────────────────────────────

    #[test]
    fn test_aabb_contains_inside() {
        let bb = Aabb::new(0.0, 0.0, 0.0, 2.0, 2.0, 2.0);
        assert!(bb.contains(1.0, 1.0, 1.0));
    }

    #[test]
    fn test_aabb_contains_on_edge() {
        let bb = Aabb::new(0.0, 0.0, 0.0, 2.0, 2.0, 2.0);
        assert!(bb.contains(0.0, 0.0, 0.0));
        assert!(bb.contains(2.0, 2.0, 2.0));
    }

    #[test]
    fn test_aabb_contains_outside() {
        let bb = Aabb::new(0.0, 0.0, 0.0, 2.0, 2.0, 2.0);
        assert!(!bb.contains(3.0, 1.0, 1.0));
    }

    #[test]
    fn test_aabb_contains_vec() {
        let bb = Aabb::new(0.0, 0.0, 0.0, 2.0, 2.0, 2.0);
        assert!(bb.contains_vec(Vec3::new(1.0, 1.0, 1.0)));
        assert!(!bb.contains_vec(Vec3::new(3.0, 1.0, 1.0)));
    }

    // ── intersect (overlap region) ──────────────────────────────────

    #[test]
    fn test_aabb_intersect_region() {
        let a = Aabb::new(0.0, 0.0, 0.0, 3.0, 3.0, 3.0);
        let b = Aabb::new(1.0, 1.0, 1.0, 5.0, 5.0, 5.0);
        let overlap = a.intersect(&b);
        assert_eq!(overlap, Aabb::new(1.0, 1.0, 1.0, 3.0, 3.0, 3.0));
    }

    // ── minmax (union) ──────────────────────────────────────────────

    #[test]
    fn test_aabb_minmax_union() {
        let a = Aabb::new(0.0, 0.0, 0.0, 2.0, 2.0, 2.0);
        let b = Aabb::new(1.0, 1.0, 1.0, 5.0, 5.0, 5.0);
        let union = a.minmax(&b);
        assert_eq!(union, Aabb::new(0.0, 0.0, 0.0, 5.0, 5.0, 5.0));
    }

    // ── distance_to_sqr ─────────────────────────────────────────────

    #[test]
    fn test_aabb_distance_to_sqr_inside() {
        let bb = Aabb::new(0.0, 0.0, 0.0, 2.0, 2.0, 2.0);
        assert!((bb.distance_to_sqr(Vec3::new(1.0, 1.0, 1.0)) - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_aabb_distance_to_sqr_outside() {
        let bb = Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        // Point at (2, 0.5, 0.5) → distance to face at x=1 is 1.0
        let dist = bb.distance_to_sqr(Vec3::new(2.0, 0.5, 0.5));
        assert!((dist - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_aabb_distance_to_sqr_corner() {
        let bb = Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        // Point at (2, 2, 2) → distance to corner (1,1,1) = sqrt(3), squared = 3
        let dist = bb.distance_to_sqr(Vec3::new(2.0, 2.0, 2.0));
        assert!((dist - 3.0).abs() < EPSILON);
    }

    // ── Display ─────────────────────────────────────────────────────

    #[test]
    fn test_aabb_display() {
        let bb = Aabb::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
        assert_eq!(format!("{bb}"), "AABB[1, 2, 3 -> 4, 5, 6]");
    }
}
