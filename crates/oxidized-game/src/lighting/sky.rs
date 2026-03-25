//! Sky light initialization for newly generated chunks.
//!
//! Computes sky light in two phases:
//! 1. **Vertical pass** — for each (x, z) column, sets sky light to 15 above
//!    the heightmap, then attenuates downward by each block's opacity.
//! 2. **Horizontal BFS** — propagates sky light sideways through transparent
//!    blocks below the heightmap (caves, overhangs).

use std::collections::VecDeque;

use oxidized_world::chunk::LevelChunk;
use oxidized_world::chunk::heightmap::HeightmapType;
use oxidized_world::registry::BlockStateId;

use super::propagation::{LightEntry, propagate_sky_light_increase};

/// Initializes sky light for a newly generated chunk.
///
/// Clears any existing sky light, runs the vertical pass, then the
/// horizontal BFS. After this call, the chunk's sky light layers are
/// fully populated.
pub fn initialize_sky_light(chunk: &mut LevelChunk) {
    let min_y = chunk.min_y();
    let max_y = chunk.max_y();

    // Phase 1: Vertical pass — top-down per (x, z) column.
    let mut bfs_seeds = VecDeque::new();

    for x in 0..16i32 {
        for z in 0..16i32 {
            let surface_y = get_surface_y(chunk, x as usize, z as usize, min_y);
            let mut level: u8 = 15;

            // Iterate from top of world downward.
            for y in (min_y..max_y).rev() {
                if y >= surface_y {
                    // Above or at the heightmap surface: full sky brightness.
                    chunk.set_sky_light_at(x, y, z, 15);
                } else {
                    // Below the surface: attenuate by opacity.
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

            // Seed horizontal BFS from blocks just below the surface where
            // sky light is still > 1 (light can spread sideways into caves).
            if surface_y > min_y {
                let start_y = surface_y - 1;
                for y in (min_y..=start_y).rev() {
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

    // Phase 2: Horizontal BFS for sky light bleeding into caves.
    let _boundary = propagate_sky_light_increase(chunk, &mut bfs_seeds, 0, 0);
}

/// Returns the highest Y coordinate at which blocks exist in this column,
/// using the MOTION_BLOCKING heightmap if available, otherwise scanning.
fn get_surface_y(chunk: &LevelChunk, x: usize, z: usize, min_y: i32) -> i32 {
    if let Some(hm) = chunk.heightmap(HeightmapType::MotionBlocking) {
        if let Ok(h) = hm.get(x, z) {
            // Heightmap value is the number of blocks above min_y,
            // so the surface is at min_y + h (first air block).
            return min_y + h as i32;
        }
    }
    // Fallback: scan from the top.
    let max_y = chunk.max_y();
    for y in (min_y..max_y).rev() {
        let state = chunk.get_block_state(x as i32, y, z as i32).unwrap_or(0);
        if state != 0 {
            return y + 1;
        }
    }
    min_y
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

        // Set heightmap.
        use oxidized_world::chunk::heightmap::{Heightmap, HeightmapType};
        use oxidized_world::chunk::level_chunk::OVERWORLD_HEIGHT;
        let mut hm = Heightmap::new(HeightmapType::MotionBlocking, OVERWORLD_HEIGHT).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                hm.set(x, z, 4).unwrap(); // 4 blocks above min_y
            }
        }
        chunk.set_heightmap(hm);
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
        // Update heightmap for the shaft column.
        if let Some(hm) = chunk.heightmap_mut(HeightmapType::MotionBlocking) {
            hm.set(8, 8, 0).unwrap(); // no solid blocks in this column
        }
        initialize_sky_light(&mut chunk);

        // Sky light should propagate down the shaft.
        assert_eq!(chunk.get_sky_light_at(8, OVERWORLD_MIN_Y + 3, 8), 15);
        assert_eq!(chunk.get_sky_light_at(8, OVERWORLD_MIN_Y + 2, 8), 15);
        assert_eq!(chunk.get_sky_light_at(8, OVERWORLD_MIN_Y + 1, 8), 15);
        assert_eq!(chunk.get_sky_light_at(8, OVERWORLD_MIN_Y, 8), 15);
    }
}
