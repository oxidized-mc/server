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

use crate::entity::Entity;
use crate::level::traits::BlockGetter;

use super::collision::{collect_obstacles, collide_with_shapes};
use super::constants::*;
use super::voxel_shape::BlockShapeProvider;

use oxidized_protocol::types::BlockPos;
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
pub fn physics_tick(
    entity: &mut Entity,
    level: &impl BlockGetter,
    shape_provider: &impl BlockShapeProvider,
    in_water: bool,
    in_lava: bool,
) {
    // 1. Determine block friction (from block below entity feet).
    let block_friction = get_block_friction(level, entity);

    // 2. Apply gravity (or fluid modifiers).
    if in_water {
        entity.velocity.y += WATER_BUOYANCY;
    } else if in_lava {
        entity.velocity.y += LAVA_BUOYANCY;
    } else {
        entity.velocity.y -= GRAVITY;
    }

    // Apply block speed factor (e.g., soul sand, honey) BEFORE collision.
    let speed_factor = get_block_speed_factor(level, entity);
    entity.velocity.x *= speed_factor;
    entity.velocity.z *= speed_factor;

    let mut dx = entity.velocity.x;
    let dy = entity.velocity.y;
    let mut dz = entity.velocity.z;

    // Apply fluid drag to the movement delta.
    if in_water {
        dx *= WATER_DRAG;
        dz *= WATER_DRAG;
    } else if in_lava {
        dx *= LAVA_DRAG;
        dz *= LAVA_DRAG;
    }

    // 3. Swept AABB collision with movement-dependent axis ordering.
    let bbox = entity.bounding_box;
    let obstacles = collect_obstacles(level, &bbox, dx, dy, dz, shape_provider);
    let (actual_dx, actual_dy, actual_dz) = collide_with_shapes(&bbox, dx, dy, dz, &obstacles);

    // 4. Apply resolved movement.
    entity.set_pos(
        entity.pos.x + actual_dx,
        entity.pos.y + actual_dy,
        entity.pos.z + actual_dz,
    );

    // 5. Detect on_ground: downward movement was reduced by collision.
    entity.is_on_ground = dy < 0.0 && (actual_dy - dy).abs() > COLLISION_EPSILON;

    // 6. Zero velocity on collision axes, with slime bounce for Y.
    if (actual_dx - dx).abs() > COLLISION_EPSILON {
        entity.velocity.x = 0.0;
    }
    if (actual_dy - dy).abs() > COLLISION_EPSILON {
        // Check for slime bounce: if landing on a slime block, negate Y velocity.
        if entity.is_on_ground && is_on_slime(level, entity) {
            entity.velocity.y = -entity.velocity.y;
        } else {
            entity.velocity.y = 0.0;
        }
    }
    if (actual_dz - dz).abs() > COLLISION_EPSILON {
        entity.velocity.z = 0.0;
    }

    // 7. Apply drag.
    let h_drag = if entity.is_on_ground {
        block_friction * HORIZONTAL_DRAG
    } else {
        HORIZONTAL_DRAG
    };

    entity.velocity.x *= h_drag;
    entity.velocity.y *= VERTICAL_DRAG;
    entity.velocity.z *= h_drag;

    // 8. Fall distance tracking.
    if !entity.is_on_ground && actual_dy < 0.0 {
        // Accumulate downward distance (actual_dy is negative, so negate).
        entity.fall_distance -= actual_dy as f32;
    } else if entity.is_on_ground {
        // Reset on landing (damage would be calculated before this reset
        // in a full implementation).
        entity.fall_distance = 0.0;
    }
}

/// Returns the friction value of the block below the entity's feet.
///
/// Reads directly from the block registry via [`BlockStateId`].
/// Returns 1.0 when not on ground (no block friction applies in air).
fn get_block_friction(level: &impl BlockGetter, entity: &Entity) -> f64 {
    if !entity.is_on_ground {
        return 1.0;
    }

    let below = BlockPos::new(
        entity.pos.x.floor() as i32,
        (entity.bounding_box.min_y - 0.5000001).floor() as i32,
        entity.pos.z.floor() as i32,
    );

    match level.get_block_state(below) {
        Ok(state_id) => BlockStateId(state_id as u16).friction(),
        Err(_) => BLOCK_FRICTION_DEFAULT,
    }
}

