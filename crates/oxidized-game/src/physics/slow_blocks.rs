//! Slow-block speed and jump modifiers.
//!
//! Certain blocks modify entity movement when standing on or inside them.
//! This module provides lookup functions that read physics properties
//! directly from the block registry via [`BlockStateId`].
//!
//! [`BlockStateId`]: oxidized_world::registry::BlockStateId

use crate::level::traits::BlockGetter;

use oxidized_protocol::types::BlockPos;
use oxidized_world::registry::BlockStateId;

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
    x: f64,
    y: f64,
    z: f64,
) -> f64 {
    let feet = BlockPos::new(x.floor() as i32, y.floor() as i32, z.floor() as i32);
    match level.get_block_state(feet) {
        Ok(state_id) => BlockStateId(state_id as u16).speed_factor(),
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
    x: f64,
    y: f64,
    z: f64,
) -> f64 {
    let feet = BlockPos::new(x.floor() as i32, y.floor() as i32, z.floor() as i32);
    match level.get_block_state(feet) {
        Ok(state_id) => BlockStateId(state_id as u16).jump_factor(),
        Err(_) => 1.0,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::level::error::LevelError;
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

    #[test]
    fn test_default_speed_factor() {
        let factor = block_speed_factor(&EmptyLevel, 0.5, 64.0, 0.5);
        assert!((factor - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_default_jump_factor() {
        let factor = block_jump_factor(&EmptyLevel, 0.5, 64.0, 0.5);
        assert!((factor - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_soul_sand_slows_movement() {
        let reg = BlockRegistry::load().expect("block registry");
        let soul_id = reg.default_state("minecraft:soul_sand").unwrap().0 as u32;
        let level = UniformLevel { state_id: soul_id };
        let factor = block_speed_factor(&level, 0.5, 64.0, 0.5);
        assert!(
            (factor - 0.4).abs() < 1e-10,
            "Soul sand speed factor should be 0.4, got {factor}"
        );
    }

    #[test]
    fn test_honey_slows_movement() {
        let reg = BlockRegistry::load().expect("block registry");
        let honey_id = reg.default_state("minecraft:honey_block").unwrap().0 as u32;
        let level = UniformLevel { state_id: honey_id };
        let factor = block_speed_factor(&level, 0.5, 64.0, 0.5);
        assert!(
            (factor - 0.4).abs() < 1e-10,
            "Honey speed factor should be 0.4, got {factor}"
        );
    }

    #[test]
    fn test_honey_reduces_jump() {
        let reg = BlockRegistry::load().expect("block registry");
        let honey_id = reg.default_state("minecraft:honey_block").unwrap().0 as u32;
        let level = UniformLevel { state_id: honey_id };
        let factor = block_jump_factor(&level, 0.5, 64.0, 0.5);
        assert!(
            (factor - 0.5).abs() < 1e-10,
            "Honey jump factor should be 0.5, got {factor}"
        );
    }

    #[test]
    fn test_powder_snow_has_default_speed_factor() {
        // Powder snow's speed reduction (0.9) comes from PowderSnowBlock's
        // makeStuckInBlock() runtime behavior, NOT the speedFactor block
        // property. The block registry correctly reports speed_factor = 1.0.
        let reg = BlockRegistry::load().expect("block registry");
        let powder_id = reg.default_state("minecraft:powder_snow").unwrap().0 as u32;
        let level = UniformLevel {
            state_id: powder_id,
        };
        let factor = block_speed_factor(&level, 0.5, 64.0, 0.5);
        assert!(
            (factor - 1.0).abs() < 1e-10,
            "Powder snow speed_factor property should be 1.0, got {factor}"
        );
    }

    #[test]
    fn test_stone_has_normal_speed() {
        let reg = BlockRegistry::load().expect("block registry");
        let stone_id = reg.default_state("minecraft:stone").unwrap().0 as u32;
        let level = UniformLevel { state_id: stone_id };
        let factor = block_speed_factor(&level, 0.5, 64.0, 0.5);
        assert!(
            (factor - 1.0).abs() < 1e-10,
            "Stone should have speed factor 1.0"
        );
    }
}
