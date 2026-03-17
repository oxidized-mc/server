# Phase 20 — World Saving

**Crate:** `oxidized-game`  
**Reward:** World persists across server restarts.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-015: Disk I/O](../adr/adr-015-disk-io.md) — spawn_blocking I/O with write coalescing
- [ADR-020: Player Session](../adr/adr-020-player-session.md) — split network actor + ECS entity architecture
- [ADR-030: Shutdown & Crash Handling](../adr/adr-030-shutdown-crash.md) — multi-layer shutdown with watchdog and crash reports


## Goal

Serialize in-memory world state to disk in Minecraft's Anvil format (`.mca` region
files), save player data as gzip-compressed NBT, and persist level metadata in
`level.dat`. Implement dirty-chunk tracking, background-thread writes, and a
flush barrier that ensures `/stop` does not return until every pending write
completes.

---

## Java Reference

| Concept | Java class |
|---------|-----------|
| Chunk serializer | `net.minecraft.world.level.chunk.storage.ChunkSerializer` |
| Chunk write pipeline | `net.minecraft.world.level.chunk.storage.RegionFileStorage` |
| Player data persistence | `net.minecraft.world.level.storage.PlayerDataStorage` |
| Level metadata | `net.minecraft.world.level.storage.PrimaryLevelData` |
| Level storage source | `net.minecraft.world.level.storage.LevelStorageSource` |
| NBT compound tag | `net.minecraft.nbt.CompoundTag` |

---

## Tasks

### 20.1 — Region file I/O (`oxidized-world/src/anvil/region_file.rs`)

