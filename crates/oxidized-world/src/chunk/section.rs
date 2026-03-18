//! A 16×16×16 chunk section containing block states and biome data.
//!
//! This is the fundamental unit of chunk storage. Each section is independently
//! palette-compressed and serialized for the network packet.

use super::paletted_container::{PalettedContainer, PalettedContainerError, Strategy};

/// A 16×16×16 section of a chunk column.
///
/// Contains palette-compressed block states and biome data, plus counts
/// of non-empty blocks and fluids for quick access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelChunkSection {
    /// Number of non-air blocks in this section.
    non_empty_block_count: i16,
    /// Number of fluid-containing blocks (used for random ticking).
    fluid_count: i16,
    /// Number of randomly tickable blocks (e.g. grass, crops).
    ticking_block_count: i16,
    /// Number of randomly tickable fluids.
    ticking_fluid_count: i16,
    /// Block states (4096 entries, 16³).
    states: PalettedContainer,
    /// Biome data (64 entries, 4³ — biomes are sampled at 4-block resolution).
    biomes: PalettedContainer,
}

impl LevelChunkSection {
    /// Creates a new empty section (all air, default biome 0).
    #[must_use]
    pub fn new() -> Self {
        Self {
            non_empty_block_count: 0,
            fluid_count: 0,
            ticking_block_count: 0,
            ticking_fluid_count: 0,
            states: PalettedContainer::empty(Strategy::BlockStates),
            biomes: PalettedContainer::empty(Strategy::Biomes),
        }
    }

    /// Creates a section filled with a single block state.
    #[must_use]
    pub fn filled(block_state_id: u32) -> Self {
        let states = PalettedContainer::new(Strategy::BlockStates, block_state_id);
        let non_empty = if block_state_id == 0 { 0 } else { 4096 };
        Self {
            non_empty_block_count: non_empty,
            fluid_count: 0,
            ticking_block_count: 0,
            ticking_fluid_count: 0,
            states,
            biomes: PalettedContainer::empty(Strategy::Biomes),
        }
    }

    /// Returns the block state ID at the given section-local coordinates.
    ///
    /// Coordinates must be in 0..16.
    ///
    /// # Errors
    ///
    /// Returns an error if coordinates are out of bounds.
    pub fn get_block_state(
        &self,
        x: usize,
        y: usize,
        z: usize,
    ) -> Result<u32, PalettedContainerError> {
        self.states.get(x, y, z)
    }

    /// Sets the block state ID at the given section-local coordinates.
    ///
    /// Updates `non_empty_block_count` automatically (assumes state 0 = air).
    ///
    /// # Errors
    ///
    /// Returns an error if coordinates are out of bounds.
    pub fn set_block_state(
        &mut self,
        x: usize,
        y: usize,
        z: usize,
        state_id: u32,
    ) -> Result<u32, PalettedContainerError> {
        let old = self.states.get_and_set(x, y, z, state_id)?;
        if old != state_id {
            if old == 0 && state_id != 0 {
                self.non_empty_block_count += 1;
            } else if old != 0 && state_id == 0 {
                self.non_empty_block_count -= 1;
            }
            // TODO: update fluid_count, ticking_block_count, ticking_fluid_count
            // once block property lookups are available
        }
        Ok(old)
    }

    /// Returns the biome ID at the given section-local coordinates.
    ///
    /// Coordinates are in biome resolution (0..4 each, representing 4-block spans).
    ///
    /// # Errors
    ///
    /// Returns an error if coordinates are out of bounds.
    pub fn get_biome(&self, x: usize, y: usize, z: usize) -> Result<u32, PalettedContainerError> {
        self.biomes.get(x, y, z)
    }

    /// Sets the biome ID at the given section-local coordinates (biome resolution).
    ///
    /// # Errors
    ///
    /// Returns an error if coordinates are out of bounds.
    pub fn set_biome(
        &mut self,
        x: usize,
        y: usize,
        z: usize,
        biome_id: u32,
    ) -> Result<(), PalettedContainerError> {
        self.biomes.set(x, y, z, biome_id)
    }

    /// Returns the count of non-air blocks in this section.
    #[must_use]
    pub fn non_empty_block_count(&self) -> i16 {
        self.non_empty_block_count
    }

