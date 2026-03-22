//! Flat chunk generator.
//!
//! Produces [`LevelChunk`] instances filled with the layers defined by
//! a [`FlatWorldConfig`]. A template set of sections and heightmaps is
//! computed once at construction; [`generate_chunk`] simply clones the
//! template and attaches the requested position.
//!
//! [`generate_chunk`]: FlatChunkGenerator::generate_chunk

use oxidized_world::chunk::heightmap::{Heightmap, HeightmapType};
use oxidized_world::chunk::level_chunk::{
    OVERWORLD_HEIGHT, OVERWORLD_MIN_Y, OVERWORLD_SECTION_COUNT,
};
use oxidized_world::chunk::section::LevelChunkSection;
use oxidized_world::chunk::{ChunkPos, DataLayer, LevelChunk};

use super::config::FlatWorldConfig;
use crate::worldgen::ChunkGenerator;

/// Default biome ID for `minecraft:plains` in the alphabetically-sorted
/// biome registry. The client receives biome entries sorted by name;
/// plains is index 40.
const PLAINS_BIOME_ID: u32 = 40;

/// Sea level for flat worlds (vanilla: −63).
const FLAT_SEA_LEVEL: i32 = -63;

/// Pre-computed template that can be cloned for every generated chunk.
#[derive(Debug, Clone)]
struct ChunkTemplate {
    sections: Vec<LevelChunkSection>,
    heightmaps: Vec<Heightmap>,
    sky_light: Vec<Option<DataLayer>>,
}

/// Resolves a biome resource key (e.g. `"minecraft:plains"`) to its
/// registry ID using alphabetical ordering (matching the protocol).
fn resolve_biome_id(biome_key: &str) -> u32 {
    // The biome registry is sent to clients in alphabetical order.
    // Rather than loading the full registry JSON at runtime, we maintain
    // a lookup for the most common flat-world biomes and fall back to
    // PLAINS_BIOME_ID for unknown keys. When a full BiomeRegistry is
    // added (future phase), this should delegate to it.
    match biome_key {
        "minecraft:plains" => PLAINS_BIOME_ID,
        "minecraft:desert" => 14,
        "minecraft:the_void" => 57,
        "minecraft:snowy_plains" => 46,
        "minecraft:mushroom_fields" => 33,
        _ => PLAINS_BIOME_ID,
    }
}

/// Builds the pre-computed chunk template from configuration.
fn build_template(config: &FlatWorldConfig) -> ChunkTemplate {
    let biome_id = resolve_biome_id(&config.biome);
    let flattened = config.flattened_layers();
    let total_height = flattened.len();

    // Build sections: for each 16-block vertical span, determine which
    // block(s) it contains and construct the section efficiently.
    let mut sections = Vec::with_capacity(OVERWORLD_SECTION_COUNT);
    for section_idx in 0..OVERWORLD_SECTION_COUNT {
        let base_y_offset = section_idx * 16;

        // Determine if this section intersects the layer stack.
        if base_y_offset >= total_height {
            // Entirely above the layers — all air, but set biome.
            let mut section = LevelChunkSection::new();
            fill_section_biome(&mut section, biome_id);
            sections.push(section);
            continue;
        }

        let end_y_offset = (base_y_offset + 16).min(total_height);
        let section_slice = &flattened[base_y_offset..end_y_offset];

        // Check if the entire section is a single block type.
        let first = section_slice[0];
        let is_uniform = section_slice.iter().all(|&b| b == first);

        if is_uniform && end_y_offset - base_y_offset == 16 {
            // Full section of one block type — use O(1) constructor.
            let mut section = LevelChunkSection::filled(u32::from(first.0));
            fill_section_biome(&mut section, biome_id);
            sections.push(section);
        } else {
            // Mixed section — fill block by block (only the Y levels
            // that have blocks; the rest are already air).
            let mut section = LevelChunkSection::new();
            for (local_y, block_id) in section_slice.iter().enumerate() {
                let state_id = u32::from(block_id.0);
                for x in 0..16usize {
                    for z in 0..16usize {
                        let _ = section.set_block_state(x, local_y, z, state_id);
                    }
                }
            }
            fill_section_biome(&mut section, biome_id);
            sections.push(section);
        }
    }

    // Build heightmaps (both client and worldgen types).
    let surface_height = total_height as u32;
    let all_types: Vec<HeightmapType> = HeightmapType::CLIENT_TYPES
        .iter()
        .chain(HeightmapType::WORLDGEN_TYPES.iter())
        .copied()
        .collect();
    let mut heightmaps = Vec::with_capacity(all_types.len());
    for htype in all_types {
        if let Ok(mut hm) = Heightmap::new(htype, OVERWORLD_HEIGHT) {
            for x in 0..16usize {
                for z in 0..16usize {
                    let _ = hm.set(x, z, surface_height);
                }
            }
            heightmaps.push(hm);
        }
    }

    // Build sky light.
    let mut sky_light = vec![None; OVERWORLD_SECTION_COUNT + 2];
    let surface_section = total_height.div_ceil(16);
    // Sections fully above the surface get full brightness.
    for slot in &mut sky_light[(surface_section + 2)..] {
        *slot = Some(DataLayer::filled(15));
    }

    ChunkTemplate {
        sections,
        heightmaps,
        sky_light,
    }
}

