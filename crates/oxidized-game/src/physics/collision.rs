//! AABB sweep collision detection.
//!
//! Implements vanilla Minecraft's per-axis sweep collision algorithm from
//! `Entity.collideWithShapes()` and `Shapes.collide()`. The axis processing
//! order is movement-dependent: Y is always first, then X and Z are ordered
//! by which has the larger absolute movement (matching `Direction.axisStepOrder`).

use oxidized_mc_types::BlockPos;
use oxidized_mc_types::aabb::Aabb;

use super::voxel_shape::BlockShapeProvider;
use crate::level::traits::BlockGetter;

/// Performs a swept AABB collision along the X axis.
///
/// Returns the maximum safe movement distance before hitting `obstacle`.
/// If there is no collision on this axis, returns the original `dx`.
pub fn clip_x(entity: &Aabb, obstacle: &Aabb, mut dx: f64) -> f64 {
    // Must overlap on Y and Z for an X-axis collision.
    if entity.max_y <= obstacle.min_y || entity.min_y >= obstacle.max_y {
        return dx;
    }
    if entity.max_z <= obstacle.min_z || entity.min_z >= obstacle.max_z {
        return dx;
    }

    if dx > 0.0 && entity.max_x <= obstacle.min_x {
        let max_move = obstacle.min_x - entity.max_x;
        if max_move < dx {
            dx = max_move;
        }
    } else if dx < 0.0 && entity.min_x >= obstacle.max_x {
        let max_move = obstacle.max_x - entity.min_x;
        if max_move > dx {
            dx = max_move;
        }
    }
    dx
}

/// Performs a swept AABB collision along the Y axis.
///
/// Returns the maximum safe movement distance before hitting `obstacle`.
pub fn clip_y(entity: &Aabb, obstacle: &Aabb, mut dy: f64) -> f64 {
    if entity.max_x <= obstacle.min_x || entity.min_x >= obstacle.max_x {
        return dy;
    }
    if entity.max_z <= obstacle.min_z || entity.min_z >= obstacle.max_z {
        return dy;
    }

    if dy > 0.0 && entity.max_y <= obstacle.min_y {
        let max_move = obstacle.min_y - entity.max_y;
        if max_move < dy {
            dy = max_move;
        }
    } else if dy < 0.0 && entity.min_y >= obstacle.max_y {
        let max_move = obstacle.max_y - entity.min_y;
        if max_move > dy {
            dy = max_move;
        }
    }
    dy
}

/// Performs a swept AABB collision along the Z axis.
///
/// Returns the maximum safe movement distance before hitting `obstacle`.
pub fn clip_z(entity: &Aabb, obstacle: &Aabb, mut dz: f64) -> f64 {
    if entity.max_x <= obstacle.min_x || entity.min_x >= obstacle.max_x {
        return dz;
    }
    if entity.max_y <= obstacle.min_y || entity.min_y >= obstacle.max_y {
        return dz;
    }

    if dz > 0.0 && entity.max_z <= obstacle.min_z {
        let max_move = obstacle.min_z - entity.max_z;
        if max_move < dz {
            dz = max_move;
        }
    } else if dz < 0.0 && entity.min_z >= obstacle.max_z {
        let max_move = obstacle.max_z - entity.min_z;
        if max_move > dz {
            dz = max_move;
        }
    }
    dz
}

/// Resolves movement against all obstacle shapes using the vanilla axis order.
///
/// Vanilla processes Y first, then orders X and Z by movement magnitude:
/// - If `|dx| >= |dz|`: Y → X → Z
/// - If `|dx| < |dz|`: Y → Z → X
///
/// This matches `Direction.axisStepOrder()` in Java.
pub fn collide_with_shapes(
    entity_aabb: &Aabb,
    dx: f64,
    dy: f64,
    dz: f64,
    obstacles: &[Aabb],
) -> (f64, f64, f64) {
    if obstacles.is_empty() {
        return (dx, dy, dz);
    }

    // Y always first.
    let mut resolved_dy = dy;
    for obs in obstacles {
        resolved_dy = clip_y(entity_aabb, obs, resolved_dy);
    }
    let aabb_after_y = entity_aabb.move_by(0.0, resolved_dy, 0.0);

    if dx.abs() >= dz.abs() {
        // Y → X → Z
        let mut resolved_dx = dx;
        for obs in obstacles {
            resolved_dx = clip_x(&aabb_after_y, obs, resolved_dx);
        }
        let aabb_after_yx = aabb_after_y.move_by(resolved_dx, 0.0, 0.0);
        let mut resolved_dz = dz;
        for obs in obstacles {
            resolved_dz = clip_z(&aabb_after_yx, obs, resolved_dz);
        }
        (resolved_dx, resolved_dy, resolved_dz)
    } else {
        // Y → Z → X
        let mut resolved_dz = dz;
        for obs in obstacles {
            resolved_dz = clip_z(&aabb_after_y, obs, resolved_dz);
        }
        let aabb_after_yz = aabb_after_y.move_by(0.0, 0.0, resolved_dz);
        let mut resolved_dx = dx;
        for obs in obstacles {
            resolved_dx = clip_x(&aabb_after_yz, obs, resolved_dx);
        }
        (resolved_dx, resolved_dy, resolved_dz)
    }
}

