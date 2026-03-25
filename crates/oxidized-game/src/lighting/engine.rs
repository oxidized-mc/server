//! Lighting engine: batched BFS sky and block light propagation.
//!
//! Processes [`super::queue::LightUpdateQueue`] entries in batched passes,
//! grouping updates by section and propagating cross-section changes.
//! See ADR-017 for the algorithm design.

use std::collections::VecDeque;

use ahash::AHashMap;
use oxidized_protocol::types::SectionPos;
use oxidized_world::chunk::LevelChunk;

use super::block_light::initialize_block_light;
use super::propagation::{
    DecreaseEntry, LightEntry, propagate_block_light_decrease, propagate_block_light_increase,
    propagate_sky_light_decrease, propagate_sky_light_increase,
};
use super::queue::LightUpdateQueue;
use super::sky::initialize_sky_light;

/// Errors that can occur during light processing.
#[derive(Debug, thiserror::Error)]
pub enum LightingError {
    /// A referenced chunk section is not loaded or available.
    #[error("chunk section unavailable at {section}")]
    SectionUnavailable {
        /// The position of the unavailable section.
        section: SectionPos,
    },
}

/// Batched BFS lighting engine.
///
/// Owns a [`LightUpdateQueue`] and processes all pending updates in one pass
/// at the end of each tick. Groups updates by section, runs decrease then
/// increase BFS passes, and propagates across section boundaries.
///
/// # Examples
///
/// ```
/// use oxidized_game::lighting::engine::LightEngine;
///
/// let engine = LightEngine::new();
/// assert!(engine.queue().is_empty());
/// ```
pub struct LightEngine {
    queue: LightUpdateQueue,
}

impl LightEngine {
    /// Creates a new lighting engine with an empty update queue.
    #[must_use]
    pub fn new() -> Self {
        Self {
            queue: LightUpdateQueue::new(),
        }
    }

    /// Returns a reference to the update queue.
    #[must_use]
    pub fn queue(&self) -> &LightUpdateQueue {
        &self.queue
    }

    /// Returns a mutable reference to the update queue.
    pub fn queue_mut(&mut self) -> &mut LightUpdateQueue {
        &mut self.queue
    }

    /// Processes all pending light updates for this tick on a single chunk.
    ///
    /// Groups updates by section, processes each section's decrease and
    /// increase BFS passes, and returns the list of sections whose light
    /// data changed.
    ///
    /// # Errors
    ///
    /// Returns [`LightingError`] if a referenced chunk section is unavailable.
    pub fn process_updates(
        &mut self,
        chunk: &mut LevelChunk,
    ) -> Result<Vec<SectionPos>, LightingError> {
        let updates = self.queue.drain();
        if updates.is_empty() {
            return Ok(Vec::new());
        }

        let chunk_x = chunk.pos.x;
        let chunk_z = chunk.pos.z;
        let chunk_base_x = chunk_x * 16;
        let chunk_base_z = chunk_z * 16;

        let mut changed_sections: AHashMap<SectionPos, ()> = AHashMap::new();
        let mut block_decrease = VecDeque::new();
        let mut block_increase = VecDeque::new();
        let mut sky_decrease = VecDeque::new();
        let mut sky_increase = VecDeque::new();

        for update in &updates {
            let section_pos = SectionPos::of_block_pos(&update.pos);
            let local_x = update.pos.x & 15;
            let local_z = update.pos.z & 15;
            let y = update.pos.y;

            // Block light: handle emission changes.
            if update.old_emission != update.new_emission {
                if update.old_emission > 0 {
                    // Remove old light source.
                    chunk.set_block_light_at(local_x, y, local_z, 0);
                    block_decrease.push_back(DecreaseEntry {
                        x: local_x,
                        y,
                        z: local_z,
                        old_level: update.old_emission,
                    });
                }
                if update.new_emission > 0 {
                    // Add new light source.
                    chunk.set_block_light_at(local_x, y, local_z, update.new_emission);
                    block_increase.push_back(LightEntry {
                        x: local_x,
                        y,
                        z: local_z,
                        level: update.new_emission,
                    });
                }
                changed_sections.insert(section_pos, ());
            }

            // Sky/Block light: handle opacity increases (block placed).
            if update.new_opacity > update.old_opacity {
                let sky_level = chunk.get_sky_light_at(local_x, y, local_z);
                let block_level = chunk.get_block_light_at(local_x, y, local_z);

                if sky_level > 0 {
                    chunk.set_sky_light_at(local_x, y, local_z, 0);
                    sky_decrease.push_back(DecreaseEntry {
                        x: local_x,
                        y,
                        z: local_z,
                        old_level: sky_level,
                    });
                }
                if block_level > 0 && update.new_emission == 0 {
                    chunk.set_block_light_at(local_x, y, local_z, 0);
                    block_decrease.push_back(DecreaseEntry {
                        x: local_x,
                        y,
                        z: local_z,
                        old_level: block_level,
                    });
                }
                changed_sections.insert(section_pos, ());
            }
        }

        // Run decrease passes first (per ADR-017).
        if !block_decrease.is_empty() {
            let _boundary = propagate_block_light_decrease(
                chunk,
                &mut block_decrease,
                &mut block_increase,
                chunk_base_x,
                chunk_base_z,
            );
        }
        if !sky_decrease.is_empty() {
            let _boundary = propagate_sky_light_decrease(
                chunk,
                &mut sky_decrease,
                &mut sky_increase,
                chunk_base_x,
                chunk_base_z,
            );
        }

        // After decrease completes, handle opacity decreases (block broken)
        // by checking neighbors for light that can now flow through.
        for update in &updates {
            if update.new_opacity < update.old_opacity {
                let local_x = update.pos.x & 15;
                let local_z = update.pos.z & 15;
                let y = update.pos.y;
                let section_pos = SectionPos::of_block_pos(&update.pos);

                check_neighbor_light_sources(
                    chunk,
                    local_x,
                    y,
                    local_z,
                    &mut sky_increase,
                    &mut block_increase,
                );
                changed_sections.insert(section_pos, ());
            }
        }

        // Then run increase passes.
        if !block_increase.is_empty() {
            let _boundary = propagate_block_light_increase(
                chunk,
                &mut block_increase,
                chunk_base_x,
                chunk_base_z,
            );
        }
        if !sky_increase.is_empty() {
            let _boundary =
                propagate_sky_light_increase(chunk, &mut sky_increase, chunk_base_x, chunk_base_z);
        }

        // Mark sections around each changed position as changed.
        for update in &updates {
            let sp = SectionPos::of_block_pos(&update.pos);
            changed_sections.insert(sp, ());
            // Also mark neighboring sections when light could propagate there.
            for dy in [-1i32, 0, 1] {
                let neighbor = SectionPos::new(sp.x, sp.y + dy, sp.z);
                changed_sections.insert(neighbor, ());
            }
        }

        Ok(changed_sections.into_keys().collect())
    }