/// Sets all 64 biome entries (4×4×4) in a section to the given biome ID.
fn fill_section_biome(section: &mut LevelChunkSection, biome_id: u32) {
    for bx in 0..4usize {
        for by in 0..4usize {
            for bz in 0..4usize {
                let _ = section.set_biome(bx, by, bz, biome_id);
            }
        }
    }
}

/// A chunk generator for flat (superflat) worlds.
///
/// Computes a template once at construction; [`generate_chunk`] clones
/// the template sections and heightmaps, yielding near-O(1) per-chunk
/// generation.
///
/// [`generate_chunk`]: FlatChunkGenerator::generate_chunk
///
/// # Examples
///
/// ```
/// use oxidized_game::worldgen::flat::{FlatChunkGenerator, FlatWorldConfig};
/// use oxidized_game::worldgen::ChunkGenerator;
/// use oxidized_world::chunk::ChunkPos;
///
/// let generator = FlatChunkGenerator::new(FlatWorldConfig::default());
/// let chunk = generator.generate_chunk(ChunkPos::new(0, 0));
/// // Default flat world: 4 layers (bedrock + 2 dirt + grass)
/// // Surface at y = -61, so heightmaps report -60 (first air block above)
/// ```
#[derive(Debug)]
pub struct FlatChunkGenerator {
    config: FlatWorldConfig,
    template: ChunkTemplate,
}

impl FlatChunkGenerator {
    /// Creates a new flat chunk generator with the given configuration.
    ///
    /// Pre-computes the template sections and heightmaps so that
    /// [`generate_chunk`](ChunkGenerator::generate_chunk) is a fast clone.
    #[must_use]
    pub fn new(config: FlatWorldConfig) -> Self {
        let template = build_template(&config);
        Self { config, template }
    }

    /// Returns a reference to the flat world configuration.
    #[must_use]
    pub fn config(&self) -> &FlatWorldConfig {
        &self.config
    }
}

impl ChunkGenerator for FlatChunkGenerator {
    fn generate_chunk(&self, pos: ChunkPos) -> LevelChunk {
        let mut chunk = LevelChunk::with_dimensions(pos, OVERWORLD_MIN_Y, OVERWORLD_SECTION_COUNT);

        // Clone template sections into the chunk.
        for (i, template_section) in self.template.sections.iter().enumerate() {
            if let Some(section) = chunk.section_mut(i) {
                *section = template_section.clone();
            }
        }

        // Clone template heightmaps.
        for hm in &self.template.heightmaps {
            chunk.set_heightmap(hm.clone());
        }

        // Clone template sky light.
        for (i, light) in self.template.sky_light.iter().enumerate() {
            if let Some(data) = light {
                chunk.set_sky_light(i, data.clone());
            }
        }

        chunk
    }

    fn find_spawn_y(&self) -> i32 {
        // One block above the topmost layer, clamped to world bounds.
        let raw = OVERWORLD_MIN_Y + self.config.total_height() as i32;
        raw.min(OVERWORLD_MIN_Y + OVERWORLD_HEIGHT as i32 - 1)
    }

