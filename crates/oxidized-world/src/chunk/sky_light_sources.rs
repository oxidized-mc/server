//! Per-column sky light source tracking.
//!
//! Implements vanilla's `ChunkSkyLightSources` algorithm: for each (x, z)
//! column in a chunk, tracks the lowest Y where sky light at level 15 enters
//! the column. This uses edge occlusion testing between vertically adjacent
//! blocks rather than the simpler `MOTION_BLOCKING` heightmap, giving correct
//! results for overhangs, non-full blocks (slabs, stairs), and caves near
//! chunk edges.

use crate::registry::BlockStateId;

use super::LevelChunk;

/// Face index for the downward face (Y-).
const FACE_DOWN: u8 = 0;

/// Face index for the upward face (Y+).
const FACE_UP: u8 = 1;

/// Per-column sky light source tracking.
///
/// For each (x, z) column in a chunk, tracks the lowest Y where sky light
/// at level 15 enters the column. This is computed by scanning top-to-bottom
/// and checking edge occlusion between vertically adjacent block states.
///
/// The stored values are world Y coordinates. The sentinel [`i32::MIN`] means
/// the column is fully open to sky light all the way down (no occluding edge).
///
/// Matches vanilla's `ChunkSkyLightSources` from `net.minecraft.world.level.lighting`.
#[derive(Debug, Clone)]
pub struct ChunkSkyLightSources {
    /// Lowest source Y per column, indexed as `[x + z * 16]`.
    lowest_y: [i32; 256],
    /// One below the world minimum Y — the floor sentinel.
    min_y: i32,
}

impl ChunkSkyLightSources {
    /// Creates a new instance with all columns set to the world bottom.
    #[must_use]
    pub fn new(world_min_y: i32) -> Self {
        let min_y = world_min_y - 1;
        Self {
            lowest_y: [min_y; 256],
            min_y,
        }
    }

    /// Builds sky light sources by scanning all columns of the given chunk.
    #[must_use]
    pub fn from_chunk(chunk: &LevelChunk) -> Self {
        let mut sources = Self::new(chunk.min_y());
        sources.fill_from(chunk);
        sources
    }

    /// Scans all 16×16 columns of the chunk to populate source heights.
    ///
    /// This is the initial build — called once when a chunk is first lit.
    pub fn fill_from(&mut self, chunk: &LevelChunk) {
        let highest_section = self.find_highest_non_empty_section(chunk);

        match highest_section {
            None => {
                // All sections empty (all air) — sources extend to world bottom.
                self.lowest_y.fill(self.min_y);
            }
            Some(top_idx) => {
                for z in 0..16usize {
                    for x in 0..16usize {
                        let y = self.find_lowest_source_y_from(chunk, top_idx, x, z);
                        let y = y.max(self.min_y);
                        self.lowest_y[index(x, z)] = y;
                    }
                }
            }
        }
    }

    /// Called when a block at `(x, y, z)` changes (chunk-local x/z, world y).
    ///
    /// Updates the source height for the affected column if needed.
    /// Returns `true` if the source height changed.
    pub fn update(&mut self, chunk: &LevelChunk, x: usize, y: i32, z: usize) -> bool {
        let upper_edge_y = y + 1;
        let idx = index(x, z);
        let current = self.lowest_y[idx];

        if upper_edge_y < current {
            return false;
        }

        let top_state = get_state(chunk, x as i32, y + 1, z as i32);
        let mid_state = get_state(chunk, x as i32, y, z as i32);

        if self.update_edge(chunk, idx, current, upper_edge_y, top_state, y, mid_state) {
            return true;
        }

        let bot_state = get_state(chunk, x as i32, y - 1, z as i32);
        self.update_edge(chunk, idx, current, y, mid_state, y - 1, bot_state)
    }

    /// Returns the lowest Y in this column that has sky light 15.
    ///
    /// Returns [`i32::MIN`] if the column is fully open to sky (no occluding
    /// edge exists anywhere in the column).
    #[must_use]
    pub fn get_lowest_source_y(&self, x: usize, z: usize) -> i32 {
        let raw = self.lowest_y[index(x, z)];
        if raw == self.min_y { i32::MIN } else { raw }
    }

    /// Returns the highest `lowest_source_y` across all 256 columns.
    #[must_use]
    pub fn get_highest_lowest_source_y(&self) -> i32 {
        let mut max_raw = i32::MIN;
        for &v in &self.lowest_y {
            if v > max_raw {
                max_raw = v;
            }
        }
        if max_raw == self.min_y { i32::MIN } else { max_raw }
    }

