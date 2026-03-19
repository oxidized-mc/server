//! Slow-block speed and jump modifiers.
//!
//! Certain blocks modify entity movement when standing on or inside them.
//! This module provides lookup functions that delegate to
//! [`PhysicsBlockProperties`] for per-block values.
//!
//! [`PhysicsBlockProperties`]: super::block_properties::PhysicsBlockProperties

use crate::level::traits::BlockGetter;

use super::block_properties::PhysicsBlockProperties;

use oxidized_protocol::types::BlockPos;

/// Returns the speed multiplier for the block at the entity's feet.
///
/// Called before computing horizontal input movement. Values < 1.0
/// reduce movement speed.
///
/// | Block | Factor |
/// |-------|--------|
/// | Soul Sand | 0.4 |
/// | Honey Block | 0.4 |
/// | Powder Snow | 0.9 |
/// | All others | 1.0 |
///
/// # Errors
///
/// Returns 1.0 if the block position is in an unloaded chunk.
pub fn block_speed_factor(
    level: &impl BlockGetter,
    block_physics: &PhysicsBlockProperties,
    x: f64,
    y: f64,
    z: f64,
) -> f64 {
    let feet = BlockPos::new(x.floor() as i32, y.floor() as i32, z.floor() as i32);
    match level.get_block_state(feet) {
        Ok(state_id) => block_physics.speed_factor(state_id),
        Err(_) => 1.0,
    }
}

/// Returns the jump factor for the block at the entity's feet.
///
/// Honey blocks reduce jump height to 50%.
///
/// # Errors
///
/// Returns 1.0 if the block position is in an unloaded chunk.
pub fn block_jump_factor(
    level: &impl BlockGetter,
    block_physics: &PhysicsBlockProperties,
    x: f64,
    y: f64,
    z: f64,
) -> f64 {
    let feet = BlockPos::new(x.floor() as i32, y.floor() as i32, z.floor() as i32);
    match level.get_block_state(feet) {
        Ok(state_id) => block_physics.jump_factor(state_id),
        Err(_) => 1.0,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::level::error::LevelError;
    use crate::physics::constants::*;
    use oxidized_world::registry::BlockRegistry;

    struct EmptyLevel;

    impl BlockGetter for EmptyLevel {
        fn get_block_state(&self, _pos: BlockPos) -> Result<u32, LevelError> {
            Ok(0)
        }
    }

    /// A level that returns a specific state ID everywhere.
    struct UniformLevel {
        state_id: u32,
    }

    impl BlockGetter for UniformLevel {
        fn get_block_state(&self, _pos: BlockPos) -> Result<u32, LevelError> {
            Ok(self.state_id)
        }
    }

    fn registry_physics() -> (BlockRegistry, PhysicsBlockProperties) {
        let reg = BlockRegistry::load().expect("block registry");
        let bp = PhysicsBlockProperties::from_registry(&reg);
        (reg, bp)
    }

    #[test]
    fn test_default_speed_factor() {
        let bp = PhysicsBlockProperties::defaults();
        let factor = block_speed_factor(&EmptyLevel, &bp, 0.5, 64.0, 0.5);
        assert!((factor - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_default_jump_factor() {
        let bp = PhysicsBlockProperties::defaults();
        let factor = block_jump_factor(&EmptyLevel, &bp, 0.5, 64.0, 0.5);
        assert!((factor - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_soul_sand_slows_movement() {
        let (reg, bp) = registry_physics();
        let soul_id = reg.default_state("minecraft:soul_sand").unwrap().0 as u32;
        let level = UniformLevel {
            state_id: soul_id,
        };
        let factor = block_speed_factor(&level, &bp, 0.5, 64.0, 0.5);
        assert!(
            (factor - SOUL_SAND_SPEED_FACTOR).abs() < 1e-10,
            "Soul sand speed factor should be {SOUL_SAND_SPEED_FACTOR}, got {factor}"
        );
    }

    #[test]
    fn test_honey_slows_movement() {
        let (reg, bp) = registry_physics();
        let honey_id = reg.default_state("minecraft:honey_block").unwrap().0 as u32;
        let level = UniformLevel {
            state_id: honey_id,
        };
        let factor = block_speed_factor(&level, &bp, 0.5, 64.0, 0.5);
        assert!(
            (factor - HONEY_BLOCK_SPEED_FACTOR).abs() < 1e-10,
            "Honey speed factor should be {HONEY_BLOCK_SPEED_FACTOR}, got {factor}"
        );
    }

    #[test]
    fn test_honey_reduces_jump() {
        let (reg, bp) = registry_physics();
        let honey_id = reg.default_state("minecraft:honey_block").unwrap().0 as u32;
        let level = UniformLevel {
            state_id: honey_id,
        };
        let factor = block_jump_factor(&level, &bp, 0.5, 64.0, 0.5);
        assert!(
            (factor - HONEY_BLOCK_JUMP_FACTOR).abs() < 1e-10,
            "Honey jump factor should be {HONEY_BLOCK_JUMP_FACTOR}, got {factor}"
        );
    }

    #[test]
    fn test_powder_snow_slows_movement() {
        let (reg, bp) = registry_physics();
        let powder_id = reg.default_state("minecraft:powder_snow").unwrap().0 as u32;
        let level = UniformLevel {
            state_id: powder_id,
        };
        let factor = block_speed_factor(&level, &bp, 0.5, 64.0, 0.5);
        assert!(
            (factor - POWDER_SNOW_SPEED_FACTOR).abs() < 1e-10,
            "Powder snow speed factor should be {POWDER_SNOW_SPEED_FACTOR}, got {factor}"
        );
    }

    #[test]
    fn test_stone_has_normal_speed() {
        let (reg, bp) = registry_physics();
        let stone_id = reg.default_state("minecraft:stone").unwrap().0 as u32;
        let level = UniformLevel {
            state_id: stone_id,
        };
        let factor = block_speed_factor(&level, &bp, 0.5, 64.0, 0.5);
        assert!(
            (factor - 1.0).abs() < 1e-10,
            "Stone should have speed factor 1.0"
        );
    }
}
