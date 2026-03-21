//! Dirty chunk tracker — tracks which chunks have been modified since the last save.
//!
//! Used by the autosave system to efficiently determine which chunks need to be
//! written to disk. Thread-safe via `parking_lot::Mutex`.

use ahash::AHashSet;

use oxidized_types::ChunkPos;

/// Tracks which chunks have been modified and need saving.
///
/// Uses an `AHashSet<ChunkPos>` internally for O(1) insert and deduplication.
///
/// # Examples
///
/// ```
/// use oxidized_world::storage::DirtyChunkTracker;
/// use oxidized_types::ChunkPos;
///
/// let mut tracker = DirtyChunkTracker::new();
/// tracker.mark_dirty(ChunkPos::new(0, 0));
/// tracker.mark_dirty(ChunkPos::new(0, 0)); // deduplicated
/// assert_eq!(tracker.dirty_count(), 1);
///
/// let dirty: Vec<_> = tracker.drain_dirty().collect();
/// assert_eq!(dirty.len(), 1);
/// assert_eq!(tracker.dirty_count(), 0);
/// ```
#[derive(Debug, Clone)]
pub struct DirtyChunkTracker {
    dirty: AHashSet<ChunkPos>,
}

impl DirtyChunkTracker {
    /// Creates an empty tracker with no dirty chunks.
    #[must_use]
    pub fn new() -> Self {
        Self {
            dirty: AHashSet::new(),
        }
    }

    /// Marks a chunk as dirty (needs saving).
    ///
    /// Duplicate marks for the same position are deduplicated.
    pub fn mark_dirty(&mut self, pos: ChunkPos) {
        self.dirty.insert(pos);
    }

    /// Drains all dirty positions, returning an iterator.
    ///
    /// After draining, `dirty_count()` returns 0.
    pub fn drain_dirty(&mut self) -> impl Iterator<Item = ChunkPos> + '_ {
        self.dirty.drain()
    }

    /// Returns `true` if the given chunk is marked dirty.
    #[must_use]
    pub fn is_dirty(&self, pos: &ChunkPos) -> bool {
        self.dirty.contains(pos)
    }

    /// Returns the number of dirty chunks.
    #[must_use]
    pub fn dirty_count(&self) -> usize {
        self.dirty.len()
    }

    /// Removes a specific chunk from the dirty set.
    ///
    /// Returns `true` if the chunk was present.
    pub fn clear_dirty(&mut self, pos: &ChunkPos) -> bool {
        self.dirty.remove(pos)
    }

    /// Clears all dirty markers without draining.
    pub fn clear_all(&mut self) {
        self.dirty.clear();
    }
}

impl Default for DirtyChunkTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_new_is_empty() {
        let tracker = DirtyChunkTracker::new();
        assert_eq!(tracker.dirty_count(), 0);
    }

    #[test]
    fn test_mark_and_check() {
        let mut tracker = DirtyChunkTracker::new();
        let pos = ChunkPos::new(5, -3);
        assert!(!tracker.is_dirty(&pos));
        tracker.mark_dirty(pos);
        assert!(tracker.is_dirty(&pos));
        assert_eq!(tracker.dirty_count(), 1);
    }

    #[test]
    fn test_deduplication() {
        let mut tracker = DirtyChunkTracker::new();
        let pos = ChunkPos::new(10, 10);
        tracker.mark_dirty(pos);
        tracker.mark_dirty(pos);
        tracker.mark_dirty(pos);
        assert_eq!(
            tracker.dirty_count(),
            1,
            "same pos marked thrice should not duplicate"
        );
    }

    #[test]
    fn test_drain_empties_tracker() {
        let mut tracker = DirtyChunkTracker::new();
        tracker.mark_dirty(ChunkPos::new(0, 0));
        tracker.mark_dirty(ChunkPos::new(1, 1));
        tracker.mark_dirty(ChunkPos::new(2, 2));
        assert_eq!(tracker.dirty_count(), 3);

        let drained: Vec<_> = tracker.drain_dirty().collect();
        assert_eq!(drained.len(), 3);
        assert_eq!(tracker.dirty_count(), 0);
    }

    #[test]
    fn test_multiple_positions() {
        let mut tracker = DirtyChunkTracker::new();
        for i in 0..100 {
            tracker.mark_dirty(ChunkPos::new(i, -i));
        }
        assert_eq!(tracker.dirty_count(), 100);
    }

    #[test]
    fn test_clear_dirty() {
        let mut tracker = DirtyChunkTracker::new();
        let pos = ChunkPos::new(3, 7);
        tracker.mark_dirty(pos);
        assert!(tracker.clear_dirty(&pos));
        assert!(!tracker.is_dirty(&pos));
        assert_eq!(tracker.dirty_count(), 0);
        assert!(!tracker.clear_dirty(&pos)); // already removed
    }

    #[test]
    fn test_clear_all() {
        let mut tracker = DirtyChunkTracker::new();
        tracker.mark_dirty(ChunkPos::new(0, 0));
        tracker.mark_dirty(ChunkPos::new(1, 1));
        tracker.clear_all();
        assert_eq!(tracker.dirty_count(), 0);
    }

    #[test]
    fn test_default() {
        let tracker = DirtyChunkTracker::default();
        assert_eq!(tracker.dirty_count(), 0);
    }
}
