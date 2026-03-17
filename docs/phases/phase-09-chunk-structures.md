# Phase 9 — Chunk Data Structures

**Crate:** `oxidized-world`  
**Reward:** In-memory chunk that matches the wire format exactly. All section
get/set operations, bit-packed palette storage, and light arrays are correct and
agree with what the vanilla client expects.

---

## Goal

Implement the in-memory representation of a Minecraft chunk: the paletted
container, chunk sections, heightmaps, and light data. The structures must
serialize/deserialize to exactly the bytes that the vanilla client sends and
expects, because Phase 13 (chunk sending) writes them directly onto the wire.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Chunk | `LevelChunk` | `net.minecraft.world.level.chunk.LevelChunk` |
| Section | `LevelChunkSection` | `net.minecraft.world.level.chunk.LevelChunkSection` |
| Paletted container (R/W) | `PalettedContainer` | `net.minecraft.world.level.chunk.PalettedContainer` |
| Paletted container (R/O) | `PalettedContainerRO` | `net.minecraft.world.level.chunk.PalettedContainerRO` |
| Bit storage interface | `BitStorage` | `net.minecraft.util.BitStorage` |
| Dense bit storage | `SimpleBitStorage` | `net.minecraft.util.SimpleBitStorage` |
| Zero-bit (single value) | `ZeroBitStorage` | `net.minecraft.util.ZeroBitStorage` |
| Heightmap | `Heightmap` | `net.minecraft.world.level.levelgen.Heightmap` |
| Heightmap type | `Heightmap.Types` | `net.minecraft.world.level.levelgen.Heightmap.Types` |

---

## Tasks

### 9.1 — `PaletteType` enum and `BitsPerEntry` logic

The bits-per-entry is chosen dynamically based on the number of distinct values
in a container. Blocks and biomes each have their own thresholds.

```rust
// crates/oxidized-world/src/chunk/palette.rs

/// Thresholds match vanilla's PalettedContainer.Strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteType {
    /// 0 bits per entry — entire container is one value.
    SingleValue,
    /// 1–4 bits per entry (blocks) or 1–3 bits (biomes).
    Linear,
    /// 5–8 bits per entry (blocks only).
    HashMap,
    /// ≥ 9 bits — indexes directly into the global palette.
    Global,
}

impl PaletteType {
    /// Choose the palette type for block states given a bit count.
    pub fn for_blocks(bits: u8) -> Self {
        match bits {
            0 => Self::SingleValue,
            1..=4 => Self::Linear,
            5..=8 => Self::HashMap,
            _ => Self::Global,
        }
    }

    /// Choose the palette type for biomes given a bit count.
    pub fn for_biomes(bits: u8) -> Self {
        match bits {
            0 => Self::SingleValue,
            1..=3 => Self::Linear,
            _ => Self::Global,
        }
    }

    /// Minimum bits that this type requires (0 for SingleValue).
    pub fn min_bits(self) -> u8 {
        match self {
            Self::SingleValue => 0,
            Self::Linear => 1,
            Self::HashMap => 5,
            Self::Global => 15,  // log2(total block states ≈ 25000)
        }
    }
}
```

### 9.2 — `SimpleBitStorage` — compact packed long array

Values are packed tightly but **never span a u64 boundary**. Each u64 holds
`floor(64 / bits_per_entry)` values. The `SimpleBitStorage` in Java uses this
same layout (see `net.minecraft.util.SimpleBitStorage`).

