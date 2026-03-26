//! Per-tick physics update.
//!
//! Implements the core physics pipeline matching
//! `LivingEntity.travel()` and `Entity.move()` from vanilla.
//!
//! # Order of Operations (matches Java)
//!
//! 1. Determine block friction from block below entity
//! 2. Apply input friction and movement (handled externally)
//! 3. Apply gravity (or fluid buoyancy)
//! 4. Sweep-collide the entity AABB against block shapes
//! 5. Detect on_ground from vertical collision
//! 6. Zero velocity on collision axes (slime bounce negates Y)
//! 7. Apply horizontal and vertical drag
//! 8. Track fall distance

use crate::entity::EntityDimensions;
use crate::entity::components::{BoundingBox, FallDistance, OnGround, Position, Velocity};
use crate::level::traits::BlockGetter;

use super::collision::{collect_obstacles, collide_with_shapes};
use super::constants::*;
use super::voxel_shape::BlockShapeProvider;

use oxidized_protocol::types::BlockPos;
use oxidized_protocol::types::aabb::Aabb;
use oxidized_world::registry::BlockStateId;

/// Applies one tick of physics to an entity.
///
/// Handles gravity, sweep collision, on_ground detection, drag,
/// slime bounce, and fall distance tracking. Fluid effects (buoyancy
/// and drag) are applied when `in_water` or `in_lava` is true.
///
/// Block physics properties (friction, speed factor, slime bounce) are
/// read directly from the compile-time block registry via [`BlockStateId`].
///
/// This function does **not** handle player input, step-up, or
/// knockback — those are applied externally before calling this.
#[allow(clippy::too_many_arguments)]
pub fn physics_tick(
    pos: &mut Position,
    vel: &mut Velocity,
    bbox: &mut BoundingBox,
    on_ground: &mut OnGround,
    fall_distance: &mut FallDistance,
    dims: &EntityDimensions,
    level: &impl BlockGetter,
    shape_provider: &impl BlockShapeProvider,
    in_water: bool,
    in_lava: bool,
) {
    // 1. Determine block friction (from block below entity feet).
    let block_friction = get_block_friction(level, pos.0.x, bbox.0.min_y, pos.0.z, on_ground.0);

    // 2. Apply gravity (or fluid modifiers).
    if in_water {
        vel.0.y += WATER_BUOYANCY;
    } else if in_lava {
        vel.0.y += LAVA_BUOYANCY;
    } else {
        vel.0.y -= GRAVITY;
    }

    // Apply block speed factor (e.g., soul sand, honey) BEFORE collision.
    let speed_factor = get_block_speed_factor(level, pos.0.x, bbox.0.min_y, pos.0.z);
    vel.0.x *= speed_factor;
    vel.0.z *= speed_factor;

    let mut dx = vel.0.x;
    let dy = vel.0.y;
    let mut dz = vel.0.z;

    // Apply fluid drag to the movement delta.
    if in_water {
        dx *= WATER_DRAG;
        dz *= WATER_DRAG;
    } else if in_lava {
        dx *= LAVA_DRAG;
        dz *= LAVA_DRAG;
    }

    // 3. Swept AABB collision with movement-dependent axis ordering.
    let obstacles = collect_obstacles(level, &bbox.0, dx, dy, dz, shape_provider);
    let (actual_dx, actual_dy, actual_dz) = collide_with_shapes(&bbox.0, dx, dy, dz, &obstacles);

    // 4. Apply resolved movement.
    let new_x = pos.0.x + actual_dx;
    let new_y = pos.0.y + actual_dy;
    let new_z = pos.0.z + actual_dz;
    pos.0.x = new_x;
    pos.0.y = new_y;
    pos.0.z = new_z;
    bbox.0 = Aabb::from_center(
        new_x,
        new_y,
        new_z,
        f64::from(dims.width),
        f64::from(dims.height),
    );

    // 5. Detect on_ground: downward movement was reduced by collision.
    on_ground.0 = dy < 0.0 && (actual_dy - dy).abs() > COLLISION_EPSILON;

    // 6. Zero velocity on collision axes, with slime bounce for Y.
    if (actual_dx - dx).abs() > COLLISION_EPSILON {
        vel.0.x = 0.0;
    }
    if (actual_dy - dy).abs() > COLLISION_EPSILON {
        // Check for slime bounce: if landing on a slime block, negate Y velocity.
        if on_ground.0 && is_on_slime(level, pos.0.x, bbox.0.min_y, pos.0.z) {
            vel.0.y = -vel.0.y;
        } else {
            vel.0.y = 0.0;
        }
    }
    if (actual_dz - dz).abs() > COLLISION_EPSILON {
        vel.0.z = 0.0;
    }

    // 7. Apply drag.
    let h_drag = if on_ground.0 {
        block_friction * HORIZONTAL_DRAG
    } else {
        HORIZONTAL_DRAG
    };

    vel.0.x *= h_drag;
    vel.0.y *= VERTICAL_DRAG;
    vel.0.z *= h_drag;

    // 8. Fall distance tracking.
    if !on_ground.0 && actual_dy < 0.0 {
        // Accumulate downward distance (actual_dy is negative, so negate).
        fall_distance.0 -= actual_dy as f32;
    } else if on_ground.0 {
        // Reset on landing (damage would be calculated before this reset
        // in a full implementation).
        fall_distance.0 = 0.0;
    }
}

