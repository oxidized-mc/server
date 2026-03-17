# Phase 11 — Server Level + Block Access

**Crate:** `oxidized-game`  
**Reward:** Query any block in the loaded world. Given a `BlockPos`, the server
returns the correct `BlockState`, loading the chunk on demand if necessary.
`set_block_state` marks the chunk dirty for eventual saving.

---

## Goal

Build `ServerLevel`: the runtime world representation that wraps the chunk map,
drives on-demand loading, handles block read/write, and manages multiple
dimensions. Implement the `BlockGetter` and `LevelWriter` traits so that all
game logic can use a uniform API regardless of which dimension it targets.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Server level | `ServerLevel` | `net.minecraft.server.level.ServerLevel` |
| Abstract level | `Level` | `net.minecraft.world.level.Level` |
| Block getter | `BlockGetter` | `net.minecraft.world.level.BlockGetter` |
| Level reader | `LevelReader` | `net.minecraft.world.level.LevelReader` |
| Level accessor | `LevelAccessor` | `net.minecraft.world.level.LevelAccessor` |
| Chunk map | `ChunkMap` | `net.minecraft.server.level.ChunkMap` |
| Chunk provider | `ServerChunkCache` | `net.minecraft.server.level.ServerChunkCache` |
| Dimension type | `DimensionType` | `net.minecraft.world.level.dimension.DimensionType` |
| Block flags | `Level` (constants) | `net.minecraft.world.level.Level` |

---

## Tasks

### 11.1 — Block update flags

```rust
// crates/oxidized-game/src/level/flags.rs

bitflags::bitflags! {
    /// Flags passed to `set_block_state`. Mirror Java's `Level` constants.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BlockFlags: u8 {
        /// Propagate block updates to neighbouring blocks.
        const UPDATE_NEIGHBORS  = 0x01;
        /// Notify clients (send block change packet).
        const UPDATE_CLIENTS    = 0x02;
        /// Suppress re-renders (used for invisible updates).
        const UPDATE_INVISIBLE  = 0x04;
        /// Skip comparator updates.
        const UPDATE_KNOWN_SHAPE = 0x10;
        /// Prevent drops when breaking blocks.
        const UPDATE_SUPPRESS_DROPS = 0x20;
        /// Default: neighbours + clients.
        const DEFAULT = Self::UPDATE_NEIGHBORS.bits() | Self::UPDATE_CLIENTS.bits();
    }
}
```

### 11.2 — `BlockGetter` and `LevelWriter` traits

```rust
// crates/oxidized-game/src/level/traits.rs

use oxidized_world::block::{BlockPos, BlockState};
use crate::level::flags::BlockFlags;

/// Read-only block access.
pub trait BlockGetter {
    fn get_block_state(&self, pos: BlockPos) -> &BlockState;

    fn is_air(&self, pos: BlockPos) -> bool {
        self.get_block_state(pos).is_air()
    }
}

/// Read-write block access.
pub trait LevelWriter: BlockGetter {
    /// Set the block state at `pos`, returning the old state.
    /// `flags` controls update propagation and client notification.
    fn set_block_state(
        &mut self,
        pos: BlockPos,
        state: BlockState,
        flags: BlockFlags,
    ) -> BlockState;

    /// Convenience: set with default flags (UPDATE_NEIGHBORS | UPDATE_CLIENTS).
    fn set_block(&mut self, pos: BlockPos, state: BlockState) -> BlockState {
        self.set_block_state(pos, state, BlockFlags::DEFAULT)
    }
}
```

### 11.3 — `DimensionType`