```rust
use std::path::PathBuf;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncSeekExt, SeekFrom};

const SECTOR_SIZE: usize = 4096;
const HEADER_SECTORS: usize = 2;      // offsets + timestamps = 8192 bytes
const MAX_CHUNKS_PER_REGION: usize = 1024; // 32 * 32

/// A single `.mca` file managing a 32×32 region of chunks.
pub struct RegionFile {
    path: PathBuf,
    offsets: [u32; MAX_CHUNKS_PER_REGION],    // 3-byte sector offset + 1-byte sector count
    timestamps: [u32; MAX_CHUNKS_PER_REGION], // unix epoch seconds
    free_sectors: Vec<bool>,                  // true = free
}

impl RegionFile {
    pub async fn open_or_create(path: PathBuf) -> anyhow::Result<Self> {
        if path.exists() {
            Self::open(path).await
        } else {
            Self::create(path).await
        }
    }

    async fn create(path: PathBuf) -> anyhow::Result<Self> {
        let mut file = File::create(&path).await?;
        // Write empty header (8192 zero bytes)
        file.write_all(&[0u8; SECTOR_SIZE * HEADER_SECTORS]).await?;
        Ok(Self {
            path,
            offsets: [0u32; MAX_CHUNKS_PER_REGION],
            timestamps: [0u32; MAX_CHUNKS_PER_REGION],
            free_sectors: vec![false, false], // sectors 0 and 1 are the header
        })
    }

    async fn open(path: PathBuf) -> anyhow::Result<Self> {
        let mut file = File::open(&path).await?;
        let mut header = [0u8; SECTOR_SIZE * HEADER_SECTORS];
        file.read_exact(&mut header).await?;

        let mut offsets = [0u32; MAX_CHUNKS_PER_REGION];
        let mut timestamps = [0u32; MAX_CHUNKS_PER_REGION];
        for i in 0..MAX_CHUNKS_PER_REGION {
            offsets[i] = u32::from_be_bytes(header[i * 4..i * 4 + 4].try_into().unwrap());
            timestamps[i] = u32::from_be_bytes(
                header[SECTOR_SIZE + i * 4..SECTOR_SIZE + i * 4 + 4].try_into().unwrap(),
            );
        }

        let file_len = tokio::fs::metadata(&path).await?.len() as usize;
        let num_sectors = (file_len + SECTOR_SIZE - 1) / SECTOR_SIZE;
        let mut free_sectors = vec![true; num_sectors.max(HEADER_SECTORS)];
        free_sectors[0] = false;
        free_sectors[1] = false;
        for &offset in &offsets {
            if offset != 0 {
                let sector_num = (offset >> 8) as usize;
                let sector_cnt = (offset & 0xFF) as usize;
                for s in sector_num..sector_num + sector_cnt {
                    if s < free_sectors.len() {
                        free_sectors[s] = false;
                    }
                }
            }
        }

        Ok(Self { path, offsets, timestamps, free_sectors })
    }

    fn chunk_index(local_x: u8, local_z: u8) -> usize {
        (local_x as usize & 31) + (local_z as usize & 31) * 32
    }

    /// Returns Some(decompressed_nbt_bytes) or None if chunk absent.
    pub async fn read_chunk(&self, local_x: u8, local_z: u8) -> anyhow::Result<Option<Vec<u8>>> {
        let idx = Self::chunk_index(local_x, local_z);
        let offset = self.offsets[idx];
        if offset == 0 { return Ok(None); }

        let sector_num = (offset >> 8) as u64;
        let mut file = File::open(&self.path).await?;
        file.seek(SeekFrom::Start(sector_num * SECTOR_SIZE as u64)).await?;

        let mut length_buf = [0u8; 4];
        file.read_exact(&mut length_buf).await?;
        let length = u32::from_be_bytes(length_buf) as usize;

        let compression_type = file.read_u8().await?;
        let mut compressed = vec![0u8; length - 1];
        file.read_exact(&mut compressed).await?;

        let decompressed = match compression_type {
            1 => decompress_gzip(&compressed)?,
            2 => decompress_zlib(&compressed)?,
            3 => compressed,  // uncompressed
            _ => anyhow::bail!("Unknown compression type {}", compression_type),
        };
        Ok(Some(decompressed))
    }

    pub async fn write_chunk(
        &mut self,
        local_x: u8,
        local_z: u8,
        nbt_bytes: &[u8],
    ) -> anyhow::Result<()> {
        // Zlib compress
        let compressed = compress_zlib(nbt_bytes)?;
        // 4-byte length + 1-byte compression type + data
        let data_len = 1 + compressed.len();
        let sector_count = (data_len + 4 + SECTOR_SIZE - 1) / SECTOR_SIZE;

        let sector_num = self.allocate_sectors(sector_count);
        let idx = Self::chunk_index(local_x, local_z);

        let mut file = OpenOptions::new().write(true).open(&self.path).await?;
        // Extend file if needed
        let needed = (sector_num + sector_count) * SECTOR_SIZE;
        let current_len = file.metadata().await?.len() as usize;
        if needed > current_len {
            file.set_len(needed as u64).await?;
        }

        file.seek(SeekFrom::Start(sector_num as u64 * SECTOR_SIZE as u64)).await?;
        file.write_all(&(data_len as u32).to_be_bytes()).await?;
        file.write_u8(2).await?;  // zlib
        file.write_all(&compressed).await?;
        // Pad to sector boundary
        let written = 4 + 1 + compressed.len();
        let pad = sector_count * SECTOR_SIZE - written;
        file.write_all(&vec![0u8; pad]).await?;

        // Update header
        self.offsets[idx] = ((sector_num as u32) << 8) | (sector_count as u32 & 0xFF);
        self.timestamps[idx] = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;

        file.seek(SeekFrom::Start((idx * 4) as u64)).await?;
        file.write_all(&self.offsets[idx].to_be_bytes()).await?;
        file.seek(SeekFrom::Start((SECTOR_SIZE + idx * 4) as u64)).await?;
        file.write_all(&self.timestamps[idx].to_be_bytes()).await?;
        file.flush().await?;
        Ok(())
    }

    fn allocate_sectors(&mut self, count: usize) -> usize {
        // Find a run of `count` consecutive free sectors
        let mut start = HEADER_SECTORS;
        'outer: loop {
            if start + count > self.free_sectors.len() {
                // Extend free_sectors list
                self.free_sectors.resize(start + count, true);
                break 'outer start;
            }
            for i in 0..count {
                if !self.free_sectors[start + i] {
                    start += i + 1;
                    continue 'outer;
                }
            }
            break 'outer start;
        };
        for s in start..start + count {
            if s < self.free_sectors.len() {
                self.free_sectors[s] = false;
            }
        }
        start
    }
}
```

### 20.2 — Chunk serializer (`oxidized-game/src/world/chunk_serializer.rs`)

