# Phase 10 — Anvil World Loading

**Crate:** `oxidized-world`  
**Reward:** Load an existing Minecraft world from disk. Given a world folder,
read `.mca` region files, decompress chunk NBT, deserialize each
`LevelChunkSection`, and populate the `LevelChunk` structs built in Phase 9.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-010: NBT](../adr/adr-010-nbt.md) — tree-based NBT with IndexMap and arena allocation
- [ADR-015: Disk I/O](../adr/adr-015-disk-io.md) — spawn_blocking I/O with write coalescing


## Goal

Implement the Anvil storage format reader: region file header parsing,
per-chunk decompression, and NBT → `LevelChunk` deserialization. Also parse
`level.dat` into `PrimaryLevelData`. All I/O must be offloaded from the async
executor via `tokio::task::spawn_blocking`.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Region file | `RegionFile` | `net.minecraft.world.level.chunk.storage.RegionFile` |
| Region file storage | `RegionFileStorage` | `net.minecraft.world.level.chunk.storage.RegionFileStorage` |
| Compression codecs | `RegionFileVersion` | `net.minecraft.world.level.chunk.storage.RegionFileVersion` |
| Chunk serializer | `ChunkSerializer` | `net.minecraft.world.level.chunk.storage.ChunkSerializer` |
| NBT I/O | `NbtIo` | `net.minecraft.nbt.NbtIo` |
| Level storage | `LevelStorageSource` | `net.minecraft.world.level.storage.LevelStorageSource` |
| Level data | `PrimaryLevelData` | `net.minecraft.world.level.storage.PrimaryLevelData` |
| World data | `ServerLevelData` | `net.minecraft.world.level.ServerLevelData` |

---

## Tasks

### 10.1 — Region file format constants

```rust
// crates/oxidized-world/src/anvil/region.rs

/// Size of one sector in bytes.
pub const SECTOR_BYTES: usize = 4096;
/// Number of chunks per region axis (32 × 32 = 1024 chunks per region file).
pub const REGION_SIZE: usize = 32;
/// Number of entries in the header (1024).
pub const SECTOR_INTS: usize = REGION_SIZE * REGION_SIZE;
/// Total header size: 4096 offset table + 4096 timestamp table.
pub const HEADER_BYTES: usize = SECTOR_BYTES * 2;

/// Offset table entry: 3-byte sector number (big-endian) + 1-byte sector count.
/// A value of 0 means the chunk is not present.
#[derive(Debug, Clone, Copy)]
pub struct OffsetEntry {
    /// First sector index (0 = header sector 0, 1 = header sector 1).
    /// Valid chunks start at sector ≥ 2.
    pub sector_number: u32,
    /// Number of consecutive sectors used.
    pub sector_count: u8,
}

impl OffsetEntry {
    pub fn is_present(self) -> bool {
        self.sector_number != 0 || self.sector_count != 0
    }

    pub fn from_u32(raw: u32) -> Self {
        Self {
            sector_number: raw >> 8,
            sector_count: (raw & 0xFF) as u8,
        }
    }

    pub fn to_u32(self) -> u32 {
        (self.sector_number << 8) | self.sector_count as u32
    }
}
```

### 10.2 — Compression codec

```rust
// crates/oxidized-world/src/anvil/compression.rs

/// Byte written before the compressed chunk data to identify the codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CompressionType {
    GZip = 1,
    Zlib = 2,
    None = 3,
    /// LZ4, added in 24w04a (rarely used in practice).
    Lz4 = 4,
    /// 128 | codec: data is stored in an external `.mcc` file.
    ExternalFlag = 128,
}

impl CompressionType {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b & 0x7F {
            1 => Some(Self::GZip),
            2 => Some(Self::Zlib),
            3 => Some(Self::None),
            4 => Some(Self::Lz4),
            _ => None,
        }
    }

    pub fn is_external(b: u8) -> bool { b & 0x80 != 0 }
}

/// Decompress `data` according to `codec`. Returns raw NBT bytes.
pub fn decompress(data: &[u8], codec: CompressionType) -> anyhow::Result<Vec<u8>> {
    use std::io::Read;
    match codec {
        CompressionType::GZip => {
            let mut decoder = flate2::read::GzDecoder::new(data);
            let mut out = Vec::new();
            decoder.read_to_end(&mut out)?;
            Ok(out)
        }
        CompressionType::Zlib => {
            let mut decoder = flate2::read::ZlibDecoder::new(data);
            let mut out = Vec::new();
            decoder.read_to_end(&mut out)?;
            Ok(out)
        }
        CompressionType::None => Ok(data.to_vec()),
        CompressionType::Lz4 => {
            Ok(lz4_flex::decompress_size_prepended(data)?)
        }
    }
}
```