```rust
// crates/oxidized-game/src/level/dimension.rs

use oxidized_world::resource::ResourceLocation;

/// Static properties of a dimension type, loaded from `dimension_type` registry.
#[derive(Debug, Clone)]
pub struct DimensionType {
    pub id: ResourceLocation,
    /// Lowest world Y (inclusive). Overworld: -64. Nether/End: 0.
    pub min_y: i32,
    /// Height of the world in blocks. Overworld: 384. Nether/End: 256.
    pub height: i32,
    /// Highest Y for the purposes of logical height. Overworld: 320.
    pub logical_height: i32,
    pub sea_level: i32,
    pub has_skylight: bool,
    pub has_ceiling: bool,
    /// No rain, water evaporates. True for the Nether.
    pub ultrawarm: bool,
    /// False for the End (no natural mob spawning rules).
    pub natural: bool,
    /// Ambient light level (0.0 in Overworld, 0.1 in Nether).
    pub ambient_light: f32,
    pub infiniburn: ResourceLocation,
    pub effects: ResourceLocation,
}

impl DimensionType {
    pub fn overworld() -> Self {
        Self {
            id: ResourceLocation::minecraft("overworld"),
            min_y: -64,
            height: 384,
            logical_height: 320,
            sea_level: 63,
            has_skylight: true,
            has_ceiling: false,
            ultrawarm: false,
            natural: true,
            ambient_light: 0.0,
            infiniburn: ResourceLocation::minecraft("infiniburn_overworld"),
            effects: ResourceLocation::minecraft("overworld"),
        }
    }

    pub fn nether() -> Self {
        Self {
            id: ResourceLocation::minecraft("the_nether"),
            min_y: 0,
            height: 256,
            logical_height: 128,
            sea_level: 32,
            has_skylight: false,
            has_ceiling: true,
            ultrawarm: true,
            natural: false,
            ambient_light: 0.1,
            infiniburn: ResourceLocation::minecraft("infiniburn_nether"),
            effects: ResourceLocation::minecraft("the_nether"),
        }
    }

    /// Number of chunk sections in this dimension.
    pub fn section_count(&self) -> usize {
        (self.height >> 4) as usize
    }

    pub fn min_section(&self) -> i32 {
        self.min_y >> 4
    }
}
```

### 11.4 — Chunk LRU cache

```rust
// crates/oxidized-game/src/level/chunk_cache.rs

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use oxidized_world::chunk::LevelChunk;
use oxidized_world::chunk::ChunkPos;

pub struct ChunkCache {
    chunks: HashMap<ChunkPos, Arc<RwLock<LevelChunk>>>,
    /// Access order for LRU eviction (most-recent at back).
    lru: std::collections::VecDeque<ChunkPos>,
    max_size: usize,
}

impl ChunkCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            chunks: HashMap::new(),
            lru: std::collections::VecDeque::new(),
            max_size,
        }
    }

    pub fn get(&mut self, pos: ChunkPos) -> Option<Arc<RwLock<LevelChunk>>> {
        if let Some(chunk) = self.chunks.get(&pos) {
            // Move to back (most recently used).
            self.lru.retain(|p| p != &pos);
            self.lru.push_back(pos);
            Some(Arc::clone(chunk))
        } else {
            None
        }
    }

    pub fn insert(&mut self, pos: ChunkPos, chunk: LevelChunk) -> Arc<RwLock<LevelChunk>> {
        if self.chunks.len() >= self.max_size {
            self.evict_oldest();
        }
        let arc = Arc::new(RwLock::new(chunk));
        self.chunks.insert(pos, Arc::clone(&arc));
        self.lru.push_back(pos);
        arc
    }

    pub fn remove(&mut self, pos: &ChunkPos) -> Option<Arc<RwLock<LevelChunk>>> {
        self.lru.retain(|p| p != pos);
        self.chunks.remove(pos)
    }

    fn evict_oldest(&mut self) {
        if let Some(oldest) = self.lru.pop_front() {
            self.chunks.remove(&oldest);
        }
    }

    pub fn len(&self) -> usize { self.chunks.len() }
}
```

### 11.5 — `ServerLevel`