```rust
use oxidized_nbt::{CompoundTag, ListTag, NbtTag};
use crate::world::{LevelChunk, ChunkStatus, BlockState};

pub struct ChunkSerializer;

impl ChunkSerializer {
    pub fn write(chunk: &LevelChunk) -> anyhow::Result<Vec<u8>> {
        let mut root = CompoundTag::new();
        root.put_int("DataVersion", DATA_VERSION);
        root.put_int("xPos", chunk.pos.x);
        root.put_int("zPos", chunk.pos.z);
        root.put_int("yPos", chunk.min_section_y);
        root.put_long("LastUpdate", chunk.last_update_tick);
        root.put_string("Status", chunk.status.name());

        // Sections
        let mut sections = ListTag::new();
        for (section_y, section) in chunk.sections.iter().enumerate() {
            if section.is_all_air() { continue; }
            let mut s = CompoundTag::new();
            s.put_byte("Y", section_y as i8 + chunk.min_section_y as i8);

            // block_states palette + data
            let palette = build_block_palette(&section.block_states);
            s.put_compound("block_states", palette);

            // biomes palette
            let biomes = build_biome_palette(&section.biomes);
            s.put_compound("biomes", biomes);

            s.put_byte_array("SkyLight", section.sky_light.as_bytes().to_vec());
            s.put_byte_array("BlockLight", section.block_light.as_bytes().to_vec());
            sections.push(NbtTag::Compound(s));
        }
        root.put_list("sections", sections);

        // Heightmaps
        let mut heightmaps = CompoundTag::new();
        heightmaps.put_long_array("WORLD_SURFACE",
            encode_heightmap(&chunk.heightmaps.world_surface));
        heightmaps.put_long_array("MOTION_BLOCKING",
            encode_heightmap(&chunk.heightmaps.motion_blocking));
        root.put_compound("Heightmaps", heightmaps);

        // Block entities
        let mut block_entities = ListTag::new();
        for be in &chunk.block_entities {
            block_entities.push(NbtTag::Compound(be.to_nbt()));
        }
        root.put_list("block_entities", block_entities);

        let mut out = Vec::new();
        oxidized_nbt::write_compound(&root, &mut out)?;
        Ok(out)
    }

    pub fn read(bytes: &[u8]) -> anyhow::Result<LevelChunk> {
        let root = oxidized_nbt::read_compound(bytes)?;
        let chunk_x = root.get_int("xPos")?;
        let chunk_z = root.get_int("zPos")?;
        // ... (mirror of write)
        todo!("deserialize chunk from NBT")
    }
}

fn encode_heightmap(heights: &[i32; 256]) -> Vec<i64> {
    // Pack 9-bit heights (values 0..512) into 64-bit longs, 7 per long
    let mut result = Vec::with_capacity(37);
    let mut current: i64 = 0;
    let mut bits_filled = 0;
    for &h in heights.iter() {
        current |= (h as i64 & 0x1FF) << bits_filled;
        bits_filled += 9;
        if bits_filled >= 64 {
            result.push(current);
            current = 0;
            bits_filled = 0;
        }
    }
    if bits_filled > 0 { result.push(current); }
    result
}
```

### 20.3 — Dirty chunk tracker (`oxidized-game/src/world/dirty_chunks.rs`)

```rust
use std::collections::HashSet;
use crate::world::ChunkPos;

pub struct DirtyChunkTracker {
    dirty: HashSet<ChunkPos>,
}

impl DirtyChunkTracker {
    pub fn new() -> Self { Self { dirty: HashSet::new() } }

    pub fn mark_dirty(&mut self, pos: ChunkPos) {
        self.dirty.insert(pos);
    }

    pub fn drain_dirty(&mut self) -> impl Iterator<Item = ChunkPos> + '_ {
        self.dirty.drain()
    }

    pub fn is_dirty(&self, pos: &ChunkPos) -> bool {
        self.dirty.contains(pos)
    }

    pub fn dirty_count(&self) -> usize {
        self.dirty.len()
    }
}
```

### 20.4 — World save orchestrator (`oxidized-game/src/world/world_save.rs`)

