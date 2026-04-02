//! A 16×16×16 chunk section containing block states and biome data.
//!
//! This is the fundamental unit of chunk storage. Each section is independently
//! palette-compressed and serialized for the network packet.

use super::paletted_container::{PalettedContainer, PalettedContainerError, Strategy};
use oxidized_registry::BlockStateId;

/// A 16×16×16 section of a chunk column.
///
/// Contains palette-compressed block states and biome data, plus counts
/// of non-empty blocks and fluids for quick access.
///
/// # Examples
///
/// ```
/// use oxidized_world::chunk::LevelChunkSection;
///
/// let mut section = LevelChunkSection::new();
/// assert_eq!(section.non_empty_block_count(), 0);
///
/// // Set a stone block (state ID 1) at position (0, 0, 0)
/// section.set_block_state(0, 0, 0, 1).unwrap();
/// assert_eq!(section.get_block_state(0, 0, 0).unwrap(), 1);
/// assert_eq!(section.non_empty_block_count(), 1);
/// ```
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
        if block_state_id == 0 {
            return Self {
                non_empty_block_count: 0,
                fluid_count: 0,
                ticking_block_count: 0,
                ticking_fluid_count: 0,
                states,
                biomes: PalettedContainer::empty(Strategy::Biomes),
            };
        }
        #[allow(clippy::cast_possible_truncation)]
        let sid = BlockStateId(block_state_id as u16);
        Self {
            non_empty_block_count: 4096,
            fluid_count: if sid.is_liquid() { 4096 } else { 0 },
            ticking_block_count: if sid.ticks_randomly() { 4096 } else { 0 },
            ticking_fluid_count: if sid.is_liquid() && sid.ticks_randomly() {
                4096
            } else {
                0
            },
            states,
            biomes: PalettedContainer::empty(Strategy::Biomes),
        }
    }

    /// Creates a section from pre-built palette containers.
    ///
    /// Recalculates all counters by scanning the states container.
    #[must_use]
    pub fn from_parts(states: PalettedContainer, biomes: PalettedContainer) -> Self {
        let mut section = Self {
            non_empty_block_count: 0,
            fluid_count: 0,
            ticking_block_count: 0,
            ticking_fluid_count: 0,
            states,
            biomes,
        };
        section.recalculate_counts();
        section
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
    /// Updates all counters automatically: `non_empty_block_count`,
    /// `fluid_count`, `ticking_block_count`, and `ticking_fluid_count`.
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
            // Decrement counters for the old state.
            #[allow(clippy::cast_possible_truncation)]
            let old_state = BlockStateId(old as u16);
            if !old_state.is_air() {
                self.non_empty_block_count -= 1;
                if old_state.ticks_randomly() {
                    self.ticking_block_count -= 1;
                }
                if old_state.is_liquid() {
                    self.fluid_count -= 1;
                    if old_state.ticks_randomly() {
                        self.ticking_fluid_count -= 1;
                    }
                }
            }

            // Increment counters for the new state.
            #[allow(clippy::cast_possible_truncation)]
            let new_state = BlockStateId(state_id as u16);
            if !new_state.is_air() {
                self.non_empty_block_count += 1;
                if new_state.ticks_randomly() {
                    self.ticking_block_count += 1;
                }
                if new_state.is_liquid() {
                    self.fluid_count += 1;
                    if new_state.ticks_randomly() {
                        self.ticking_fluid_count += 1;
                    }
                }
            }
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

    /// Recalculates all counters by scanning all 4096 positions.
    ///
    /// Uses [`BlockStateId`] property lookups to determine fluid and ticking
    /// status for each block state.
    pub fn recalculate_counts(&mut self) {
        let mut non_empty: i16 = 0;
        let mut fluid: i16 = 0;
        let mut ticking_block: i16 = 0;
        let mut ticking_fluid: i16 = 0;

        self.states.for_each_value(|raw| {
            #[allow(clippy::cast_possible_truncation)]
            let sid = BlockStateId(raw as u16);
            if !sid.is_air() {
                non_empty += 1;
                if sid.ticks_randomly() {
                    ticking_block += 1;
                }
                if sid.is_liquid() {
                    fluid += 1;
                    if sid.ticks_randomly() {
                        ticking_fluid += 1;
                    }
                }
            }
        });

        self.non_empty_block_count = non_empty;
        self.fluid_count = fluid;
        self.ticking_block_count = ticking_block;
        self.ticking_fluid_count = ticking_fluid;
    }

    /// Returns a clone of the block states container.
    #[must_use]
    pub fn states_clone(&self) -> PalettedContainer {
        self.states.clone()
    }

    /// Returns a clone of the biomes container.
    #[must_use]
    pub fn biomes_clone(&self) -> PalettedContainer {
        self.biomes.clone()
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
#[allow(clippy::unwrap_used, clippy::panic)]
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

    /// Helper: look up the default state ID for a block by name.
    fn default_state(name: &str) -> u32 {
        use oxidized_registry::BlockRegistry;
        u32::from(
            BlockRegistry
                .default_state(name)
                .unwrap_or_else(|| panic!("{name} missing from registry"))
                .0,
        )
    }

    #[test]
    fn test_fluid_count_increments_for_water() {
        let water = default_state("minecraft:water");
        let mut section = LevelChunkSection::new();

        section.set_block_state(0, 0, 0, water).unwrap();
        assert_eq!(section.non_empty_block_count(), 1);
        assert_eq!(section.fluid_count(), 1);

        section.set_block_state(1, 0, 0, water).unwrap();
        assert_eq!(section.fluid_count(), 2);

        // Remove one water → decrement
        section.set_block_state(0, 0, 0, 0).unwrap();
        assert_eq!(section.fluid_count(), 1);
        assert_eq!(section.non_empty_block_count(), 1);
    }

    #[test]
    fn test_fluid_count_increments_for_lava() {
        let lava = default_state("minecraft:lava");
        let mut section = LevelChunkSection::new();

        section.set_block_state(0, 0, 0, lava).unwrap();
        assert_eq!(section.fluid_count(), 1);

        section.set_block_state(0, 0, 0, 0).unwrap();
        assert_eq!(section.fluid_count(), 0);
    }

    #[test]
    fn test_ticking_block_count_for_grass() {
        let grass = default_state("minecraft:grass_block");
        let stone = default_state("minecraft:stone");
        let mut section = LevelChunkSection::new();

        section.set_block_state(0, 0, 0, grass).unwrap();
        assert_eq!(section.ticking_block_count(), 1);
        assert_eq!(section.non_empty_block_count(), 1);

        // Stone doesn't tick randomly.
        section.set_block_state(1, 0, 0, stone).unwrap();
        assert_eq!(section.ticking_block_count(), 1);
        assert_eq!(section.non_empty_block_count(), 2);

        // Replace grass with stone → ticking decrements.
        section.set_block_state(0, 0, 0, stone).unwrap();
        assert_eq!(section.ticking_block_count(), 0);
        assert_eq!(section.non_empty_block_count(), 2);
    }

    #[test]
    fn test_stone_does_not_affect_fluid_or_ticking() {
        let stone = default_state("minecraft:stone");
        let mut section = LevelChunkSection::new();

        section.set_block_state(0, 0, 0, stone).unwrap();
        assert_eq!(section.non_empty_block_count(), 1);
        assert_eq!(section.fluid_count(), 0);
        assert_eq!(section.ticking_block_count(), 0);
        assert_eq!(section.ticking_fluid_count(), 0);
    }

    #[test]
    fn test_replace_water_with_stone_updates_all_counters() {
        let water = default_state("minecraft:water");
        let stone = default_state("minecraft:stone");
        let mut section = LevelChunkSection::new();

        section.set_block_state(0, 0, 0, water).unwrap();
        assert_eq!(section.fluid_count(), 1);

        section.set_block_state(0, 0, 0, stone).unwrap();
        assert_eq!(section.non_empty_block_count(), 1);
        assert_eq!(section.fluid_count(), 0);
    }

    #[test]
    fn test_filled_water_section_counters() {
        let water = default_state("minecraft:water");
        let section = LevelChunkSection::filled(water);
        assert_eq!(section.non_empty_block_count(), 4096);
        assert_eq!(section.fluid_count(), 4096);
    }

    #[test]
    fn test_recalculate_counts_with_fluids_and_ticking() {
        let water = default_state("minecraft:water");
        let grass = default_state("minecraft:grass_block");
        let stone = default_state("minecraft:stone");
        let mut section = LevelChunkSection::new();

        section.set_block_state(0, 0, 0, water).unwrap();
        section.set_block_state(1, 0, 0, grass).unwrap();
        section.set_block_state(2, 0, 0, stone).unwrap();

        // Force wrong counts.
        section.non_empty_block_count = 99;
        section.fluid_count = 99;
        section.ticking_block_count = 99;
        section.ticking_fluid_count = 99;

        section.recalculate_counts();
        assert_eq!(section.non_empty_block_count(), 3);
        assert_eq!(section.fluid_count(), 1);
        assert_eq!(section.ticking_block_count(), 1); // grass
        assert_eq!(section.ticking_fluid_count(), 0); // water doesn't tick randomly
    }

    #[test]
    fn test_set_same_state_no_counter_change() {
        let water = default_state("minecraft:water");
        let mut section = LevelChunkSection::new();

        section.set_block_state(0, 0, 0, water).unwrap();
        assert_eq!(section.fluid_count(), 1);

        // Setting the same state should not change counters.
        section.set_block_state(0, 0, 0, water).unwrap();
        assert_eq!(section.fluid_count(), 1);
        assert_eq!(section.non_empty_block_count(), 1);
    }

    #[test]
    fn test_from_parts_recalculates_all_counters() {
        let water = default_state("minecraft:water");
        let grass = default_state("minecraft:grass_block");

        let mut section = LevelChunkSection::new();
        section.set_block_state(0, 0, 0, water).unwrap();
        section.set_block_state(1, 0, 0, grass).unwrap();

        // Rebuild from parts — counters should be fully recalculated.
        let rebuilt = LevelChunkSection::from_parts(section.states_clone(), section.biomes_clone());
        assert_eq!(rebuilt.non_empty_block_count(), 2);
        assert_eq!(rebuilt.fluid_count(), 1);
        assert_eq!(rebuilt.ticking_block_count(), 1);
        assert_eq!(rebuilt.ticking_fluid_count(), 0);
    }
}
