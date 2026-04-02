//! Read-only and read-write block access traits.
//!
//! All block access in the game layer goes through [`BlockGetter`] (reads)
//! or [`LevelWriter`] (writes). Implementations include [`ServerLevel`]
//! and any future snapshot or test stubs.
//!
//! Block states are represented as `u32` IDs matching the vanilla flat
//! state ID space (0 = air, 1 = stone, etc.). Use [`BlockStateId`] for
//! O(1) access to block metadata and property transitions.
//!
//! [`ServerLevel`]: super::ServerLevel
//! [`BlockStateId`]: oxidized_registry::BlockStateId

use oxidized_mc_types::BlockPos;

use super::error::LevelError;
use super::flags::BlockFlags;

/// The block state ID for air (always 0 in vanilla).
pub const AIR_STATE_ID: u32 = 0;

/// Read-only block access.
pub trait BlockGetter {
    /// Returns the block state ID at the given position.
    ///
    /// # Errors
    ///
    /// Returns [`LevelError`] if the position is in an unloaded chunk or
    /// outside valid world bounds.
    fn get_block_state(&self, pos: BlockPos) -> Result<u32, LevelError>;

    /// Returns `true` if the block at `pos` is air.
    ///
    /// # Errors
    ///
    /// Returns [`LevelError`] if the chunk is not loaded.
    fn is_air(&self, pos: BlockPos) -> Result<bool, LevelError> {
        Ok(self.get_block_state(pos)? == AIR_STATE_ID)
    }
}

/// Read-write block access.
pub trait LevelWriter: BlockGetter {
    /// Sets the block state at `pos`, returning the old state ID.
    ///
    /// `flags` controls update propagation and client notification.
    ///
    /// # Errors
    ///
    /// Returns [`LevelError`] if the chunk is not loaded or the position
    /// is outside valid world bounds.
    fn set_block_state(
        &mut self,
        pos: BlockPos,
        state: u32,
        flags: BlockFlags,
    ) -> Result<u32, LevelError>;

    /// Convenience: set with default flags (`UPDATE_NEIGHBORS | UPDATE_CLIENTS`).
    ///
    /// # Errors
    ///
    /// Returns [`LevelError`] if the chunk is not loaded.
    fn set_block(&mut self, pos: BlockPos, state: u32) -> Result<u32, LevelError> {
        self.set_block_state(pos, state, BlockFlags::DEFAULT)
    }
}
