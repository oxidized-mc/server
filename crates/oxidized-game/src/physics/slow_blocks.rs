//! Slow-block speed and jump modifiers.
//!
//! Certain blocks modify entity movement when standing on or inside them.
//! This module provides lookup functions for those modifiers.
//!
//! In a full implementation, these would be driven by the block registry.
//! For now, the functions accept block state IDs and return modifier values
//! based on known block types.

use super::constants::*;
use crate::level::traits::BlockGetter;

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
pub fn block_speed_factor(level: &impl BlockGetter, x: f64, y: f64, z: f64) -> f64 {
    let feet = BlockPos::new(x.floor() as i32, y.floor() as i32, z.floor() as i32);
    match level.get_block_state(feet) {
        Ok(state_id) => speed_factor_for_state(state_id),
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
pub fn block_jump_factor(level: &impl BlockGetter, x: f64, y: f64, z: f64) -> f64 {
    let feet = BlockPos::new(x.floor() as i32, y.floor() as i32, z.floor() as i32);
    match level.get_block_state(feet) {
        Ok(state_id) => jump_factor_for_state(state_id),
        Err(_) => 1.0,
    }
}

/// Maps a block state ID to its speed factor.
///
/// TODO(p08): Drive from block registry once available.
fn speed_factor_for_state(_state_id: u32) -> f64 {
    // Placeholder — all blocks return 1.0 until the block registry maps
    // state IDs to block properties. Known slow blocks:
    // - Soul Sand → SOUL_SAND_SPEED_FACTOR (0.4)
    // - Honey Block → HONEY_BLOCK_SPEED_FACTOR (0.4)
    // - Powder Snow → POWDER_SNOW_SPEED_FACTOR (0.9)
    let _ = SOUL_SAND_SPEED_FACTOR;
    let _ = HONEY_BLOCK_SPEED_FACTOR;
    let _ = POWDER_SNOW_SPEED_FACTOR;
    1.0
}

/// Maps a block state ID to its jump factor.
///
/// TODO(p08): Drive from block registry once available.
fn jump_factor_for_state(_state_id: u32) -> f64 {
    // Placeholder — all blocks return 1.0 until the block registry maps
    // state IDs. Honey blocks would return HONEY_BLOCK_JUMP_FACTOR (0.5).
    let _ = HONEY_BLOCK_JUMP_FACTOR;
    1.0
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::level::error::LevelError;

    struct EmptyLevel;

    impl BlockGetter for EmptyLevel {
        fn get_block_state(&self, _pos: BlockPos) -> Result<u32, LevelError> {
            Ok(0)
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
}
