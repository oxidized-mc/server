# Phase 23 — Flat World Generation

**Status:** ✅ Complete  
**Crate:** `oxidized-game`  
**Reward:** New worlds generate as a flat world; the player spawns on ground.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-016: Worldgen Pipeline](../adr/adr-016-worldgen-pipeline.md) — Rayon thread pool with dependency-aware scheduling


## Goal

Implement an on-demand flat world generator: when a chunk is requested and not
found on disk, generate it from the flat layer configuration, calculate heightmaps,
assign the plains biome, and mark the chunk as `FULL` status so it is served
to clients. Find the highest non-air block at the world origin to determine the
initial player spawn position.

---

## Java Reference

| Concept | Java class |
|---------|-----------|
| Flat level source | `net.minecraft.world.level.levelgen.flat.FlatLevelSource` |
| Flat chunk generator | `net.minecraft.world.level.levelgen.flat.FlatChunkGenerator` |
| Flat layer info | `net.minecraft.world.level.levelgen.flat.FlatLayerInfo` |
| Chunk status pipeline | `net.minecraft.world.level.chunk.status.ChunkStatus` |
| Chunk during generation | `net.minecraft.world.level.chunk.ProtoChunk` |
| Loaded level chunk | `net.minecraft.world.level.chunk.LevelChunk` |
| Chunk section | `net.minecraft.world.level.chunk.LevelChunkSection` |
| Heightmap | `net.minecraft.world.level.levelgen.Heightmap` |
| Level chunk section during gen | `net.minecraft.world.level.chunk.LevelChunkSection` |

---

## Tasks

### 23.1 — Flat layer configuration (`oxidized-game/src/worldgen/flat/config.rs`)

```rust
/// One layer in the flat world stack.
#[derive(Debug, Clone, PartialEq)]
pub struct FlatLayerInfo {
    pub block: BlockState,
    pub height: u32,   // number of blocks in this layer (≥1)
}

/// Complete flat world configuration.
#[derive(Debug, Clone)]
pub struct FlatWorldConfig {
    pub layers: Vec<FlatLayerInfo>,
    pub biome: String,              // resource key, e.g. "minecraft:plains"
    pub features: bool,             // place structures/decorations (default false)
    pub lakes: bool,
}

impl Default for FlatWorldConfig {
    /// Vanilla default: 1 bedrock + 2 dirt + 1 grass_block (total height = 4 layers)
    /// Bottom of world is y = MIN_BUILD_HEIGHT (-64), so:
    ///   y=-64: bedrock
    ///   y=-63: dirt
    ///   y=-62: dirt
    ///   y=-61: grass_block  ← surface (player spawns at y=-60)
    fn default() -> Self {
        Self {
            layers: vec![
                FlatLayerInfo { block: BlockState::bedrock(), height: 1 },
                FlatLayerInfo { block: BlockState::dirt(),    height: 2 },
                FlatLayerInfo { block: BlockState::grass_block(), height: 1 },
            ],
            biome: "minecraft:plains".into(),
            features: false,
            lakes: false,
        }
    }
}

impl FlatWorldConfig {
    /// Total height of all layers combined.
    pub fn total_height(&self) -> u32 {
        self.layers.iter().map(|l| l.height).sum()
    }

    /// Returns the block at a given absolute Y coordinate,
    /// where y_start is the first Y of the bottom-most layer.
    pub fn block_at_y(&self, y: i32, y_start: i32) -> Option<&BlockState> {
        let offset = y - y_start;
        if offset < 0 { return None; }
        let mut cursor = 0u32;
        for layer in &self.layers {
            if (offset as u32) < cursor + layer.height {
                return Some(&layer.block);
            }
            cursor += layer.height;
        }
        None
    }

    /// Parse from the "layers" string format used in server.properties/level.dat.
    /// Format: "block_id*height,block_id*height,..." (bottom to top)
    /// Example: "minecraft:bedrock,minecraft:dirt*2,minecraft:grass_block"
    pub fn from_layers_string(s: &str) -> anyhow::Result<Self> {
        let mut layers = Vec::new();
        for part in s.split(',') {
            let part = part.trim();
            if let Some((id, count_str)) = part.split_once('*') {
                let count: u32 = count_str.parse()?;
                layers.push(FlatLayerInfo {
                    block: BlockState::from_id(id.trim()),
                    height: count,
                });
            } else {
                layers.push(FlatLayerInfo {
                    block: BlockState::from_id(part),
                    height: 1,
                });
            }
        }
        anyhow::ensure!(!layers.is_empty(), "flat world must have at least one layer");
        Ok(Self { layers, ..Default::default() })
    }
}
```