```rust
use tokio::task::JoinSet;

pub struct WorldSaver {
    world_dir: PathBuf,
}

impl WorldSaver {
    pub async fn save_all(
        &self,
        level: &ServerLevel,
        flush: bool,
    ) -> anyhow::Result<()> {
        let mut join_set = JoinSet::new();

        // 1. Save dirty chunks
        let dirty: Vec<_> = level.dirty_chunks.drain_dirty().collect();
        tracing::info!("Saving {} dirty chunk(s)...", dirty.len());

        for pos in dirty {
            if let Some(chunk) = level.loaded_chunks.get(&pos) {
                let nbt_bytes = ChunkSerializer::write(chunk)?;
                let region_path = self.region_path(pos.region_x(), pos.region_z());
                let local_x = (pos.x & 31) as u8;
                let local_z = (pos.z & 31) as u8;

                join_set.spawn(tokio::task::spawn_blocking(move || {
                    tokio::runtime::Handle::current().block_on(async {
                        let mut region = RegionFile::open_or_create(region_path).await?;
                        region.write_chunk(local_x, local_z, &nbt_bytes).await
                    })
                }));
            }
        }

        if flush {
            while let Some(result) = join_set.join_next().await {
                result??; // propagate any I/O errors
            }
        }

        Ok(())
    }

    fn region_path(&self, region_x: i32, region_z: i32) -> PathBuf {
        self.world_dir.join("region").join(format!("r.{}.{}.mca", region_x, region_z))
    }
}
```

### 20.5 — PlayerDataStorage (`oxidized-game/src/player/player_data.rs`)

```rust
pub struct PlayerDataStorage {
    pub dir: PathBuf,  // <world>/playerdata/
}

impl PlayerDataStorage {
    pub async fn save(&self, player: &ServerPlayer) -> anyhow::Result<()> {
        let nbt = self.serialize_player(player)?;
        let path = self.dir.join(format!("{}.dat", player.uuid));
        let tmp  = path.with_extension("dat.tmp");

        // Write to .tmp first, then rename for atomicity
        let bytes = nbt_to_gzip_bytes(&nbt)?;
        tokio::fs::write(&tmp, &bytes).await?;
        tokio::fs::rename(&tmp, &path).await?;
        Ok(())
    }

    pub async fn load(&self, uuid: uuid::Uuid) -> anyhow::Result<Option<ServerPlayer>> {
        let path = self.dir.join(format!("{}.dat", uuid));
        if !path.exists() { return Ok(None); }
        let bytes = tokio::fs::read(&path).await?;
        let nbt = gzip_bytes_to_nbt(&bytes)?;
        Ok(Some(self.deserialize_player(uuid, &nbt)?))
    }

    fn serialize_player(&self, player: &ServerPlayer) -> anyhow::Result<CompoundTag> {
        let mut tag = CompoundTag::new();
        // Position
        tag.put_list("Pos", ListTag::from_doubles([
            player.position.x, player.position.y, player.position.z,
        ]));
        // Rotation
        tag.put_list("Rotation", ListTag::from_floats([
            player.rotation.yaw, player.rotation.pitch,
        ]));
        tag.put_float("Health", player.health);
        tag.put_int("FoodLevel", player.food_level);
        tag.put_float("FoodSaturationLevel", player.food_saturation);
        tag.put_int("Score", player.score);
        tag.put_int("PlayerGameType", player.game_mode as i32);
        tag.put_int("XpLevel", player.xp_level);
        tag.put_float("XpP", player.xp_progress);
        tag.put_int("XpTotal", player.xp_total);
        tag.put_string("Dimension", player.dimension.resource_key());

        // Spawn point
        if let Some(sp) = &player.spawn_point {
            tag.put_int("SpawnX", sp.x);
            tag.put_int("SpawnY", sp.y);
            tag.put_int("SpawnZ", sp.z);
            tag.put_bool("SpawnForced", sp.forced);
        }

        // Abilities
        let mut abilities = CompoundTag::new();
        abilities.put_bool("invulnerable", player.abilities.invulnerable);
        abilities.put_bool("flying", player.abilities.flying);
        abilities.put_bool("mayfly", player.abilities.may_fly);
        abilities.put_bool("instabuild", player.abilities.instant_build);
        abilities.put_float("flySpeed", player.abilities.fly_speed);
        abilities.put_float("walkSpeed", player.abilities.walk_speed);
        tag.put_compound("abilities", abilities);

        // Inventory
        let mut inventory = ListTag::new();
        for (slot, stack) in player.inventory.all_slots() {
            if !stack.is_empty() {
                let mut slot_tag = CompoundTag::new();
                slot_tag.put_byte("Slot", slot as i8);
                slot_tag.put_string("id", &stack.item.resource_key());
                slot_tag.put_int("Count", stack.count);
                // DataComponentPatch for custom components (1.20.5+)
                if let Some(components) = stack.components.as_nbt() {
                    slot_tag.put_compound("components", components);
                }
                inventory.push(NbtTag::Compound(slot_tag));
            }
        }
        tag.put_list("Inventory", inventory);

        Ok(tag)
    }
}
```

