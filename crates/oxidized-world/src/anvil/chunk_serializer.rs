//! Anvil chunk serializer: serializes [`LevelChunk`] to NBT bytes.
//!
//! The inverse of [`AnvilChunkLoader`](super::chunk_loader::AnvilChunkLoader).
//! Produces the on-disk Anvil chunk format: a root NBT compound containing
//! section data, heightmaps, and metadata.

use std::sync::Arc;

use oxidized_nbt::{NbtCompound, NbtList, NbtTag, TAG_COMPOUND, TAG_STRING};

use super::error::AnvilError;
use crate::chunk::LevelChunk;
use crate::chunk::data_layer::DATA_LAYER_SIZE;
use crate::chunk::heightmap::HeightmapType;
use crate::chunk::level_chunk::OVERWORLD_MIN_Y;
use crate::registry::{BlockRegistry, BlockStateId};

/// Data version for Minecraft 26.1-pre-3.
const DATA_VERSION: i32 = 4782;

/// Minimum section Y index for the overworld.
const MIN_SECTION_Y: i32 = OVERWORLD_MIN_Y >> 4;

/// Serializes [`LevelChunk`] to Anvil NBT format.
///
/// Requires a [`BlockRegistry`] to resolve block state IDs back to
/// registry names and properties for the palette.
pub struct ChunkSerializer {
    block_registry: Arc<BlockRegistry>,
}

impl ChunkSerializer {
    /// Creates a new chunk serializer with the given block registry.
    pub fn new(block_registry: Arc<BlockRegistry>) -> Self {
        Self { block_registry }
    }

    /// Serializes a chunk to uncompressed NBT bytes.
    ///
    /// The result can be compressed with [`compress_zlib`](super::compress_zlib)
    /// before writing to a region file.
    ///
    /// # Errors
    ///
    /// Returns an error if NBT serialization fails.
    pub fn serialize(&self, chunk: &LevelChunk) -> Result<Vec<u8>, AnvilError> {
        let root = self.to_nbt(chunk);
        oxidized_nbt::write_bytes(&root).map_err(AnvilError::Nbt)
    }

    /// Serializes a chunk to an NBT compound.
    #[must_use]
    pub fn to_nbt(&self, chunk: &LevelChunk) -> NbtCompound {
        let mut root = NbtCompound::new();
        root.put_int("DataVersion", DATA_VERSION);
        root.put_int("xPos", chunk.pos.x);
        root.put_int("zPos", chunk.pos.z);
        root.put_int("yPos", chunk.min_y() >> 4);
        root.put_string("Status", "minecraft:full");

        // Sections
        let mut sections_list = NbtList::new(TAG_COMPOUND);
        for i in 0..chunk.section_count() {
            let section_nbt = self.serialize_section(chunk, i);
            // unwrap is safe: we're pushing Compound into a TAG_COMPOUND list
            let _ = sections_list.push(NbtTag::Compound(section_nbt));
        }
        root.put("sections", NbtTag::List(sections_list));

        // Heightmaps
        let hms = self.serialize_heightmaps(chunk);
        root.put("Heightmaps", NbtTag::Compound(hms));

        root
    }

    /// Serializes a single section to an NBT compound.
    fn serialize_section(&self, chunk: &LevelChunk, section_idx: usize) -> NbtCompound {
        let mut nbt = NbtCompound::new();

        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let section_y = (section_idx as i32 + MIN_SECTION_Y) as i8;
        nbt.put_byte("Y", section_y);

        if let Some(section) = chunk.section(section_idx) {
            // Block states
            let block_states = self.serialize_block_states(section);
            nbt.put("block_states", NbtTag::Compound(block_states));

            // Biomes
            let biomes = self.serialize_biomes(section);
            nbt.put("biomes", NbtTag::Compound(biomes));
        }

        // Light data — index = section_idx + 1 (offset for below-chunk light)
        let light_idx = section_idx + 1;

        if let Some(sky_light) = chunk.sky_light(light_idx) {
            let bytes = sky_light.as_bytes();
            if bytes.len() == DATA_LAYER_SIZE {
                let signed: Vec<i8> = bytes.iter().map(|&b| b as i8).collect();
                nbt.put("SkyLight", NbtTag::ByteArray(signed));
            }
        }

        if let Some(block_light) = chunk.block_light(light_idx) {
            let bytes = block_light.as_bytes();
            if bytes.len() == DATA_LAYER_SIZE {
                let signed: Vec<i8> = bytes.iter().map(|&b| b as i8).collect();
                nbt.put("BlockLight", NbtTag::ByteArray(signed));
            }
        }

        nbt
    }