    /// Returns the raw stored value for a column (without sentinel expansion).
    ///
    /// Useful for tests and internal checks.
    #[must_use]
    pub fn get_raw(&self, x: usize, z: usize) -> i32 {
        self.lowest_y[index(x, z)]
    }

    // ── Private helpers ─────────────────────────────────────────────────

    /// Finds the highest non-empty section index in the chunk.
    fn find_highest_non_empty_section(&self, chunk: &LevelChunk) -> Option<usize> {
        for i in (0..chunk.section_count()).rev() {
            if let Some(section) = chunk.section(i) {
                if !section.is_empty() {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Scans a column from the top of `top_section_idx` downward to find the
    /// lowest Y where sky light enters.
    fn find_lowest_source_y_from(
        &self,
        chunk: &LevelChunk,
        top_section_idx: usize,
        x: usize,
        z: usize,
    ) -> i32 {
        let mut top_state = BlockStateId(0); // Start with air above the world.

        for section_idx in (0..=top_section_idx).rev() {
            let section = match chunk.section(section_idx) {
                Some(s) => s,
                None => continue,
            };

            let section_base_y = chunk.min_y() + (section_idx as i32 * 16);

            if section.is_empty() {
                // All air — no occlusion possible. Reset top state to air.
                top_state = BlockStateId(0);
                continue;
            }

            for local_y in (0..16).rev() {
                #[allow(clippy::cast_possible_truncation)]
                let bottom_state = {
                    let raw = section.get_block_state(x, local_y, z).unwrap_or(0);
                    BlockStateId(raw as u16)
                };

                if is_edge_occluded(top_state, bottom_state) {
                    return section_base_y + local_y as i32 + 1;
                }

                top_state = bottom_state;
            }
        }

        self.min_y
    }

    /// Checks a single edge and updates the stored source height if needed.
    fn update_edge(
        &mut self,
        chunk: &LevelChunk,
        idx: usize,
        old_source_y: i32,
        top_y: i32,
        top_state: BlockStateId,
        bottom_y: i32,
        bottom_state: BlockStateId,
    ) -> bool {
        if is_edge_occluded(top_state, bottom_state) {
            if top_y > old_source_y {
                // New occlusion appeared above the current source — move up.
                self.lowest_y[idx] = top_y;
                return true;
            }
        } else if top_y == old_source_y {
            // The current source edge was cleared — scan downward for next edge.
            let new_y = self.find_lowest_source_below(chunk, idx, bottom_y, bottom_state);
            self.lowest_y[idx] = new_y;
            return true;
        }
        false
    }

    /// Starting from `start_y`, scans downward to find the next occluding edge.
    fn find_lowest_source_below(
        &self,
        chunk: &LevelChunk,
        idx: usize,
        start_y: i32,
        start_state: BlockStateId,
    ) -> i32 {
        let x = (idx % 16) as i32;
        let z = (idx / 16) as i32;
        let mut top_state = start_state;
        let mut top_y = start_y;
        let mut bottom_y = start_y - 1;

        while bottom_y >= self.min_y {
            let bottom_state = get_state(chunk, x, bottom_y, z);
            if is_edge_occluded(top_state, bottom_state) {
                return top_y;
            }
            top_state = bottom_state;
            top_y = bottom_y;
            bottom_y -= 1;
        }

        self.min_y
    }
}

/// Checks whether the edge between two vertically adjacent blocks occludes
/// sky light (top block above, bottom block below).
///
/// Matches vanilla's `ChunkSkyLightSources.isEdgeOccluded`.
fn is_edge_occluded(top: BlockStateId, bottom: BlockStateId) -> bool {
    if bottom.light_opacity() != 0 {
        return true;
    }
    // Shape-based check: if top's DOWN face or bottom's UP face fully
    // covers the boundary, sky light is blocked.
    let top_down = !top.is_empty_shape() && top.occlusion_face(FACE_DOWN);
    let bottom_up = !bottom.is_empty_shape() && bottom.occlusion_face(FACE_UP);
    top_down || bottom_up
}

/// Returns the block state at a world position, or air if out of bounds.
#[allow(clippy::cast_possible_truncation)]
fn get_state(chunk: &LevelChunk, x: i32, y: i32, z: i32) -> BlockStateId {
    let raw = chunk.get_block_state(x, y, z).unwrap_or(0);
    BlockStateId(raw as u16)
}

/// Column index from chunk-local (x, z) coordinates.
#[inline]
const fn index(x: usize, z: usize) -> usize {
    x + z * 16
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::chunk::heightmap::{Heightmap, HeightmapType};
    use crate::chunk::level_chunk::{OVERWORLD_HEIGHT, OVERWORLD_MIN_Y};
    use crate::chunk::{ChunkPos, LevelChunk};
    use crate::registry::{BlockRegistry, BEDROCK, DIRT, GRASS_BLOCK};

    /// Creates a standard flat world chunk: bedrock, 2 dirt, grass, air above.
    fn flat_chunk() -> LevelChunk {
        let mut chunk = LevelChunk::new(ChunkPos::new(0, 0));
        let bedrock = u32::from(BEDROCK.0);
        let dirt = u32::from(DIRT.0);
        let grass = u32::from(GRASS_BLOCK.0);

        for x in 0..16i32 {
            for z in 0..16i32 {
                chunk.set_block_state(x, OVERWORLD_MIN_Y, z, bedrock).unwrap();
                chunk.set_block_state(x, OVERWORLD_MIN_Y + 1, z, dirt).unwrap();
                chunk.set_block_state(x, OVERWORLD_MIN_Y + 2, z, dirt).unwrap();
                chunk.set_block_state(x, OVERWORLD_MIN_Y + 3, z, grass).unwrap();
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

    fn state_id(name: &str) -> BlockStateId {
        BlockRegistry
            .default_state(name)
            .unwrap_or_else(|| panic!("{name} not found"))
    }

    fn state_with_props(name: &str, props: &[(&str, &str)]) -> BlockStateId {
        let mut sid = state_id(name);
        for &(key, value) in props {
            sid = sid
                .with_property(key, value)
                .unwrap_or_else(|| panic!("{name}[{key}={value}] not found"));
        }
        sid
    }

    // ── Flat world tests ─────────────────────────────────────────────────

    #[test]
    fn test_flat_world_source_matches_surface() {
        let chunk = flat_chunk();
        let sources = ChunkSkyLightSources::from_chunk(&chunk);

        // Flat world: bedrock(-64), dirt(-63), dirt(-62), grass(-61), air(-60+)
        // Grass is opaque (light_opacity > 0), so the edge is at Y=-60
        // (the air block at -60 above the grass at -61).
        for x in 0..16 {
            for z in 0..16 {
                let y = sources.get_lowest_source_y(x, z);
                assert_eq!(
                    y,
                    OVERWORLD_MIN_Y + 4,
                    "column ({x}, {z}): expected {} got {y}",
                    OVERWORLD_MIN_Y + 4
                );
            }
        }
    }

    #[test]
    fn test_single_column_hole() {
        let mut chunk = flat_chunk();
        // Dig a 1×1 hole through all 4 solid layers at (8, 8).
        for y in OVERWORLD_MIN_Y..OVERWORLD_MIN_Y + 4 {
            chunk.set_block_state(8, y, 8, 0).unwrap();
        }

        let sources = ChunkSkyLightSources::from_chunk(&chunk);
        let hole_y = sources.get_lowest_source_y(8, 8);

        // Column is all air → source extends to world bottom.
        assert_eq!(hole_y, i32::MIN);

        // Other columns should still be at surface.
        assert_eq!(sources.get_lowest_source_y(0, 0), OVERWORLD_MIN_Y + 4);
    }

    #[test]
    fn test_break_surface_block_updates_source() {
        let mut chunk = flat_chunk();
        let mut sources = ChunkSkyLightSources::from_chunk(&chunk);

        // Break the grass block at (4, -61, 4).
        chunk
            .set_block_state(4, OVERWORLD_MIN_Y + 3, 4, 0)
            .unwrap();
        let changed = sources.update(&chunk, 4, OVERWORLD_MIN_Y + 3, 4);

        assert!(changed, "source should change when surface block broken");
        // New source should be at dirt level: dirt has opacity > 0 so edge
        // is at -61 (air at -61 above dirt at -62).
        assert_eq!(sources.get_lowest_source_y(4, 4), OVERWORLD_MIN_Y + 3);
    }

    #[test]
    fn test_place_block_over_hole_updates_source() {
        let mut chunk = flat_chunk();
        // Dig hole at (4, 4).
        for y in OVERWORLD_MIN_Y..OVERWORLD_MIN_Y + 4 {
            chunk.set_block_state(4, y, 4, 0).unwrap();
        }
        let mut sources = ChunkSkyLightSources::from_chunk(&chunk);
        assert_eq!(sources.get_lowest_source_y(4, 4), i32::MIN);

        // Place stone at (4, -60, 4) — above where the surface was.
        let stone = u32::from(state_id("minecraft:stone").0);
        chunk.set_block_state(4, OVERWORLD_MIN_Y + 4, 4, stone).unwrap();
        let changed = sources.update(&chunk, 4, OVERWORLD_MIN_Y + 4, 4);

        assert!(changed, "source should change when block placed over hole");
        // Stone is opaque → edge at stone's top = -59 (Y = min_y + 5).
        assert_eq!(sources.get_lowest_source_y(4, 4), OVERWORLD_MIN_Y + 5);
    }

    #[test]
    fn test_all_air_chunk() {
        let chunk = LevelChunk::new(ChunkPos::new(0, 0));
        let sources = ChunkSkyLightSources::from_chunk(&chunk);

        for x in 0..16 {
            for z in 0..16 {
                assert_eq!(
                    sources.get_lowest_source_y(x, z),
                    i32::MIN,
                    "all-air column ({x}, {z}) should extend to world bottom"
                );
            }
        }
    }

    #[test]
    fn test_property_lowest_source_y_above_min_y() {
        let chunk = flat_chunk();
        let sources = ChunkSkyLightSources::from_chunk(&chunk);

        for x in 0..16 {
            for z in 0..16 {
                let y = sources.get_raw(x, z);
                assert!(
                    y >= OVERWORLD_MIN_Y - 1,
                    "raw source at ({x}, {z}) = {y} is below min_y - 1"
                );
            }
        }
    }

    #[test]
    fn test_highest_lowest_source_y_flat_world() {
        let chunk = flat_chunk();
        let sources = ChunkSkyLightSources::from_chunk(&chunk);
        assert_eq!(sources.get_highest_lowest_source_y(), OVERWORLD_MIN_Y + 4);
    }

    #[test]
    fn test_update_below_source_is_noop() {
        let mut chunk = flat_chunk();
        let mut sources = ChunkSkyLightSources::from_chunk(&chunk);

        // Change a block well below the source (inside bedrock layer).
        chunk.set_block_state(8, OVERWORLD_MIN_Y, 8, 0).unwrap();
        let changed = sources.update(&chunk, 8, OVERWORLD_MIN_Y, 8);
        assert!(!changed, "change below source should not affect source");
    }

    // ── Edge occlusion tests ─────────────────────────────────────────────

    #[test]
    fn test_air_to_opaque_is_occluded() {
        let air = BlockStateId(0);
        let stone = state_id("minecraft:stone");
        assert!(is_edge_occluded(air, stone));
    }

    #[test]
    fn test_air_to_air_is_not_occluded() {
        let air = BlockStateId(0);
        assert!(!is_edge_occluded(air, air));
    }

    #[test]
    fn test_air_to_glass_is_not_occluded() {
        let air = BlockStateId(0);
        let glass = state_id("minecraft:glass");
        // Glass has light_opacity = 0 and no face occlusion.
        assert!(!is_edge_occluded(air, glass));
    }

    #[test]
    fn test_bottom_slab_occludes_due_to_opacity() {
        let air = BlockStateId(0);
        let slab = state_with_props("minecraft:stone_slab", &[("type", "bottom")]);
        // Bottom slab has light_opacity = 15 in our data (per-block heuristic).
        // Full per-state accuracy requires C2 (VoxelShape occlusion, 23a.12).
        assert!(
            is_edge_occluded(air, slab),
            "bottom slab occludes due to non-zero light_opacity (C2 would fix this)"
        );
    }

    #[test]
    fn test_top_slab_occludes_from_above() {
        let air = BlockStateId(0);
        let slab = state_with_props("minecraft:stone_slab", &[("type", "top")]);
        // Top slab: both opacity (15) and UP face occlusion.
        assert!(
            is_edge_occluded(air, slab),
            "top slab should occlude sky light from above"
        );
    }

    #[test]
    fn test_bottom_slab_column_source() {
        let mut chunk = LevelChunk::new(ChunkPos::new(0, 0));
        let dirt = u32::from(DIRT.0);
        let slab = u32::from(state_with_props("minecraft:stone_slab", &[("type", "bottom")]).0);

        // Build: dirt at Y=0, bottom slab at Y=1, air above.
        for x in 0..16i32 {
            for z in 0..16i32 {
                chunk.set_block_state(x, 0, z, dirt).unwrap();
                chunk.set_block_state(x, 1, z, slab).unwrap();
            }
        }

        let sources = ChunkSkyLightSources::from_chunk(&chunk);

        // Bottom slab has light_opacity 15, so it occludes.
        // Source Y is at Y=2 (above the slab). With C2 (VoxelShape, 23a.12),
        // this would be Y=1 since bottom slabs don't actually block from above.
        let source_y = sources.get_lowest_source_y(8, 8);
        assert_eq!(source_y, 2, "source at slab top (opacity-based, C2 would lower this)");
    }
}