```rust
// crates/oxidized-game/src/level/server_level.rs

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use oxidized_world::chunk::{ChunkPos, LevelChunk};
use oxidized_world::block::{BlockPos, BlockState};
use oxidized_world::storage::PrimaryLevelData;

pub struct ServerLevel {
    pub dimension_type: DimensionType,
    pub level_data: Arc<RwLock<PrimaryLevelData>>,
    chunk_cache: Arc<std::sync::Mutex<ChunkCache>>,
    chunk_loader: Arc<AsyncChunkLoader>,
    /// Dirty chunk positions awaiting save.
    dirty_chunks: Arc<std::sync::Mutex<std::collections::HashSet<ChunkPos>>>,
}

impl ServerLevel {
    pub fn new(
        dimension_type: DimensionType,
        level_data: Arc<RwLock<PrimaryLevelData>>,
        chunk_loader: AsyncChunkLoader,
        cache_size: usize,
    ) -> Self {
        Self {
            dimension_type,
            level_data,
            chunk_cache: Arc::new(std::sync::Mutex::new(ChunkCache::new(cache_size))),
            chunk_loader: Arc::new(chunk_loader),
            dirty_chunks: Default::default(),
        }
    }

    /// Load a chunk from cache or disk (async).
    pub async fn get_or_load_chunk(
        &self, pos: ChunkPos,
    ) -> anyhow::Result<Option<Arc<std::sync::RwLock<LevelChunk>>>> {
        // Check cache first.
        if let Some(c) = self.chunk_cache.lock().unwrap().get(pos) {
            return Ok(Some(c));
        }
        // Load from disk.
        match self.chunk_loader.load_chunk(pos.x, pos.z).await? {
            None => Ok(None),
            Some(chunk) => {
                let arc = self.chunk_cache.lock().unwrap().insert(pos, chunk);
                Ok(Some(arc))
            }
        }
    }

    /// Synchronous block read — panics if chunk is not loaded.
    pub fn get_block_state_loaded(&self, pos: BlockPos) -> BlockState {
        let cpos = ChunkPos::from_block_pos(pos);
        let cache = self.chunk_cache.lock().unwrap();
        let arc = cache.chunks.get(&cpos)
            .expect("chunk not loaded");
        arc.read().unwrap().get_block_state(pos).clone()
    }

    /// Set a block, marking the chunk dirty.
    pub fn set_block_state_loaded(
        &self, pos: BlockPos, state: BlockState, flags: BlockFlags,
    ) -> BlockState {
        let cpos = ChunkPos::from_block_pos(pos);
        let cache = self.chunk_cache.lock().unwrap();
        let arc = cache.chunks.get(&cpos)
            .expect("chunk not loaded");
        let old = arc.write().unwrap().set_block_state(pos, state);
        if flags.contains(BlockFlags::UPDATE_CLIENTS) {
            // TODO: queue block change packet to nearby players (Phase 13).
        }
        self.dirty_chunks.lock().unwrap().insert(cpos);
        old
    }

    /// Take the current set of dirty chunks and clear it.
    pub fn drain_dirty_chunks(
        &self,
    ) -> std::collections::HashSet<ChunkPos> {
        std::mem::take(&mut *self.dirty_chunks.lock().unwrap())
    }
}

impl BlockGetter for ServerLevel {
    fn get_block_state(&self, pos: BlockPos) -> &BlockState {
        // For trait impl; use get_block_state_loaded in practice.
        todo!("use get_block_state_loaded or async get_or_load_chunk")
    }
}
```

### 11.6 — Multi-dimension manager

```rust
// crates/oxidized-game/src/level/dimension_manager.rs

use std::collections::HashMap;
use std::sync::Arc;
use oxidized_world::resource::ResourceLocation;

pub struct DimensionManager {
    levels: HashMap<ResourceLocation, Arc<tokio::sync::RwLock<ServerLevel>>>,
}

impl DimensionManager {
    pub fn new() -> Self { Self { levels: HashMap::new() } }

    pub fn register(&mut self, id: ResourceLocation, level: ServerLevel) {
        self.levels.insert(id, Arc::new(tokio::sync::RwLock::new(level)));
    }

    pub fn get(
        &self, id: &ResourceLocation,
    ) -> Option<Arc<tokio::sync::RwLock<ServerLevel>>> {
        self.levels.get(id).map(Arc::clone)
    }

    pub fn overworld(
        &self,
    ) -> Arc<tokio::sync::RwLock<ServerLevel>> {
        self.get(&ResourceLocation::minecraft("overworld"))
            .expect("overworld not registered")
    }

    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (&ResourceLocation, &Arc<tokio::sync::RwLock<ServerLevel>>)> {
        self.levels.iter()
    }
}
```

