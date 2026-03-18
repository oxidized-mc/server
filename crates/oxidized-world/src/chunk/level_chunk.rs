//! A full chunk column containing 24 sections (overworld: y=-64 to y=319).
//!
//! This is the main chunk type used by the server level. It stores sections,
//! heightmaps, and provides coordinate-based block access.

use std::collections::HashMap;

use super::data_layer::DataLayer;
use super::heightmap::{Heightmap, HeightmapType};
use super::paletted_container::PalettedContainerError;
use super::section::LevelChunkSection;

/// Number of chunk sections in the overworld (y=-64 to y=319, 384 blocks / 16).
pub const OVERWORLD_SECTION_COUNT: usize = 24;

/// Minimum Y coordinate in the overworld.
pub const OVERWORLD_MIN_Y: i32 = -64;

/// Maximum Y coordinate in the overworld (exclusive).
pub const OVERWORLD_MAX_Y: i32 = 320;

/// Total world height in the overworld.
pub const OVERWORLD_HEIGHT: u32 = 384;

/// Errors that can occur during chunk operations.
#[derive(Debug, thiserror::Error)]
pub enum ChunkError {
    /// Block position is outside the valid world bounds.
    #[error("position out of bounds: ({x}, {y}, {z})")]
    OutOfBounds {
        /// X coordinate.
        x: i32,
        /// Y coordinate.
        y: i32,
        /// Z coordinate.
        z: i32,
    },

    /// Palette/container error.
    #[error("container error: {0}")]
    Container(#[from] PalettedContainerError),
}

/// A full chunk column in a server level.
#[derive(Debug, Clone)]
pub struct LevelChunk {
    /// Chunk position in chunk coordinates.
    pub pos: ChunkPos,
    /// 24 sections for overworld (index 0 = y=-64...-48, index 23 = y=304...319).
    sections: Vec<LevelChunkSection>,
    /// Heightmaps for this chunk.
    heightmaps: HashMap<HeightmapType, Heightmap>,
    /// Per-section sky light data. Index matches sections.
    sky_light: Vec<Option<DataLayer>>,
    /// Per-section block light data. Index matches sections.
    block_light: Vec<Option<DataLayer>>,
    /// Minimum Y coordinate for this chunk's dimension.
    min_y: i32,
    /// Number of sections in this chunk.
    section_count: usize,
}

/// Chunk position in chunk coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkPos {
    /// Chunk X coordinate.
    pub x: i32,
    /// Chunk Z coordinate.
    pub z: i32,
}

impl ChunkPos {
    /// Creates a new chunk position.
    #[must_use]
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    /// Returns the chunk position containing the given block coordinates.
    #[must_use]
    pub const fn from_block(block_x: i32, block_z: i32) -> Self {
        Self {
            x: block_x >> 4,
            z: block_z >> 4,
        }
    }

    /// Returns the minimum block X for this chunk.
    #[must_use]
    pub const fn min_block_x(self) -> i32 {
        self.x << 4
    }

    /// Returns the minimum block Z for this chunk.
    #[must_use]
    pub const fn min_block_z(self) -> i32 {
        self.z << 4
    }

    /// Packs into a single `i64` for use as a map key.
    #[must_use]
    pub const fn to_long(self) -> i64 {
        ((self.x as i64) & 0xFFFF_FFFF) | (((self.z as i64) & 0xFFFF_FFFF) << 32)
    }
}

impl std::fmt::Display for ChunkPos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}, {}]", self.x, self.z)
    }
}

impl LevelChunk {
    /// Creates a new empty chunk at the given position for the overworld.
    #[must_use]
    pub fn new(pos: ChunkPos) -> Self {
        Self::with_dimensions(pos, OVERWORLD_MIN_Y, OVERWORLD_SECTION_COUNT)
    }

    /// Creates a new empty chunk with custom dimensions.
    #[must_use]
    pub fn with_dimensions(pos: ChunkPos, min_y: i32, section_count: usize) -> Self {
        let sections = (0..section_count)
            .map(|_| LevelChunkSection::new())
            .collect();
        let sky_light = vec![None; section_count + 2]; // +2 for above/below
        let block_light = vec![None; section_count + 2];

        Self {
            pos,
            sections,
            heightmaps: HashMap::new(),
            sky_light,
            block_light,
            min_y,
            section_count,
        }
    }