```rust
// crates/oxidized-world/src/chunk/bit_storage.rs

/// Compact array of fixed-width integer values backed by Vec<u64>.
///
/// Layout per u64: values are packed from LSB to MSB.
/// A value NEVER spans two u64s — padding bits at the top of each u64 are
/// left zero. This matches vanilla's `SimpleBitStorage`.
pub struct SimpleBitStorage {
    data: Vec<u64>,
    bits: u8,
    size: usize,
    values_per_long: usize,
    mask: u64,
}

impl SimpleBitStorage {
    pub fn new(bits: u8, size: usize) -> Self {
        assert!(bits > 0 && bits <= 32);
        let values_per_long = 64 / bits as usize;
        let longs_needed = (size + values_per_long - 1) / values_per_long;
        Self {
            data: vec![0u64; longs_needed],
            bits,
            size,
            values_per_long,
            mask: (1u64 << bits) - 1,
        }
    }

    pub fn get(&self, index: usize) -> u32 {
        debug_assert!(index < self.size);
        let long_index = index / self.values_per_long;
        let bit_index = (index % self.values_per_long) * self.bits as usize;
        ((self.data[long_index] >> bit_index) & self.mask) as u32
    }

    pub fn set(&mut self, index: usize, value: u32) {
        debug_assert!(index < self.size);
        debug_assert!((value as u64) <= self.mask);
        let long_index = index / self.values_per_long;
        let bit_index = (index % self.values_per_long) * self.bits as usize;
        self.data[long_index] &= !(self.mask << bit_index);
        self.data[long_index] |= (value as u64) << bit_index;
    }

    pub fn get_and_set(&mut self, index: usize, value: u32) -> u32 {
        let old = self.get(index);
        self.set(index, value);
        old
    }

    /// Raw longs slice — used when serializing to wire format.
    pub fn raw(&self) -> &[u64] {
        &self.data
    }

    pub fn bits(&self) -> u8 { self.bits }
    pub fn size(&self) -> usize { self.size }
}
```

### 9.3 — `PalettedContainer<T>`

Holds 4096 block-state (or 64 biome) values in one of four palette modes.
Upgrades automatically when the palette overflows.

```rust
// crates/oxidized-world/src/chunk/paletted_container.rs

use std::collections::HashMap;

pub enum Palette<T: Clone + Eq> {
    Single(T),
    Linear(Vec<T>),       // index → value
    Map(HashMap<T, u32>), // value → id (plus Linear vec for reverse)
}

pub struct PalettedContainer<T: Clone + Eq + std::hash::Hash> {
    palette: Palette<T>,
    /// None when palette is Single (implicit single long of zeros).
    storage: Option<SimpleBitStorage>,
    /// Total entry count (4096 for blocks, 64 for biomes).
    size: usize,
    default: T,
    /// Configuration: max bits before upgrading.
    kind: ContainerKind,
}

#[derive(Clone, Copy)]
pub enum ContainerKind { Blocks, Biomes }

impl<T: Clone + Eq + std::hash::Hash> PalettedContainer<T> {
    pub fn new_single(value: T, size: usize, kind: ContainerKind) -> Self { /* ... */ }

    pub fn get(&self, x: usize, y: usize, z: usize) -> &T {
        let index = Self::index(x, y, z, self.size);
        match &self.palette {
            Palette::Single(v) => v,
            Palette::Linear(vec) => {
                let id = self.storage.as_ref().unwrap().get(index) as usize;
                &vec[id]
            }
            Palette::Map(map) => {
                // reverse lookup stored separately
                todo!()
            }
        }
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, value: T) { /* ... */ }

    /// XZY packing: index = (y * 16 + z) * 16 + x  (blocks)
    fn index(x: usize, y: usize, z: usize, _size: usize) -> usize {
        (y << 8) | (z << 4) | x
    }

    /// Serialize to the network wire format into `buf`.
    pub fn write_to_buf(&self, buf: &mut Vec<u8>) { /* ... */ }

    /// Deserialize from wire format.
    pub fn read_from_buf(buf: &mut &[u8], global_palette_size: u32) -> anyhow::Result<Self>
    where
        T: serde::de::DeserializeOwned,
    { /* ... */ }
}
```

**Wire format** (matching `PalettedContainer.write` in Java):

```
[bits_per_entry: u8]
if bits == 0:
    [single_value: VarInt]
    [data_array_length: VarInt = 0]
elif bits <= linear_max:
    [palette_length: VarInt]
    [palette_entry: VarInt] × palette_length
    [data_array_length: VarInt]
    [long: u64] × data_array_length
else:  // global palette
    // no palette section
    [data_array_length: VarInt]
    [long: u64] × data_array_length
```