### 20.6 — level.dat serialization (`oxidized-game/src/world/level_dat.rs`)

```rust
pub fn write_level_dat(data: &LevelData, path: &Path) -> anyhow::Result<()> {
    let mut root = CompoundTag::new();
    let mut data_tag = CompoundTag::new();

    data_tag.put_int("DataVersion", DATA_VERSION);
    data_tag.put_string("LevelName", &data.level_name);
    data_tag.put_long("Time", data.game_time);
    data_tag.put_long("DayTime", data.day_time);
    data_tag.put_int("GameType", data.game_type as i32);
    data_tag.put_int("SpawnX", data.spawn_x);
    data_tag.put_int("SpawnY", data.spawn_y);
    data_tag.put_int("SpawnZ", data.spawn_z);
    data_tag.put_bool("raining", data.raining);
    data_tag.put_int("rainTime", data.rain_time);
    data_tag.put_bool("thundering", data.thundering);
    data_tag.put_int("thunderTime", data.thunder_time);

    // GameRules subtag
    let mut rules_tag = CompoundTag::new();
    for (key, value) in data.game_rules.all_rules() {
        let name = GameRules::name_of(key);
        match value {
            GameRuleValue::Bool(b) => rules_tag.put_string(name, if *b { "true" } else { "false" }),
            GameRuleValue::Int(i)  => rules_tag.put_string(name, &i.to_string()),
        }
    }
    data_tag.put_compound("GameRules", rules_tag);

    root.put_compound("Data", data_tag);

    let bytes = nbt_to_gzip_bytes(&root)?;
    let tmp = path.with_extension("dat_old");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn read_level_dat(path: &Path) -> anyhow::Result<LevelData> {
    let bytes = std::fs::read(path)?;
    let root = gzip_bytes_to_nbt(&bytes)?;
    let data = root.get_compound("Data")?;
    // ... mirror of write
    todo!("deserialize level.dat")
}
```

### 20.7 — Save triggers (`oxidized-game/src/server/save.rs`)

```rust
impl MinecraftServer {
    /// /save-all [flush] and auto-save handler.
    pub async fn save_all(&self, flush: bool) {
        tracing::info!("Saving the game (flush={})", flush);
        for (_, level) in &self.levels {
            let level = level.read().await;
            if let Err(e) = self.world_saver.save_all(&level, flush).await {
                tracing::error!("Failed to save level: {e}");
            }
        }
        for player in self.players.values() {
            let player = player.read().await;
            if let Err(e) = self.player_data.save(&player).await {
                tracing::error!("Failed to save player {}: {e}", player.uuid);
            }
        }
        if let Err(e) = write_level_dat(&self.primary_level_data, &self.level_dat_path) {
            tracing::error!("Failed to write level.dat: {e}");
        }
        tracing::info!("Saved the game");
    }

    /// Called from /stop — flushes everything and blocks until done.
    pub async fn shutdown_save(&self) {
        // Broadcast "Saving world..." to all players via action bar
        let msg_packet = ClientboundSystemChatPacket {
            content: Component::text("Saving world...").with_style(Style::color(NamedColor::Yellow)),
            overlay: false,
        };
        self.broadcast_system_packet(msg_packet).await;

        self.save_all(true).await;

        // Also write level.dat after all chunks flushed
        if let Err(e) = write_level_dat(&self.primary_level_data, &self.level_dat_path) {
            tracing::error!("level.dat write failed on shutdown: {e}");
        }
    }
}
```

---

## Data Structures