    /// Returns the section index for a world Y coordinate.
    fn section_index(&self, y: i32) -> Option<usize> {
        let shifted = y - self.min_y;
        if shifted < 0 {
            return None;
        }
        let idx = (shifted >> 4) as usize;
        if idx >= self.section_count {
            None
        } else {
            Some(idx)
        }
    }

    /// Returns the block state ID at the given world position.
    ///
    /// # Errors
    ///
    /// Returns [`ChunkError::OutOfBounds`] if the position is outside this chunk.
    pub fn get_block_state(&self, x: i32, y: i32, z: i32) -> Result<u32, ChunkError> {
        let idx = self
            .section_index(y)
            .ok_or(ChunkError::OutOfBounds { x, y, z })?;
        let lx = (x & 15) as usize;
        let ly = (y & 15) as usize;
        let lz = (z & 15) as usize;
        Ok(self.sections[idx].get_block_state(lx, ly, lz)?)
    }

    /// Sets the block state ID at the given world position.
    ///
    /// Returns the previous block state ID.
    ///
    /// # Errors
    ///
    /// Returns [`ChunkError::OutOfBounds`] if the position is outside this chunk.
    pub fn set_block_state(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        state_id: u32,
    ) -> Result<u32, ChunkError> {
        let idx = self
            .section_index(y)
            .ok_or(ChunkError::OutOfBounds { x, y, z })?;
        let lx = (x & 15) as usize;
        let ly = (y & 15) as usize;
        let lz = (z & 15) as usize;
        Ok(self.sections[idx].set_block_state(lx, ly, lz, state_id)?)
    }

    /// Returns a reference to the section at the given index.
    #[must_use]
    pub fn section(&self, index: usize) -> Option<&LevelChunkSection> {
        self.sections.get(index)
    }

    /// Returns a mutable reference to the section at the given index.
    #[must_use]
    pub fn section_mut(&mut self, index: usize) -> Option<&mut LevelChunkSection> {
        self.sections.get_mut(index)
    }

    /// Returns all sections.
    #[must_use]
    pub fn sections(&self) -> &[LevelChunkSection] {
        &self.sections
    }

    /// Returns the number of sections.
    #[must_use]
    pub fn section_count(&self) -> usize {
        self.section_count
    }

    /// Returns the minimum world Y for this chunk.
    #[must_use]
    pub fn min_y(&self) -> i32 {
        self.min_y
    }

    /// Returns the maximum world Y (exclusive) for this chunk.
    #[must_use]
    pub fn max_y(&self) -> i32 {
        self.min_y + (self.section_count as i32 * 16)
    }

    /// Inserts or replaces a heightmap.
    pub fn set_heightmap(&mut self, heightmap: Heightmap) {
        self.heightmaps
            .insert(heightmap.heightmap_type(), heightmap);
    }

    /// Returns a reference to a heightmap by type.
    #[must_use]
    pub fn heightmap(&self, htype: HeightmapType) -> Option<&Heightmap> {
        self.heightmaps.get(&htype)
    }

    /// Returns a mutable reference to a heightmap by type.
    #[must_use]
    pub fn heightmap_mut(&mut self, htype: HeightmapType) -> Option<&mut Heightmap> {
        self.heightmaps.get_mut(&htype)
    }

    /// Returns all heightmaps.
    #[must_use]
    pub fn heightmaps(&self) -> &HashMap<HeightmapType, Heightmap> {
        &self.heightmaps
    }

    /// Sets sky light data for a section (index includes +1 offset for below-chunk light).
    pub fn set_sky_light(&mut self, light_index: usize, layer: DataLayer) {
        if light_index < self.sky_light.len() {
            self.sky_light[light_index] = Some(layer);
        }
    }

    /// Returns sky light data for a section.
    #[must_use]
    pub fn sky_light(&self, light_index: usize) -> Option<&DataLayer> {
        self.sky_light.get(light_index)?.as_ref()
    }