### 23.2 — ProtoChunk (`oxidized-game/src/worldgen/proto_chunk.rs`)

```rust
use crate::world::{BlockState, ChunkPos, ChunkSection, Heightmap};

pub const MIN_BUILD_HEIGHT: i32 = -64;
pub const MAX_BUILD_HEIGHT: i32 = 320;
pub const SECTION_COUNT: usize  = 24; // (320 - (-64)) / 16 = 24

/// A chunk in the process of being generated.
/// Can be upgraded to a `LevelChunk` when status reaches FULL.
pub struct ProtoChunk {
    pub pos: ChunkPos,
    pub sections: [ChunkSection; SECTION_COUNT],
    pub status: ChunkStatus,
    pub heightmaps: ProtoHeightmaps,
    pub biomes: BiomeContainer,
}

impl ProtoChunk {
    pub fn empty(pos: ChunkPos) -> Self {
        Self {
            pos,
            sections: std::array::from_fn(|_| ChunkSection::empty()),
            status: ChunkStatus::Empty,
            heightmaps: ProtoHeightmaps::default(),
            biomes: BiomeContainer::uniform("minecraft:plains"),
        }
    }

    /// Convert section_index (0-based from bottom) to absolute section Y.
    pub fn section_index_to_y(idx: usize) -> i32 {
        MIN_BUILD_HEIGHT / 16 + idx as i32
    }

    pub fn get_block_state(&self, x: i32, y: i32, z: i32) -> &BlockState {
        let section_idx = ((y - MIN_BUILD_HEIGHT) / 16) as usize;
        let local_y = ((y - MIN_BUILD_HEIGHT) % 16) as usize;
        self.sections[section_idx].get(x as usize & 15, local_y, z as usize & 15)
    }

    pub fn set_block_state(&mut self, x: i32, y: i32, z: i32, state: BlockState) {
        let section_idx = ((y - MIN_BUILD_HEIGHT) / 16) as usize;
        let local_y = ((y - MIN_BUILD_HEIGHT) % 16) as usize;
        self.sections[section_idx].set(x as usize & 15, local_y, z as usize & 15, state);
    }

    /// Upgrade to a fully usable LevelChunk (status = FULL).
    pub fn into_level_chunk(mut self) -> LevelChunk {
        self.build_heightmaps();
        LevelChunk::from_proto(self)
    }

    fn build_heightmaps(&mut self) {
        for x in 0..16i32 {
            for z in 0..16i32 {
                for y in (MIN_BUILD_HEIGHT..MAX_BUILD_HEIGHT).rev() {
                    let block = self.get_block_state(x, y, z);
                    if !block.is_air() {
                        self.heightmaps.world_surface
                            [(x as usize) + (z as usize) * 16] = y + 1;
                        self.heightmaps.motion_blocking
                            [(x as usize) + (z as usize) * 16] = y + 1;
                        break;
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProtoHeightmaps {
    /// Absolute Y of the first air block above ground for each column.
    pub world_surface: [i32; 256],
    pub motion_blocking: [i32; 256],
    pub ocean_floor: [i32; 256],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChunkStatus {
    Empty,
    StructureStarts,
    StructureReferences,
    Biomes,
    Noise,
    Surface,
    Carvers,
    Features,
    InitializeLight,
    Light,
    Spawn,
    Full,
}

impl ChunkStatus {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Empty              => "minecraft:empty",
            Self::StructureStarts    => "minecraft:structure_starts",
            Self::StructureReferences=> "minecraft:structure_references",
            Self::Biomes             => "minecraft:biomes",
            Self::Noise              => "minecraft:noise",
            Self::Surface            => "minecraft:surface",
            Self::Carvers            => "minecraft:carvers",
            Self::Features           => "minecraft:features",
            Self::InitializeLight    => "minecraft:initialize_light",
            Self::Light              => "minecraft:light",
            Self::Spawn              => "minecraft:spawn",
            Self::Full               => "minecraft:full",
        }
    }
}
```

