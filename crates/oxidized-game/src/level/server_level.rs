//! [`ServerLevel`] — runtime representation of a single dimension's world.
//!
//! Wraps the chunk cache, async chunk loader, and level metadata.
//! Provides block read/write operations and dirty chunk tracking.

use std::collections::HashSet;
use std::sync::Arc;

use parking_lot::Mutex;

use oxidized_protocol::types::BlockPos;
use oxidized_world::anvil::AsyncChunkLoader;
use oxidized_world::chunk::{ChunkPos, LevelChunk};
use oxidized_world::storage::PrimaryLevelData;

use super::chunk_cache::ChunkCache;
use super::dimension::DimensionType;
use super::error::LevelError;
use super::flags::BlockFlags;
use super::traits::{BlockGetter, LevelWriter};

/// Default chunk cache size (number of chunks).
const DEFAULT_CACHE_SIZE: usize = 1024;

/// Runtime representation of a single dimension's world.
///
/// Owns the chunk cache, async chunk loader, and level data.
/// All synchronous block operations require the target chunk to be
/// pre-loaded in the cache.
pub struct ServerLevel {
    /// Static dimension properties (height, min_y, etc.).
    pub dimension_type: DimensionType,
    /// World metadata (spawn, time, weather).
    pub level_data: Arc<tokio::sync::RwLock<PrimaryLevelData>>,
    /// Cached chunks, protected by a mutex for single-writer access.
    chunk_cache: Mutex<ChunkCache>,
    /// Async chunk loader for disk I/O.
    chunk_loader: Arc<AsyncChunkLoader>,
    /// Dirty chunk positions awaiting save.
    dirty_chunks: Mutex<HashSet<ChunkPos>>,
}

impl ServerLevel {
    /// Creates a new `ServerLevel` for the given dimension.
    pub fn new(
        dimension_type: DimensionType,
        level_data: Arc<tokio::sync::RwLock<PrimaryLevelData>>,
        chunk_loader: AsyncChunkLoader,
        cache_size: usize,
    ) -> Self {
        Self {
            dimension_type,
            level_data,
            chunk_cache: Mutex::new(ChunkCache::new(cache_size)),
            chunk_loader: Arc::new(chunk_loader),
            dirty_chunks: Mutex::new(HashSet::new()),
        }
    }

    /// Creates a new `ServerLevel` with default cache size.
    pub fn with_defaults(
        dimension_type: DimensionType,
        level_data: Arc<tokio::sync::RwLock<PrimaryLevelData>>,
        chunk_loader: AsyncChunkLoader,
    ) -> Self {
        Self::new(dimension_type, level_data, chunk_loader, DEFAULT_CACHE_SIZE)
    }

    /// Loads a chunk from cache or disk (async).
    ///
    /// Returns `None` if the chunk does not exist on disk.
    ///
    /// # Errors
    ///
    /// Returns [`LevelError::Io`] on disk I/O failure.
    pub async fn get_or_load_chunk(
        &self,
        pos: ChunkPos,
    ) -> Result<Option<Arc<parking_lot::RwLock<LevelChunk>>>, LevelError> {
        // Check cache first.
        if let Some(c) = self.chunk_cache.lock().get(pos) {
            return Ok(Some(c));
        }
        // Load from disk.
        match self.chunk_loader.load_chunk(pos.x, pos.z).await {
            Ok(None) => Ok(None),
            Ok(Some(chunk)) => {
                let arc = self.chunk_cache.lock().insert(pos, chunk);
                Ok(Some(arc))
            },
            Err(e) => Err(LevelError::Io(e.to_string())),
        }
    }

    /// Synchronous block read — returns an error if the chunk is not loaded.
    ///
    /// # Errors
    ///
    /// Returns [`LevelError::ChunkNotLoaded`] if the chunk is not in the cache.
    /// Returns [`LevelError::Chunk`] if the position is out of bounds within the chunk.
    pub fn get_block_state_loaded(&self, pos: BlockPos) -> Result<u32, LevelError> {
        let cpos = ChunkPos::from_block_coords(pos.x, pos.z);
        let cache = self.chunk_cache.lock();
        let arc = cache.peek(&cpos).ok_or(LevelError::ChunkNotLoaded {
            chunk_x: cpos.x,
            chunk_z: cpos.z,
        })?;
        let chunk = arc.read();
        Ok(chunk.get_block_state(pos.x, pos.y, pos.z)?)
    }