    fn generator_type(&self) -> &'static str {
        "minecraft:flat"
    }

    fn sea_level(&self) -> i32 {
        FLAT_SEA_LEVEL
    }

    fn min_y(&self) -> i32 {
        OVERWORLD_MIN_Y
    }

    fn world_height(&self) -> u32 {
        OVERWORLD_HEIGHT
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use oxidized_world::chunk::heightmap::HeightmapType;
    use oxidized_world::registry::{BEDROCK, DIRT, GRASS_BLOCK, SAND, STONE};

    use super::*;

    fn default_generator() -> FlatChunkGenerator {
        FlatChunkGenerator::new(FlatWorldConfig::default())
    }

    #[test]
    fn generate_chunk_status_is_usable() {
        let generator = default_generator();
        let chunk = generator.generate_chunk(ChunkPos::new(0, 0));
        assert!(chunk.heightmap(HeightmapType::MotionBlocking).is_some());
        assert!(chunk.heightmap(HeightmapType::WorldSurface).is_some());
    }

    #[test]
    fn generate_chunk_bedrock_at_bottom() {
        let generator = default_generator();
        let chunk = generator.generate_chunk(ChunkPos::new(0, 0));
        assert_eq!(
            chunk.get_block_state(8, OVERWORLD_MIN_Y, 8).unwrap(),
            u32::from(BEDROCK.0)
        );
    }

    #[test]
    fn generate_chunk_dirt_layers() {
        let generator = default_generator();
        let chunk = generator.generate_chunk(ChunkPos::new(0, 0));
        assert_eq!(
            chunk.get_block_state(8, OVERWORLD_MIN_Y + 1, 8).unwrap(),
            u32::from(DIRT.0)
        );
        assert_eq!(
            chunk.get_block_state(8, OVERWORLD_MIN_Y + 2, 8).unwrap(),
            u32::from(DIRT.0)
        );
    }

    #[test]
    fn generate_chunk_grass_at_surface() {
        let generator = default_generator();
        let chunk = generator.generate_chunk(ChunkPos::new(0, 0));
        assert_eq!(
            chunk.get_block_state(8, OVERWORLD_MIN_Y + 3, 8).unwrap(),
            u32::from(GRASS_BLOCK.0)
        );
    }

    #[test]
    fn generate_chunk_air_above_surface() {
        let generator = default_generator();
        let chunk = generator.generate_chunk(ChunkPos::new(0, 0));
        assert_eq!(
            chunk.get_block_state(0, OVERWORLD_MIN_Y + 4, 0).unwrap(),
            0 // AIR
        );
    }

    #[test]
    fn generate_chunk_all_columns_uniform() {
        let generator = default_generator();
        let chunk = generator.generate_chunk(ChunkPos::new(5, -3));
        let surface_y = OVERWORLD_MIN_Y + 3;
        let grass_id = u32::from(GRASS_BLOCK.0);
        for x in 0..16i32 {
            for z in 0..16i32 {
                assert_eq!(
                    chunk.get_block_state(x, surface_y, z).unwrap(),
                    grass_id,
                    "column ({x},{z}) should have grass at y={surface_y}"
                );
            }
        }
    }

    #[test]
    fn generate_chunk_heightmap_values() {
        let generator = default_generator();
        let chunk = generator.generate_chunk(ChunkPos::new(0, 0));
        let expected_height = 4u32;
        let hm = chunk
            .heightmap(HeightmapType::MotionBlocking)
            .expect("should have MOTION_BLOCKING");
        for x in 0..16usize {
            for z in 0..16usize {
                assert_eq!(
                    hm.get(x, z).unwrap(),
                    expected_height,
                    "column ({x},{z}) heightmap mismatch"
                );
            }
        }
    }

    #[test]
    fn find_spawn_y_above_surface() {
        let generator = default_generator();
        let spawn_y = generator.find_spawn_y();
        assert_eq!(spawn_y, OVERWORLD_MIN_Y + 4);
    }

    #[test]
    fn generator_type_is_flat() {
        let generator = default_generator();
        assert_eq!(generator.generator_type(), "minecraft:flat");
    }

    #[test]
    fn sea_level_is_minus_63() {
        let generator = default_generator();
        assert_eq!(generator.sea_level(), -63);
    }

    #[test]
    fn min_y_and_world_height() {
        let generator = default_generator();
        assert_eq!(generator.min_y(), OVERWORLD_MIN_Y);
        assert_eq!(generator.world_height(), OVERWORLD_HEIGHT);
    }

    #[test]
    fn generate_different_positions_identical() {
        let generator = default_generator();
        let c1 = generator.generate_chunk(ChunkPos::new(0, 0));
        let c2 = generator.generate_chunk(ChunkPos::new(100, -50));
        for y in OVERWORLD_MIN_Y..OVERWORLD_MIN_Y + 4 {
            assert_eq!(
                c1.get_block_state(0, y, 0).unwrap(),
                c2.get_block_state(0, y, 0).unwrap(),
                "blocks differ at y={y}"
            );
        }
    }

    #[test]
    fn custom_config_generates_correctly() {
        let config = FlatWorldConfig::from_layers(&[(STONE, 10), (SAND, 3)]);
        let generator = FlatChunkGenerator::new(config);
        let chunk = generator.generate_chunk(ChunkPos::new(0, 0));

        // Stone at bottom 10 layers.
        assert_eq!(
            chunk.get_block_state(0, OVERWORLD_MIN_Y, 0).unwrap(),
            u32::from(STONE.0)
        );
        assert_eq!(
            chunk.get_block_state(0, OVERWORLD_MIN_Y + 9, 0).unwrap(),
            u32::from(STONE.0)
        );
        // Sand at layers 10-12.
        assert_eq!(
            chunk.get_block_state(0, OVERWORLD_MIN_Y + 10, 0).unwrap(),
            u32::from(SAND.0)
        );
        assert_eq!(
            chunk.get_block_state(0, OVERWORLD_MIN_Y + 12, 0).unwrap(),
            u32::from(SAND.0)
        );
        // Air above.
        assert_eq!(
            chunk.get_block_state(0, OVERWORLD_MIN_Y + 13, 0).unwrap(),
            0
        );
    }

    #[test]
    fn biomes_set_to_plains() {
        let generator = default_generator();
        let chunk = generator.generate_chunk(ChunkPos::new(0, 0));
        // Check a section that has blocks and one that's all air.
        for section_idx in [0, 12, 23] {
            let section = chunk.section(section_idx).unwrap();
            for bx in 0..4usize {
                for bz in 0..4usize {
                    assert_eq!(
                        section.get_biome(bx, 0, bz).unwrap(),
                        PLAINS_BIOME_ID,
                        "section {section_idx} biome ({bx},0,{bz}) should be plains"
                    );
                }
            }
        }
    }

    #[test]
    fn template_reuse_is_consistent() {
        let generator = default_generator();
        let c1 = generator.generate_chunk(ChunkPos::new(0, 0));
        let c2 = generator.generate_chunk(ChunkPos::new(1, 1));
        // Both chunks should have identical block data.
        for y in OVERWORLD_MIN_Y..OVERWORLD_MIN_Y + 4 {
            for x in 0..16i32 {
                assert_eq!(
                    c1.get_block_state(x, y, 0).unwrap(),
                    c2.get_block_state(x, y, 0).unwrap(),
                );
            }
        }
        // But different positions.
        assert_eq!(c1.pos, ChunkPos::new(0, 0));
        assert_eq!(c2.pos, ChunkPos::new(1, 1));
    }

    #[test]
    fn uniform_section_uses_filled_optimization() {
        // 16 blocks of stone = exactly one full section of the same block.
        let config = FlatWorldConfig::from_layers(&[(STONE, 16)]);
        let generator = FlatChunkGenerator::new(config);
        let chunk = generator.generate_chunk(ChunkPos::new(0, 0));
        let section = chunk.section(0).unwrap();
        assert_eq!(section.non_empty_block_count(), 4096);
        assert_eq!(
            section.get_block_state(0, 0, 0).unwrap(),
            u32::from(STONE.0)
        );
        assert_eq!(
            section.get_block_state(15, 15, 15).unwrap(),
            u32::from(STONE.0)
        );
    }
}
