//! Core BFS propagation algorithms for light increase and decrease.
//!
//! These functions implement the two-pass BFS described in ADR-017:
//! 1. **Decrease pass** — removes light from positions where the source was
//!    removed or weakened, re-seeding the increase queue for neighbors with
//!    independent light sources.
//! 2. **Increase pass** — propagates light outward from sources, attenuating
//!    by each neighbor's opacity.
//!
//! Both passes work on a single chunk column using world-Y coordinates.
//! Cross-chunk boundary entries are collected for later propagation.

use std::collections::VecDeque;

use oxidized_protocol::types::Direction;
use oxidized_world::chunk::LevelChunk;
use oxidized_world::registry::BlockStateId;

use super::occlusion::get_light_block_into;

/// Six cardinal directions as (dx, dy, dz, Direction) tuples.
pub(crate) const DIRECTIONS: [(i32, i32, i32, Direction); 6] = [
    (1, 0, 0, Direction::East),
    (-1, 0, 0, Direction::West),
    (0, 1, 0, Direction::Up),
    (0, -1, 0, Direction::Down),
    (0, 0, 1, Direction::South),
    (0, 0, -1, Direction::North),
];

/// Entry in the BFS increase queue.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LightEntry {
    /// Chunk-local X (0–15).
    pub x: i32,
    /// World Y coordinate.
    pub y: i32,
    /// Chunk-local Z (0–15).
    pub z: i32,
    /// Light level to propagate from this position.
    pub level: u8,
}

/// Entry in the BFS decrease queue.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DecreaseEntry {
    /// Chunk-local X (0–15).
    pub x: i32,
    /// World Y coordinate.
    pub y: i32,
    /// Chunk-local Z (0–15).
    pub z: i32,
    /// The light level that was at this position before removal.
    pub old_level: u8,
}

/// A cross-boundary entry produced when BFS reaches the edge of a chunk.
#[derive(Debug, Clone, Copy)]
pub struct BoundaryEntry {
    /// World X of the boundary block (outside this chunk).
    pub world_x: i32,
    /// World Y of the boundary block.
    pub world_y: i32,
    /// World Z of the boundary block (outside this chunk).
    pub world_z: i32,
    /// Light level to propagate into the neighbor.
    pub level: u8,
}

/// Returns the effective light attenuation between a source block and its neighbor.
///
/// Uses face-occlusion-aware logic: if either the source's exit face or the
/// target's entry face fully covers the boundary, returns 16 (fully blocked).
/// Otherwise returns the target's scalar light opacity.
fn get_effective_opacity(
    chunk: &LevelChunk,
    from: BlockStateId,
    nx: i32,
    ny: i32,
    nz: i32,
    dir: Direction,
) -> u8 {
    let state_id = chunk.get_block_state(nx & 15, ny, nz & 15).unwrap_or(0);
    #[allow(clippy::cast_possible_truncation)]
    let to = BlockStateId(state_id as u16);
    get_light_block_into(from, to, dir)
}

/// Returns the block state at the given chunk-local position.
fn get_block_state_id(chunk: &LevelChunk, x: i32, y: i32, z: i32) -> BlockStateId {
    let state_id = chunk.get_block_state(x & 15, y, z & 15).unwrap_or(0);
    #[allow(clippy::cast_possible_truncation)]
    BlockStateId(state_id as u16)
}

/// Checks whether a position is within the chunk column's Y bounds.
fn in_y_bounds(chunk: &LevelChunk, y: i32) -> bool {
    y >= chunk.min_y() && y < chunk.max_y()
}