---

## Data Structures Summary

```
oxidized-game::level
  ├── BlockFlags          — bitflag set for set_block_state
  ├── BlockGetter         — trait: read-only block access
  ├── LevelWriter         — trait: read-write block access
  ├── DimensionType       — static dimension properties
  ├── ChunkCache          — LRU HashMap<ChunkPos, Arc<RwLock<LevelChunk>>>
  ├── ServerLevel         — runtime world, owns ChunkCache + AsyncChunkLoader
  └── DimensionManager    — HashMap<ResourceLocation, Arc<RwLock<ServerLevel>>>
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use oxidized_world::block::{BlockPos, BlockState, STONE, AIR};

    fn make_loaded_level() -> ServerLevel {
        // Build a ServerLevel with a single pre-loaded chunk containing stone
        // at y=64 and air everywhere else.
        todo!("build minimal ServerLevel for testing")
    }

    /// get_block_state returns the correct state for a loaded chunk.
    #[test]
    fn get_block_state_in_loaded_chunk() {
        let level = make_loaded_level();
        let pos = BlockPos::new(0, 64, 0);
        assert_eq!(level.get_block_state_loaded(pos), STONE);
        let air_pos = BlockPos::new(0, 65, 0);
        assert_eq!(level.get_block_state_loaded(air_pos), AIR);
    }

    /// set_block_state changes the block and marks the chunk dirty.
    #[test]
    fn set_block_state_marks_chunk_dirty() {
        let level = make_loaded_level();
        let pos = BlockPos::new(0, 64, 0);
        let old = level.set_block_state_loaded(pos, AIR, BlockFlags::DEFAULT);
        assert_eq!(old, STONE);
        assert_eq!(level.get_block_state_loaded(pos), AIR);

        let dirty = level.drain_dirty_chunks();
        assert!(dirty.contains(&ChunkPos::from_block_pos(pos)));

        // After draining, dirty set is empty.
        let dirty2 = level.drain_dirty_chunks();
        assert!(dirty2.is_empty());
    }

    /// ChunkCache evicts oldest entry when max_size is exceeded.
    #[test]
    fn chunk_cache_lru_eviction() {
        let mut cache = ChunkCache::new(2);
        let chunk_a = LevelChunk::new_empty(0, 0, BiomeId::PLAINS);
        let chunk_b = LevelChunk::new_empty(1, 0, BiomeId::PLAINS);
        let chunk_c = LevelChunk::new_empty(2, 0, BiomeId::PLAINS);

        cache.insert(ChunkPos::new(0, 0), chunk_a);
        cache.insert(ChunkPos::new(1, 0), chunk_b);
        // Access (0,0) to make it most-recently-used.
        cache.get(ChunkPos::new(0, 0));
        // Insert (2,0) — should evict (1,0) as least-recently-used.
        cache.insert(ChunkPos::new(2, 0), chunk_c);

        assert_eq!(cache.len(), 2);
        assert!(cache.chunks.contains_key(&ChunkPos::new(0, 0)));
        assert!(!cache.chunks.contains_key(&ChunkPos::new(1, 0)));
        assert!(cache.chunks.contains_key(&ChunkPos::new(2, 0)));
    }

    /// DimensionType section_count and min_section are correct for overworld.
    #[test]
    fn dimension_type_overworld_sections() {
        let dt = DimensionType::overworld();
        // height=384, 384/16=24 sections
        assert_eq!(dt.section_count(), 24);
        // min_y=-64, -64/16=-4
        assert_eq!(dt.min_section(), -4);
    }
}
```
