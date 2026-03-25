//! Cross-chunk light propagation.
//!
//! When BFS reaches x=0/15 or z=0/15 within a chunk, the light must continue
//! into the neighboring chunk. This module handles that boundary propagation.

use std::collections::VecDeque;

use oxidized_world::chunk::LevelChunk;

use super::propagation::{
    BoundaryEntry, LightEntry, propagate_block_light_increase, propagate_sky_light_increase,
};

/// Accessor for the 4 horizontal neighbors of a chunk.
pub struct ChunkNeighbors<'a> {
    /// -Z neighbor.
    pub north: Option<&'a mut LevelChunk>,
    /// +Z neighbor.
    pub south: Option<&'a mut LevelChunk>,
    /// +X neighbor.
    pub east: Option<&'a mut LevelChunk>,
    /// -X neighbor.
    pub west: Option<&'a mut LevelChunk>,
}

/// Propagates block light boundary entries into neighboring chunks.
///
/// For each boundary entry, determines which neighbor chunk owns that
/// position, converts to chunk-local coordinates, and runs BFS increase.
pub fn propagate_block_light_cross_chunk(
    neighbors: &mut ChunkNeighbors<'_>,
    boundary_entries: &[BoundaryEntry],
) {
    for entry in boundary_entries {
        propagate_boundary_block_light(neighbors, entry);
    }
}

/// Propagates sky light boundary entries into neighboring chunks.
pub fn propagate_sky_light_cross_chunk(
    neighbors: &mut ChunkNeighbors<'_>,
    boundary_entries: &[BoundaryEntry],
) {
    for entry in boundary_entries {
        propagate_boundary_sky_light(neighbors, entry);
    }
}

fn propagate_boundary_block_light(neighbors: &mut ChunkNeighbors<'_>, entry: &BoundaryEntry) {
    let (chunk, local_x, local_z, base_x, base_z) =
        match resolve_neighbor(neighbors, entry.world_x, entry.world_z) {
            Some(v) => v,
            None => return,
        };

    let current = chunk.get_block_light_at(local_x, entry.world_y, local_z);
    if entry.level > current {
        chunk.set_block_light_at(local_x, entry.world_y, local_z, entry.level);
        let mut queue = VecDeque::new();
        queue.push_back(LightEntry {
            x: local_x,
            y: entry.world_y,
            z: local_z,
            level: entry.level,
        });
        let _further = propagate_block_light_increase(chunk, &mut queue, base_x, base_z);
        // Further cross-chunk propagation is not pursued recursively;
        // it will be handled in the next tick or full-lighting pass.
    }
}

fn propagate_boundary_sky_light(neighbors: &mut ChunkNeighbors<'_>, entry: &BoundaryEntry) {
    let (chunk, local_x, local_z, base_x, base_z) =
        match resolve_neighbor(neighbors, entry.world_x, entry.world_z) {
            Some(v) => v,
            None => return,
        };

    let current = chunk.get_sky_light_at(local_x, entry.world_y, local_z);
    if entry.level > current {
        chunk.set_sky_light_at(local_x, entry.world_y, local_z, entry.level);
        let mut queue = VecDeque::new();
        queue.push_back(LightEntry {
            x: local_x,
            y: entry.world_y,
            z: local_z,
            level: entry.level,
        });
        let _further = propagate_sky_light_increase(chunk, &mut queue, base_x, base_z);
    }
}

/// Resolves which neighbor chunk owns a world position and returns
/// (chunk_ref, local_x, local_z, chunk_base_x, chunk_base_z).
fn resolve_neighbor<'a>(
    neighbors: &'a mut ChunkNeighbors<'_>,
    world_x: i32,
    world_z: i32,
) -> Option<(&'a mut LevelChunk, i32, i32, i32, i32)> {
    // Determine which neighbor based on which coordinate is out of bounds.
    // Note: for corner cases (both x and z out of range), we skip (diagonal
    // neighbors are not directly accessible in this model).
    let chunk_x = world_x.div_euclid(16);
    let chunk_z = world_z.div_euclid(16);
    let local_x = world_x.rem_euclid(16);
    let local_z = world_z.rem_euclid(16);

    // We need to figure out which neighbor this maps to based on relative position.
    // The center chunk is at (0, 0) in relative terms; neighbors differ by ±1.
    let chunk = match (chunk_x, chunk_z) {
        (1, 0) => neighbors.east.as_deref_mut()?,
        (-1, 0) => neighbors.west.as_deref_mut()?,
        (0, 1) => neighbors.south.as_deref_mut()?,
        (0, -1) => neighbors.north.as_deref_mut()?,
        _ => return None, // Diagonal or same chunk — skip.
    };

    let base_x = chunk_x * 16;
    let base_z = chunk_z * 16;
    Some((chunk, local_x, local_z, base_x, base_z))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use oxidized_world::chunk::{ChunkPos, LevelChunk};

    #[test]
    fn test_missing_neighbor_no_panic() {
        let mut neighbors = ChunkNeighbors {
            north: None,
            south: None,
            east: None,
            west: None,
        };
        let entries = vec![BoundaryEntry {
            world_x: -1,
            world_y: 64,
            world_z: 8,
            level: 13,
        }];
        // Should not panic.
        propagate_block_light_cross_chunk(&mut neighbors, &entries);
    }

    #[test]
    fn test_boundary_propagation_into_east_neighbor() {
        let mut east = LevelChunk::new(ChunkPos::new(1, 0));
        let entries = vec![BoundaryEntry {
            world_x: 16, // x=16 → chunk (1,0), local x=0
            world_y: 64,
            world_z: 8,
            level: 12,
        }];

        let mut neighbors = ChunkNeighbors {
            north: None,
            south: None,
            east: Some(&mut east),
            west: None,
        };
        propagate_block_light_cross_chunk(&mut neighbors, &entries);

        assert_eq!(east.get_block_light_at(0, 64, 8), 12);
        // Should propagate further into the east chunk.
        assert_eq!(east.get_block_light_at(1, 64, 8), 11);
    }
}