    /// Serializes block states for a section.
    fn serialize_block_states(
        &self,
        section: &crate::chunk::section::LevelChunkSection,
    ) -> NbtCompound {
        let states = section.states_clone();
        let (palette_ids, data_longs) = states.to_nbt_data();

        let mut nbt = NbtCompound::new();

        // Build palette as list of compound entries
        let mut palette_list = NbtList::new(TAG_COMPOUND);
        for &state_id in &palette_ids {
            let entry = self.block_state_to_nbt(state_id);
            let _ = palette_list.push(NbtTag::Compound(entry));
        }
        nbt.put("palette", NbtTag::List(palette_list));

        if !data_longs.is_empty() {
            nbt.put("data", NbtTag::LongArray(data_longs));
        }

        nbt
    }

    /// Converts a block state ID to an NBT palette entry.
    fn block_state_to_nbt(&self, state_id: u32) -> NbtCompound {
        let mut entry = NbtCompound::new();

        #[allow(clippy::cast_possible_truncation)]
        let bsid = BlockStateId(state_id as u16);

        if (bsid.0 as usize) < self.block_registry.state_count() {
            entry.put_string("Name", bsid.block_name());

            let props = bsid.properties();
            if !props.is_empty() {
                let mut nbt_props = NbtCompound::new();
                for (key, value) in &props {
                    nbt_props.put_string(*key, *value);
                }
                entry.put("Properties", NbtTag::Compound(nbt_props));
            }

            return entry;
        }

        // Fallback for unknown state IDs
        entry.put_string("Name", "minecraft:air");
        entry
    }

    /// Serializes biome data for a section.
    fn serialize_biomes(&self, section: &crate::chunk::section::LevelChunkSection) -> NbtCompound {
        let biomes = section.biomes_clone();
        let (palette_ids, data_longs) = biomes.to_nbt_data();

        let mut nbt = NbtCompound::new();

        // Biome palette is a list of strings
        let mut palette_list = NbtList::new(TAG_STRING);
        for &biome_id in &palette_ids {
            let name = self.biome_name(biome_id);
            let _ = palette_list.push(NbtTag::String(name));
        }
        nbt.put("palette", NbtTag::List(palette_list));

        if !data_longs.is_empty() {
            nbt.put("data", NbtTag::LongArray(data_longs));
        }

        nbt
    }

    /// Resolves a biome ID to a registry name.
    ///
    /// Uses the biome registry for all 65 vanilla biomes.
    /// IDs outside the valid range fall back to `"minecraft:plains"`.
    fn biome_name(&self, biome_id: u32) -> String {
        crate::registry::biome_id_to_name(biome_id)
            .unwrap_or("minecraft:plains")
            .to_owned()
    }