    /// Returns true if the section contains only air.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.non_empty_block_count == 0
    }

    /// Serializes this section to bytes matching the network wire format.
    ///
    /// Format: `[i16 non_empty_count] [i16 fluid_count] [states data] [biomes data]`
    #[must_use]
    pub fn write_to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.non_empty_block_count.to_be_bytes());
        buf.extend_from_slice(&self.fluid_count.to_be_bytes());
        buf.extend(self.states.write_to_bytes());
        buf.extend(self.biomes.write_to_bytes());
        buf
    }

    /// Deserializes a section from bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the data is malformed.
    pub fn read_from_bytes(data: &mut &[u8]) -> Result<Self, PalettedContainerError> {
        if data.len() < 4 {
            return Err(PalettedContainerError::InsufficientData {
                expected: 4,
                actual: data.len(),
            });
        }
        let non_empty_block_count = i16::from_be_bytes([data[0], data[1]]);
        let fluid_count = i16::from_be_bytes([data[2], data[3]]);
        *data = &data[4..];

        let states = PalettedContainer::read_from_bytes(Strategy::BlockStates, data)?;
        let biomes = PalettedContainer::read_from_bytes(Strategy::Biomes, data)?;

        Ok(Self {
            non_empty_block_count,
            fluid_count,
            ticking_block_count: 0,
            ticking_fluid_count: 0,
            states,
            biomes,
        })
    }

    /// Recalculates `non_empty_block_count` by scanning all 4096 positions.
    ///
    /// Note: `fluid_count`, `ticking_block_count`, and `ticking_fluid_count` require
    /// block property lookups and are not recalculated here.
    pub fn recalculate_counts(&mut self) {
        self.non_empty_block_count = self.states.count_non_zero() as i16;
    }

    /// Returns the fluid count.
    #[must_use]
    pub fn fluid_count(&self) -> i16 {
        self.fluid_count
    }

    /// Returns the ticking block count.
    #[must_use]
    pub fn ticking_block_count(&self) -> i16 {
        self.ticking_block_count
    }

    /// Returns the ticking fluid count.
    #[must_use]
    pub fn ticking_fluid_count(&self) -> i16 {
        self.ticking_fluid_count
    }
}

impl Default for LevelChunkSection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_new_is_empty() {
        let section = LevelChunkSection::new();
        assert!(section.is_empty());
        assert_eq!(section.non_empty_block_count(), 0);
        assert_eq!(section.get_block_state(0, 0, 0).unwrap(), 0);
    }

    #[test]
    fn test_filled_section() {
        let section = LevelChunkSection::filled(1); // Stone
        assert!(!section.is_empty());
        assert_eq!(section.non_empty_block_count(), 4096);
        assert_eq!(section.get_block_state(0, 0, 0).unwrap(), 1);
    }

    #[test]
    fn test_set_updates_count() {
        let mut section = LevelChunkSection::new();
        section.set_block_state(0, 0, 0, 1).unwrap(); // Air → Stone
        assert_eq!(section.non_empty_block_count(), 1);

        section.set_block_state(0, 0, 0, 2).unwrap(); // Stone → Granite (non-air → non-air)
        assert_eq!(section.non_empty_block_count(), 1);

        section.set_block_state(0, 0, 0, 0).unwrap(); // Granite → Air
        assert_eq!(section.non_empty_block_count(), 0);
    }

    #[test]
    fn test_set_returns_old_value() {
        let mut section = LevelChunkSection::new();
        let old = section.set_block_state(5, 5, 5, 42).unwrap();
        assert_eq!(old, 0);
        let old = section.set_block_state(5, 5, 5, 99).unwrap();
        assert_eq!(old, 42);
    }

    #[test]
    fn test_biome_access() {
        let mut section = LevelChunkSection::new();
        section.set_biome(0, 0, 0, 5).unwrap();
        section.set_biome(3, 3, 3, 10).unwrap();
        assert_eq!(section.get_biome(0, 0, 0).unwrap(), 5);
        assert_eq!(section.get_biome(3, 3, 3).unwrap(), 10);
    }

    #[test]
    fn test_serialize_roundtrip() {
        let mut section = LevelChunkSection::new();
        section.set_block_state(0, 0, 0, 1).unwrap();
        section.set_block_state(1, 0, 0, 2).unwrap();
        section.set_biome(0, 0, 0, 3).unwrap();

        let bytes = section.write_to_bytes();
        let mut cursor = bytes.as_slice();
        let section2 = LevelChunkSection::read_from_bytes(&mut cursor).unwrap();

        assert_eq!(section2.non_empty_block_count(), 2);
        assert_eq!(section2.get_block_state(0, 0, 0).unwrap(), 1);
        assert_eq!(section2.get_block_state(1, 0, 0).unwrap(), 2);
        assert_eq!(section2.get_biome(0, 0, 0).unwrap(), 3);
    }

    #[test]
    fn test_recalculate_counts() {
        let mut section = LevelChunkSection::new();
        section.set_block_state(0, 0, 0, 1).unwrap();
        section.set_block_state(1, 1, 1, 2).unwrap();
        // Force wrong count
        section.non_empty_block_count = 99;
        section.recalculate_counts();
        assert_eq!(section.non_empty_block_count(), 2);
    }
}