/// Collects world-space collision boxes for all blocks in the swept volume.
///
/// Expands the entity AABB by the movement vector, then iterates all block
/// positions within the expanded region and gathers their collision shapes.
///
/// # Errors
///
/// Silently skips blocks in unloaded chunks (returns no obstacle for them).
pub fn collect_obstacles(
    level: &impl BlockGetter,
    entity: &Aabb,
    dx: f64,
    dy: f64,
    dz: f64,
    shape_provider: &impl BlockShapeProvider,
) -> Vec<Aabb> {
    let expanded = entity.expand_towards(dx, dy, dz);

    let min_x = expanded.min_x.floor() as i32 - 1;
    let max_x = expanded.max_x.ceil() as i32 + 1;
    let min_y = expanded.min_y.floor() as i32 - 1;
    let max_y = expanded.max_y.ceil() as i32 + 1;
    let min_z = expanded.min_z.floor() as i32 - 1;
    let max_z = expanded.max_z.ceil() as i32 + 1;

    let mut obstacles = Vec::new();
    for bx in min_x..max_x {
        for by in min_y..max_y {
            for bz in min_z..max_z {
                let pos = BlockPos::new(bx, by, bz);
                let state_id = match level.get_block_state(pos) {
                    Ok(id) => id,
                    Err(_) => continue,
                };
                let shape = shape_provider.get_shape(state_id);
                if !shape.is_empty() {
                    obstacles.extend(shape.translated(bx, by, bz));
                }
            }
        }
    }
    obstacles
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_clip_x_positive_collision() {
        let entity = Aabb::new(0.0, 0.0, 0.0, 0.6, 1.8, 0.6);
        let obstacle = Aabb::new(1.0, 0.0, 0.0, 2.0, 1.0, 1.0);
        let limited = clip_x(&entity, &obstacle, 2.0);
        assert!(
            (limited - 0.4).abs() < 1e-9,
            "Expected dx=0.4, got {limited}"
        );
    }

    #[test]
    fn test_clip_x_negative_collision() {
        let entity = Aabb::new(2.0, 0.0, 0.0, 2.6, 1.8, 0.6);
        let obstacle = Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        let limited = clip_x(&entity, &obstacle, -3.0);
        assert!(
            (limited - (-1.0)).abs() < 1e-9,
            "Expected dx=-1.0, got {limited}"
        );
    }

    #[test]
    fn test_clip_x_no_y_overlap() {
        let entity = Aabb::new(0.0, 2.0, 0.0, 0.6, 3.8, 0.6);
        let obstacle = Aabb::new(1.0, 0.0, 0.0, 2.0, 1.0, 1.0);
        let result = clip_x(&entity, &obstacle, 2.0);
        assert!((result - 2.0).abs() < 1e-9, "No collision expected");
    }

    #[test]
    fn test_clip_y_downward_collision() {
        let entity = Aabb::new(0.0, 2.0, 0.0, 0.6, 3.8, 0.6);
        let obstacle = Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        let limited = clip_y(&entity, &obstacle, -5.0);
        assert!(
            (limited - (-1.0)).abs() < 1e-9,
            "Expected dy=-1.0, got {limited}"
        );
    }

    #[test]
    fn test_clip_z_positive_collision() {
        let entity = Aabb::new(0.0, 0.0, 0.0, 0.6, 1.8, 0.6);
        let obstacle = Aabb::new(0.0, 0.0, 1.0, 1.0, 1.0, 2.0);
        let limited = clip_z(&entity, &obstacle, 2.0);
        assert!(
            (limited - 0.4).abs() < 1e-9,
            "Expected dz=0.4, got {limited}"
        );
    }

    #[test]
    fn test_collide_with_shapes_empty_obstacles() {
        let entity = Aabb::new(0.0, 0.0, 0.0, 0.6, 1.8, 0.6);
        let (dx, dy, dz) = collide_with_shapes(&entity, 1.0, -1.0, 0.5, &[]);
        assert!((dx - 1.0).abs() < 1e-9);
        assert!((dy - (-1.0)).abs() < 1e-9);
        assert!((dz - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_collide_y_first_then_xz() {
        let entity = Aabb::new(0.0, 5.0, 0.0, 0.6, 6.8, 0.6);
        let floor = Aabb::new(-10.0, 0.0, -10.0, 10.0, 1.0, 10.0);
        let (dx, dy, dz) = collide_with_shapes(&entity, 1.0, -10.0, 1.0, &[floor]);
        // Y resolves first: entity at y=5, floor top at y=1 → dy = -(5-1) = -4
        assert!((dy - (-4.0)).abs() < 1e-9, "Expected dy=-4.0, got {dy}");
        // X and Z should be unaffected (no horizontal collision with floor)
        assert!((dx - 1.0).abs() < 1e-9);
        assert!((dz - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_axis_order_x_dominant() {
        // When |dx| >= |dz|, order is Y→X→Z.
        // Entity at origin, obstacle at (1.5, 0, 1.5).
        // No initial Z overlap → X is unrestricted.
        // After X move, entity overlaps obstacle in X → Z gets clamped.
        let entity = Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        let obstacle = Aabb::new(1.5, 0.0, 1.5, 2.5, 1.0, 2.5);
        let (dx, _dy, dz) = collide_with_shapes(&entity, 2.0, 0.0, 2.0, &[obstacle]);
        // X: no Z overlap initially → dx = 2.0 (unrestricted)
        assert!(
            (dx - 2.0).abs() < 1e-9,
            "Expected dx=2.0 (no X collision), got {dx}"
        );
        // After X move, entity is at [2,3]×[0,1]×[0,1].
        // Now X overlaps obstacle [1.5,2.5] and Y overlaps → Z clamped to 0.5.
        assert!(
            (dz - 0.5).abs() < 1e-9,
            "Expected dz=0.5 (Z clamped after X move), got {dz}"
        );
    }
}
