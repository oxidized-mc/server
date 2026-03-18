//! Anvil chunk loader: deserializes chunk NBT into [`LevelChunk`].
//!
//! The on-disk chunk format uses NBT compounds with palettized block
//! state and biome data per section. This module reads that format and
//! populates the in-memory chunk structures.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use oxidized_nbt::{NbtCompound, NbtTag};

use super::error::AnvilError;
use super::region::RegionFile;
use crate::chunk::data_layer::{DataLayer, DATA_LAYER_SIZE};
use crate::chunk::heightmap::{Heightmap, HeightmapType};
use crate::chunk::level_chunk::{
    ChunkPos, LevelChunk, OVERWORLD_HEIGHT, OVERWORLD_MIN_Y, OVERWORLD_SECTION_COUNT,
};
use crate::chunk::paletted_container::{PalettedContainer, Strategy};
use crate::chunk::section::LevelChunkSection;
use crate::registry::BlockRegistry;

/// Minimum section Y index for the overworld.
const MIN_SECTION_Y: i32 = OVERWORLD_MIN_Y >> 4;

/// Synchronous Anvil chunk loader.
///
/// Reads `.mca` region files and deserializes chunk NBT into [`LevelChunk`].
/// Must be called from a blocking context (e.g. `tokio::task::spawn_blocking`).
pub struct AnvilChunkLoader {
    region_dir: PathBuf,
    block_registry: Arc<BlockRegistry>,
    open_regions: HashMap<(i32, i32), RegionFile>,
}

impl AnvilChunkLoader {
    /// Creates a new chunk loader for the given dimension's region directory.
    ///
    /// The `region_dir` should be e.g. `<world>/region` for the overworld.
    pub fn new(region_dir: &Path, block_registry: Arc<BlockRegistry>) -> Self {
        Self {
            region_dir: region_dir.to_path_buf(),
            block_registry,
            open_regions: HashMap::new(),
        }
    }

