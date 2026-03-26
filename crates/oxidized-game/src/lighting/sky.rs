//! Sky light initialization for newly generated chunks.
//!
//! Computes sky light in two phases:
//! 1. **Vertical pass** — for each (x, z) column, sets sky light to 15 at and
//!    above the sky light source height (from [`ChunkSkyLightSources`]), then
//!    attenuates downward by each block's opacity.
//! 2. **Horizontal BFS** — propagates sky light sideways through transparent
//!    blocks below the source height (caves, overhangs).

use std::collections::VecDeque;

use oxidized_world::chunk::LevelChunk;
use oxidized_world::chunk::sky_light_sources::ChunkSkyLightSources;
use oxidized_world::registry::BlockStateId;

use super::propagation::{BoundaryEntry, LightEntry, propagate_sky_light_increase};

/// Initializes sky light for a newly generated chunk.
///
/// Builds [`ChunkSkyLightSources`] for per-column source tracking, then runs
/// the vertical pass and horizontal BFS. After this call, the chunk's sky
/// light layers are fully populated and the sources are stored in the chunk.
///
/// Returns boundary entries for positions where sky light reached a chunk
/// edge and needs to propagate into a neighbor chunk.
pub fn initialize_sky_light(chunk: &mut LevelChunk) -> Vec<BoundaryEntry> {
    let sources = ChunkSkyLightSources::from_chunk(chunk);
    let min_y = chunk.min_y();
    let max_y = chunk.max_y();

    // Phase 1: Vertical pass — top-down per (x, z) column.
    let mut bfs_seeds = VecDeque::new();

    for x in 0..16i32 {
        for z in 0..16i32 {
            let source_y = sources.get_lowest_source_y(x as usize, z as usize);
            let mut level: u8 = 15;

            // Iterate from top of world downward.
            for y in (min_y..max_y).rev() {
                if y >= source_y {
                    // At or above the source height: full sky brightness.
                    chunk.set_sky_light_at(x, y, z, 15);
                } else {
                    // Below the source: attenuate by opacity.
                    let state_id = chunk.get_block_state(x & 15, y, z & 15).unwrap_or(0);
                    #[allow(clippy::cast_possible_truncation)]
                    let opacity = BlockStateId(state_id as u16).light_opacity();
                    level = level.saturating_sub(opacity.max(1));
                    chunk.set_sky_light_at(x, y, z, level);
                    if level == 0 {
                        break;
                    }
                }
            }

            // Seed horizontal BFS from blocks just below the source where
            // sky light is still > 1 (light can spread sideways into caves).
            let seed_start = source_y.min(max_y).saturating_sub(1);
            if seed_start >= min_y {
                for y in (min_y..=seed_start).rev() {
                    let sky = chunk.get_sky_light_at(x, y, z);
                    if sky > 1 {
                        bfs_seeds.push_back(LightEntry {
                            x,
                            y,
                            z,
                            level: sky,
                        });
                    }
                    if sky == 0 {
                        break;
                    }
                }
            }
        }
    }

    // Store the sources in the chunk for incremental updates.
    chunk.set_sky_light_sources(sources);

    // Phase 2: Horizontal BFS for sky light bleeding into caves.
    let chunk_base_x = chunk.pos.x * 16;
    let chunk_base_z = chunk.pos.z * 16;
    propagate_sky_light_increase(chunk, &mut bfs_seeds, chunk_base_x, chunk_base_z)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use oxidized_world::chunk::level_chunk::OVERWORLD_MIN_Y;
    use oxidized_world::chunk::{ChunkPos, LevelChunk};
    use oxidized_world::registry::{BEDROCK, DIRT, GRASS_BLOCK};

    /// Creates a standard flat world chunk: bedrock, 2 dirt, grass, air above.
    fn flat_chunk() -> LevelChunk {
        let mut chunk = LevelChunk::new(ChunkPos::new(0, 0));
        let bedrock = u32::from(BEDROCK.0);
        let dirt = u32::from(DIRT.0);
        let grass = u32::from(GRASS_BLOCK.0);

        for x in 0..16i32 {
            for z in 0..16i32 {
                chunk
                    .set_block_state(x, OVERWORLD_MIN_Y, z, bedrock)
                    .unwrap();
                chunk
                    .set_block_state(x, OVERWORLD_MIN_Y + 1, z, dirt)
                    .unwrap();
                chunk
                    .set_block_state(x, OVERWORLD_MIN_Y + 2, z, dirt)
                    .unwrap();
                chunk
                    .set_block_state(x, OVERWORLD_MIN_Y + 3, z, grass)
                    .unwrap();
            }
        }

        chunk
    }

    #[test]
    fn test_flat_world_sky_light_above_surface() {
        let mut chunk = flat_chunk();
        initialize_sky_light(&mut chunk);

        // One block above surface (y = -60).
        assert_eq!(chunk.get_sky_light_at(8, OVERWORLD_MIN_Y + 4, 8), 15);
        // High up (y = 100).
        assert_eq!(chunk.get_sky_light_at(8, 100, 8), 15);
    }

    #[test]
    fn test_flat_world_sky_light_at_surface() {
        let mut chunk = flat_chunk();
        initialize_sky_light(&mut chunk);

        // The grass block at y=-61 is opaque — sky light should be 0 inside it.
        assert_eq!(chunk.get_sky_light_at(8, OVERWORLD_MIN_Y + 3, 8), 0);
    }

    #[test]
    fn test_flat_world_sky_light_all_zero_below_solid() {
        let mut chunk = flat_chunk();
        initialize_sky_light(&mut chunk);

        // Below the surface layer, everything is opaque. Light should be 0.
        assert_eq!(chunk.get_sky_light_at(8, OVERWORLD_MIN_Y + 2, 8), 0);
        assert_eq!(chunk.get_sky_light_at(8, OVERWORLD_MIN_Y + 1, 8), 0);
        assert_eq!(chunk.get_sky_light_at(8, OVERWORLD_MIN_Y, 8), 0);
    }

    #[test]
    fn test_shaft_propagation() {
        let mut chunk = flat_chunk();
        // Dig a 1x1 shaft through all 4 solid layers at (8, 8).
        for y in OVERWORLD_MIN_Y..OVERWORLD_MIN_Y + 4 {
            chunk.set_block_state(8, y, 8, 0).unwrap(); // air
        }
        initialize_sky_light(&mut chunk);

        // Sky light should propagate down the shaft — sources detect the
        // all-air column and set source Y to world bottom.
        assert_eq!(chunk.get_sky_light_at(8, OVERWORLD_MIN_Y + 3, 8), 15);
        assert_eq!(chunk.get_sky_light_at(8, OVERWORLD_MIN_Y + 2, 8), 15);
        assert_eq!(chunk.get_sky_light_at(8, OVERWORLD_MIN_Y + 1, 8), 15);
        assert_eq!(chunk.get_sky_light_at(8, OVERWORLD_MIN_Y, 8), 15);
    }

    #[test]
    fn test_sources_stored_in_chunk_after_init() {
        let mut chunk = flat_chunk();
        initialize_sky_light(&mut chunk);

        let sources = chunk.sky_light_sources().expect("sources should be set");
        assert_eq!(sources.get_lowest_source_y(8, 8), OVERWORLD_MIN_Y + 4);
    }
}