    /// Computes full sky + block light for a newly generated chunk.
    ///
    /// Called by the worldgen pipeline at the Light status (ADR-016).
    /// Initializes sky light top-down from the heightmap, seeds block light
    /// from emitters, and runs BFS propagation for both light types.
    ///
    /// # Errors
    ///
    /// Returns [`LightingError`] if a chunk section is unavailable.
    pub fn light_chunk(&mut self, chunk: &mut LevelChunk) -> Result<(), LightingError> {
        initialize_sky_light(chunk);
        initialize_block_light(chunk);
        Ok(())
    }
}

/// Checks all 6 neighbors of a position for light sources and seeds the
/// increase queues if any neighbor has light that can now flow through.
fn check_neighbor_light_sources(
    chunk: &LevelChunk,
    x: i32,
    y: i32,
    z: i32,
    sky_increase: &mut VecDeque<LightEntry>,
    block_increase: &mut VecDeque<LightEntry>,
) {
    const DIRS: [(i32, i32, i32); 6] = [
        (1, 0, 0),
        (-1, 0, 0),
        (0, 1, 0),
        (0, -1, 0),
        (0, 0, 1),
        (0, 0, -1),
    ];

    for &(dx, dy, dz) in &DIRS {
        let nx = x + dx;
        let ny = y + dy;
        let nz = z + dz;

        if nx < 0 || nx >= 16 || nz < 0 || nz >= 16 {
            continue;
        }
        if ny < chunk.min_y() || ny >= chunk.max_y() {
            continue;
        }

        let sky = chunk.get_sky_light_at(nx, ny, nz);
        if sky > 1 {
            sky_increase.push_back(LightEntry {
                x: nx,
                y: ny,
                z: nz,
                level: sky,
            });
        }

        let block = chunk.get_block_light_at(nx, ny, nz);
        if block > 1 {
            block_increase.push_back(LightEntry {
                x: nx,
                y: ny,
                z: nz,
                level: block,
            });
        }
    }
}