    /// Sets block light data for a section.
    pub fn set_block_light(&mut self, light_index: usize, layer: DataLayer) {
        if light_index < self.block_light.len() {
            self.block_light[light_index] = Some(layer);
        }
    }

    /// Returns block light data for a section.
    #[must_use]
    pub fn block_light(&self, light_index: usize) -> Option<&DataLayer> {
        self.block_light.get(light_index)?.as_ref()
    }

    /// Returns the sky light layers (for packet serialization).
    #[must_use]
    pub fn sky_light_layers(&self) -> &[Option<DataLayer>] {
        &self.sky_light
    }

    /// Returns the block light layers (for packet serialization).
    #[must_use]
    pub fn block_light_layers(&self) -> &[Option<DataLayer>] {
        &self.block_light
    }

    /// Serializes all section data to bytes (for `ClientboundLevelChunkPacketData.buffer`).
    #[must_use]
    pub fn write_sections_to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.section_count * 128);
        for section in &self.sections {
            buf.extend(section.write_to_bytes());
        }
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_pos_from_block() {
        let pos = ChunkPos::from_block(32, -48);
        assert_eq!(pos.x, 2);
        assert_eq!(pos.z, -3);
    }

    #[test]
    fn test_chunk_pos_min_block() {
        let pos = ChunkPos::new(2, -3);
        assert_eq!(pos.min_block_x(), 32);
        assert_eq!(pos.min_block_z(), -48);
    }

    #[test]
    fn test_new_chunk_is_empty() {
        let chunk = LevelChunk::new(ChunkPos::new(0, 0));
        assert_eq!(chunk.section_count(), 24);
        assert_eq!(chunk.min_y(), -64);
        assert_eq!(chunk.max_y(), 320);
    }

    #[test]
    fn test_get_set_block() {
        let mut chunk = LevelChunk::new(ChunkPos::new(0, 0));
        // y=0 → section_index = (0-(-64))/16 = 4
        chunk.set_block_state(0, 0, 0, 1).unwrap();
        assert_eq!(chunk.get_block_state(0, 0, 0).unwrap(), 1);

        // y=-64 → section 0
        chunk.set_block_state(0, -64, 0, 7).unwrap();
        assert_eq!(chunk.get_block_state(0, -64, 0).unwrap(), 7);

        // y=319 → section 23
        chunk.set_block_state(0, 319, 0, 42).unwrap();
        assert_eq!(chunk.get_block_state(0, 319, 0).unwrap(), 42);
    }

    #[test]
    fn test_out_of_bounds() {
        let chunk = LevelChunk::new(ChunkPos::new(0, 0));
        assert!(chunk.get_block_state(0, -65, 0).is_err());
        assert!(chunk.get_block_state(0, 320, 0).is_err());
    }

    #[test]
    fn test_section_access() {
        let mut chunk = LevelChunk::new(ChunkPos::new(0, 0));
        chunk.set_block_state(5, 10, 3, 99).unwrap();
        // y=10 → section (10+64)/16 = 4 (integer), so section_index = (10-(-64))/16 = 4
        let section = chunk.section(4).unwrap();
        assert_eq!(section.get_block_state(5, 10, 3).unwrap(), 99);
    }

    #[test]
    fn test_heightmap_storage() {
        let mut chunk = LevelChunk::new(ChunkPos::new(0, 0));
        let mut hm = Heightmap::new(HeightmapType::MotionBlocking, OVERWORLD_HEIGHT).unwrap();
        hm.set(5, 5, 100).unwrap();
        chunk.set_heightmap(hm);

        let hm2 = chunk.heightmap(HeightmapType::MotionBlocking).unwrap();
        assert_eq!(hm2.get(5, 5).unwrap(), 100);
    }

    #[test]
    fn test_light_data() {
        let mut chunk = LevelChunk::new(ChunkPos::new(0, 0));
        let sky = DataLayer::filled(15);
        chunk.set_sky_light(1, sky);
        assert!(chunk.sky_light(1).is_some());
        assert!(chunk.sky_light(0).is_none());
    }

    #[test]
    fn test_write_sections() {
        let chunk = LevelChunk::new(ChunkPos::new(0, 0));
        let bytes = chunk.write_sections_to_bytes();
        assert!(!bytes.is_empty());
    }
}