### 10.3 — `RegionFile` struct

```rust
// crates/oxidized-world/src/anvil/region.rs (continued)

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

pub struct RegionFile {
    file: File,
    offsets: [OffsetEntry; SECTOR_INTS],
    timestamps: [u32; SECTOR_INTS],
    /// Sorted list of free sectors for writing.
    used_sectors: Vec<bool>,
    path: std::path::PathBuf,
}

impl RegionFile {
    /// Open (or create) a region file. Reads and validates the 8 KiB header.
    pub fn open(path: &Path) -> anyhow::Result<Self> { /* ... */ }

    /// Returns the local chunk index (0–1023) for chunk pos within region.
    pub fn chunk_index(chunk_x: i32, chunk_z: i32) -> usize {
        let lx = ((chunk_x % 32) + 32) as usize % 32;
        let lz = ((chunk_z % 32) + 32) as usize % 32;
        lz * 32 + lx
    }

    /// Read raw chunk bytes (already decompressed) or `None` if not present.
    pub fn read_chunk_data(
        &mut self, chunk_x: i32, chunk_z: i32,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        let idx = Self::chunk_index(chunk_x, chunk_z);
        let entry = self.offsets[idx];
        if !entry.is_present() {
            return Ok(None);
        }
        let byte_offset = (entry.sector_number as u64) * SECTOR_BYTES as u64;
        self.file.seek(SeekFrom::Start(byte_offset))?;

        // 4 bytes BE: byte length of the payload
        let mut len_buf = [0u8; 4];
        self.file.read_exact(&mut len_buf)?;
        let payload_len = u32::from_be_bytes(len_buf) as usize;

        // 1 byte: compression type
        let mut codec_byte = [0u8; 1];
        self.file.read_exact(&mut codec_byte)?;

        let mut compressed = vec![0u8; payload_len - 1];
        self.file.read_exact(&mut compressed)?;

        let codec = CompressionType::from_byte(codec_byte[0])
            .ok_or_else(|| anyhow::anyhow!("unknown compression type {}", codec_byte[0]))?;
        decompress(&compressed, codec).map(Some)
    }

    /// Write chunk NBT bytes (will compress with Zlib and update header).
    pub fn write_chunk_data(
        &mut self, chunk_x: i32, chunk_z: i32, nbt_bytes: &[u8],
    ) -> anyhow::Result<()> { /* compress, find free sectors, write, update header */ }

    fn read_header(&mut self) -> anyhow::Result<()> { /* parse offsets and timestamps */ }
    fn flush_header(&mut self) -> anyhow::Result<()> { /* write offsets and timestamps */ }
}
```

### 10.4 — `AnvilChunkLoader` — NBT → `LevelChunk`

The on-disk NBT layout for a chunk's sections (Java 21w43a format and later):

```
Chunk root:
  DataVersion: Int
  Status: String  ("minecraft:full" for fully generated)
  xPos: Int
  zPos: Int
  yPos: Int (minimum section Y)
  sections: List<Compound>
    each section:
      Y: Byte (section Y index, e.g. -4 .. 19)
      block_states: Compound
        palette: List<Compound>
          each entry:
            Name: String  (e.g. "minecraft:stone")
            Properties: Compound (optional)
        data: LongArray (optional — absent if palette has one entry)
      biomes: Compound
        palette: List<String>
        data: LongArray (optional)
      SkyLight: ByteArray (2048 bytes, optional)
      BlockLight: ByteArray (2048 bytes, optional)
  Heightmaps: Compound
    WORLD_SURFACE: LongArray
    MOTION_BLOCKING: LongArray
    MOTION_BLOCKING_NO_LEAVES: LongArray (server-only)
    OCEAN_FLOOR: LongArray (server-only)
  block_entities: List<Compound>
```