/// Returns the friction value of the block below the entity's feet.
///
/// Reads directly from the block registry via [`BlockStateId`].
/// Returns 1.0 when not on ground (no block friction applies in air).
fn get_block_friction(
    level: &impl BlockGetter,
    pos_x: f64,
    bbox_min_y: f64,
    pos_z: f64,
    is_on_ground: bool,
) -> f64 {
    if !is_on_ground {
        return 1.0;
    }

    let below = BlockPos::new(
        pos_x.floor() as i32,
        (bbox_min_y - 0.5000001).floor() as i32,
        pos_z.floor() as i32,
    );

    match level.get_block_state(below) {
        Ok(state_id) => BlockStateId(state_id as u16).friction(),
        Err(_) => BLOCK_FRICTION_DEFAULT,
    }
}

/// Returns `true` if the block below the entity's feet is a slime block.
fn is_on_slime(level: &impl BlockGetter, pos_x: f64, bbox_min_y: f64, pos_z: f64) -> bool {
    let below = BlockPos::new(
        pos_x.floor() as i32,
        (bbox_min_y - 0.5000001).floor() as i32,
        pos_z.floor() as i32,
    );

    match level.get_block_state(below) {
        Ok(state_id) => BlockStateId(state_id as u16).block_name() == "minecraft:slime_block",
        Err(_) => false,
    }
}

/// Returns the speed factor of the block below the entity's feet.
///
/// Reads directly from the block registry via [`BlockStateId`].
/// Returns 1.0 for normal blocks and < 1.0 for slow blocks (e.g.,
/// soul sand at 0.4, honey at 0.4, powder snow at 0.9).
fn get_block_speed_factor(
    level: &impl BlockGetter,
    pos_x: f64,
    bbox_min_y: f64,
    pos_z: f64,
) -> f64 {
    let below = BlockPos::new(
        pos_x.floor() as i32,
        (bbox_min_y - 0.5000001).floor() as i32,
        pos_z.floor() as i32,
    );

    match level.get_block_state(below) {
        Ok(state_id) => BlockStateId(state_id as u16).speed_factor(),
        Err(_) => 1.0,
    }
}

