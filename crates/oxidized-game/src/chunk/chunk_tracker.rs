//! Per-player chunk tracking.
//!
//! Maintains the set of chunk positions currently loaded by a specific
//! player and computes load/unload deltas when the player crosses chunk
//! boundaries.

use std::collections::HashSet;

use oxidized_world::chunk::ChunkPos;

use super::view_distance::{chunks_to_load, chunks_to_unload, spiral_chunks};

/// Tracks which chunks a player has loaded.
///
/// Wraps the view-distance delta functions with a persistent set of
/// loaded chunks per player.
#[derive(Debug)]
pub struct PlayerChunkTracker {
    /// Current chunk the player is in.
    pub center: ChunkPos,
    /// View distance in chunks (Chebyshev radius).
    pub view_distance: i32,
    /// All chunks currently loaded by this player.
    pub loaded: HashSet<ChunkPos>,
}

impl PlayerChunkTracker {
    /// Creates a new tracker centered at the given chunk position.
    ///
    /// The loaded set is initialized to all chunks within `view_distance`.
    pub fn new(center: ChunkPos, view_distance: i32) -> Self {
        let loaded: HashSet<ChunkPos> = spiral_chunks(center, view_distance).collect();
        Self {
            center,
            view_distance,
            loaded,
        }
    }

    /// Updates the center chunk and returns chunks to load/unload.
    ///
    /// If the player hasn't moved to a new chunk, returns empty vectors.
    ///
    /// # Returns
    ///
    /// `(to_load, to_unload)` — chunks the server should send and forget.
    pub fn update_center(&mut self, new_center: ChunkPos) -> (Vec<ChunkPos>, Vec<ChunkPos>) {
        if new_center == self.center {
            return (vec![], vec![]);
        }
        let to_load = chunks_to_load(self.center, new_center, self.view_distance);
        let to_unload = chunks_to_unload(self.center, new_center, self.view_distance);
        self.center = new_center;
        for p in &to_load {
            self.loaded.insert(*p);
        }
        for p in &to_unload {
            self.loaded.remove(p);
        }
        (to_load, to_unload)
    }

    /// Returns `true` if the given chunk is currently loaded for this player.
    pub fn is_loaded(&self, pos: &ChunkPos) -> bool {
        self.loaded.contains(pos)
    }

    /// Returns the number of loaded chunks.
    pub fn loaded_count(&self) -> usize {
        self.loaded.len()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_new_populates_loaded() {
        let tracker = PlayerChunkTracker::new(ChunkPos::new(0, 0), 2);
        // (2*2+1)^2 = 25 chunks
        assert_eq!(tracker.loaded_count(), 25);
        assert!(tracker.is_loaded(&ChunkPos::new(0, 0)));
        assert!(tracker.is_loaded(&ChunkPos::new(2, 2)));
        assert!(tracker.is_loaded(&ChunkPos::new(-2, -2)));
        assert!(!tracker.is_loaded(&ChunkPos::new(3, 0)));
    }

    #[test]
    fn test_no_movement_no_changes() {
        let mut tracker = PlayerChunkTracker::new(ChunkPos::new(0, 0), 2);
        let (to_load, to_unload) = tracker.update_center(ChunkPos::new(0, 0));
        assert!(to_load.is_empty());
        assert!(to_unload.is_empty());
    }

    #[test]
    fn test_move_one_chunk() {
        let mut tracker = PlayerChunkTracker::new(ChunkPos::new(0, 0), 2);
        let initial_count = tracker.loaded_count();
        let (to_load, to_unload) = tracker.update_center(ChunkPos::new(1, 0));

        // Load and unload counts should match (same view distance)
        assert_eq!(to_load.len(), to_unload.len());
        assert!(!to_load.is_empty());

        // Total loaded should remain the same
        assert_eq!(tracker.loaded_count(), initial_count);

        // New center should be updated
        assert_eq!(tracker.center, ChunkPos::new(1, 0));

        // Loaded chunks should include new ones
        for pos in &to_load {
            assert!(tracker.is_loaded(pos));
        }

        // Unloaded chunks should be gone
        for pos in &to_unload {
            assert!(!tracker.is_loaded(pos));
        }
    }

    #[test]
    fn test_move_produces_correct_columns() {
        let mut tracker = PlayerChunkTracker::new(ChunkPos::new(0, 0), 2);
        let (to_load, to_unload) = tracker.update_center(ChunkPos::new(1, 0));

        // Moving +1 in X: new column at x=3, old column at x=-2
        assert!(
            to_load.iter().all(|p| p.x == 3),
            "load should be x=3: {to_load:?}"
        );
        assert!(
            to_unload.iter().all(|p| p.x == -2),
            "unload should be x=-2: {to_unload:?}"
        );
    }

    #[test]
    fn test_large_jump() {
        let mut tracker = PlayerChunkTracker::new(ChunkPos::new(0, 0), 1);
        let (to_load, to_unload) = tracker.update_center(ChunkPos::new(100, 100));

        // All old chunks should be unloaded (no overlap at distance 100)
        assert_eq!(to_unload.len(), 9); // 3×3
        assert_eq!(to_load.len(), 9); // 3×3 new area
    }
}