### 23.3 — ChunkSection (`oxidized-game/src/world/chunk_section.rs`)

```rust
/// A 16×16×16 section of blocks with palette-based storage.
pub struct ChunkSection {
    pub non_empty_block_count: u16,
    pub block_states: PalettedContainer<BlockState>,
    pub biomes: PalettedContainer<BiomeId>,
    pub sky_light: LightData,
    pub block_light: LightData,
}

impl ChunkSection {
    pub fn empty() -> Self {
        Self {
            non_empty_block_count: 0,
            block_states: PalettedContainer::single(BlockState::AIR),
            biomes: PalettedContainer::single(BiomeId::from("minecraft:plains")),
            sky_light: LightData::default(),
            block_light: LightData::default(),
        }
    }

    pub fn is_all_air(&self) -> bool {
        self.non_empty_block_count == 0
    }

    pub fn get(&self, x: usize, y: usize, z: usize) -> &BlockState {
        self.block_states.get(x + z * 16 + y * 256)
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, state: BlockState) {
        let was_air = self.get(x, y, z).is_air();
        let is_air  = state.is_air();
        self.block_states.set(x + z * 16 + y * 256, state);
        if was_air && !is_air { self.non_empty_block_count += 1; }
        if !was_air && is_air { self.non_empty_block_count -= 1; }
    }
}

/// Palette-backed container (compact for uniform data, indirect for mixed).
pub struct PalettedContainer<T: Clone + Eq> {
    storage: PaletteStorage<T>,
}

enum PaletteStorage<T: Clone + Eq> {
    Single(T),
    Indirect { palette: Vec<T>, data: PackedBitArray },
    Direct(Vec<T>),
}

impl<T: Clone + Eq> PalettedContainer<T> {
    pub fn single(value: T) -> Self {
        Self { storage: PaletteStorage::Single(value) }
    }

    pub fn get(&self, index: usize) -> &T {
        match &self.storage {
            PaletteStorage::Single(v)              => v,
            PaletteStorage::Indirect { palette, data } => &palette[data.get(index) as usize],
            PaletteStorage::Direct(v)              => &v[index],
        }
    }

    pub fn set(&mut self, index: usize, value: T) {
        // Upgrade storage tier if needed
        match &mut self.storage {
            PaletteStorage::Single(v) if *v == value => {},
            PaletteStorage::Single(old) => {
                // Expand to indirect
                let old = old.clone();
                let mut palette = vec![old];
                if !palette.contains(&value) { palette.push(value.clone()); }
                let bits = bits_for_palette_size(palette.len());
                let mut data = PackedBitArray::new(bits, 4096);
                data.set(index, palette.iter().position(|p| *p == value).unwrap() as u64);
                self.storage = PaletteStorage::Indirect { palette, data };
            },
            PaletteStorage::Indirect { palette, data } => {
                let palette_id = if let Some(i) = palette.iter().position(|p| *p == value) {
                    i
                } else {
                    palette.push(value.clone());
                    palette.len() - 1
                };
                // Resize packed array if bits per entry increased
                data.set(index, palette_id as u64);
            },
            PaletteStorage::Direct(v) => v[index] = value,
        }
    }
}
```

### 23.4 — FlatChunkGenerator (`oxidized-game/src/worldgen/flat/generator.rs`)