/// Returns `true` if the block below the entity's feet is a slime block.
fn is_on_slime(level: &impl BlockGetter, entity: &Entity) -> bool {
    let below = BlockPos::new(
        entity.pos.x.floor() as i32,
        (entity.bounding_box.min_y - 0.5000001).floor() as i32,
        entity.pos.z.floor() as i32,
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
fn get_block_speed_factor(level: &impl BlockGetter, entity: &Entity) -> f64 {
    let below = BlockPos::new(
        entity.pos.x.floor() as i32,
        (entity.bounding_box.min_y - 0.5000001).floor() as i32,
        entity.pos.z.floor() as i32,
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
    use crate::level::error::LevelError;
    use crate::physics::voxel_shape::FullCubeShapeProvider;
    use oxidized_protocol::types::resource_location::ResourceLocation;
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

    fn make_floating_entity() -> Entity {
        let mut e = Entity::new(ResourceLocation::minecraft("pig"), 0.6, 1.8);
        e.set_pos(8.0, 100.0, 8.0);
        e
    }

    fn make_entity_above_floor(height: f64) -> Entity {
        let mut e = Entity::new(ResourceLocation::minecraft("pig"), 0.6, 1.8);
        // Floor is at y=0..1 (full cube). Entity feet at y = 1.0 + height.
        e.set_pos(0.5, 1.0 + height, 0.5);
        e
    }

    #[test]
    fn test_gravity_acceleration() {
        let shapes = FullCubeShapeProvider::new();
        let mut entity = make_floating_entity();
        entity.velocity.y = 0.0;

        // Tick 1: gravity: vy = 0 - 0.08 = -0.08
        // After drag: vy = -0.08 * 0.98 = -0.0784
        physics_tick(&mut entity, &EmptyLevel, &shapes, false, false);
        assert!(
            (entity.velocity.y - (-0.0784)).abs() < 0.0001,
            "After tick 1: vy = {} (expected ≈ -0.0784)",
            entity.velocity.y
        );

        // Tick 2: gravity: vy = -0.0784 - 0.08 = -0.1584
        // After drag: vy ≈ -0.1584 * 0.98 ≈ -0.155232
        physics_tick(&mut entity, &EmptyLevel, &shapes, false, false);
        assert!(
            entity.velocity.y < -0.15,
            "After tick 2: vy = {} (should be < -0.15)",
            entity.velocity.y
        );
    }

    #[test]
    fn test_collision_stops_downward_movement() {
        let shapes = FullCubeShapeProvider::new();
        let mut entity = make_entity_above_floor(0.5);
        entity.velocity.y = -5.0;

        physics_tick(&mut entity, &FloorLevel, &shapes, false, false);

        assert!(entity.is_on_ground, "Entity should be on ground");
        assert!(
            entity.velocity.y.abs() < 0.001,
            "vy should be zeroed: {}",
            entity.velocity.y
        );
        assert!(
            entity.pos.y >= 1.0 - 0.001,
            "Entity should not pass through floor: y={}",
            entity.pos.y
        );
    }

    #[test]
    fn test_water_buoyancy() {
        let shapes = FullCubeShapeProvider::new();
        let mut entity = make_floating_entity();
        entity.velocity.y = 0.0;
        entity.velocity.x = 1.0;

        physics_tick(&mut entity, &EmptyLevel, &shapes, true, false);

        // In water: buoyancy pushes up, no gravity.
        assert!(entity.velocity.y > 0.0, "Water buoyancy should push upward");
        // Horizontal drag: vx should be reduced.
        assert!(
            entity.velocity.x < 1.0,
            "Water should reduce horizontal velocity"
        );
    }

    #[test]
    fn test_fall_distance_accumulates() {
        let shapes = FullCubeShapeProvider::new();
        let mut entity = make_floating_entity();
        entity.velocity.y = 0.0;

        physics_tick(&mut entity, &EmptyLevel, &shapes, false, false);
        assert!(
            entity.fall_distance > 0.0,
            "Fall distance should increase while falling"
        );

        let fd1 = entity.fall_distance;
        physics_tick(&mut entity, &EmptyLevel, &shapes, false, false);
        assert!(
            entity.fall_distance > fd1,
            "Fall distance should keep increasing"
        );
    }

    #[test]
    fn test_fall_distance_resets_on_landing() {
        let shapes = FullCubeShapeProvider::new();
        let mut entity = make_entity_above_floor(0.5);
        entity.velocity.y = -1.0;
        entity.fall_distance = 5.0;

        physics_tick(&mut entity, &FloorLevel, &shapes, false, false);

        assert!(entity.is_on_ground, "Entity should land");
        assert!(
            (entity.fall_distance).abs() < 0.001,
            "Fall distance should reset on landing: {}",
            entity.fall_distance
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
        let mut entity = make_entity_above_floor(0.0);
        entity.is_on_ground = true;
        entity.velocity.x = 0.0;
        entity.velocity.y = 0.0;
        entity.velocity.z = 0.0;

        let y_before = entity.pos.y;
        physics_tick(&mut entity, &FloorLevel, &shapes, false, false);

        // Entity should stay on ground (gravity pulls down, floor stops it).
        assert!(entity.is_on_ground, "Entity should remain on ground");
        assert!(
            (entity.pos.y - y_before).abs() < 0.01,
            "Entity should stay near same Y: before={y_before}, after={}",
            entity.pos.y
        );
    }

    #[test]
    fn test_lava_buoyancy() {
        let shapes = FullCubeShapeProvider::new();
        let mut entity = make_floating_entity();
        entity.velocity.y = 0.0;

        physics_tick(&mut entity, &EmptyLevel, &shapes, false, true);

        // Lava buoyancy is less than water.
        assert!(
            entity.velocity.y > 0.0,
            "Lava buoyancy should push upward: vy={}",
            entity.velocity.y
        );
    }

    #[test]
    fn test_single_block_collision() {
        let shapes = FullCubeShapeProvider::new();
        let level = SingleBlockLevel {
            pos: BlockPos::new(0, 0, 0),
            state_id: 1, // stone
        };

        // Entity above the single block, falling.
        let mut entity = Entity::new(ResourceLocation::minecraft("pig"), 0.6, 1.8);
        entity.set_pos(0.5, 1.5, 0.5);
        entity.velocity.y = -5.0;

        physics_tick(&mut entity, &level, &shapes, false, false);

        assert!(entity.is_on_ground, "Entity should land on single block");
        assert!(
            entity.pos.y >= 1.0 - 0.001,
            "Entity should not pass through block: y={}",
            entity.pos.y
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

        let mut entity = make_entity_above_floor(0.0);
        entity.is_on_ground = true;
        entity.velocity.x = 1.0;
        entity.velocity.y = 0.0;
        entity.velocity.z = 0.0;

        physics_tick(&mut entity, &ice_level, &shapes, false, false);

        // On ice: drag = 0.98 * 0.91 = 0.8918
        // On normal: drag = 0.6 * 0.91 = 0.546
        // Ice should preserve more velocity.
        let ice_vx = entity.velocity.x;
        assert!(
            ice_vx > 0.8,
            "Ice should preserve high velocity: vx={ice_vx}"
        );

        // Compare with stone floor.
        let mut entity2 = make_entity_above_floor(0.0);
        entity2.is_on_ground = true;
        entity2.velocity.x = 1.0;
        entity2.velocity.y = 0.0;
        entity2.velocity.z = 0.0;

        physics_tick(&mut entity2, &FloorLevel, &shapes, false, false);
        let stone_vx = entity2.velocity.x;

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

        let mut entity = make_entity_above_floor(0.5);
        entity.velocity.y = -1.0;

        physics_tick(&mut entity, &slime_level, &shapes, false, false);

        // Entity should bounce: vy negated then drag applied.
        // Pre-bounce vy was some negative value after gravity.
        // After negate, it should be positive. After * VERTICAL_DRAG, still positive.
        assert!(entity.is_on_ground, "Entity should land on slime");
        assert!(
            entity.velocity.y > 0.0,
            "Slime should bounce entity upward: vy={}",
            entity.velocity.y
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

        let mut entity = make_entity_above_floor(0.0);
        entity.is_on_ground = true;
        entity.velocity.x = 1.0;
        entity.velocity.y = 0.0;
        entity.velocity.z = 0.0;

        physics_tick(&mut entity, &soul_level, &shapes, false, false);
        let soul_vx = entity.velocity.x;

        // Compare with normal floor.
        let mut entity2 = make_entity_above_floor(0.0);
        entity2.is_on_ground = true;
        entity2.velocity.x = 1.0;
        entity2.velocity.y = 0.0;
        entity2.velocity.z = 0.0;

        physics_tick(&mut entity2, &FloorLevel, &shapes, false, false);
        let normal_vx = entity2.velocity.x;

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

        let mut entity = make_entity_above_floor(0.0);
        entity.is_on_ground = true;
        entity.velocity.x = 1.0;
        entity.velocity.y = 0.0;
        entity.velocity.z = 0.0;

        physics_tick(&mut entity, &honey_level, &shapes, false, false);
        let honey_vx = entity.velocity.x;

        // Compare with normal floor.
        let mut entity2 = make_entity_above_floor(0.0);
        entity2.is_on_ground = true;
        entity2.velocity.x = 1.0;
        entity2.velocity.y = 0.0;
        entity2.velocity.z = 0.0;

        physics_tick(&mut entity2, &FloorLevel, &shapes, false, false);
        let normal_vx = entity2.velocity.x;

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
