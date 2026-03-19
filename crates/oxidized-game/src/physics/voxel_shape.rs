//! Block collision shapes.
//!
//! A [`VoxelShape`] is one or more axis-aligned boxes describing a block's
//! collision geometry in block-local coordinates (0.0–1.0 per axis). For
//! example, a full cube is a single 1×1×1 box, a slab is 1×0.5×1, and a
//! stair is two boxes.
//!
//! [`BlockShapeProvider`] maps block state IDs to their collision shapes.

use oxidized_protocol::types::aabb::Aabb;

/// An axis-aligned box in block-local space (0.0–1.0 per axis).
///
/// # Examples
///
/// ```
/// use oxidized_game::physics::voxel_shape::ShapeBox;
///
/// let slab = ShapeBox::new(0.0, 0.0, 0.0, 1.0, 0.5, 1.0);
/// assert!((slab.max_y - 0.5).abs() < 1e-10);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeBox {
    /// Minimum X coordinate (block-local).
    pub min_x: f64,
    /// Minimum Y coordinate (block-local).
    pub min_y: f64,
    /// Minimum Z coordinate (block-local).
    pub min_z: f64,
    /// Maximum X coordinate (block-local).
    pub max_x: f64,
    /// Maximum Y coordinate (block-local).
    pub max_y: f64,
    /// Maximum Z coordinate (block-local).
    pub max_z: f64,
}

impl ShapeBox {
    /// Creates a new shape box with the given extents.
    pub const fn new(
        min_x: f64,
        min_y: f64,
        min_z: f64,
        max_x: f64,
        max_y: f64,
        max_z: f64,
    ) -> Self {
        Self {
            min_x,
            min_y,
            min_z,
            max_x,
            max_y,
            max_z,
        }
    }
}

/// A voxel shape composed of one or more [`ShapeBox`] fragments.
///
/// Most blocks have 0 (air) or 1 (full cube, slab) box. Complex blocks
/// like stairs or fences have 2–5 boxes.
///
/// # Examples
///
/// ```
/// use oxidized_game::physics::voxel_shape::VoxelShape;
///
/// let full = VoxelShape::full();
/// assert!(!full.is_empty());
/// assert_eq!(full.boxes().len(), 1);
///
/// let air = VoxelShape::empty();
/// assert!(air.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct VoxelShape {
    boxes: Vec<ShapeBox>,
}

impl VoxelShape {
    /// Creates a shape from a list of component boxes.
    pub fn new(boxes: Vec<ShapeBox>) -> Self {
        Self { boxes }
    }

    /// A solid full-cube shape (1×1×1).
    pub fn full() -> Self {
        Self {
            boxes: vec![ShapeBox::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0)],
        }
    }

    /// An empty shape (no collision — air, flowers, etc.).
    pub fn empty() -> Self {
        Self { boxes: Vec::new() }
    }

    /// Returns `true` if this shape has no collision boxes.
    pub fn is_empty(&self) -> bool {
        self.boxes.is_empty()
    }

    /// Returns the component boxes.
    pub fn boxes(&self) -> &[ShapeBox] {
        &self.boxes
    }

    /// Translates this shape to world coordinates at the given block origin,
    /// returning world-space [`Aabb`]s.
    pub fn translated(&self, bx: i32, by: i32, bz: i32) -> Vec<Aabb> {
        let ox = f64::from(bx);
        let oy = f64::from(by);
        let oz = f64::from(bz);
        self.boxes
            .iter()
            .map(|b| Aabb {
                min_x: ox + b.min_x,
                min_y: oy + b.min_y,
                min_z: oz + b.min_z,
                max_x: ox + b.max_x,
                max_y: oy + b.max_y,
                max_z: oz + b.max_z,
            })
            .collect()
    }
}

/// Provides collision shapes for block state IDs.
///
/// Implementations map `u32` block state IDs to their [`VoxelShape`].
/// The default implementation treats state 0 (air) as empty and all
/// others as full cubes.
pub trait BlockShapeProvider {
    /// Returns the collision shape for the given block state ID.
    fn get_shape(&self, state_id: u32) -> &VoxelShape;
}

/// A simple shape provider that returns full cubes for all non-air blocks.
///
/// Suitable for basic physics before the full block registry is available.
pub struct FullCubeShapeProvider {
    full: VoxelShape,
    empty: VoxelShape,
}

impl FullCubeShapeProvider {
    /// Creates a new provider.
    pub fn new() -> Self {
        Self {
            full: VoxelShape::full(),
            empty: VoxelShape::empty(),
        }
    }
}

impl Default for FullCubeShapeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockShapeProvider for FullCubeShapeProvider {
    fn get_shape(&self, state_id: u32) -> &VoxelShape {
        if state_id == 0 {
            &self.empty
        } else {
            &self.full
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_full_shape_is_unit_cube() {
        let shape = VoxelShape::full();
        assert_eq!(shape.boxes().len(), 1);
        let b = &shape.boxes()[0];
        assert!((b.min_x).abs() < 1e-10);
        assert!((b.min_y).abs() < 1e-10);
        assert!((b.min_z).abs() < 1e-10);
        assert!((b.max_x - 1.0).abs() < 1e-10);
        assert!((b.max_y - 1.0).abs() < 1e-10);
        assert!((b.max_z - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_empty_shape() {
        let shape = VoxelShape::empty();
        assert!(shape.is_empty());
        assert_eq!(shape.boxes().len(), 0);
    }

    #[test]
    fn test_translated_produces_world_aabbs() {
        let shape = VoxelShape::full();
        let world_boxes = shape.translated(10, 64, 20);
        assert_eq!(world_boxes.len(), 1);
        let aabb = &world_boxes[0];
        assert!((aabb.min_x - 10.0).abs() < 1e-10);
        assert!((aabb.min_y - 64.0).abs() < 1e-10);
        assert!((aabb.min_z - 20.0).abs() < 1e-10);
        assert!((aabb.max_x - 11.0).abs() < 1e-10);
        assert!((aabb.max_y - 65.0).abs() < 1e-10);
        assert!((aabb.max_z - 21.0).abs() < 1e-10);
    }

    #[test]
    fn test_slab_shape_translated() {
        let slab = VoxelShape::new(vec![ShapeBox::new(0.0, 0.0, 0.0, 1.0, 0.5, 1.0)]);
        let boxes = slab.translated(0, 0, 0);
        assert_eq!(boxes.len(), 1);
        assert!((boxes[0].max_y - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_full_cube_provider_air_is_empty() {
        let provider = FullCubeShapeProvider::new();
        assert!(provider.get_shape(0).is_empty());
    }

    #[test]
    fn test_full_cube_provider_non_air_is_full() {
        let provider = FullCubeShapeProvider::new();
        assert!(!provider.get_shape(1).is_empty());
        assert!(!provider.get_shape(100).is_empty());
    }
}