```rust
/// Trait that all world generators must implement.
pub trait ChunkGenerator: Send + Sync {
    /// Fill blocks into the ProtoChunk from the generator's noise/layer data.
    fn fill_from_noise(&self, chunk: &mut ProtoChunk);

    /// Apply surface rules (e.g. grass over dirt). For flat world this is a no-op.
    fn build_surface(&self, chunk: &mut ProtoChunk) {}

    /// Scatter features (ores, trees, etc.). For flat world this is a no-op.
    fn place_features(&self, _chunk: &mut LevelChunk) {}

    fn generator_type(&self) -> &'static str;
}

pub struct FlatChunkGenerator {
    pub config: FlatWorldConfig,
}

impl ChunkGenerator for FlatChunkGenerator {
    fn generator_type(&self) -> &'static str { "minecraft:flat" }

    fn fill_from_noise(&self, chunk: &mut ProtoChunk) {
        let y_start = MIN_BUILD_HEIGHT;
        let mut y = y_start;

        for layer in &self.config.layers {
            for _ in 0..layer.height {
                // Every column in the 16×16 chunk gets this layer
                for x in 0..16i32 {
                    for z in 0..16i32 {
                        chunk.set_block_state(x, y, z, layer.block.clone());
                    }
                }
                y += 1;
            }
        }
        chunk.status = ChunkStatus::Noise;
    }

    fn build_surface(&self, chunk: &mut ProtoChunk) {
        // Flat world: layers already represent the final surface
        chunk.status = ChunkStatus::Surface;
    }
}

impl FlatChunkGenerator {
    pub fn new(config: FlatWorldConfig) -> Self {
        Self { config }
    }

    /// Generate a complete LevelChunk on demand (all relevant phases in one call).
    pub fn generate_chunk(&self, pos: ChunkPos) -> LevelChunk {
        let mut proto = ProtoChunk::empty(pos);

        // Phase: BIOMES
        proto.biomes = BiomeContainer::uniform(&self.config.biome);
        proto.status = ChunkStatus::Biomes;

        // Phase: NOISE (place blocks)
        self.fill_from_noise(&mut proto);

        // Phase: SURFACE
        self.build_surface(&mut proto);

        // Phase: FULL (build heightmaps and finalize)
        proto.status = ChunkStatus::Full;
        proto.into_level_chunk()
    }

    /// Return the Y coordinate the player should spawn at (one block above surface).
    pub fn find_spawn_y(&self) -> i32 {
        MIN_BUILD_HEIGHT + self.config.total_height() as i32
    }
}
```

### 23.5 — On-demand chunk loader (`oxidized-game/src/level/chunk_loader.rs`)

```rust
impl ServerLevel {
    /// Load a chunk from disk, or generate it if absent.
    pub async fn get_or_generate_chunk(&mut self, pos: ChunkPos) -> &LevelChunk {
        if !self.loaded_chunks.contains_key(&pos) {
            let chunk = self.load_or_generate(pos).await;
            self.loaded_chunks.insert(pos, chunk);
        }
        &self.loaded_chunks[&pos]
    }

    async fn load_or_generate(&self, pos: ChunkPos) -> LevelChunk {
        // 1. Try disk
        let region_path = self.world_saver.region_path(pos.region_x(), pos.region_z());
        if region_path.exists() {
            if let Ok(mut region) = RegionFile::open_or_create(region_path).await {
                if let Ok(Some(bytes)) = region.read_chunk(pos.local_x(), pos.local_z()).await {
                    if let Ok(chunk) = ChunkSerializer::read(&bytes) {
                        // Only accept if fully generated (Status = "minecraft:full")
                        if chunk.status == ChunkStatus::Full {
                            tracing::trace!("Loaded chunk {:?} from disk", pos);
                            return chunk;
                        }
                    }
                }
            }
        }

        // 2. Generate
        tracing::trace!("Generating chunk {:?}", pos);
        self.chunk_generator.generate_chunk(pos)
    }
}
```

### 23.6 — Spawn position finder (`oxidized-game/src/level/spawn.rs`)

```rust
impl ServerLevel {
    /// Find a safe spawn position: highest non-air block at (origin_x, origin_z).
    pub async fn find_spawn_position(&mut self) -> (i32, i32, i32) {
        // For flat worlds use the generator's known spawn Y
        if let Some(flat) = self.chunk_generator.as_flat() {
            let spawn_y = flat.find_spawn_y();
            return (0, spawn_y, 0);
        }

        // General case: scan from top to bottom at (0, 0)
        let chunk = self.get_or_generate_chunk(ChunkPos { x: 0, z: 0 }).await;
        let surface_y = chunk.heightmaps.world_surface[0 + 0 * 16]; // column (0,0)
        (0, surface_y, 0)
    }
}
```

---

## Data Structures

