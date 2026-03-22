//! Flat chunk generator.
//!
//! Produces [`LevelChunk`] instances filled with the layers defined by
//! a [`FlatWorldConfig`]. Every generated chunk is identical — block
//! layers, biomes, and heightmaps are uniform across all positions.

use oxidized_world::chunk::heightmap::{Heightmap, HeightmapType};
use oxidized_world::chunk::level_chunk::{OVERWORLD_HEIGHT, OVERWORLD_MIN_Y};
use oxidized_world::chunk::{ChunkPos, LevelChunk};

use super::config::FlatWorldConfig;
use crate::worldgen::ChunkGenerator;

/// A chunk generator for flat (superflat) worlds.
///
/// Fills every chunk with the same layer configuration and computes
/// the correct heightmaps for client rendering.
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
pub struct FlatChunkGenerator {
    config: FlatWorldConfig,
}

impl FlatChunkGenerator {
    /// Creates a new flat chunk generator with the given configuration.
    #[must_use]
    pub fn new(config: FlatWorldConfig) -> Self {
        Self { config }
    }

    /// Returns a reference to the flat world configuration.
    #[must_use]
    pub fn config(&self) -> &FlatWorldConfig {
        &self.config
    }
}

impl ChunkGenerator for FlatChunkGenerator {
    fn generate_chunk(&self, pos: ChunkPos) -> LevelChunk {
        let mut chunk = LevelChunk::new(pos);

        // Fill blocks layer by layer.
        let mut y = OVERWORLD_MIN_Y;
        for layer in &self.config.layers {
            let state_id = u32::from(layer.block.0);
            for _ in 0..layer.height {
                if y >= OVERWORLD_MIN_Y + (chunk.section_count() as i32 * 16) {
                    break;
                }
                for x in 0..16i32 {
                    for z in 0..16i32 {
                        // Infallible: y is within bounds by construction.
                        let _ = chunk.set_block_state(x, y, z, state_id);
                    }
                }
                y += 1;
            }
        }

        // Compute heightmaps.
        // For flat worlds, every column has the same surface height.
        let surface_y = OVERWORLD_MIN_Y + self.config.total_height() as i32;
        // Heightmap values are stored as the Y of the first air block above the surface,
        // offset from min_y. The value stored = (surface_y - min_y).
        let height_value = surface_y - OVERWORLD_MIN_Y;

        for htype in [
            HeightmapType::MotionBlocking,
            HeightmapType::WorldSurface,
            HeightmapType::MotionBlockingNoLeaves,
        ] {
            if let Ok(mut hm) = Heightmap::new(htype, OVERWORLD_HEIGHT) {
                for x in 0..16usize {
                    for z in 0..16usize {
                        let _ = hm.set(x, z, height_value as u32);
                    }
                }
                chunk.set_heightmap(hm);
            }
        }

        chunk
    }

    fn find_spawn_y(&self) -> i32 {
        // One block above the topmost layer.
        OVERWORLD_MIN_Y + self.config.total_height() as i32
    }

    fn generator_type(&self) -> &'static str {
        "minecraft:flat"
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
        // Chunk should have heightmaps and valid sections.
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
        let expected_height = 4u32; // 4 blocks above min_y
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
        // Default: 4 layers starting at -64, so spawn at -60.
        assert_eq!(spawn_y, OVERWORLD_MIN_Y + 4);
    }

    #[test]
    fn generator_type_is_flat() {
        let generator = default_generator();
        assert_eq!(generator.generator_type(), "minecraft:flat");
    }

    #[test]
    fn generate_different_positions_identical() {
        let generator = default_generator();
        let c1 = generator.generate_chunk(ChunkPos::new(0, 0));
        let c2 = generator.generate_chunk(ChunkPos::new(100, -50));
        // Both chunks should have the same blocks at the same Y.
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
        let config = FlatWorldConfig {
            layers: vec![
                super::super::config::FlatLayerInfo {
                    block: STONE,
                    height: 10,
                },
                super::super::config::FlatLayerInfo {
                    block: SAND,
                    height: 3,
                },
            ],
            biome: "minecraft:desert".into(),
            features: false,
            lakes: false,
        };
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
}