### 9.4 — `LevelChunkSection`

```rust
// crates/oxidized-world/src/chunk/section.rs

use crate::block::BlockState;
use crate::biome::BiomeId;

pub struct LevelChunkSection {
    /// Number of non-air blocks (maintained incrementally).
    pub non_empty_block_count: i16,
    /// Fluid count kept separately from block count.
    pub fluid_count: i16,
    pub block_states: PalettedContainer<BlockState>,
    pub biomes: PalettedContainer<BiomeId>,
}

impl LevelChunkSection {
    /// Create a section filled entirely with air and the given biome.
    pub fn new_empty(default_biome: BiomeId) -> Self { /* ... */ }

    pub fn get_block_state(&self, x: usize, y: usize, z: usize) -> &BlockState {
        self.block_states.get(x, y, z)
    }

    /// Returns the previous block state at (x,y,z).
    pub fn set_block_state(
        &mut self, x: usize, y: usize, z: usize, state: BlockState
    ) -> BlockState { /* updates non_empty_block_count */ }

    pub fn has_only_air(&self) -> bool { self.non_empty_block_count == 0 }

    /// Serialised size in bytes (for pre-allocating the chunk buffer).
    pub fn serialized_size(&self) -> usize { /* ... */ }

    /// Write to network buffer — matches `LevelChunkSection.write`.
    pub fn write_to_buf(&self, buf: &mut Vec<u8>) {
        // i16 LE: non_empty_block_count
        // i16 LE: fluid_count
        // block_states.write_to_buf(buf)
        // biomes.write_to_buf(buf)
    }

    pub fn read_from_buf(buf: &mut &[u8]) -> anyhow::Result<Self> { /* ... */ }
}
```

### 9.5 — `Heightmap`

The server keeps four heightmaps per chunk; two are sent to the client.

```rust
// crates/oxidized-world/src/chunk/heightmap.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeightmapType {
    /// Highest non-air block (sent to client).
    WorldSurface,
    /// Highest motion-blocking block (sent to client).
    MotionBlocking,
    /// MotionBlocking but ignoring leaves.
    MotionBlockingNoLeaves,
    /// Highest non-fluid solid block (server-only).
    OceanFloor,
}

impl HeightmapType {
    pub fn send_to_client(self) -> bool {
        matches!(self, Self::WorldSurface | Self::MotionBlocking)
    }
}

/// 256 height values (one per XZ column), stored in a packed long array.
/// Values are stored in a `SimpleBitStorage` with 9 bits per entry
/// (range 0–511 covers y = -64 to 320 + 64 padding).
pub struct Heightmap {
    data: SimpleBitStorage, // 256 entries × 9 bits = 36 u64s
    kind: HeightmapType,
}

impl Heightmap {
    pub const BITS: u8 = 9;

    pub fn new(kind: HeightmapType) -> Self {
        Self { data: SimpleBitStorage::new(Self::BITS, 256), kind }
    }

    pub fn get(&self, x: usize, z: usize) -> i32 {
        self.data.get(z * 16 + x) as i32
    }

    pub fn set(&mut self, x: usize, z: usize, y: i32) {
        self.data.set(z * 16 + x, y as u32);
    }

    /// Raw longs for NBT/network serialization.
    pub fn raw_data(&self) -> &[u64] { self.data.raw() }
}
```

### 9.6 — `LightData`