/// Returns `true` if the velocity changed significantly between two states.
///
/// Used to decide whether to send a
/// `ClientboundSetEntityMotionPacket` to watching players.
pub fn velocity_changed_significantly(old: (f64, f64, f64), new: (f64, f64, f64)) -> bool {
    (old.0 - new.0).abs() > 0.01 || (old.1 - new.1).abs() > 0.01 || (old.2 - new.2).abs() > 0.01
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::entity::EntityDimensions;
    use crate::level::error::LevelError;
    use crate::physics::voxel_shape::FullCubeShapeProvider;
    use glam::DVec3;
    use oxidized_world::registry::BlockRegistry;

    /// A level with no blocks (everything is air / unloaded).
    struct EmptyLevel;

    impl BlockGetter for EmptyLevel {
        fn get_block_state(&self, _pos: BlockPos) -> Result<u32, LevelError> {
            Ok(0) // air
        }
    }

    /// A level with a single solid block at the given position.
    struct SingleBlockLevel {
        pos: BlockPos,
        state_id: u32,
    }

    impl BlockGetter for SingleBlockLevel {
        fn get_block_state(&self, pos: BlockPos) -> Result<u32, LevelError> {
            if pos.x == self.pos.x && pos.y == self.pos.y && pos.z == self.pos.z {
                Ok(self.state_id)
            } else {
                Ok(0)
            }
        }
    }

    /// A level with a full floor at y=0 (solid blocks at y=0 for all x,z).
    struct FloorLevel;

    impl BlockGetter for FloorLevel {
        fn get_block_state(&self, pos: BlockPos) -> Result<u32, LevelError> {
            if pos.y == 0 {
                Ok(1) // stone
            } else {
                Ok(0)
            }
        }
    }

    /// A floor made of a specific block state.
    struct SpecificFloorLevel {
        floor_state_id: u32,
    }

    impl BlockGetter for SpecificFloorLevel {
        fn get_block_state(&self, pos: BlockPos) -> Result<u32, LevelError> {
            if pos.y == 0 {
                Ok(self.floor_state_id)
            } else {
                Ok(0)
            }
        }
    }

    /// Physics state for a pig-sized entity (0.6 × 1.8).
    const PIG_DIMS: EntityDimensions = EntityDimensions {
        width: 0.6,
        height: 1.8,
    };

    fn make_floating_state() -> (Position, Velocity, BoundingBox, OnGround, FallDistance) {
        let p = DVec3::new(8.0, 100.0, 8.0);
        (
            Position(p),
            Velocity(DVec3::ZERO),
            BoundingBox(Aabb::from_center(p.x, p.y, p.z, 0.6, 1.8)),
            OnGround(false),
            FallDistance(0.0),
        )
    }

    fn make_above_floor_state(
        height: f64,
    ) -> (Position, Velocity, BoundingBox, OnGround, FallDistance) {
        // Floor is at y=0..1 (full cube). Entity feet at y = 1.0 + height.
        let p = DVec3::new(0.5, 1.0 + height, 0.5);
        (
            Position(p),
            Velocity(DVec3::ZERO),
            BoundingBox(Aabb::from_center(p.x, p.y, p.z, 0.6, 1.8)),
            OnGround(false),
            FallDistance(0.0),
        )
    }

    #[test]
    fn test_gravity_acceleration() {
        let shapes = FullCubeShapeProvider::new();
        let (mut pos, mut vel, mut bbox, mut og, mut fd) = make_floating_state();

        // Tick 1: gravity: vy = 0 - 0.08 = -0.08
        // After drag: vy = -0.08 * 0.98 = -0.0784
        physics_tick(
            &mut pos,
            &mut vel,
            &mut bbox,
            &mut og,
            &mut fd,
            &PIG_DIMS,
            &EmptyLevel,
            &shapes,
            false,
            false,
        );
        assert!(
            (vel.0.y - (-0.0784)).abs() < 0.0001,
            "After tick 1: vy = {} (expected ≈ -0.0784)",
            vel.0.y
        );

        // Tick 2: gravity: vy = -0.0784 - 0.08 = -0.1584
        // After drag: vy ≈ -0.1584 * 0.98 ≈ -0.155232
        physics_tick(
            &mut pos,
            &mut vel,
            &mut bbox,
            &mut og,
            &mut fd,
            &PIG_DIMS,
            &EmptyLevel,
            &shapes,
            false,
            false,
        );
        assert!(
            vel.0.y < -0.15,
            "After tick 2: vy = {} (should be < -0.15)",
            vel.0.y
        );
    }

    #[test]
    fn test_collision_stops_downward_movement() {
        let shapes = FullCubeShapeProvider::new();
        let (mut pos, mut vel, mut bbox, mut og, mut fd) = make_above_floor_state(0.5);
        vel.0.y = -5.0;

        physics_tick(
            &mut pos,
            &mut vel,
            &mut bbox,
            &mut og,
            &mut fd,
            &PIG_DIMS,
            &FloorLevel,
            &shapes,
            false,
            false,
        );

        assert!(og.0, "Entity should be on ground");
        assert!(vel.0.y.abs() < 0.001, "vy should be zeroed: {}", vel.0.y);
        assert!(
            pos.0.y >= 1.0 - 0.001,
            "Entity should not pass through floor: y={}",
            pos.0.y
        );
    }

    #[test]
    fn test_water_buoyancy() {
        let shapes = FullCubeShapeProvider::new();
        let (mut pos, mut vel, mut bbox, mut og, mut fd) = make_floating_state();
        vel.0.x = 1.0;

        physics_tick(
            &mut pos,
            &mut vel,
            &mut bbox,
            &mut og,
            &mut fd,
            &PIG_DIMS,
            &EmptyLevel,
            &shapes,
            true,
            false,
        );

        assert!(vel.0.y > 0.0, "Water buoyancy should push upward");
        assert!(vel.0.x < 1.0, "Water should reduce horizontal velocity");
    }

    #[test]
    fn test_fall_distance_accumulates() {
        let shapes = FullCubeShapeProvider::new();
        let (mut pos, mut vel, mut bbox, mut og, mut fd) = make_floating_state();

        physics_tick(
            &mut pos,
            &mut vel,
            &mut bbox,
            &mut og,
            &mut fd,
            &PIG_DIMS,
            &EmptyLevel,
            &shapes,
            false,
            false,
        );
        assert!(fd.0 > 0.0, "Fall distance should increase while falling");

        let fd1 = fd.0;
        physics_tick(
            &mut pos,
            &mut vel,
            &mut bbox,
            &mut og,
            &mut fd,
            &PIG_DIMS,
            &EmptyLevel,
            &shapes,
            false,
            false,
        );
        assert!(fd.0 > fd1, "Fall distance should keep increasing");
    }

    #[test]
    fn test_fall_distance_resets_on_landing() {
        let shapes = FullCubeShapeProvider::new();
        let (mut pos, mut vel, mut bbox, mut og, mut fd) = make_above_floor_state(0.5);
        vel.0.y = -1.0;
        fd.0 = 5.0;

        physics_tick(
            &mut pos,
            &mut vel,
            &mut bbox,
            &mut og,
            &mut fd,
            &PIG_DIMS,
            &FloorLevel,
            &shapes,
            false,
            false,
        );

        assert!(og.0, "Entity should land");
        assert!(
            fd.0.abs() < 0.001,
            "Fall distance should reset on landing: {}",
            fd.0
        );
    }

    #[test]
    fn test_velocity_changed_significantly() {
        assert!(!velocity_changed_significantly(
            (0.0, 0.0, 0.0),
            (0.005, 0.005, 0.005)
        ));
        assert!(velocity_changed_significantly(
            (0.0, 0.0, 0.0),
            (0.02, 0.0, 0.0)
        ));
        assert!(velocity_changed_significantly(
            (1.0, 2.0, 3.0),
            (1.0, 2.0, 3.02)
        ));
    }

    #[test]
    fn test_entity_at_rest_on_ground() {
        let shapes = FullCubeShapeProvider::new();
        let (mut pos, mut vel, mut bbox, mut og, mut fd) = make_above_floor_state(0.0);
        og.0 = true;

        let y_before = pos.0.y;
        physics_tick(
            &mut pos,
            &mut vel,
            &mut bbox,
            &mut og,
            &mut fd,
            &PIG_DIMS,
            &FloorLevel,
            &shapes,
            false,
            false,
        );

        assert!(og.0, "Entity should remain on ground");
        assert!(
            (pos.0.y - y_before).abs() < 0.01,
            "Entity should stay near same Y: before={y_before}, after={}",
            pos.0.y
        );
    }

    #[test]
    fn test_lava_buoyancy() {
        let shapes = FullCubeShapeProvider::new();
        let (mut pos, mut vel, mut bbox, mut og, mut fd) = make_floating_state();

        physics_tick(
            &mut pos,
            &mut vel,
            &mut bbox,
            &mut og,
            &mut fd,
            &PIG_DIMS,
            &EmptyLevel,
            &shapes,
            false,
            true,
        );

        assert!(
            vel.0.y > 0.0,
            "Lava buoyancy should push upward: vy={}",
            vel.0.y
        );
    }

    #[test]
    fn test_single_block_collision() {
        let shapes = FullCubeShapeProvider::new();
        let level = SingleBlockLevel {
            pos: BlockPos::new(0, 0, 0),
            state_id: 1, // stone
        };

        let p = DVec3::new(0.5, 1.5, 0.5);
        let mut pos = Position(p);
        let mut vel = Velocity(DVec3::new(0.0, -5.0, 0.0));
        let mut bbox = BoundingBox(Aabb::from_center(p.x, p.y, p.z, 0.6, 1.8));
        let mut og = OnGround(false);
        let mut fd = FallDistance(0.0);

        physics_tick(
            &mut pos, &mut vel, &mut bbox, &mut og, &mut fd, &PIG_DIMS, &level, &shapes, false,
            false,
        );

        assert!(og.0, "Entity should land on single block");
        assert!(
            pos.0.y >= 1.0 - 0.001,
            "Entity should not pass through block: y={}",
            pos.0.y
        );
    }

    #[test]
    fn test_ice_reduces_friction() {
        let shapes = FullCubeShapeProvider::new();
        let reg = BlockRegistry::load().unwrap();
        let ice_id = reg.default_state("minecraft:ice").unwrap().0 as u32;
        let ice_level = SpecificFloorLevel {
            floor_state_id: ice_id,
        };

        let (mut pos, mut vel, mut bbox, mut og, mut fd) = make_above_floor_state(0.0);
        og.0 = true;
        vel.0.x = 1.0;

        physics_tick(
            &mut pos, &mut vel, &mut bbox, &mut og, &mut fd, &PIG_DIMS, &ice_level, &shapes, false,
            false,
        );

        // On ice: drag = 0.98 * 0.91 = 0.8918
        // On normal: drag = 0.6 * 0.91 = 0.546
        // Ice should preserve more velocity.
        let ice_vx = vel.0.x;
        assert!(
            ice_vx > 0.8,
            "Ice should preserve high velocity: vx={ice_vx}"
        );

        // Compare with stone floor.
        let (mut pos2, mut vel2, mut bbox2, mut og2, mut fd2) = make_above_floor_state(0.0);
        og2.0 = true;
        vel2.0.x = 1.0;

        physics_tick(
            &mut pos2,
            &mut vel2,
            &mut bbox2,
            &mut og2,
            &mut fd2,
            &PIG_DIMS,
            &FloorLevel,
            &shapes,
            false,
            false,
        );
        let stone_vx = vel2.0.x;

        assert!(
            ice_vx > stone_vx,
            "Ice should be more slippery: ice_vx={ice_vx} > stone_vx={stone_vx}"
        );
    }

    #[test]
    fn test_slime_bounce() {
        let shapes = FullCubeShapeProvider::new();
        let reg = BlockRegistry::load().unwrap();
        let slime_id = reg.default_state("minecraft:slime_block").unwrap().0 as u32;
        let slime_level = SpecificFloorLevel {
            floor_state_id: slime_id,
        };

        let (mut pos, mut vel, mut bbox, mut og, mut fd) = make_above_floor_state(0.5);
        vel.0.y = -1.0;

        physics_tick(
            &mut pos,
            &mut vel,
            &mut bbox,
            &mut og,
            &mut fd,
            &PIG_DIMS,
            &slime_level,
            &shapes,
            false,
            false,
        );

        assert!(og.0, "Entity should land on slime");
        assert!(
            vel.0.y > 0.0,
            "Slime should bounce entity upward: vy={}",
            vel.0.y
        );
    }

    #[test]
    fn test_soul_sand_slows_movement() {
        let shapes = FullCubeShapeProvider::new();
        let reg = BlockRegistry::load().unwrap();
        let soul_id = reg.default_state("minecraft:soul_sand").unwrap().0 as u32;
        let soul_level = SpecificFloorLevel {
            floor_state_id: soul_id,
        };

        let (mut pos, mut vel, mut bbox, mut og, mut fd) = make_above_floor_state(0.0);
        og.0 = true;
        vel.0.x = 1.0;

        physics_tick(
            &mut pos,
            &mut vel,
            &mut bbox,
            &mut og,
            &mut fd,
            &PIG_DIMS,
            &soul_level,
            &shapes,
            false,
            false,
        );
        let soul_vx = vel.0.x;

        // Compare with normal floor.
        let (mut pos2, mut vel2, mut bbox2, mut og2, mut fd2) = make_above_floor_state(0.0);
        og2.0 = true;
        vel2.0.x = 1.0;

        physics_tick(
            &mut pos2,
            &mut vel2,
            &mut bbox2,
            &mut og2,
            &mut fd2,
            &PIG_DIMS,
            &FloorLevel,
            &shapes,
            false,
            false,
        );
        let normal_vx = vel2.0.x;

        assert!(
            soul_vx < normal_vx,
            "Soul sand should slow movement: soul_vx={soul_vx} < normal_vx={normal_vx}"
        );
    }

    #[test]
    fn test_honey_block_slows_movement() {
        let shapes = FullCubeShapeProvider::new();
        let reg = BlockRegistry::load().unwrap();
        let honey_id = reg.default_state("minecraft:honey_block").unwrap().0 as u32;
        let honey_level = SpecificFloorLevel {
            floor_state_id: honey_id,
        };

        let (mut pos, mut vel, mut bbox, mut og, mut fd) = make_above_floor_state(0.0);
        og.0 = true;
        vel.0.x = 1.0;

        physics_tick(
            &mut pos,
            &mut vel,
            &mut bbox,
            &mut og,
            &mut fd,
            &PIG_DIMS,
            &honey_level,
            &shapes,
            false,
            false,
        );
        let honey_vx = vel.0.x;

        // Compare with normal floor.
        let (mut pos2, mut vel2, mut bbox2, mut og2, mut fd2) = make_above_floor_state(0.0);
        og2.0 = true;
        vel2.0.x = 1.0;

        physics_tick(
            &mut pos2,
            &mut vel2,
            &mut bbox2,
            &mut og2,
            &mut fd2,
            &PIG_DIMS,
            &FloorLevel,
            &shapes,
            false,
            false,
        );
        let normal_vx = vel2.0.x;

        assert!(
            honey_vx < normal_vx,
            "Honey should slow movement: honey_vx={honey_vx} < normal_vx={normal_vx}"
        );
    }

    // --- R5.5 verification: registry values match vanilla ---

    #[test]
    fn test_registry_ice_friction_098() {
        let reg = BlockRegistry::load().unwrap();
        for name in [
            "minecraft:ice",
            "minecraft:packed_ice",
            "minecraft:frosted_ice",
        ] {
            let state = reg.default_state(name).unwrap();
            assert!(
                (state.friction() - 0.98).abs() < 1e-6,
                "{name} friction should be 0.98, got {}",
                state.friction()
            );
        }
    }

    #[test]
    fn test_registry_blue_ice_friction_0989() {
        let reg = BlockRegistry::load().unwrap();
        let state = reg.default_state("minecraft:blue_ice").unwrap();
        assert!(
            (state.friction() - 0.989).abs() < 1e-6,
            "Blue ice friction should be 0.989, got {}",
            state.friction()
        );
    }

    #[test]
    fn test_registry_soul_sand_speed_04() {
        let reg = BlockRegistry::load().unwrap();
        let state = reg.default_state("minecraft:soul_sand").unwrap();
        assert!(
            (state.speed_factor() - 0.4).abs() < 1e-6,
            "Soul sand speed_factor should be 0.4, got {}",
            state.speed_factor()
        );
    }

    #[test]
    fn test_registry_honey_speed_04_jump_05() {
        let reg = BlockRegistry::load().unwrap();
        let state = reg.default_state("minecraft:honey_block").unwrap();
        assert!(
            (state.speed_factor() - 0.4).abs() < 1e-6,
            "Honey speed_factor should be 0.4, got {}",
            state.speed_factor()
        );
        assert!(
            (state.jump_factor() - 0.5).abs() < 1e-6,
            "Honey jump_factor should be 0.5, got {}",
            state.jump_factor()
        );
    }

    #[test]
    fn test_registry_slime_friction_08() {
        let reg = BlockRegistry::load().unwrap();
        let state = reg.default_state("minecraft:slime_block").unwrap();
        assert!(
            (state.friction() - 0.8).abs() < 1e-6,
            "Slime friction should be 0.8, got {}",
            state.friction()
        );
    }

    #[test]
    fn test_registry_stone_has_default_physics() {
        let reg = BlockRegistry::load().unwrap();
        let state = reg.default_state("minecraft:stone").unwrap();
        assert!(
            (state.friction() - 0.6).abs() < 1e-6,
            "Stone friction should be default 0.6"
        );
        assert!(
            (state.speed_factor() - 1.0).abs() < 1e-6,
            "Stone speed_factor should be 1.0"
        );
        assert!(
            (state.jump_factor() - 1.0).abs() < 1e-6,
            "Stone jump_factor should be 1.0"
        );
    }

    #[test]
    fn test_registry_frosted_ice_all_states_have_ice_friction() {
        let reg = BlockRegistry::load().unwrap();
        let def = reg.get_block_def("minecraft:frosted_ice").unwrap();
        assert_eq!(def.state_count, 4, "Frosted ice should have 4 states");
        for offset in 0..def.state_count {
            let sid = BlockStateId(def.first_state + offset);
            assert!(
                (sid.friction() - 0.98).abs() < 1e-6,
                "Frosted ice state {} friction should be 0.98, got {}",
                sid.0,
                sid.friction()
            );
        }
    }

    #[test]
    fn test_no_block_name_strings_in_physics_lookups() {
        // Verify slime detection uses block_name() method (not hardcoded state IDs).
        // This is a compile-time guarantee since we removed PHYSICS_OVERRIDES,
        // but we verify the mechanism works correctly.
        let reg = BlockRegistry::load().unwrap();
        let slime = reg.default_state("minecraft:slime_block").unwrap();
        assert_eq!(slime.block_name(), "minecraft:slime_block");
        // Non-slime blocks must not be detected as slime.
        let stone = reg.default_state("minecraft:stone").unwrap();
        assert_ne!(stone.block_name(), "minecraft:slime_block");
    }
}