    /// Loads a chunk synchronously. Returns `None` if the chunk does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the region file is corrupted or the NBT data is
    /// malformed.
    pub fn load_chunk(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<Option<LevelChunk>, AnvilError> {
        let rx = chunk_x >> 5;
        let rz = chunk_z >> 5;
        let region_path = self.region_path(chunk_x, chunk_z);

        if !region_path.exists() {
            return Ok(None);
        }

        let region = self
            .open_regions
            .entry((rx, rz))
            .or_insert(RegionFile::open(&region_path)?);

        let nbt_bytes = match region.read_chunk_data(chunk_x, chunk_z)? {
            None => return Ok(None),
            Some(b) => b,
        };

        let root = oxidized_nbt::read_bytes(&nbt_bytes)?;
        Ok(Some(self.deserialize_chunk(&root, chunk_x, chunk_z)?))
    }

    /// Closes all open region file handles.
    pub fn close_all(&mut self) {
        self.open_regions.clear();
    }

    /// Returns the region file path for the given chunk coordinates.
    fn region_path(&self, chunk_x: i32, chunk_z: i32) -> PathBuf {
        let rx = chunk_x >> 5;
        let rz = chunk_z >> 5;
        self.region_dir.join(format!("r.{rx}.{rz}.mca"))
    }

    /// Deserializes a chunk from its root NBT compound.
    fn deserialize_chunk(
        &self,
        root: &NbtCompound,
        expected_x: i32,
        expected_z: i32,
    ) -> Result<LevelChunk, AnvilError> {
        let chunk_x = root
            .get_int("xPos")
            .ok_or(AnvilError::MissingField { field: "xPos" })?;
        let chunk_z = root
            .get_int("zPos")
            .ok_or(AnvilError::MissingField { field: "zPos" })?;

        if chunk_x != expected_x || chunk_z != expected_z {
            tracing::warn!(
                expected_x,
                expected_z,
                actual_x = chunk_x,
                actual_z = chunk_z,
                "chunk coordinates mismatch"
            );
        }

        let mut chunk = LevelChunk::new(ChunkPos::new(chunk_x, chunk_z));

        // Parse sections
        if let Some(sections_list) = root.get_list("sections") {
            for section_tag in sections_list.iter() {
                if let NbtTag::Compound(section_nbt) = section_tag {
                    self.load_section(&mut chunk, section_nbt)?;
                }
            }
        }

        // Parse heightmaps
        if let Some(hms) = root.get_compound("Heightmaps") {
            self.load_heightmaps(&mut chunk, hms);
        }

        Ok(chunk)
    }

    /// Loads a single section from NBT into the chunk.
    fn load_section(
        &self,
        chunk: &mut LevelChunk,
        section_nbt: &NbtCompound,
    ) -> Result<(), AnvilError> {
        let y = section_nbt
            .get_byte("Y")
            .ok_or(AnvilError::MissingField { field: "Y" })?;

        let si = (y as i32 - MIN_SECTION_Y) as usize;
        if si >= OVERWORLD_SECTION_COUNT {
            return Ok(()); // Out of range section, skip
        }

        let section = chunk
            .section_mut(si)
            .ok_or(AnvilError::MissingField { field: "Y" })?;

        // Block states
        if let Some(bs) = section_nbt.get_compound("block_states") {
            let container = self.deserialize_block_states(bs)?;
            *section = LevelChunkSection::from_parts(container, section.biomes_clone());
        }

        // Biomes
        if let Some(bio) = section_nbt.get_compound("biomes") {
            let container = self.deserialize_biomes(bio)?;
            *section = LevelChunkSection::from_parts(section.states_clone(), container);
        }

        // Light data
        // Light index = section_index + 1 (offset for below-chunk light layer)
        let light_idx = si + 1;

        if let Some(sky_bytes) = section_nbt.get_byte_array("SkyLight") {
            if sky_bytes.len() == DATA_LAYER_SIZE {
                let raw: Vec<u8> = sky_bytes.iter().map(|&b| b as u8).collect();
                if let Some(layer) = DataLayer::from_bytes(&raw) {
                    chunk.set_sky_light(light_idx, layer);
                }
            }
        }

        if let Some(block_bytes) = section_nbt.get_byte_array("BlockLight") {
            if block_bytes.len() == DATA_LAYER_SIZE {
                let raw: Vec<u8> = block_bytes.iter().map(|&b| b as u8).collect();
                if let Some(layer) = DataLayer::from_bytes(&raw) {
                    chunk.set_block_light(light_idx, layer);
                }
            }
        }

        Ok(())
    }

    /// Deserializes block states from an NBT compound.
    ///
    /// The compound contains:
    /// - `palette`: List<Compound> with `Name` (and optional `Properties`)
    /// - `data`: LongArray of packed palette indices (absent if palette has 1 entry)
    fn deserialize_block_states(&self, nbt: &NbtCompound) -> Result<PalettedContainer, AnvilError> {
        let palette_list = nbt
            .get_list("palette")
            .ok_or(AnvilError::MissingField { field: "palette" })?;

        let mut palette_ids = Vec::with_capacity(palette_list.len());

        for tag in palette_list.iter() {
            if let NbtTag::Compound(entry) = tag {
                let name = entry
                    .get_string("Name")
                    .ok_or(AnvilError::MissingField { field: "Name" })?;

                let state_id = if let Some(props) = entry.get_compound("Properties") {
                    self.lookup_block_state(name, Some(props))?
                } else {
                    self.lookup_block_state(name, None)?
                };

                palette_ids.push(state_id);
            }
        }

        let data_longs = nbt.get_long_array("data").unwrap_or(&[]);

        PalettedContainer::from_nbt_data(Strategy::BlockStates, palette_ids, data_longs)
            .map_err(AnvilError::from)
    }

    /// Deserializes biome data from an NBT compound.
    ///
    /// The compound contains:
    /// - `palette`: List<String> of biome resource names
    /// - `data`: LongArray of packed palette indices (absent if palette has 1 entry)
    fn deserialize_biomes(&self, nbt: &NbtCompound) -> Result<PalettedContainer, AnvilError> {
        let palette_list = nbt
            .get_list("palette")
            .ok_or(AnvilError::MissingField { field: "palette" })?;

        // For now, use sequential IDs for biomes — actual biome registry
        // mapping will be implemented when biome registries are available.
        let mut palette_ids = Vec::with_capacity(palette_list.len());
        for (i, _tag) in palette_list.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            palette_ids.push(i as u32);
        }

        let data_longs = nbt.get_long_array("data").unwrap_or(&[]);

        PalettedContainer::from_nbt_data(Strategy::Biomes, palette_ids, data_longs)
            .map_err(AnvilError::from)
    }

    /// Looks up a block state ID from the registry by name and properties.
    fn lookup_block_state(
        &self,
        name: &str,
        properties: Option<&NbtCompound>,
    ) -> Result<u32, AnvilError> {
        // First find the block by name
        let block = self
            .block_registry
            .get_block(name)
            .ok_or_else(|| AnvilError::UnknownBlock(name.to_owned()))?;

        // If no properties, return the default state
        let props = match properties {
            Some(p) if block.states.len() > 1 => p,
            _ => return Ok(block.default_state.0 as u32),
        };

        // Find the matching state by looking up each state ID in the registry
        for &state_id in &block.states {
            if let Some(state) = self.block_registry.get_state(state_id) {
                if self.state_matches_properties(state, props) {
                    return Ok(state.id.0 as u32);
                }
            }
        }

        // Fallback to default state if no exact match
        tracing::warn!(
            block = name,
            "no exact state match found, using default state"
        );
        Ok(block.default_state.0 as u32)
    }

