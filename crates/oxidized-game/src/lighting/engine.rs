//! Lighting engine: batched BFS sky and block light propagation.
//!
//! Processes [`super::queue::LightUpdateQueue`] entries in batched passes,
//! grouping updates by section and propagating cross-section changes.
//! See ADR-017 for the algorithm design.
//!
//! # Status
//!
//! This is a skeleton — all methods return `todo!()`. The BFS propagation
//! logic will be implemented in Phase 23a (Lighting Engine).

use oxidized_protocol::types::SectionPos;
use oxidized_world::chunk::LevelChunk;

use super::queue::LightUpdateQueue;

/// Errors that can occur during light processing.
#[derive(Debug, thiserror::Error)]
pub enum LightingError {
    /// A referenced chunk section is not loaded or available.
    #[error("chunk section unavailable at {section}")]
    SectionUnavailable {
        /// The position of the unavailable section.
        section: SectionPos,
    },
}

/// Batched BFS lighting engine.
///
/// Owns a [`LightUpdateQueue`] and processes all pending updates in one pass
/// at the end of each tick. Groups updates by section, runs decrease then
/// increase BFS passes, and propagates across section boundaries.
///
/// # Examples
///
/// ```
/// use oxidized_game::lighting::engine::LightEngine;
///
/// let engine = LightEngine::new();
/// assert!(engine.queue().is_empty());
/// ```
pub struct LightEngine {
    queue: LightUpdateQueue,
}

impl LightEngine {
    /// Creates a new lighting engine with an empty update queue.
    #[must_use]
    pub fn new() -> Self {
        Self {
            queue: LightUpdateQueue::new(),
        }
    }

    /// Returns a reference to the update queue.
    #[must_use]
    pub fn queue(&self) -> &LightUpdateQueue {
        &self.queue
    }

    /// Returns a mutable reference to the update queue.
    pub fn queue_mut(&mut self) -> &mut LightUpdateQueue {
        &mut self.queue
    }

    /// Processes all pending light updates for this tick.
    ///
    /// Groups updates by section, processes each section's decrease and
    /// increase BFS passes, and propagates cross-section changes.
    /// Returns the list of sections whose light data changed.
    ///
    /// See ADR-017 for the batched BFS algorithm.
    ///
    /// # Errors
    ///
    /// Returns [`LightingError`] if a referenced chunk section is unavailable.
    pub fn process_updates(&mut self) -> Result<Vec<SectionPos>, LightingError> {
        todo!("ADR-017: BFS propagation — implemented in Phase 23a")
    }

    /// Computes full sky + block light for a newly generated chunk.
    ///
    /// Called by the worldgen pipeline at the Light status (ADR-016).
    /// Initializes sky light top-down from the heightmap, seeds block light
    /// from emitters, and runs BFS propagation for both light types.
    ///
    /// # Errors
    ///
    /// Returns [`LightingError`] if a chunk section is unavailable.
    pub fn light_chunk(&mut self, _chunk: &LevelChunk) -> Result<(), LightingError> {
        todo!("ADR-017: Full chunk lighting — implemented in Phase 23a")
    }
}

impl Default for LightEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::lighting::queue::LightUpdate;
    use oxidized_protocol::types::BlockPos;

    #[test]
    fn test_engine_new_has_empty_queue() {
        let engine = LightEngine::new();
        assert!(engine.queue().is_empty());
    }

    #[test]
    fn test_engine_queue_mut() {
        let mut engine = LightEngine::new();
        engine.queue_mut().push(LightUpdate {
            pos: BlockPos::new(0, 64, 0),
            old_emission: 0,
            new_emission: 14,
            old_opacity: 0,
            new_opacity: 0,
        });
        assert_eq!(engine.queue().len(), 1);
    }
}