/// BFS increase pass for block light within a single chunk.
///
/// For each entry, checks all 6 neighbors. If the neighbor's current light
/// level is less than `(entry level - max(1, neighbor opacity))`, sets the
/// neighbor to that value and enqueues it.
///
/// Returns boundary entries for positions that fall outside the chunk column.
pub(crate) fn propagate_block_light_increase(
    chunk: &mut LevelChunk,
    queue: &mut VecDeque<LightEntry>,
    chunk_base_x: i32,
    chunk_base_z: i32,
) -> Vec<BoundaryEntry> {
    let mut boundary = Vec::new();

    while let Some(entry) = queue.pop_front() {
        if entry.level <= 1 {
            continue;
        }

        let from_state = get_block_state_id(chunk, entry.x, entry.y, entry.z);

        for &(dx, dy, dz, dir) in &DIRECTIONS {
            let nx = entry.x + dx;
            let ny = entry.y + dy;
            let nz = entry.z + dz;

            // Check for cross-chunk boundary. Pass source level un-attenuated;
            // the cross-chunk code reads the target block's actual opacity.
            if !(0..16).contains(&nx) || !(0..16).contains(&nz) {
                boundary.push(BoundaryEntry {
                    world_x: chunk_base_x + nx,
                    world_y: ny,
                    world_z: chunk_base_z + nz,
                    level: entry.level,
                });
                continue;
            }

            if !in_y_bounds(chunk, ny) {
                continue;
            }

            let opacity = get_effective_opacity(chunk, from_state, nx, ny, nz, dir);
            if opacity >= 16 {
                continue;
            }
            let new_level = entry.level.saturating_sub(opacity.max(1));
            if new_level == 0 {
                continue;
            }

            let current = chunk.get_block_light_at(nx, ny, nz);
            if new_level > current {
                chunk.set_block_light_at(nx, ny, nz, new_level);
                queue.push_back(LightEntry {
                    x: nx,
                    y: ny,
                    z: nz,
                    level: new_level,
                });
            }
        }
    }

    boundary
}

/// BFS decrease pass for block light within a single chunk.
///
/// For each entry, checks all 6 neighbors. If the neighbor's light was
/// dependent on the removed source (level <= old_level - 1), clears it
/// and enqueues for further decrease. If the neighbor has an independent
/// brighter source, enqueues it on the increase queue for re-propagation.
///
/// Returns boundary entries for positions that need cross-chunk decrease.
pub(crate) fn propagate_block_light_decrease(
    chunk: &mut LevelChunk,
    decrease_queue: &mut VecDeque<DecreaseEntry>,
    increase_queue: &mut VecDeque<LightEntry>,
    chunk_base_x: i32,
    chunk_base_z: i32,
) -> Vec<BoundaryEntry> {
    let mut boundary = Vec::new();

    while let Some(entry) = decrease_queue.pop_front() {
        for &(dx, dy, dz, _dir) in &DIRECTIONS {
            let nx = entry.x + dx;
            let ny = entry.y + dy;
            let nz = entry.z + dz;

            if !(0..16).contains(&nx) || !(0..16).contains(&nz) {
                // Boundary decrease — neighbor chunk needs to handle this.
                if entry.old_level > 1 {
                    boundary.push(BoundaryEntry {
                        world_x: chunk_base_x + nx,
                        world_y: ny,
                        world_z: chunk_base_z + nz,
                        level: entry.old_level,
                    });
                }
                continue;
            }

            if !in_y_bounds(chunk, ny) {
                continue;
            }

            let neighbor_level = chunk.get_block_light_at(nx, ny, nz);
            if neighbor_level == 0 {
                continue;
            }

            if neighbor_level < entry.old_level {
                // This neighbor's light was dependent on the removed source.
                let neighbor_state = get_block_state_id(chunk, nx, ny, nz);
                let emission = neighbor_state.light_emission();

                chunk.set_block_light_at(nx, ny, nz, 0);

                // Only continue decrease if the block's own emission didn't
                // account for its light level (vanilla: toEmission < toLevel).
                if emission < neighbor_level {
                    decrease_queue.push_back(DecreaseEntry {
                        x: nx,
                        y: ny,
                        z: nz,
                        old_level: neighbor_level,
                    });
                }

                // Always re-seed emitters so they restore their own light.
                if emission > 0 {
                    increase_queue.push_back(LightEntry {
                        x: nx,
                        y: ny,
                        z: nz,
                        level: emission,
                    });
                }
            } else {
                // Neighbor has an equal or brighter independent source;
                // re-seed the increase queue so it can re-propagate.
                increase_queue.push_back(LightEntry {
                    x: nx,
                    y: ny,
                    z: nz,
                    level: neighbor_level,
                });
            }
        }
    }

    boundary
}