```rust
// crates/oxidized-world/src/chunk/light.rs

/// Light data for all 26 sections (-4 inclusive to 20 inclusive + 2 border).
pub struct LightData {
    /// 2048 bytes per section (4 bits per block), indexed [section][nibble_idx].
    /// None means "all dark" (empty section in light update packet).
    pub sky_light: Vec<Option<Box<[u8; 2048]>>>,
    pub block_light: Vec<Option<Box<[u8; 2048]>>>,
}

impl LightData {
    pub fn new(section_count: usize) -> Self {
        Self {
            sky_light: vec![None; section_count],
            block_light: vec![None; section_count],
        }
    }

    /// Get the sky-light level at local block coordinates within a section.
    pub fn get_sky_light(&self, section: usize, x: u8, y: u8, z: u8) -> u8 {
        match &self.sky_light[section] {
            None => 0,
            Some(arr) => {
                let idx = ((y as usize) << 8) | ((z as usize) << 4) | x as usize;
                let byte = arr[idx >> 1];
                if idx & 1 == 0 { byte & 0x0F } else { (byte >> 4) & 0x0F }
            }
        }
    }

    pub fn set_sky_light(&mut self, section: usize, x: u8, y: u8, z: u8, level: u8) {
        let arr = self.sky_light[section].get_or_insert_with(|| Box::new([0u8; 2048]));
        let idx = ((y as usize) << 8) | ((z as usize) << 4) | x as usize;
        if idx & 1 == 0 {
            arr[idx >> 1] = (arr[idx >> 1] & 0xF0) | (level & 0x0F);
        } else {
            arr[idx >> 1] = (arr[idx >> 1] & 0x0F) | ((level & 0x0F) << 4);
        }
    }
}
```

### 9.7 — `LevelChunk`

```rust
// crates/oxidized-world/src/chunk/level_chunk.rs

use std::collections::HashMap;
use crate::block::{BlockPos, BlockState};

/// Number of sections in the overworld: Y = -4 to +19 (24 sections).
pub const SECTION_COUNT: usize = 24;
/// Minimum section Y index.
pub const MIN_SECTION_Y: i32 = -4;

pub struct LevelChunk {
    pub chunk_x: i32,
    pub chunk_z: i32,
    /// sections[0] = Y = -4, sections[23] = Y = 19.
    pub sections: Box<[LevelChunkSection; SECTION_COUNT]>,
    pub heightmaps: HashMap<HeightmapType, Heightmap>,
    pub light: LightData,
    /// True when a block has been changed since the last save.
    pub dirty: bool,
}

impl LevelChunk {
    pub fn new_empty(chunk_x: i32, chunk_z: i32, default_biome: BiomeId) -> Self { /* ... */ }

    /// Convert a world-Y coordinate to a section index (0-based from bottom).
    pub fn section_index(world_y: i32) -> usize {
        ((world_y >> 4) - MIN_SECTION_Y) as usize
    }

    /// Convert a world-Y coordinate to the section-local Y (0–15).
    pub fn section_local_y(world_y: i32) -> usize {
        (world_y & 0xF) as usize
    }

    pub fn get_block_state(&self, pos: BlockPos) -> &BlockState {
        let section = &self.sections[Self::section_index(pos.y)];
        let ly = Self::section_local_y(pos.y);
        section.get_block_state(
            (pos.x & 0xF) as usize,
            ly,
            (pos.z & 0xF) as usize,
        )
    }

    pub fn set_block_state(
        &mut self, pos: BlockPos, state: BlockState
    ) -> BlockState {
        let si = Self::section_index(pos.y);
        let ly = Self::section_local_y(pos.y);
        self.dirty = true;
        self.sections[si].set_block_state(
            (pos.x & 0xF) as usize,
            ly,
            (pos.z & 0xF) as usize,
            state,
        )
    }

    /// Recalculate all four heightmaps from scratch.
    pub fn recalculate_heightmaps(&mut self) { /* ... */ }
}
```

---

## Data Structures Summary