```rust
// crates/oxidized-world/src/anvil/chunk_loader.rs

use oxidized_nbt::NbtCompound;
use crate::chunk::{LevelChunk, LevelChunkSection, SECTION_COUNT, MIN_SECTION_Y};
use crate::block::BlockState;
use crate::registry::BlockRegistry;

pub struct AnvilChunkLoader {
    region_dir: std::path::PathBuf,
    block_registry: std::sync::Arc<BlockRegistry>,
    /// Open region files, keyed by (region_x, region_z).
    open_regions: std::collections::HashMap<(i32, i32), RegionFile>,
}

impl AnvilChunkLoader {
    pub fn new(
        dimension_dir: &std::path::Path,
        block_registry: std::sync::Arc<BlockRegistry>,
    ) -> Self { /* ... */ }

    /// Region file path for a given chunk coordinate.
    fn region_path(&self, chunk_x: i32, chunk_z: i32) -> std::path::PathBuf {
        let rx = chunk_x >> 5;
        let rz = chunk_z >> 5;
        self.region_dir.join(format!("r.{rx}.{rz}.mca"))
    }

    /// Load a chunk synchronously (call from spawn_blocking).
    pub fn load_chunk_sync(
        &mut self, chunk_x: i32, chunk_z: i32,
    ) -> anyhow::Result<Option<LevelChunk>> {
        let rx = chunk_x >> 5;
        let rz = chunk_z >> 5;
        let region = self.open_regions
            .entry((rx, rz))
            .or_insert_with(|| RegionFile::open(&self.region_path(chunk_x, chunk_z)).unwrap());

        let nbt_bytes = match region.read_chunk_data(chunk_x, chunk_z)? {
            None => return Ok(None),
            Some(b) => b,
        };

        let root: NbtCompound = oxidized_nbt::from_bytes(&nbt_bytes)?;
        Ok(Some(self.deserialize_chunk(root)?))
    }

    fn deserialize_chunk(&self, root: NbtCompound) -> anyhow::Result<LevelChunk> {
        let chunk_x: i32 = root.get_int("xPos")?;
        let chunk_z: i32 = root.get_int("zPos")?;
        let mut chunk = LevelChunk::new_empty(chunk_x, chunk_z, BiomeId::PLAINS);

        if let Some(sections) = root.get_list("sections") {
            for section_nbt in sections {
                let y: i8 = section_nbt.get_byte("Y")?;
                let si = (y as i32 - MIN_SECTION_Y) as usize;
                if si >= SECTION_COUNT { continue; }

                // block_states
                if let Some(bs) = section_nbt.get_compound("block_states") {
                    chunk.sections[si].block_states =
                        self.deserialize_block_states(bs)?;
                }

                // biomes
                if let Some(bio) = section_nbt.get_compound("biomes") {
                    chunk.sections[si].biomes =
                        self.deserialize_biomes(bio)?;
                }

                // light
                if let Some(sky) = section_nbt.get_byte_array("SkyLight") {
                    if sky.len() == 2048 {
                        chunk.light.sky_light[si + 1] =
                            Some(Box::new(sky.try_into().unwrap()));
                    }
                }
                if let Some(block) = section_nbt.get_byte_array("BlockLight") {
                    if block.len() == 2048 {
                        chunk.light.block_light[si + 1] =
                            Some(Box::new(block.try_into().unwrap()));
                    }
                }
            }
        }

        // Heightmaps
        if let Some(hms) = root.get_compound("Heightmaps") {
            self.load_heightmaps(&mut chunk, hms);
        }

        Ok(chunk)
    }

    fn deserialize_block_states(
        &self, nbt: &NbtCompound,
    ) -> anyhow::Result<PalettedContainer<BlockState>> { /* ... */ }

    fn deserialize_biomes(
        &self, nbt: &NbtCompound,
    ) -> anyhow::Result<PalettedContainer<BiomeId>> { /* ... */ }
}
```

### 10.5 — Async loading wrapper

```rust
// crates/oxidized-world/src/anvil/chunk_loader.rs (continued)

use std::sync::{Arc, Mutex};
use tokio::task;

/// Thread-safe, async-friendly wrapper around `AnvilChunkLoader`.
pub struct AsyncChunkLoader {
    inner: Arc<Mutex<AnvilChunkLoader>>,
}

impl AsyncChunkLoader {
    pub fn new(loader: AnvilChunkLoader) -> Self {
        Self { inner: Arc::new(Mutex::new(loader)) }
    }

    pub async fn load_chunk(
        &self, chunk_x: i32, chunk_z: i32,
    ) -> anyhow::Result<Option<LevelChunk>> {
        let inner = Arc::clone(&self.inner);
        task::spawn_blocking(move || {
            inner.lock().unwrap().load_chunk_sync(chunk_x, chunk_z)
        })
        .await?
    }
}
```

