//! Axis-aligned bounding box for entity collision and hit detection.
//!
//! Mirrors `net.minecraft.world.phys.AABB` in vanilla.

/// Axis-aligned bounding box defined by two corner points.
///
/// The box spans from `(min_x, min_y, min_z)` to `(max_x, max_y, max_z)`.
/// Used for entity collision detection, hit testing, and spatial queries.
///
/// # Examples
///
/// ```
/// use oxidized_game::entity::aabb::Aabb;
///
/// let bbox = Aabb::from_center(0.0, 0.0, 0.0, 0.6, 1.8);
/// assert!(bbox.contains(0.0, 0.9, 0.0));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Aabb {
    /// Minimum X coordinate.
    pub min_x: f64,
    /// Minimum Y coordinate (bottom of entity).
    pub min_y: f64,
    /// Minimum Z coordinate.
    pub min_z: f64,
    /// Maximum X coordinate.
    pub max_x: f64,
    /// Maximum Y coordinate (top of entity).
    pub max_y: f64,
    /// Maximum Z coordinate.
    pub max_z: f64,
}

impl Aabb {
    /// Creates an AABB centered at `(x, y_bottom, z)` with the given
    /// `width` and `height`.
    ///
    /// The box extends `width/2` in the X and Z directions from center,
    /// and `height` upward from `y` (entities stand on their Y position).
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidized_game::entity::aabb::Aabb;
    ///
    /// let bbox = Aabb::from_center(5.0, 64.0, 5.0, 0.6, 1.8);
    /// assert!((bbox.min_x - 4.7).abs() < 1e-10);
    /// assert!((bbox.max_y - 65.8).abs() < 1e-10);
    /// ```
    pub fn from_center(x: f64, y: f64, z: f64, width: f64, height: f64) -> Self {
        let half_w = width / 2.0;
        Self {
            min_x: x - half_w,
            min_y: y,
            min_z: z - half_w,
            max_x: x + half_w,
            max_y: y + height,
            max_z: z + half_w,
        }
    }

    /// Returns `true` if the point `(x, y, z)` is inside this box (inclusive).
    pub fn contains(&self, x: f64, y: f64, z: f64) -> bool {
        x >= self.min_x
            && x <= self.max_x
            && y >= self.min_y
            && y <= self.max_y
            && z >= self.min_z
            && z <= self.max_z
    }

    /// Returns `true` if this box overlaps with `other` (exclusive edges).
    ///
    /// Matches vanilla's `AABB.intersects()` — touching edges do not count.
    pub fn intersects(&self, other: &Aabb) -> bool {
        self.max_x > other.min_x
            && self.min_x < other.max_x
            && self.max_y > other.min_y
            && self.min_y < other.max_y
            && self.max_z > other.min_z
            && self.min_z < other.max_z
    }

    /// Returns the volume of this bounding box.
    pub fn volume(&self) -> f64 {
        (self.max_x - self.min_x)
            * (self.max_y - self.min_y)
            * (self.max_z - self.min_z)
    }

    /// Returns the center point of this bounding box.
    pub fn center(&self) -> (f64, f64, f64) {
        (
            (self.min_x + self.max_x) / 2.0,
            (self.min_y + self.max_y) / 2.0,
            (self.min_z + self.max_z) / 2.0,
        )
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_from_center_dimensions() {
        let bbox = Aabb::from_center(0.0, 0.0, 0.0, 1.0, 2.0);
        assert!((bbox.min_x - (-0.5)).abs() < 1e-10);
        assert!((bbox.max_x - 0.5).abs() < 1e-10);
        assert!((bbox.min_y - 0.0).abs() < 1e-10);
        assert!((bbox.max_y - 2.0).abs() < 1e-10);
        assert!((bbox.min_z - (-0.5)).abs() < 1e-10);
        assert!((bbox.max_z - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_contains_center() {
        let bbox = Aabb::from_center(0.0, 0.0, 0.0, 1.0, 2.0);
        assert!(bbox.contains(0.0, 1.0, 0.0));
    }

    #[test]
    fn test_contains_boundary() {
        let bbox = Aabb::from_center(0.0, 0.0, 0.0, 1.0, 2.0);
        assert!(bbox.contains(0.5, 0.0, 0.5));
        assert!(bbox.contains(-0.5, 2.0, -0.5));
    }

    #[test]
    fn test_contains_outside() {
        let bbox = Aabb::from_center(0.0, 0.0, 0.0, 1.0, 2.0);
        assert!(!bbox.contains(1.0, 0.0, 0.0));
        assert!(!bbox.contains(0.0, 3.0, 0.0));
    }

    #[test]
    fn test_intersects_overlapping() {
        let a = Aabb::from_center(0.0, 0.0, 0.0, 1.0, 2.0);
        let b = Aabb::from_center(0.5, 0.0, 0.0, 1.0, 2.0);
        assert!(a.intersects(&b));
        assert!(b.intersects(&a));
    }

    #[test]
    fn test_intersects_disjoint() {
        let a = Aabb::from_center(0.0, 0.0, 0.0, 1.0, 2.0);
        let c = Aabb::from_center(5.0, 0.0, 0.0, 1.0, 2.0);
        assert!(!a.intersects(&c));
        assert!(!c.intersects(&a));
    }

    #[test]
    fn test_intersects_touching_edges_not_overlapping() {
        let a = Aabb::from_center(0.0, 0.0, 0.0, 1.0, 2.0);
        // Exactly touching at x=0.5, should NOT intersect (exclusive edges).
        let b = Aabb::from_center(1.0, 0.0, 0.0, 1.0, 2.0);
        assert!(!a.intersects(&b));
    }

    #[test]
    fn test_volume() {
        let bbox = Aabb::from_center(0.0, 0.0, 0.0, 2.0, 3.0);
        assert!((bbox.volume() - 12.0).abs() < 1e-10);
    }

    #[test]
    fn test_center() {
        let bbox = Aabb::from_center(5.0, 10.0, 15.0, 2.0, 4.0);
        let (cx, cy, cz) = bbox.center();
        assert!((cx - 5.0).abs() < 1e-10);
        assert!((cy - 12.0).abs() < 1e-10); // y=10 to y=14, center=12
        assert!((cz - 15.0).abs() < 1e-10);
    }
}