```rust
// oxidized-game/src/world/chunk_pos.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl ChunkPos {
    pub fn region_x(&self) -> i32 { self.x >> 5 }
    pub fn region_z(&self) -> i32 { self.z >> 5 }
    pub fn local_x(&self) -> u8   { (self.x & 31) as u8 }
    pub fn local_z(&self) -> u8   { (self.z & 31) as u8 }
}

// Player NBT schema (for reference; see serialize_player above)
//   Pos          [double, double, double]
//   Rotation     [float, float]       yaw, pitch
//   Health       float
//   FoodLevel    int
//   FoodSaturationLevel float
//   Score        int
//   Inventory    list<compound>       Slot byte, id string, Count int, components compound
//   Dimension    string               "minecraft:overworld" etc.
//   SpawnX/Y/Z   int
//   SpawnForced  byte
//   PlayerGameType int                0=survival, 1=creative, 2=adventure, 3=spectator
//   XpLevel      int
//   XpP          float
//   XpTotal      int
//   abilities    compound             invulnerable, flying, mayfly, instabuild, flySpeed, walkSpeed
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // --- Region file ---

    #[tokio::test]
    async fn region_file_create_and_write_chunk() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("r.0.0.mca");
        let mut region = RegionFile::open_or_create(path.clone()).await.unwrap();
        let nbt = b"fake_nbt_data_for_chunk_0_0";
        region.write_chunk(0, 0, nbt).await.unwrap();

        // Re-open and read back
        let region2 = RegionFile::open_or_create(path).await.unwrap();
        let read_back = region2.read_chunk(0, 0).await.unwrap().unwrap();
        assert_eq!(read_back, decompress_zlib(&compress_zlib(nbt).unwrap()).unwrap());
    }

    #[tokio::test]
    async fn region_file_absent_chunk_returns_none() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("r.0.0.mca");
        let region = RegionFile::open_or_create(path).await.unwrap();
        let result = region.read_chunk(5, 7).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn region_file_overwrites_existing_chunk() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("r.0.0.mca");
        let mut region = RegionFile::open_or_create(path.clone()).await.unwrap();
        region.write_chunk(1, 1, b"first_version").await.unwrap();
        region.write_chunk(1, 1, b"second_version").await.unwrap();

        let region2 = RegionFile::open_or_create(path).await.unwrap();
        let data = region2.read_chunk(1, 1).await.unwrap().unwrap();
        // The decompressed content should match "second_version"
        assert!(data.windows(14).any(|w| w == b"second_version"),
            "overwrite should store latest data");
    }

    // --- Heightmap encoding ---

    #[test]
    fn encode_heightmap_all_zeros_is_zero_longs() {
        let heights = [0i32; 256];
        let encoded = encode_heightmap(&heights);
        assert!(encoded.iter().all(|&v| v == 0));
    }

    #[test]
    fn encode_heightmap_first_entry_in_first_long() {
        let mut heights = [0i32; 256];
        heights[0] = 64; // 64 = 0b001000000
        let encoded = encode_heightmap(&heights);
        // lowest 9 bits of first long should be 64
        assert_eq!(encoded[0] & 0x1FF, 64);
    }

    // --- ChunkPos ---

    #[test]
    fn chunk_pos_region_coords() {
        let pos = ChunkPos { x: 35, z: -3 };
        assert_eq!(pos.region_x(), 1);   // 35 >> 5 = 1
        assert_eq!(pos.region_z(), -1);  // -3 >> 5 = -1
        assert_eq!(pos.local_x(), 3);    // 35 & 31 = 3
        assert_eq!(pos.local_z(), 29);   // -3 & 31 = 29
    }

    // --- DirtyChunkTracker ---

    #[test]
    fn dirty_chunk_tracker_marks_and_drains() {
        let mut tracker = DirtyChunkTracker::new();
        let pos = ChunkPos { x: 0, z: 0 };
        tracker.mark_dirty(pos);
        assert_eq!(tracker.dirty_count(), 1);
        let drained: Vec<_> = tracker.drain_dirty().collect();
        assert_eq!(drained.len(), 1);
        assert_eq!(tracker.dirty_count(), 0);
    }

    #[test]
    fn dirty_chunk_tracker_deduplicates() {
        let mut tracker = DirtyChunkTracker::new();
        let pos = ChunkPos { x: 5, z: 5 };
        tracker.mark_dirty(pos);
        tracker.mark_dirty(pos);
        assert_eq!(tracker.dirty_count(), 1, "same pos marked twice should not duplicate");
    }

    // --- PlayerDataStorage (unit-level, no tokio) ---

    #[test]
    fn player_serialize_includes_position() {
        let storage = PlayerDataStorage { dir: PathBuf::from("/tmp") };
        let player = make_test_player(uuid::Uuid::new_v4(), (1.5, 64.0, -2.5));
        let tag = storage.serialize_player(&player).unwrap();
        let pos = tag.get_list("Pos").unwrap();
        assert_eq!(pos.get_double(0).unwrap(), 1.5);
        assert_eq!(pos.get_double(1).unwrap(), 64.0);
        assert_eq!(pos.get_double(2).unwrap(), -2.5);
    }
}
```