    /// Sets a block, marking the chunk dirty.
    ///
    /// # Errors
    ///
    /// Returns [`LevelError::ChunkNotLoaded`] if the chunk is not in the cache.
    /// Returns [`LevelError::Chunk`] if the position is out of bounds.
    pub fn set_block_state_loaded(
        &self,
        pos: BlockPos,
        state: u32,
        flags: BlockFlags,
    ) -> Result<u32, LevelError> {
        let cpos = ChunkPos::from_block_coords(pos.x, pos.z);
        let cache = self.chunk_cache.lock();
        let arc = cache.peek(&cpos).ok_or(LevelError::ChunkNotLoaded {
            chunk_x: cpos.x,
            chunk_z: cpos.z,
        })?;
        let mut chunk = arc.write();
        let old = chunk.set_block_state(pos.x, pos.y, pos.z, state)?;
        drop(chunk);
        drop(cache);

        if flags.contains(BlockFlags::UPDATE_CLIENTS) {
            // TODO: queue block change packet to nearby players (Phase 13).
        }
        self.dirty_chunks.lock().insert(cpos);
        Ok(old)
    }

    /// Takes the current set of dirty chunks and clears it.
    ///
    /// Returns the set of chunk positions that have been modified since
    /// the last drain.
    pub fn drain_dirty_chunks(&self) -> HashSet<ChunkPos> {
        std::mem::take(&mut *self.dirty_chunks.lock())
    }

    /// Returns `true` if the given chunk is loaded in the cache.
    #[must_use]
    pub fn is_chunk_loaded(&self, pos: &ChunkPos) -> bool {
        self.chunk_cache.lock().contains(pos)
    }

    /// Returns the number of loaded chunks.
    #[must_use]
    pub fn loaded_chunk_count(&self) -> usize {
        self.chunk_cache.lock().len()
    }

    /// Inserts a pre-built chunk into the cache (for testing or worldgen).
    ///
    /// # Panics
    ///
    /// Panics if the chunk's dimensions (min_y, section_count) do not match
    /// this level's [`DimensionType`].
    #[allow(clippy::expect_used)]
    pub fn insert_chunk(
        &self,
        pos: ChunkPos,
        chunk: LevelChunk,
    ) -> Arc<parking_lot::RwLock<LevelChunk>> {
        assert_eq!(
            chunk.min_y(),
            self.dimension_type.min_y,
            "chunk min_y mismatch: expected {}, got {}",
            self.dimension_type.min_y,
            chunk.min_y()
        );
        assert_eq!(
            chunk.section_count(),
            self.dimension_type.section_count(),
            "chunk section count mismatch: expected {}, got {}",
            self.dimension_type.section_count(),
            chunk.section_count()
        );
        self.chunk_cache.lock().insert(pos, chunk)
    }

    /// Creates a new empty chunk with dimensions matching this level's
    /// [`DimensionType`] and inserts it into the cache.
    pub fn create_empty_chunk(&self, pos: ChunkPos) -> Arc<parking_lot::RwLock<LevelChunk>> {
        let chunk = LevelChunk::with_dimensions(
            pos,
            self.dimension_type.min_y,
            self.dimension_type.section_count(),
        );
        self.chunk_cache.lock().insert(pos, chunk)
    }
}

impl BlockGetter for ServerLevel {
    fn get_block_state(&self, pos: BlockPos) -> Result<u32, LevelError> {
        self.get_block_state_loaded(pos)
    }
}