impl Default for LightEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::lighting::queue::LightUpdate;
    use oxidized_protocol::types::BlockPos;
    use oxidized_world::chunk::heightmap::{Heightmap, HeightmapType};
    use oxidized_world::chunk::level_chunk::{OVERWORLD_HEIGHT, OVERWORLD_MIN_Y};
    use oxidized_world::chunk::{ChunkPos, LevelChunk};
    use oxidized_world::registry::{BEDROCK, BlockRegistry, BlockStateId, DIRT, GRASS_BLOCK};

    fn stone_id() -> u32 {
        u32::from(
            BlockRegistry
                .default_state("minecraft:stone")
                .expect("stone missing")
                .0,
        )
    }

    fn glowstone_id() -> u32 {
        u32::from(
            BlockRegistry
                .default_state("minecraft:glowstone")
                .expect("glowstone missing")
                .0,
        )
    }

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

        let mut hm = Heightmap::new(HeightmapType::MotionBlocking, OVERWORLD_HEIGHT).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                hm.set(x, z, 4).unwrap();
            }
        }
        chunk.set_heightmap(hm);
        chunk
    }

    #[test]
    fn test_engine_new_has_empty_queue() {
        let engine = LightEngine::new();
        assert!(engine.queue().is_empty());
    }

    #[test]
    fn test_engine_queue_mut() {
        let mut engine = LightEngine::new();
        engine.queue_mut().push(LightUpdate {
            pos: BlockPos::new(0, 64, 0),
            old_emission: 0,
            new_emission: 14,
            old_opacity: 0,
            new_opacity: 0,
        });
        assert_eq!(engine.queue().len(), 1);
    }

    #[test]
    fn test_light_chunk_sky_light() {
        let mut engine = LightEngine::new();
        let mut chunk = flat_chunk();
        engine.light_chunk(&mut chunk).unwrap();

        // Above surface.
        assert_eq!(chunk.get_sky_light_at(8, OVERWORLD_MIN_Y + 4, 8), 15);
        // Inside solid.
        assert_eq!(chunk.get_sky_light_at(8, OVERWORLD_MIN_Y + 2, 8), 0);
    }

    #[test]
    fn test_process_updates_empty_queue() {
        let mut engine = LightEngine::new();
        let mut chunk = flat_chunk();
        let result = engine.process_updates(&mut chunk).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_process_updates_place_glowstone() {
        let mut engine = LightEngine::new();
        let mut chunk = flat_chunk();
        engine.light_chunk(&mut chunk).unwrap();

        // Place glowstone at (8, -56, 8) — above the surface in air.
        let gs = glowstone_id();
        let emission = BlockStateId(gs as u16).light_emission();
        chunk.set_block_state(8, -56, 8, gs).unwrap();
        engine.queue_mut().push(LightUpdate {
            pos: BlockPos::new(8, -56, 8),
            old_emission: 0,
            new_emission: emission,
            old_opacity: 0,
            new_opacity: BlockStateId(gs as u16).light_opacity(),
        });

        let changed = engine.process_updates(&mut chunk).unwrap();
        assert!(!changed.is_empty());
        assert_eq!(chunk.get_block_light_at(8, -56, 8), emission);
        assert_eq!(chunk.get_block_light_at(9, -56, 8), emission - 1);
    }

    #[test]
    fn test_process_updates_break_glowstone() {
        let mut engine = LightEngine::new();
        let mut chunk = flat_chunk();
        engine.light_chunk(&mut chunk).unwrap();

        // Place glowstone.
        let gs = glowstone_id();
        let emission = BlockStateId(gs as u16).light_emission();
        chunk.set_block_state(8, -56, 8, gs).unwrap();
        engine.queue_mut().push(LightUpdate {
            pos: BlockPos::new(8, -56, 8),
            old_emission: 0,
            new_emission: emission,
            old_opacity: 0,
            new_opacity: BlockStateId(gs as u16).light_opacity(),
        });
        engine.process_updates(&mut chunk).unwrap();

        // Now break it (set to air).
        chunk.set_block_state(8, -56, 8, 0).unwrap();
        engine.queue_mut().push(LightUpdate {
            pos: BlockPos::new(8, -56, 8),
            old_emission: emission,
            new_emission: 0,
            old_opacity: BlockStateId(gs as u16).light_opacity(),
            new_opacity: 0,
        });
        engine.process_updates(&mut chunk).unwrap();

        assert_eq!(chunk.get_block_light_at(8, -56, 8), 0);
        assert_eq!(chunk.get_block_light_at(9, -56, 8), 0);
    }

    #[test]
    fn test_process_updates_place_opaque_block() {
        let mut engine = LightEngine::new();
        let mut chunk = flat_chunk();
        engine.light_chunk(&mut chunk).unwrap();

        // Verify sky light above surface.
        assert_eq!(chunk.get_sky_light_at(8, OVERWORLD_MIN_Y + 5, 8), 15);

        // Place stone at the surface+1 position.
        let st = stone_id();
        let opacity = BlockStateId(st as u16).light_opacity();
        chunk
            .set_block_state(8, OVERWORLD_MIN_Y + 4, 8, st)
            .unwrap();
        engine.queue_mut().push(LightUpdate {
            pos: BlockPos::new(8, OVERWORLD_MIN_Y + 4, 8),
            old_emission: 0,
            new_emission: 0,
            old_opacity: 0,
            new_opacity: opacity,
        });
        let changed = engine.process_updates(&mut chunk).unwrap();
        assert!(!changed.is_empty());
    }
}