```rust
// oxidized-game/src/worldgen/mod.rs

/// Biome palette for a chunk section (4×4×4 grid of biome entries per section).
pub struct BiomeContainer {
    pub storage: PalettedContainer<BiomeId>,
}

impl BiomeContainer {
    pub fn uniform(biome: &str) -> Self {
        Self { storage: PalettedContainer::single(BiomeId::from(biome)) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BiomeId(pub String);

impl From<&str> for BiomeId {
    fn from(s: &str) -> Self { BiomeId(s.to_string()) }
}

// Packed bit array for palette storage
pub struct PackedBitArray {
    bits_per_entry: u8,
    data: Vec<u64>,
    size: usize,
}

impl PackedBitArray {
    pub fn new(bits_per_entry: u8, size: usize) -> Self {
        let entries_per_long = 64 / bits_per_entry as usize;
        let num_longs = (size + entries_per_long - 1) / entries_per_long;
        Self { bits_per_entry, data: vec![0u64; num_longs], size }
    }

    pub fn get(&self, index: usize) -> u64 {
        let entries_per_long = 64 / self.bits_per_entry as usize;
        let long_idx   = index / entries_per_long;
        let bit_offset = (index % entries_per_long) * self.bits_per_entry as usize;
        let mask = (1u64 << self.bits_per_entry) - 1;
        (self.data[long_idx] >> bit_offset) & mask
    }

    pub fn set(&mut self, index: usize, value: u64) {
        let entries_per_long = 64 / self.bits_per_entry as usize;
        let long_idx   = index / entries_per_long;
        let bit_offset = (index % entries_per_long) * self.bits_per_entry as usize;
        let mask = (1u64 << self.bits_per_entry) - 1;
        self.data[long_idx] &= !(mask << bit_offset);
        self.data[long_idx] |=  (value & mask) << bit_offset;
    }
}

fn bits_for_palette_size(size: usize) -> u8 {
    // Minimum 4 bits for block states
    (usize::BITS - size.saturating_sub(1).leading_zeros()) as u8
}
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // --- FlatWorldConfig ---

    #[test]
    fn default_flat_config_total_height_is_4() {
        let config = FlatWorldConfig::default();
        assert_eq!(config.total_height(), 4); // 1 bedrock + 2 dirt + 1 grass
    }

    #[test]
    fn flat_config_block_at_y_bottom_is_bedrock() {
        let config = FlatWorldConfig::default();
        let block = config.block_at_y(MIN_BUILD_HEIGHT, MIN_BUILD_HEIGHT).unwrap();
        assert_eq!(block, &BlockState::bedrock());
    }

    #[test]
    fn flat_config_block_at_y_surface_is_grass() {
        let config = FlatWorldConfig::default();
        // y = MIN_BUILD_HEIGHT + 3 (0-indexed: 1 bedrock + 2 dirt = index 3)
        let y = MIN_BUILD_HEIGHT + 3;
        let block = config.block_at_y(y, MIN_BUILD_HEIGHT).unwrap();
        assert_eq!(block, &BlockState::grass_block());
    }

    #[test]
    fn flat_config_block_above_layers_returns_none() {
        let config = FlatWorldConfig::default();
        let y = MIN_BUILD_HEIGHT + 10; // above all layers
        assert!(config.block_at_y(y, MIN_BUILD_HEIGHT).is_none());
    }

    #[test]
    fn flat_config_from_layers_string_parses_correctly() {
        let config = FlatWorldConfig::from_layers_string(
            "minecraft:bedrock,minecraft:dirt*2,minecraft:grass_block"
        ).unwrap();
        assert_eq!(config.total_height(), 4);
        assert_eq!(config.layers[0].block, BlockState::bedrock());
        assert_eq!(config.layers[1].height, 2);
        assert_eq!(config.layers[2].block, BlockState::grass_block());
    }

    #[test]
    fn flat_config_from_layers_string_rejects_empty() {
        assert!(FlatWorldConfig::from_layers_string("").is_err());
    }

    // --- FlatChunkGenerator ---

    #[test]
    fn generate_chunk_has_grass_at_surface_y() {
        let gen = FlatChunkGenerator::new(FlatWorldConfig::default());
        let chunk = gen.generate_chunk(ChunkPos { x: 0, z: 0 });
        let surface_y = MIN_BUILD_HEIGHT + 3;
        // Check column (8, 8) for centre of chunk
        let block = chunk.get_block_state(8, surface_y, 8);
        assert_eq!(block, &BlockState::grass_block());
    }

    #[test]
    fn generate_chunk_has_air_above_surface() {
        let gen = FlatChunkGenerator::new(FlatWorldConfig::default());
        let chunk = gen.generate_chunk(ChunkPos { x: 0, z: 0 });
        let above_surface = MIN_BUILD_HEIGHT + 4;
        let block = chunk.get_block_state(0, above_surface, 0);
        assert!(block.is_air(), "block above surface should be air, got {:?}", block);
    }

    #[test]
    fn generate_chunk_status_is_full() {
        let gen = FlatChunkGenerator::new(FlatWorldConfig::default());
        let chunk = gen.generate_chunk(ChunkPos { x: 0, z: 0 });
        assert_eq!(chunk.status, ChunkStatus::Full);
    }

    #[test]
    fn find_spawn_y_equals_one_above_surface() {
        let gen = FlatChunkGenerator::new(FlatWorldConfig::default());
        let y = gen.find_spawn_y();
        assert_eq!(y, MIN_BUILD_HEIGHT + 4, "spawn should be one above the surface layer");
    }

    #[test]
    fn all_columns_filled_uniformly() {
        let gen = FlatChunkGenerator::new(FlatWorldConfig::default());
        let chunk = gen.generate_chunk(ChunkPos { x: 5, z: -3 });
        let surface_y = MIN_BUILD_HEIGHT + 3;
        // Every column in the chunk must have grass at the surface
        for x in 0..16 {
            for z in 0..16 {
                let block = chunk.get_block_state(x, surface_y, z);
                assert_eq!(block, &BlockState::grass_block(),
                    "column ({x},{z}) should have grass at y={surface_y}");
            }
        }
    }

    // --- ChunkSection ---

    #[test]
    fn empty_chunk_section_is_all_air() {
        let section = ChunkSection::empty();
        assert!(section.is_all_air());
        assert_eq!(section.non_empty_block_count, 0);
    }

    #[test]
    fn chunk_section_set_non_air_increments_count() {
        let mut section = ChunkSection::empty();
        section.set(0, 0, 0, BlockState::stone());
        assert_eq!(section.non_empty_block_count, 1);
        assert!(!section.is_all_air());
    }

    #[test]
    fn chunk_section_set_air_decrements_count() {
        let mut section = ChunkSection::empty();
        section.set(0, 0, 0, BlockState::stone());
        section.set(0, 0, 0, BlockState::AIR);
        assert_eq!(section.non_empty_block_count, 0);
        assert!(section.is_all_air());
    }

    // --- PackedBitArray ---

    #[test]
    fn packed_bit_array_set_and_get_roundtrip() {
        let mut arr = PackedBitArray::new(4, 16);
        arr.set(3, 7);
        assert_eq!(arr.get(3), 7);
        assert_eq!(arr.get(0), 0, "unset entry should be 0");
    }

    #[test]
    fn packed_bit_array_multiple_entries_no_overlap() {
        let mut arr = PackedBitArray::new(4, 64);
        for i in 0..64 {
            arr.set(i, (i % 15) as u64);
        }
        for i in 0..64 {
            assert_eq!(arr.get(i), (i % 15) as u64, "index {i} mismatch");
        }
    }

    // --- ProtoChunk heightmap ---

    #[test]
    fn proto_chunk_heightmap_correct_after_fill() {
        let gen = FlatChunkGenerator::new(FlatWorldConfig::default());
        let chunk = gen.generate_chunk(ChunkPos { x: 0, z: 0 });
        // World surface height = one above top layer = MIN_BUILD_HEIGHT + 4
        let expected_surface = MIN_BUILD_HEIGHT + 4;
        for x in 0..16usize {
            for z in 0..16usize {
                let h = chunk.heightmaps.world_surface[x + z * 16];
                assert_eq!(h, expected_surface,
                    "column ({x},{z}) world_surface should be {expected_surface}, got {h}");
            }
        }
    }
}
```