impl LevelWriter for ServerLevel {
    fn set_block_state(
        &mut self,
        pos: BlockPos,
        state: u32,
        flags: BlockFlags,
    ) -> Result<u32, LevelError> {
        self.set_block_state_loaded(pos, state, flags)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Arc;

    use oxidized_world::anvil::AnvilChunkLoader;
    use oxidized_world::registry::BlockRegistry;

    fn test_level() -> ServerLevel {
        let registry = Arc::new(BlockRegistry::load().unwrap());
        let loader = AnvilChunkLoader::new(Path::new("/tmp/oxidized_test_nonexistent"), registry);
        let async_loader = AsyncChunkLoader::new(loader);
        let level_data = PrimaryLevelData::from_nbt(&oxidized_nbt::NbtCompound::new()).unwrap();

        ServerLevel::new(
            DimensionType::overworld(),
            Arc::new(tokio::sync::RwLock::new(level_data)),
            async_loader,
            64,
        )
    }

    fn test_level_with_stone_chunk() -> ServerLevel {
        let level = test_level();
        let mut chunk = LevelChunk::new(ChunkPos::new(0, 0));
        // Set stone (state ID 1) at y=64.
        chunk.set_block_state(0, 64, 0, 1).unwrap();
        level.insert_chunk(ChunkPos::new(0, 0), chunk);
        level
    }

    #[test]
    fn get_block_state_in_loaded_chunk() {
        let level = test_level_with_stone_chunk();
        let pos = BlockPos::new(0, 64, 0);
        assert_eq!(level.get_block_state_loaded(pos).unwrap(), 1);
        let air_pos = BlockPos::new(0, 65, 0);
        assert_eq!(level.get_block_state_loaded(air_pos).unwrap(), 0);
    }

    #[test]
    fn get_block_state_unloaded_chunk() {
        let level = test_level();
        let pos = BlockPos::new(100, 64, 100);
        let err = level.get_block_state_loaded(pos).unwrap_err();
        assert!(matches!(err, LevelError::ChunkNotLoaded { .. }));
    }

    #[test]
    fn set_block_state_marks_chunk_dirty() {
        let level = test_level_with_stone_chunk();
        let pos = BlockPos::new(0, 64, 0);
        let old = level
            .set_block_state_loaded(pos, 0, BlockFlags::DEFAULT)
            .unwrap();
        assert_eq!(old, 1); // was stone
        assert_eq!(level.get_block_state_loaded(pos).unwrap(), 0); // now air

        let dirty = level.drain_dirty_chunks();
        assert!(dirty.contains(&ChunkPos::from_block_coords(0, 0)));

        // After draining, dirty set is empty.
        let dirty2 = level.drain_dirty_chunks();
        assert!(dirty2.is_empty());
    }

    #[test]
    fn block_getter_trait() {
        let level = test_level_with_stone_chunk();
        // Use trait method.
        assert_eq!(level.get_block_state(BlockPos::new(0, 64, 0)).unwrap(), 1);
        assert!(level.is_air(BlockPos::new(0, 65, 0)).unwrap());
        assert!(!level.is_air(BlockPos::new(0, 64, 0)).unwrap());
    }

    #[test]
    fn level_writer_trait() {
        let mut level = test_level_with_stone_chunk();
        let old = level
            .set_block_state(BlockPos::new(0, 64, 0), 0, BlockFlags::DEFAULT)
            .unwrap();
        assert_eq!(old, 1);
        assert!(level.is_air(BlockPos::new(0, 64, 0)).unwrap());
    }

    #[test]
    fn set_block_convenience() {
        let mut level = test_level_with_stone_chunk();
        let old = level.set_block(BlockPos::new(0, 64, 0), 0).unwrap();
        assert_eq!(old, 1);
    }

    #[test]
    fn is_chunk_loaded() {
        let level = test_level_with_stone_chunk();
        assert!(level.is_chunk_loaded(&ChunkPos::new(0, 0)));
        assert!(!level.is_chunk_loaded(&ChunkPos::new(99, 99)));
    }

    #[test]
    fn loaded_chunk_count() {
        let level = test_level();
        assert_eq!(level.loaded_chunk_count(), 0);
        level.insert_chunk(ChunkPos::new(0, 0), LevelChunk::new(ChunkPos::new(0, 0)));
        assert_eq!(level.loaded_chunk_count(), 1);
    }

    #[test]
    #[should_panic(expected = "chunk min_y mismatch")]
    fn insert_chunk_dimension_mismatch_panics() {
        let registry = Arc::new(BlockRegistry::load().unwrap());
        let loader = AnvilChunkLoader::new(Path::new("/tmp/oxidized_test_nonexistent"), registry);
        let async_loader = AsyncChunkLoader::new(loader);
        let level_data = PrimaryLevelData::from_nbt(&oxidized_nbt::NbtCompound::new()).unwrap();

        // Nether level expects min_y=0, 16 sections.
        let level = ServerLevel::new(
            DimensionType::nether(),
            Arc::new(tokio::sync::RwLock::new(level_data)),
            async_loader,
            64,
        );
        // Overworld chunk has min_y=-64, 24 sections — should panic.
        level.insert_chunk(ChunkPos::new(0, 0), LevelChunk::new(ChunkPos::new(0, 0)));
    }

    #[test]
    fn create_empty_chunk_matches_dimension() {
        let registry = Arc::new(BlockRegistry::load().unwrap());
        let loader = AnvilChunkLoader::new(Path::new("/tmp/oxidized_test_nonexistent"), registry);
        let async_loader = AsyncChunkLoader::new(loader);
        let level_data = PrimaryLevelData::from_nbt(&oxidized_nbt::NbtCompound::new()).unwrap();

        let level = ServerLevel::new(
            DimensionType::nether(),
            Arc::new(tokio::sync::RwLock::new(level_data)),
            async_loader,
            64,
        );
        let arc = level.create_empty_chunk(ChunkPos::new(0, 0));
        let chunk = arc.read();
        assert_eq!(chunk.min_y(), 0);
        assert_eq!(chunk.section_count(), 16);
    }
}