```rust
// Key types exposed by oxidized-world::chunk

pub use bit_storage::SimpleBitStorage;
pub use palette::{PaletteType, PalettedContainer, ContainerKind};
pub use section::LevelChunkSection;
pub use heightmap::{Heightmap, HeightmapType};
pub use light::LightData;
pub use level_chunk::{LevelChunk, SECTION_COUNT, MIN_SECTION_Y};
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Values round-trip through SimpleBitStorage for every valid bit width.
    #[test]
    fn bit_storage_roundtrip() {
        for bits in 1u8..=15 {
            let mut s = SimpleBitStorage::new(bits, 4096);
            let max_val = ((1u64 << bits) - 1) as u32;
            for i in 0..4096 {
                let v = (i as u32) & max_val;
                s.set(i, v);
                assert_eq!(s.get(i), v, "bits={bits}, index={i}");
            }
        }
    }

    /// Values never bleed between adjacent entries (no cross-long contamination).
    #[test]
    fn bit_storage_no_cross_boundary_bleed() {
        let mut s = SimpleBitStorage::new(5, 4096);
        s.set(0, 31);   // max value, 5 bits
        s.set(1, 0);
        assert_eq!(s.get(0), 31);
        assert_eq!(s.get(1), 0);
        // values_per_long for 5 bits = floor(64/5) = 12
        // set index 12 (first entry of second long) and verify index 11 unchanged
        s.set(11, 15);
        s.set(12, 7);
        assert_eq!(s.get(11), 15);
        assert_eq!(s.get(12), 7);
    }

    /// Section index calculation matches Java's SectionPos logic.
    #[test]
    fn section_index_from_y() {
        assert_eq!(LevelChunk::section_index(-64), 0);   // y=-64 → section 0 (Y=-4)
        assert_eq!(LevelChunk::section_index(-48), 1);
        assert_eq!(LevelChunk::section_index(0), 4);    // y=0 → section 4
        assert_eq!(LevelChunk::section_index(319), 23); // top section
    }

    /// SingleValue palette serializes to: [bits=0][value VarInt][array_len=0].
    #[test]
    fn palette_single_value_wire_format() {
        let container: PalettedContainer<u32> =
            PalettedContainer::new_single(42u32, 4096, ContainerKind::Blocks);
        let mut buf = Vec::new();
        container.write_to_buf(&mut buf);
        // bits_per_entry = 0
        assert_eq!(buf[0], 0u8);
        // VarInt(42) = [0x2A]
        assert_eq!(buf[1], 0x2A);
        // data array length = VarInt(0)
        assert_eq!(buf[2], 0x00);
        assert_eq!(buf.len(), 3);
    }

    /// Palette serialization round-trips through write_to_buf → read_from_buf.
    #[test]
    fn paletted_container_roundtrip_linear() {
        let mut c: PalettedContainer<u32> =
            PalettedContainer::new_single(0u32, 4096, ContainerKind::Blocks);
        // Write a few distinct values to trigger a Linear palette.
        for i in 0..8usize {
            c.set(i, 0, 0, i as u32);
        }
        let mut buf = Vec::new();
        c.write_to_buf(&mut buf);
        let restored = PalettedContainer::<u32>::read_from_buf(
            &mut buf.as_slice(), 25000
        ).unwrap();
        for i in 0..8usize {
            assert_eq!(restored.get(i, 0, 0), &(i as u32));
        }
    }

    /// Heightmap XZ indexing: column (x=3, z=7) → index 7*16+3 = 115.
    #[test]
    fn heightmap_xz_indexing() {
        let mut hm = Heightmap::new(HeightmapType::WorldSurface);
        hm.set(3, 7, 64);
        assert_eq!(hm.get(3, 7), 64);
        // Adjacent columns must be unaffected.
        assert_eq!(hm.get(2, 7), 0);
        assert_eq!(hm.get(3, 6), 0);
    }

    /// Light nibble packing: lower nibble is even index, upper is odd.
    #[test]
    fn light_nibble_packing() {
        let mut ld = LightData::new(SECTION_COUNT + 2);
        ld.set_sky_light(0, 2, 3, 4, 15);
        assert_eq!(ld.get_sky_light(0, 2, 3, 4), 15);
        // Adjacent block must still be 0.
        assert_eq!(ld.get_sky_light(0, 3, 3, 4), 0);
    }
}
```