    /// Checks if a block state's properties match the NBT properties compound.
    fn state_matches_properties(
        &self,
        state: &crate::registry::BlockState,
        nbt_props: &NbtCompound,
    ) -> bool {
        if state.properties.len() != nbt_props.len() {
            return false;
        }

        for (key, state_value) in &state.properties {
            if let Some(nbt_value) = nbt_props.get_string(key) {
                if state_value != nbt_value {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }

    /// Loads heightmap data from the Heightmaps NBT compound.
    fn load_heightmaps(&self, chunk: &mut LevelChunk, hms: &NbtCompound) {
        let types = [
            HeightmapType::MotionBlocking,
            HeightmapType::WorldSurface,
            HeightmapType::OceanFloor,
            HeightmapType::MotionBlockingNoLeaves,
        ];

        for htype in &types {
            if let Some(longs) = hms.get_long_array(htype.nbt_key()) {
                let raw: Vec<u64> = longs.iter().map(|&l| l as u64).collect();
                match Heightmap::from_raw(*htype, OVERWORLD_HEIGHT, raw) {
                    Ok(hm) => chunk.set_heightmap(hm),
                    Err(e) => tracing::warn!(
                        heightmap = htype.nbt_key(),
                        error = %e,
                        "failed to load heightmap"
                    ),
                }
            }
        }
    }
}

/// Async-friendly wrapper around [`AnvilChunkLoader`].
///
/// Uses `tokio::task::spawn_blocking` to offload synchronous file I/O
/// from the async executor.
pub struct AsyncChunkLoader {
    inner: Arc<Mutex<AnvilChunkLoader>>,
}

impl AsyncChunkLoader {
    /// Creates a new async chunk loader wrapping the given synchronous loader.
    pub fn new(loader: AnvilChunkLoader) -> Self {
        Self {
            inner: Arc::new(Mutex::new(loader)),
        }
    }

    /// Loads a chunk asynchronously. Returns `None` if the chunk does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error on I/O failure or malformed chunk data.
    pub async fn load_chunk(
        &self,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<Option<LevelChunk>, AnvilError> {
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || {
            let mut loader = inner
                .lock()
                .map_err(|_| AnvilError::Decompression("mutex poisoned".into()))?;
            loader.load_chunk(chunk_x, chunk_z)
        })
        .await
        .map_err(|e| AnvilError::Decompression(format!("spawn_blocking failed: {e}")))?
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::registry::BlockRegistry;
    use oxidized_nbt::NbtList;

    fn test_registry() -> Arc<BlockRegistry> {
        Arc::new(BlockRegistry::load().unwrap())
    }

    #[test]
    fn test_deserialize_single_block_section() {
        let registry = test_registry();
        let loader = AnvilChunkLoader::new(Path::new("/tmp"), registry);

        // Build a minimal chunk NBT with one section containing only stone
        let mut root = NbtCompound::new();
        root.put_int("xPos", 0);
        root.put_int("zPos", 0);
        root.put_int("yPos", -4);

        // Build section at Y=0
        let mut section = NbtCompound::new();
        section.put_byte("Y", 0);

        // Block states with single palette entry (stone)
        let mut block_states = NbtCompound::new();
        let mut palette = NbtList::new(oxidized_nbt::TAG_COMPOUND);
        let mut stone_entry = NbtCompound::new();
        stone_entry.put_string("Name", "minecraft:stone");
        palette.push(NbtTag::Compound(stone_entry)).unwrap();
        block_states.put("palette", NbtTag::List(palette));
        section.put("block_states", NbtTag::Compound(block_states));

        // Biomes with single entry
        let mut biomes = NbtCompound::new();
        let mut biome_palette = NbtList::new(oxidized_nbt::TAG_STRING);
        biome_palette
            .push(NbtTag::String("minecraft:plains".into()))
            .unwrap();
        biomes.put("palette", NbtTag::List(biome_palette));
        section.put("biomes", NbtTag::Compound(biomes));

        let mut sections_list = NbtList::new(oxidized_nbt::TAG_COMPOUND);
        sections_list.push(NbtTag::Compound(section)).unwrap();
        root.put("sections", NbtTag::List(sections_list));

        let chunk = loader.deserialize_chunk(&root, 0, 0).unwrap();

        // Section at y=0 is index 4 (since min_y=-64, section 0 = y=-64)
        let section_idx = 4;
        let section = chunk.section(section_idx).unwrap();

        // Stone default state ID
        let stone_id = loader
            .block_registry
            .get_block("minecraft:stone")
            .unwrap()
            .default_state
            .0 as u32;

        assert_eq!(section.get_block_state(0, 0, 0).unwrap(), stone_id);
        assert_eq!(section.get_block_state(15, 15, 15).unwrap(), stone_id);
    }

    #[test]
    fn test_deserialize_mixed_block_section() {
        let registry = test_registry();
        let loader = AnvilChunkLoader::new(Path::new("/tmp"), registry);

        let mut root = NbtCompound::new();
        root.put_int("xPos", 5);
        root.put_int("zPos", -3);
        root.put_int("yPos", -4);

        // Build section at Y=-4 (section index 0)
        let mut section = NbtCompound::new();
        section.put_byte("Y", -4);

        // Block states with two palette entries: air (0) and stone
        let mut block_states = NbtCompound::new();
        let mut palette = NbtList::new(oxidized_nbt::TAG_COMPOUND);

        let mut air_entry = NbtCompound::new();
        air_entry.put_string("Name", "minecraft:air");
        palette.push(NbtTag::Compound(air_entry)).unwrap();

        let mut stone_entry = NbtCompound::new();
        stone_entry.put_string("Name", "minecraft:stone");
        palette.push(NbtTag::Compound(stone_entry)).unwrap();

        // With 2 palette entries, we need 1 bit per entry → 4 bits (clamped min)
        // Actually the storage bits = Strategy::storage_bits(bits_for_count(2))
        // bits_for_count(2) = 1, storage_bits(BlockStates, 1) = 4
        // 4096 entries at 4 bits = 256 longs
        // All zeros = all air. We'll create a simple data array.
        let values_per_long = 64 / 4;
        let num_longs = (4096 + values_per_long - 1) / values_per_long;
        let data_longs: Vec<i64> = vec![0i64; num_longs];

        block_states.put("palette", NbtTag::List(palette));
        block_states.put("data", NbtTag::LongArray(data_longs));
        section.put("block_states", NbtTag::Compound(block_states));

        // Biomes
        let mut biomes = NbtCompound::new();
        let mut biome_palette = NbtList::new(oxidized_nbt::TAG_STRING);
        biome_palette
            .push(NbtTag::String("minecraft:plains".into()))
            .unwrap();
        biomes.put("palette", NbtTag::List(biome_palette));
        section.put("biomes", NbtTag::Compound(biomes));

        let mut sections_list = NbtList::new(oxidized_nbt::TAG_COMPOUND);
        sections_list.push(NbtTag::Compound(section)).unwrap();
        root.put("sections", NbtTag::List(sections_list));

        let chunk = loader.deserialize_chunk(&root, 5, -3).unwrap();
        assert_eq!(chunk.pos.x, 5);
        assert_eq!(chunk.pos.z, -3);

        // Section index 0 — all air since data is all zeros (palette index 0 = air)
        let section = chunk.section(0).unwrap();
        assert_eq!(section.get_block_state(0, 0, 0).unwrap(), 0); // air = state 0
    }

    #[test]
    fn test_loader_nonexistent_region_returns_none() {
        let registry = test_registry();
        let mut loader = AnvilChunkLoader::new(Path::new("/tmp/nonexistent_world_dir"), registry);
        let result = loader.load_chunk(0, 0).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_heightmap_loading() {
        let registry = test_registry();
        let loader = AnvilChunkLoader::new(Path::new("/tmp"), registry);

        let mut root = NbtCompound::new();
        root.put_int("xPos", 0);
        root.put_int("zPos", 0);

        // Empty sections list
        let sections_list = NbtList::new(oxidized_nbt::TAG_COMPOUND);
        root.put("sections", NbtTag::List(sections_list));

        // Heightmaps with MOTION_BLOCKING
        let mut hms = NbtCompound::new();
        // 9 bits per entry, 256 entries → ceil(256*9/64) = 37 longs
        let longs = vec![0i64; 37];
        hms.put("MOTION_BLOCKING", NbtTag::LongArray(longs));
        root.put("Heightmaps", NbtTag::Compound(hms));

        let chunk = loader.deserialize_chunk(&root, 0, 0).unwrap();
        assert!(chunk.heightmap(HeightmapType::MotionBlocking).is_some());
    }
}
