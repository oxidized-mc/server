//! [`ChunkCache`] — LRU cache for loaded chunks.
//!
//! Uses the `lru` crate for O(1) get/insert/evict operations.
//! Entries are `Arc<RwLock<LevelChunk>>` for concurrent read access.

use std::num::NonZeroUsize;
use std::sync::Arc;

use parking_lot::RwLock;

use oxidized_world::chunk::{ChunkPos, LevelChunk};

/// An LRU cache of loaded chunks.
///
/// Entries are wrapped in `Arc<RwLock<LevelChunk>>` to allow concurrent
/// reads from multiple systems (chunk sending, entity ticking, etc.)
/// while permitting exclusive writes (block placement, lighting updates).
///
/// All operations (get, peek, insert, remove) are O(1).
pub struct ChunkCache {
    inner: lru::LruCache<ChunkPos, Arc<RwLock<LevelChunk>>>,
}

impl ChunkCache {
    /// Creates a new chunk cache with the given maximum capacity.
    ///
    /// # Panics
    ///
    /// Panics if `max_size` is 0.
    #[must_use]
    #[allow(clippy::expect_used)]
    pub fn new(max_size: usize) -> Self {
        let cap = NonZeroUsize::new(max_size).expect("cache max_size must be > 0");
        Self {
            inner: lru::LruCache::new(cap),
        }
    }

    /// Returns the chunk at `pos` if cached, promoting it to most-recently-used.
    pub fn get(&mut self, pos: ChunkPos) -> Option<Arc<RwLock<LevelChunk>>> {
        self.inner.get(&pos).map(Arc::clone)
    }

    /// Returns the chunk at `pos` without updating LRU order.
    ///
    /// Useful for read-only access patterns that should not affect eviction.
    #[must_use]
    pub fn peek(&self, pos: &ChunkPos) -> Option<Arc<RwLock<LevelChunk>>> {
        self.inner.peek(pos).map(Arc::clone)
    }

    /// Inserts a chunk into the cache, evicting the oldest entry if at capacity.
    ///
    /// Returns the `Arc<RwLock<LevelChunk>>` for the inserted chunk.
    pub fn insert(&mut self, pos: ChunkPos, chunk: LevelChunk) -> Arc<RwLock<LevelChunk>> {
        let arc = Arc::new(RwLock::new(chunk));
        self.inner.put(pos, Arc::clone(&arc));
        arc
    }

    /// Removes a chunk from the cache by position.
    pub fn remove(&mut self, pos: &ChunkPos) -> Option<Arc<RwLock<LevelChunk>>> {
        self.inner.pop(pos)
    }

    /// Returns `true` if the cache contains a chunk at `pos`.
    #[must_use]
    pub fn contains(&self, pos: &ChunkPos) -> bool {
        self.inner.contains(pos)
    }

    /// Returns the number of cached chunks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn empty_chunk(x: i32, z: i32) -> LevelChunk {
        LevelChunk::new(ChunkPos::new(x, z))
    }

    #[test]
    fn insert_and_get() {
        let mut cache = ChunkCache::new(4);
        let chunk = empty_chunk(0, 0);
        cache.insert(ChunkPos::new(0, 0), chunk);

        assert_eq!(cache.len(), 1);
        assert!(cache.get(ChunkPos::new(0, 0)).is_some());
        assert!(cache.get(ChunkPos::new(1, 0)).is_none());
    }

    #[test]
    fn lru_eviction() {
        let mut cache = ChunkCache::new(2);
        cache.insert(ChunkPos::new(0, 0), empty_chunk(0, 0));
        cache.insert(ChunkPos::new(1, 0), empty_chunk(1, 0));

        // Access (0,0) to make it most-recently-used.
        cache.get(ChunkPos::new(0, 0));

        // Insert (2,0) — should evict (1,0) as least-recently-used.
        cache.insert(ChunkPos::new(2, 0), empty_chunk(2, 0));

        assert_eq!(cache.len(), 2);
        assert!(cache.contains(&ChunkPos::new(0, 0)));
        assert!(!cache.contains(&ChunkPos::new(1, 0)));
        assert!(cache.contains(&ChunkPos::new(2, 0)));
    }

    #[test]
    fn remove() {
        let mut cache = ChunkCache::new(4);
        cache.insert(ChunkPos::new(0, 0), empty_chunk(0, 0));
        cache.insert(ChunkPos::new(1, 0), empty_chunk(1, 0));

        let removed = cache.remove(&ChunkPos::new(0, 0));
        assert!(removed.is_some());
        assert_eq!(cache.len(), 1);
        assert!(!cache.contains(&ChunkPos::new(0, 0)));
    }

    #[test]
    fn insert_same_pos_does_not_grow() {
        let mut cache = ChunkCache::new(2);
        cache.insert(ChunkPos::new(0, 0), empty_chunk(0, 0));
        cache.insert(ChunkPos::new(0, 0), empty_chunk(0, 0));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn peek_does_not_update_lru() {
        let mut cache = ChunkCache::new(2);
        cache.insert(ChunkPos::new(0, 0), empty_chunk(0, 0));
        cache.insert(ChunkPos::new(1, 0), empty_chunk(1, 0));

        // Peek at (0,0) — should NOT update LRU.
        let _ = cache.peek(&ChunkPos::new(0, 0));

        // Insert (2,0) — should evict (0,0) since peek didn't update LRU.
        cache.insert(ChunkPos::new(2, 0), empty_chunk(2, 0));

        assert_eq!(cache.len(), 2);
        assert!(!cache.contains(&ChunkPos::new(0, 0)));
        assert!(cache.contains(&ChunkPos::new(1, 0)));
        assert!(cache.contains(&ChunkPos::new(2, 0)));
    }

    #[test]
    fn empty_cache() {
        let cache = ChunkCache::new(4);
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }
}