    /// Serializes heightmaps for a chunk.
    fn serialize_heightmaps(&self, chunk: &LevelChunk) -> NbtCompound {
        let mut nbt = NbtCompound::new();

        let types = [
            HeightmapType::MotionBlocking,
            HeightmapType::WorldSurface,
            HeightmapType::OceanFloor,
            HeightmapType::MotionBlockingNoLeaves,
        ];

        for htype in &types {
            if let Some(hm) = chunk.heightmap(*htype) {
                let longs = hm.to_nbt_longs();
                nbt.put(htype.nbt_key(), NbtTag::LongArray(longs));
            }
        }

        nbt
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::chunk::data_layer::DataLayer;
    use crate::chunk::heightmap::Heightmap;
    use crate::chunk::level_chunk::{LevelChunk, OVERWORLD_HEIGHT, OVERWORLD_SECTION_COUNT};
    use oxidized_types::ChunkPos;

    fn test_registry() -> Arc<BlockRegistry> {
        Arc::new(BlockRegistry::load().unwrap())
    }

    #[test]
    fn test_serialize_empty_chunk() {
        let registry = test_registry();
        let serializer = ChunkSerializer::new(registry);
        let chunk = LevelChunk::new(ChunkPos::new(5, -3));

        let nbt = serializer.to_nbt(&chunk);

        assert_eq!(nbt.get_int("xPos").unwrap(), 5);
        assert_eq!(nbt.get_int("zPos").unwrap(), -3);
        assert_eq!(nbt.get_int("yPos").unwrap(), -4);
        assert_eq!(nbt.get_int("DataVersion").unwrap(), 4782);
        assert_eq!(nbt.get_string("Status").unwrap(), "minecraft:full");

        let sections = nbt.get_list("sections").unwrap();
        assert_eq!(sections.len(), OVERWORLD_SECTION_COUNT);
    }

    #[test]
    fn test_serialize_chunk_with_blocks() {
        let registry = test_registry();
        let serializer = ChunkSerializer::new(Arc::clone(&registry));
        let mut chunk = LevelChunk::new(ChunkPos::new(0, 0));

        // Get stone state ID
        let stone_id = registry
            .get_block_def("minecraft:stone")
            .unwrap()
            .default_state as u32;

        // Set some blocks at y=0 (section index 4)
        chunk.set_block_state(0, 0, 0, stone_id).unwrap();
        chunk.set_block_state(1, 0, 0, stone_id).unwrap();

        let bytes = serializer.serialize(&chunk).unwrap();
        assert!(!bytes.is_empty());

        // Parse the NBT back
        let parsed = oxidized_nbt::read_bytes(&bytes).unwrap();
        let sections = parsed.get_list("sections").unwrap();

        // Section at y=0 is index 4 (relative to section list)
        if let Some(NbtTag::Compound(sec)) = sections.get(4) {
            let bs = sec.get_compound("block_states").unwrap();
            let palette = bs.get_list("palette").unwrap();
            // Should have at least 2 entries: air and stone
            assert!(palette.len() >= 2, "palette should have air + stone");
        }
    }

    #[test]
    fn test_serialize_roundtrip_with_loader() {
        let registry = test_registry();
        let serializer = ChunkSerializer::new(Arc::clone(&registry));
        let loader = super::super::AnvilChunkLoader::new(
            std::path::Path::new("/tmp"),
            Arc::clone(&registry),
        );

        let mut chunk = LevelChunk::new(ChunkPos::new(10, -5));

        // Put a stone block at y=0 (section 4)
        let stone_id = registry
            .get_block_def("minecraft:stone")
            .unwrap()
            .default_state as u32;
        chunk.set_block_state(5, 0, 5, stone_id).unwrap();

        // Serialize to NBT
        let nbt = serializer.to_nbt(&chunk);

        // Deserialize with the loader
        let loaded = loader.deserialize_chunk(&nbt, 10, -5).unwrap();

        // Verify the block state roundtripped
        assert_eq!(loaded.get_block_state(5, 0, 5).unwrap(), stone_id);
        // Air blocks should still be 0
        assert_eq!(loaded.get_block_state(0, 0, 0).unwrap(), 0);
    }

    #[test]
    fn test_serialize_bytes_roundtrip() {
        let registry = test_registry();
        let serializer = ChunkSerializer::new(Arc::clone(&registry));
        let loader = super::super::AnvilChunkLoader::new(
            std::path::Path::new("/tmp"),
            Arc::clone(&registry),
        );

        let chunk = LevelChunk::new(ChunkPos::new(0, 0));
        let bytes = serializer.serialize(&chunk).unwrap();
        let nbt = oxidized_nbt::read_bytes(&bytes).unwrap();
        let loaded = loader.deserialize_chunk(&nbt, 0, 0).unwrap();

        assert_eq!(loaded.pos.x, 0);
        assert_eq!(loaded.pos.z, 0);
        assert_eq!(loaded.section_count(), OVERWORLD_SECTION_COUNT);
    }

    #[test]
    fn test_serialize_heightmaps() {
        let registry = test_registry();
        let serializer = ChunkSerializer::new(registry);
        let mut chunk = LevelChunk::new(ChunkPos::new(0, 0));

        let mut hm = Heightmap::new(HeightmapType::MotionBlocking, OVERWORLD_HEIGHT).unwrap();
        hm.set(5, 5, 100).unwrap();
        chunk.set_heightmap(hm);

        let nbt = serializer.to_nbt(&chunk);
        let hms = nbt.get_compound("Heightmaps").unwrap();
        assert!(hms.get_long_array("MOTION_BLOCKING").is_some());
    }

    #[test]
    fn test_serialize_light_data() {
        let registry = test_registry();
        let serializer = ChunkSerializer::new(registry);
        let mut chunk = LevelChunk::new(ChunkPos::new(0, 0));

        let sky = DataLayer::filled(15);
        chunk.set_sky_light(5, sky); // section index 4, light_idx 5

        let nbt = serializer.to_nbt(&chunk);
        let sections = nbt.get_list("sections").unwrap();

        // Section 4 should have SkyLight
        let Some(NbtTag::Compound(sec)) = sections.get(4) else {
            unreachable!("expected compound");
        };
        assert!(sec.get_byte_array("SkyLight").is_some());
    }
}