### 10.6 — `LevelStorageSource` and `PrimaryLevelData`

```rust
// crates/oxidized-world/src/storage/level_storage.rs

use std::path::{Path, PathBuf};

/// Locates the world folder and provides access to per-dimension subdirectories.
pub struct LevelStorageSource {
    /// Root world folder (e.g. `./world`).
    world_dir: PathBuf,
}

impl LevelStorageSource {
    pub fn new(world_dir: impl Into<PathBuf>) -> Self {
        Self { world_dir: world_dir.into() }
    }

    /// Path to `level.dat`.
    pub fn level_dat_path(&self) -> PathBuf {
        self.world_dir.join("level.dat")
    }

    /// Region directory for a dimension.
    /// Overworld: `<world>/region`, Nether: `<world>/DIM-1/region`,
    /// End: `<world>/DIM1/region`.
    pub fn region_dir(&self, dimension: Dimension) -> PathBuf {
        match dimension {
            Dimension::Overworld => self.world_dir.join("region"),
            Dimension::Nether => self.world_dir.join("DIM-1").join("region"),
            Dimension::End => self.world_dir.join("DIM1").join("region"),
        }
    }

    /// Player data directory.
    pub fn player_data_dir(&self) -> PathBuf {
        self.world_dir.join("playerdata")
    }
}

// crates/oxidized-world/src/storage/primary_level_data.rs

use uuid::Uuid;

/// Mirrors the fields of `net.minecraft.world.level.storage.PrimaryLevelData`
/// that the server reads from `level.dat`'s `Data` compound.
#[derive(Debug, Clone)]
pub struct PrimaryLevelData {
    pub level_name: String,
    pub data_version: i32,
    pub game_type: GameMode,
    pub spawn_x: i32,
    pub spawn_y: i32,
    pub spawn_z: i32,
    pub spawn_angle: f32,
    /// Total world age in game ticks.
    pub time: i64,
    /// Time of day within a 24000-tick day cycle.
    pub day_time: i64,
    pub is_raining: bool,
    pub is_thundering: bool,
    pub rain_time: i32,
    pub thunder_time: i32,
    pub hardcore: bool,
    pub difficulty: Difficulty,
    pub sea_level: i32,
}

impl PrimaryLevelData {
    /// Parse from the `Data` compound inside `level.dat`.
    pub fn from_nbt(data: &NbtCompound) -> anyhow::Result<Self> {
        Ok(Self {
            level_name: data.get_string("LevelName")?.to_owned(),
            data_version: data.get_int("DataVersion").unwrap_or(0),
            game_type: GameMode::from_id(data.get_int("GameType").unwrap_or(0)),
            spawn_x: data.get_int("SpawnX").unwrap_or(0),
            spawn_y: data.get_int("SpawnY").unwrap_or(64),
            spawn_z: data.get_int("SpawnZ").unwrap_or(0),
            spawn_angle: data.get_float("SpawnAngle").unwrap_or(0.0),
            time: data.get_long("Time").unwrap_or(0),
            day_time: data.get_long("DayTime").unwrap_or(0),
            is_raining: data.get_byte("raining").unwrap_or(0) != 0,
            is_thundering: data.get_byte("thundering").unwrap_or(0) != 0,
            rain_time: data.get_int("rainTime").unwrap_or(0),
            thunder_time: data.get_int("thunderTime").unwrap_or(0),
            hardcore: data.get_byte("hardcore").unwrap_or(0) != 0,
            difficulty: Difficulty::from_id(
                data.get_byte("Difficulty").unwrap_or(2) as i32
            ),
            sea_level: data.get_int("SeaLevel").unwrap_or(63),
        })
    }

    /// Load directly from a `level.dat` path (GZip-compressed NBT).
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path)?;
        let mut decoder = flate2::read::GzDecoder::new(bytes.as_slice());
        let mut raw = Vec::new();
        std::io::Read::read_to_end(&mut decoder, &mut raw)?;
        let root: NbtCompound = oxidized_nbt::from_bytes(&raw)?;
        let data = root.get_compound("Data")
            .ok_or_else(|| anyhow::anyhow!("missing Data compound in level.dat"))?;
        Self::from_nbt(data)
    }
}
```