/// BFS increase pass for sky light within a single chunk.
///
/// Same algorithm as block light increase, but operates on the sky light
/// channel.
pub(crate) fn propagate_sky_light_increase(
    chunk: &mut LevelChunk,
    queue: &mut VecDeque<LightEntry>,
    chunk_base_x: i32,
    chunk_base_z: i32,
) -> Vec<BoundaryEntry> {
    let mut boundary = Vec::new();

    while let Some(entry) = queue.pop_front() {
        if entry.level <= 1 {
            continue;
        }

        let from_state = get_block_state_id(chunk, entry.x, entry.y, entry.z);

        for &(dx, dy, dz, dir) in &DIRECTIONS {
            let nx = entry.x + dx;
            let ny = entry.y + dy;
            let nz = entry.z + dz;

            if !(0..16).contains(&nx) || !(0..16).contains(&nz) {
                boundary.push(BoundaryEntry {
                    world_x: chunk_base_x + nx,
                    world_y: ny,
                    world_z: chunk_base_z + nz,
                    level: entry.level,
                });
                continue;
            }

            if !in_y_bounds(chunk, ny) {
                continue;
            }

            let opacity = get_effective_opacity(chunk, from_state, nx, ny, nz, dir);
            if opacity >= 16 {
                continue;
            }
            let new_level = entry.level.saturating_sub(opacity.max(1));
            if new_level == 0 {
                continue;
            }

            let current = chunk.get_sky_light_at(nx, ny, nz);
            if new_level > current {
                chunk.set_sky_light_at(nx, ny, nz, new_level);
                queue.push_back(LightEntry {
                    x: nx,
                    y: ny,
                    z: nz,
                    level: new_level,
                });
            }
        }
    }

    boundary
}

/// BFS decrease pass for sky light within a single chunk.
pub(crate) fn propagate_sky_light_decrease(
    chunk: &mut LevelChunk,
    decrease_queue: &mut VecDeque<DecreaseEntry>,
    increase_queue: &mut VecDeque<LightEntry>,
    chunk_base_x: i32,
    chunk_base_z: i32,
) -> Vec<BoundaryEntry> {
    let mut boundary = Vec::new();

    while let Some(entry) = decrease_queue.pop_front() {
        for &(dx, dy, dz, _dir) in &DIRECTIONS {
            let nx = entry.x + dx;
            let ny = entry.y + dy;
            let nz = entry.z + dz;

            if !(0..16).contains(&nx) || !(0..16).contains(&nz) {
                if entry.old_level > 1 {
                    boundary.push(BoundaryEntry {
                        world_x: chunk_base_x + nx,
                        world_y: ny,
                        world_z: chunk_base_z + nz,
                        level: entry.old_level,
                    });
                }
                continue;
            }

            if !in_y_bounds(chunk, ny) {
                continue;
            }

            let neighbor_level = chunk.get_sky_light_at(nx, ny, nz);
            if neighbor_level == 0 {
                continue;
            }

            if neighbor_level < entry.old_level {
                chunk.set_sky_light_at(nx, ny, nz, 0);
                decrease_queue.push_back(DecreaseEntry {
                    x: nx,
                    y: ny,
                    z: nz,
                    old_level: neighbor_level,
                });
            } else {
                increase_queue.push_back(LightEntry {
                    x: nx,
                    y: ny,
                    z: nz,
                    level: neighbor_level,
                });
            }
        }
    }

    boundary
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use oxidized_world::chunk::{ChunkPos, LevelChunk};
    use oxidized_world::registry::BlockRegistry;

    fn air_chunk() -> LevelChunk {
        LevelChunk::new(ChunkPos::new(0, 0))
    }

    fn stone_id() -> u32 {
        u32::from(
            BlockRegistry
                .default_state("minecraft:stone")
                .expect("stone missing")
                .0,
        )
    }

    #[test]
    fn test_increase_torch_in_air() {
        let mut chunk = air_chunk();
        // Place a "torch" at (8, 64, 8) with emission 14.
        chunk.set_block_light_at(8, 64, 8, 14);
        let mut queue = VecDeque::new();
        queue.push_back(LightEntry {
            x: 8,
            y: 64,
            z: 8,
            level: 14,
        });
        let _boundary = propagate_block_light_increase(&mut chunk, &mut queue, 0, 0);

        // Light at distance 1 should be 13.
        assert_eq!(chunk.get_block_light_at(9, 64, 8), 13);
        assert_eq!(chunk.get_block_light_at(7, 64, 8), 13);
        assert_eq!(chunk.get_block_light_at(8, 65, 8), 13);
        assert_eq!(chunk.get_block_light_at(8, 64, 9), 13);

        // Light at distance 2 should be 12.
        assert_eq!(chunk.get_block_light_at(10, 64, 8), 12);

        // Light at distance 6 should be 8.
        assert_eq!(chunk.get_block_light_at(8, 64, 2), 8);

        // The source itself should still be 14.
        assert_eq!(chunk.get_block_light_at(8, 64, 8), 14);
    }

    #[test]
    fn test_increase_stops_at_distance_14() {
        let mut chunk = air_chunk();
        chunk.set_block_light_at(8, 64, 8, 14);
        let mut queue = VecDeque::new();
        queue.push_back(LightEntry {
            x: 8,
            y: 64,
            z: 8,
            level: 14,
        });
        propagate_block_light_increase(&mut chunk, &mut queue, 0, 0);

        // At distance 14, light should be 0 (14 - 14 = 0, never set).
        // But distance 13 should be 1.
        assert_eq!(chunk.get_block_light_at(8, 64 + 13, 8), 1);
    }

    #[test]
    fn test_stone_blocks_propagation() {
        let mut chunk = air_chunk();
        // Place a full-height stone wall at x=9.
        for y in chunk.min_y()..chunk.max_y() {
            for z in 0..16 {
                chunk.set_block_state(9, y, z, stone_id()).unwrap();
            }
        }
        chunk.set_block_light_at(8, 64, 8, 14);
        let mut queue = VecDeque::new();
        queue.push_back(LightEntry {
            x: 8,
            y: 64,
            z: 8,
            level: 14,
        });
        propagate_block_light_increase(&mut chunk, &mut queue, 0, 0);

        // Light should not pass through stone (opacity=15).
        assert_eq!(chunk.get_block_light_at(10, 64, 8), 0);
    }

    #[test]
    fn test_decrease_removes_light() {
        let mut chunk = air_chunk();
        // Set up light as if a torch was at (8, 64, 8).
        chunk.set_block_light_at(8, 64, 8, 14);
        let mut queue = VecDeque::new();
        queue.push_back(LightEntry {
            x: 8,
            y: 64,
            z: 8,
            level: 14,
        });
        propagate_block_light_increase(&mut chunk, &mut queue, 0, 0);

        // Now remove the torch.
        chunk.set_block_light_at(8, 64, 8, 0);
        let mut decrease_queue = VecDeque::new();
        let mut increase_queue = VecDeque::new();
        decrease_queue.push_back(DecreaseEntry {
            x: 8,
            y: 64,
            z: 8,
            old_level: 14,
        });
        propagate_block_light_decrease(&mut chunk, &mut decrease_queue, &mut increase_queue, 0, 0);
        // Re-propagate any re-seeded entries.
        propagate_block_light_increase(&mut chunk, &mut increase_queue, 0, 0);

        // All light should be gone.
        assert_eq!(chunk.get_block_light_at(8, 64, 8), 0);
        assert_eq!(chunk.get_block_light_at(9, 64, 8), 0);
        assert_eq!(chunk.get_block_light_at(8, 65, 8), 0);
        assert_eq!(chunk.get_block_light_at(7, 64, 7), 0);
    }

    #[test]
    fn test_boundary_entries_generated() {
        let mut chunk = air_chunk();
        // Place light near chunk edge.
        chunk.set_block_light_at(0, 64, 8, 14);
        let mut queue = VecDeque::new();
        queue.push_back(LightEntry {
            x: 0,
            y: 64,
            z: 8,
            level: 14,
        });
        let boundary = propagate_block_light_increase(&mut chunk, &mut queue, 0, 0);

        // Should have entries crossing into x=-1 (neighboring chunk).
        assert!(
            boundary.iter().any(|b| b.world_x < 0),
            "expected boundary entries crossing into negative X"
        );
    }

    #[test]
    fn test_two_sources_take_max() {
        let mut chunk = air_chunk();
        // Two torches.
        chunk.set_block_light_at(5, 64, 8, 14);
        chunk.set_block_light_at(11, 64, 8, 14);
        let mut queue = VecDeque::new();
        queue.push_back(LightEntry {
            x: 5,
            y: 64,
            z: 8,
            level: 14,
        });
        queue.push_back(LightEntry {
            x: 11,
            y: 64,
            z: 8,
            level: 14,
        });
        propagate_block_light_increase(&mut chunk, &mut queue, 0, 0);

        // At the midpoint (8, 64, 8), both torches contribute.
        // Distance from (5,64,8) = 3 → level 11
        // Distance from (11,64,8) = 3 → level 11
        // Both give 11, so max = 11.
        assert_eq!(chunk.get_block_light_at(8, 64, 8), 11);
    }

    #[test]
    fn test_decrease_preserves_nearby_dimmer_emitter() {
        let mut chunk = air_chunk();
        // Simulate glowstone (emission=15) at (8,64,8) and a torch (emission=14)
        // at (5,64,8). We need the torch to be a real emitting block.
        let torch_id = u32::from(
            BlockRegistry
                .default_state("minecraft:torch")
                .expect("torch missing")
                .0,
        );
        chunk.set_block_state(5, 64, 8, torch_id).unwrap();

        // Light both sources via BFS.
        chunk.set_block_light_at(8, 64, 8, 15);
        chunk.set_block_light_at(5, 64, 8, 14);
        let mut queue = VecDeque::new();
        queue.push_back(LightEntry { x: 8, y: 64, z: 8, level: 15 });
        queue.push_back(LightEntry { x: 5, y: 64, z: 8, level: 14 });
        propagate_block_light_increase(&mut chunk, &mut queue, 0, 0);

        // Now remove the glowstone (decrease from 15).
        chunk.set_block_light_at(8, 64, 8, 0);
        let mut decrease_queue = VecDeque::new();
        let mut increase_queue = VecDeque::new();
        decrease_queue.push_back(DecreaseEntry { x: 8, y: 64, z: 8, old_level: 15 });
        propagate_block_light_decrease(
            &mut chunk, &mut decrease_queue, &mut increase_queue, 0, 0,
        );
        propagate_block_light_increase(&mut chunk, &mut increase_queue, 0, 0);

        // The torch at (5,64,8) should still have its own emission level.
        assert_eq!(chunk.get_block_light_at(5, 64, 8), 14);
        // And it should propagate: distance 1 from torch = 13.
        assert_eq!(chunk.get_block_light_at(6, 64, 8), 13);
        // The removed glowstone position should now be lit by the torch.
        // Distance from torch (5) to (8) = 3, so level = 14 - 3 = 11.
        assert_eq!(chunk.get_block_light_at(8, 64, 8), 11);
    }
}