---

## Data Structures Summary

```
oxidized-world::anvil
  ├── OffsetEntry          — 4-byte region header slot
  ├── CompressionType      — GZip/Zlib/None/Lz4
  ├── RegionFile           — r.X.Z.mca reader/writer
  ├── AnvilChunkLoader     — NBT deserializer (sync, use in spawn_blocking)
  └── AsyncChunkLoader     — tokio async wrapper

oxidized-world::storage
  ├── LevelStorageSource   — world folder resolver
  └── PrimaryLevelData     — level.dat contents
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Parse a known-good 8 KiB region header with one chunk at slot 0.
    #[test]
    fn region_header_parse_single_chunk() {
        let mut header = vec![0u8; HEADER_BYTES];
        // Slot 0 (chunk 0,0): sector_number=2, sector_count=1
        // stored as big-endian u32 = (2 << 8) | 1 = 513
        let raw: u32 = (2 << 8) | 1;
        header[0..4].copy_from_slice(&raw.to_be_bytes());
        // Timestamp for slot 0 = 1700000000
        let ts: u32 = 1_700_000_000u32;
        header[SECTOR_BYTES..SECTOR_BYTES + 4].copy_from_slice(&ts.to_be_bytes());

        let entry = OffsetEntry::from_u32(u32::from_be_bytes(
            header[0..4].try_into().unwrap()
        ));
        assert!(entry.is_present());
        assert_eq!(entry.sector_number, 2);
        assert_eq!(entry.sector_count, 1);

        let ts_parsed = u32::from_be_bytes(
            header[SECTOR_BYTES..SECTOR_BYTES + 4].try_into().unwrap()
        );
        assert_eq!(ts_parsed, 1_700_000_000);
    }

    /// chunk_index maps (0,0)→0, (31,0)→31, (0,31)→992, (31,31)→1023.
    #[test]
    fn region_chunk_index_corners() {
        assert_eq!(RegionFile::chunk_index(0, 0), 0);
        assert_eq!(RegionFile::chunk_index(31, 0), 31);
        assert_eq!(RegionFile::chunk_index(0, 31), 992);
        assert_eq!(RegionFile::chunk_index(31, 31), 1023);
    }

    /// Negative chunk coordinates map correctly into region-local space.
    #[test]
    fn region_chunk_index_negative_coords() {
        // Chunk (-1, -1) is in region (-1, -1), local (31, 31).
        assert_eq!(RegionFile::chunk_index(-1, -1), 1023);
        // Chunk (-32, -32) is in region (-1,-1), local (0,0).
        assert_eq!(RegionFile::chunk_index(-32, -32), 0);
    }

    /// Zlib decompression round-trips.
    #[test]
    fn zlib_roundtrip() {
        let original = b"hello world NBT data here";
        let mut encoder = flate2::write::ZlibEncoder::new(
            Vec::new(), flate2::Compression::default()
        );
        std::io::Write::write_all(&mut encoder, original).unwrap();
        let compressed = encoder.finish().unwrap();
        let decompressed = decompress(&compressed, CompressionType::Zlib).unwrap();
        assert_eq!(decompressed, original);
    }

    /// PrimaryLevelData fields are parsed correctly from a minimal NBT compound.
    #[test]
    fn primary_level_data_from_nbt() {
        // Build a minimal NbtCompound manually.
        let mut data = NbtCompound::new();
        data.put_string("LevelName", "TestWorld");
        data.put_int("GameType", 0);
        data.put_int("SpawnX", 100);
        data.put_int("SpawnY", 64);
        data.put_int("SpawnZ", -200);
        data.put_long("Time", 12000);
        data.put_long("DayTime", 6000);
        data.put_byte("raining", 1);
        data.put_byte("hardcore", 0);
        data.put_byte("Difficulty", 2);

        let level = PrimaryLevelData::from_nbt(&data).unwrap();
        assert_eq!(level.level_name, "TestWorld");
        assert_eq!(level.spawn_x, 100);
        assert_eq!(level.spawn_z, -200);
        assert!(level.is_raining);
        assert!(!level.hardcore);
    }
}
```
