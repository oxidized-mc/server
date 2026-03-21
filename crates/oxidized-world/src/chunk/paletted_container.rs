//! A palette-compressed container for block states or biome IDs.
//!
//! Automatically selects the most compact palette strategy based on the
//! number of distinct values, and upgrades when necessary.

use super::bit_storage::{BitStorage, BitStorageError};
use super::palette::{HashMapPalette, LinearPalette, PaletteAddResult, SingleValuePalette};
use super::palette_codec::{
    bits_for_count, build_palette_data_from_entries, build_palette_data_from_values,
    read_bit_storage, read_u8, read_varint, write_longs, write_varint,
};
use std::collections::HashSet;
use thiserror::Error;

/// Errors from [`PalettedContainer`] operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PalettedContainerError {
    /// Coordinate out of bounds.
    #[error("coordinate ({x}, {y}, {z}) out of bounds for size {size}")]
    OutOfBounds {
        /// X coordinate.
        x: usize,
        /// Y coordinate.
        y: usize,
        /// Z coordinate.
        z: usize,
        /// Container side length.
        size: usize,
    },

    /// Bit storage error.
    #[error("bit storage: {0}")]
    BitStorage(#[from] BitStorageError),

    /// Invalid palette type on wire.
    #[error("invalid bits per entry on wire: {0}")]
    InvalidBitsPerEntry(u8),

    /// Not enough data to deserialize.
    #[error("insufficient data: expected at least {expected} bytes, got {actual}")]
    InsufficientData {
        /// Minimum bytes expected.
        expected: usize,
        /// Bytes available.
        actual: usize,
    },

    /// Malformed VarInt encoding.
    #[error("malformed VarInt: exceeded 5-byte limit")]
    MalformedVarInt,
}

/// Strategy configuration for a paletted container.
///
/// Determines palette type thresholds for block states vs biomes.
///
/// # Examples
///
/// ```
/// use oxidized_world::chunk::paletted_container::Strategy;
///
/// assert_eq!(Strategy::BlockStates.size(), 4096); // 16³
/// assert_eq!(Strategy::Biomes.size(), 64);         // 4³
/// assert_eq!(Strategy::BlockStates.side(), 16);
/// assert_eq!(Strategy::Biomes.side(), 4);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// Block states: 4096 entries (16³), palette thresholds 0/1-4/5-8/9+.
    BlockStates,
    /// Biomes: 64 entries (4³), palette thresholds 0/1-3/4+.
    Biomes,
}

impl Strategy {
    /// Container size (number of entries).
    #[must_use]
    pub const fn size(self) -> usize {
        match self {
            Self::BlockStates => 4096,
            Self::Biomes => 64,
        }
    }

    /// Side length of the container.
    #[must_use]
    pub const fn side(self) -> usize {
        match self {
            Self::BlockStates => 16,
            Self::Biomes => 4,
        }
    }

    /// The minimum bits-per-entry for the global palette.
    #[must_use]
    pub const fn global_bits_threshold(self) -> u8 {
        match self {
            Self::BlockStates => 9,
            Self::Biomes => 4,
        }
    }

    /// The bits-per-entry to use for the Global palette's [`BitStorage`].
    ///
    /// Vanilla computes this as `ceillog2(registry.size())`:
    /// - Block states: 15 bits for 29,873 states
    /// - Biomes: 7 bits for 65 biomes
    ///
    /// The wire format byte and the actual long packing must both use this
    /// value so vanilla clients can reconstruct the correct `BitStorage`.
    #[must_use]
    pub const fn global_palette_bits(self) -> u8 {
        match self {
            Self::BlockStates => 15,
            Self::Biomes => 7,
        }
    }

    /// The minimum bits-per-entry for the hash map palette (blocks only).
    #[must_use]
    pub const fn hashmap_bits_threshold(self) -> u8 {
        match self {
            Self::BlockStates => 5,
            Self::Biomes => 255, // Never used for biomes
        }
    }

    /// The actual bits to use on the wire for a given logical bits count.
    /// Vanilla clamps block state linear palettes to a minimum of 4 bits.
    #[must_use]
    pub const fn storage_bits(self, bits: u8) -> u8 {
        match self {
            Self::BlockStates => {
                if bits <= 4 {
                    4
                } else {
                    bits
                }
            },
            Self::Biomes => bits,
        }
    }
}

/// The active palette variant inside a [`PalettedContainer`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PaletteData {
    /// 0 bits — single value fills the entire container.
    Single(SingleValuePalette),
    /// 1–4 bits (blocks) or 1–3 bits (biomes).
    Linear(LinearPalette, BitStorage),
    /// 5–8 bits (blocks only).
    HashMap(HashMapPalette, BitStorage),
    /// Direct registry IDs, no local palette.
    Global(BitStorage),
}

/// A palette-compressed container matching vanilla's `PalettedContainer`.
///
/// Stores either block state IDs (4096 entries, 16³) or biome IDs
/// (64 entries, 4³), automatically selecting the most compact
/// representation.
///
/// # Examples
///
/// ```
/// use oxidized_world::chunk::paletted_container::{PalettedContainer, Strategy};
///
/// let mut container = PalettedContainer::new(Strategy::BlockStates, 0);
/// container.set(1, 2, 3, 42).unwrap();
/// assert_eq!(container.get(1, 2, 3).unwrap(), 42);
/// assert_eq!(container.get(0, 0, 0).unwrap(), 0); // default value
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PalettedContainer {
    strategy: Strategy,
    data: PaletteData,
}

impl PalettedContainer {
    /// Creates a new container filled with a single value.
    #[must_use]
    pub fn new(strategy: Strategy, default_value: u32) -> Self {
        Self {
            strategy,
            data: PaletteData::Single(SingleValuePalette::with_value(default_value)),
        }
    }

    /// Creates an empty container (single-value with air/default=0).
    #[must_use]
    pub fn empty(strategy: Strategy) -> Self {
        Self::new(strategy, 0)
    }

    /// Returns the value at coordinates `(x, y, z)`.
    ///
    /// # Errors
    ///
    /// Returns an error if coordinates are out of bounds.
    pub fn get(&self, x: usize, y: usize, z: usize) -> Result<u32, PalettedContainerError> {
        let index = self.index(x, y, z)?;
        match &self.data {
            PaletteData::Single(p) => Ok(p.value().unwrap_or(0)),
            PaletteData::Linear(palette, storage) => {
                let palette_idx = storage.get(index)?;
                #[allow(clippy::cast_possible_truncation)]
                Ok(palette.value_for(palette_idx as u32).unwrap_or(0))
            },
            PaletteData::HashMap(palette, storage) => {
                let palette_idx = storage.get(index)?;
                #[allow(clippy::cast_possible_truncation)]
                Ok(palette.value_for(palette_idx as u32).unwrap_or(0))
            },
            PaletteData::Global(storage) => {
                let val = storage.get(index)?;
                #[allow(clippy::cast_possible_truncation)]
                Ok(val as u32)
            },
        }
    }

    /// Sets the value at coordinates `(x, y, z)`.
    ///
    /// May trigger a palette upgrade if the current palette is full.
    ///
    /// # Errors
    ///
    /// Returns an error if coordinates are out of bounds.
    pub fn set(
        &mut self,
        x: usize,
        y: usize,
        z: usize,
        value: u32,
    ) -> Result<(), PalettedContainerError> {
        let index = self.index(x, y, z)?;
        self.set_by_index(index, value)
    }

    /// Sets the value at coordinates `(x, y, z)` and returns the previous value.
    ///
    /// More efficient than calling `get()` then `set()` separately, as it
    /// avoids redundant index calculation and storage access.
    ///
    /// # Errors
    ///
    /// Returns an error if coordinates are out of bounds.
    pub fn get_and_set(
        &mut self,
        x: usize,
        y: usize,
        z: usize,
        value: u32,
    ) -> Result<u32, PalettedContainerError> {
        let index = self.index(x, y, z)?;
        let old = self.get_by_index(index);
        if old != value {
            self.set_by_index(index, value)?;
        }
        Ok(old)
    }

    /// Sets the value at a flat index.
    fn set_by_index(&mut self, index: usize, value: u32) -> Result<(), PalettedContainerError> {
        match &mut self.data {
            PaletteData::Single(palette) => {
                let result = palette.index_or_insert(value);
                if result == PaletteAddResult::NeedsResize {
                    self.upgrade_and_set(index, value)?;
                }
            },
            PaletteData::Linear(palette, storage) => {
                let result = palette.index_or_insert(value);
                match result {
                    PaletteAddResult::Existing(idx) | PaletteAddResult::New(idx) => {
                        storage.set(index, u64::from(idx))?;
                    },
                    PaletteAddResult::NeedsResize => {
                        self.upgrade_and_set(index, value)?;
                    },
                }
            },
            PaletteData::HashMap(palette, storage) => {
                let result = palette.index_or_insert(value);
                match result {
                    PaletteAddResult::Existing(idx) | PaletteAddResult::New(idx) => {
                        storage.set(index, u64::from(idx))?;
                    },
                    PaletteAddResult::NeedsResize => {
                        self.upgrade_and_set(index, value)?;
                    },
                }
            },
            PaletteData::Global(storage) => {
                storage.set(index, u64::from(value))?;
            },
        }
        Ok(())
    }

    /// Upgrades the palette to the next tier and sets the value.
    fn upgrade_and_set(
        &mut self,
        set_index: usize,
        set_value: u32,
    ) -> Result<(), PalettedContainerError> {
        let size = self.strategy.size();

        // Collect all current values
        let mut values = Vec::with_capacity(size);
        for i in 0..size {
            values.push(self.get_by_index(i));
        }
        values[set_index] = set_value;

        // Count distinct values to determine the right palette tier
        let distinct_count = {
            let mut seen = HashSet::new();
            for &v in &values {
                seen.insert(v);
            }
            seen.len()
        };
        let bits_needed = bits_for_count(distinct_count);

        self.data = build_palette_data_from_values(self.strategy, bits_needed, &values)?;
        Ok(())
    }

    /// Returns the value at a flat index (no bounds check on container coordinates).
    fn get_by_index(&self, index: usize) -> u32 {
        match &self.data {
            PaletteData::Single(p) => p.value().unwrap_or(0),
            PaletteData::Linear(palette, storage) => {
                #[allow(clippy::cast_possible_truncation)]
                let palette_idx = storage.get(index).unwrap_or(0) as u32;
                palette.value_for(palette_idx).unwrap_or(0)
            },
            PaletteData::HashMap(palette, storage) => {
                #[allow(clippy::cast_possible_truncation)]
                let palette_idx = storage.get(index).unwrap_or(0) as u32;
                palette.value_for(palette_idx).unwrap_or(0)
            },
            PaletteData::Global(storage) => {
                #[allow(clippy::cast_possible_truncation)]
                {
                    storage.get(index).unwrap_or(0) as u32
                }
            },
        }
    }

    /// Computes the flat index for 3D coordinates.
    fn index(&self, x: usize, y: usize, z: usize) -> Result<usize, PalettedContainerError> {
        let side = self.strategy.side();
        if x >= side || y >= side || z >= side {
            return Err(PalettedContainerError::OutOfBounds {
                x,
                y,
                z,
                size: side,
            });
        }
        // Vanilla index order: ((y * side) + z) * side + x
        Ok(((y * side) + z) * side + x)
    }

    /// Serializes this container to bytes matching the Minecraft wire format.
    ///
    /// Format: `[u8 bits_per_entry] [palette data] [VarInt num_longs] [longs...]`
    #[must_use]
    pub fn write_to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        match &self.data {
            PaletteData::Single(palette) => {
                buf.push(0); // 0 bits per entry
                // Palette: single VarInt
                write_varint(&mut buf, palette.value().unwrap_or(0) as i32);
                // Data: 0 longs
                write_varint(&mut buf, 0);
            },
            PaletteData::Linear(palette, storage) => {
                buf.push(storage.bits());
                // Palette: VarInt count + VarInt entries
                write_varint(&mut buf, palette.len() as i32);
                for &v in palette.entries() {
                    write_varint(&mut buf, v as i32);
                }
                // Data longs
                write_longs(&mut buf, storage.raw());
            },
            PaletteData::HashMap(palette, storage) => {
                buf.push(storage.bits());
                write_varint(&mut buf, palette.len() as i32);
                for &v in palette.entries() {
                    write_varint(&mut buf, v as i32);
                }
                write_longs(&mut buf, storage.raw());
            },
            PaletteData::Global(storage) => {
                buf.push(self.strategy.global_palette_bits());
                // Global palette: no palette data on wire
                // Data longs
                write_longs(&mut buf, storage.raw());
            },
        }
        buf
    }

    /// Deserializes a container from bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the data is malformed.
    pub fn read_from_bytes(
        strategy: Strategy,
        data: &mut &[u8],
    ) -> Result<Self, PalettedContainerError> {
        let bits_per_entry = read_u8(data)?;
        let size = strategy.size();

        if bits_per_entry == 0 {
            // Single value
            let value = read_varint(data)? as u32;
            let _num_longs = read_varint(data)?; // Should be 0
            Ok(Self {
                strategy,
                data: PaletteData::Single(SingleValuePalette::with_value(value)),
            })
        } else if bits_per_entry >= strategy.global_bits_threshold() {
            // Global palette — use registry-derived bits for BitStorage, not wire byte
            let global_bits = strategy.global_palette_bits();
            let storage = read_bit_storage(global_bits, size, data)?;
            Ok(Self {
                strategy,
                data: PaletteData::Global(storage),
            })
        } else {
            // Linear or HashMap palette — read entries then select tier
            let palette_len = read_varint(data)? as usize;
            let mut entries = Vec::with_capacity(palette_len);
            for _ in 0..palette_len {
                entries.push(read_varint(data)? as u32);
            }
            let storage = read_bit_storage(bits_per_entry, size, data)?;
            let palette_data =
                build_palette_data_from_entries(strategy, bits_per_entry, entries, storage);
            Ok(Self {
                strategy,
                data: palette_data,
            })
        }
    }

    /// Returns the palette strategy.
    #[must_use]
    pub fn strategy(&self) -> Strategy {
        self.strategy
    }

    /// Returns the bits per entry for the current palette variant.
    ///
    /// - `0` for single-value palette
    /// - `4..=8` for linear/hashmap palettes (block states)
    /// - `1..=3` for linear palettes (biomes)
    /// - `global_palette_bits()` for global palette
    #[must_use]
    pub fn bits_per_entry(&self) -> u8 {
        match &self.data {
            PaletteData::Single(_) => 0,
            PaletteData::Linear(_, storage)
            | PaletteData::HashMap(_, storage)
            | PaletteData::Global(storage) => storage.bits(),
        }
    }

    /// Creates a container from NBT disk data: a list of palette entry IDs
    /// and a packed `i64` long array.
    ///
    /// This mirrors the Anvil on-disk format where the palette is stored as
    /// NBT and the data is a `LongArray` of packed indices.
    ///
    /// If `palette_ids` has exactly one entry and `data_longs` is empty, a
    /// single-value palette is used. Otherwise, the appropriate palette tier
    /// is selected based on palette size.
    ///
    /// # Errors
    ///
    /// Returns an error if the data is malformed or palette IDs are invalid.
    pub fn from_nbt_data(
        strategy: Strategy,
        palette_ids: Vec<u32>,
        data_longs: &[i64],
    ) -> Result<Self, PalettedContainerError> {
        let size = strategy.size();

        if palette_ids.len() == 1 && data_longs.is_empty() {
            return Ok(Self {
                strategy,
                data: PaletteData::Single(SingleValuePalette::with_value(palette_ids[0])),
            });
        }

        let bits = bits_for_count(palette_ids.len());
        let storage_bits = strategy.storage_bits(bits);

        // Convert i64 longs to u64
        let raw: Vec<u64> = data_longs.iter().map(|&l| l as u64).collect();
        let storage = BitStorage::from_raw(storage_bits, size, raw)?;

        let data = if storage_bits >= strategy.global_bits_threshold() {
            // Global palette — entries ARE the registry IDs, stored directly.
            // Remap from palette indices to actual registry IDs.
            let global_bits = strategy.global_palette_bits();
            let mut global_storage = BitStorage::new(global_bits, size)?;
            for i in 0..size {
                #[allow(clippy::cast_possible_truncation)]
                let palette_idx = storage.get(i)? as usize;
                let registry_id = palette_ids.get(palette_idx).copied().unwrap_or(0);
                global_storage.set(i, u64::from(registry_id))?;
            }
            PaletteData::Global(global_storage)
        } else {
            build_palette_data_from_entries(strategy, storage_bits, palette_ids, storage)
        };

        Ok(Self { strategy, data })
    }

    /// Counts distinct non-zero values (useful for `non_empty_block_count`).
    #[must_use]
    pub fn count_non_zero(&self) -> u16 {
        let size = self.strategy.size();
        let mut count = 0u16;
        for i in 0..size {
            if self.get_by_index(i) != 0 {
                count = count.saturating_add(1);
            }
        }
        count
    }

    /// Serializes this container to NBT disk format.
    ///
    /// Returns `(palette_ids, data_longs)` matching the Anvil on-disk format.
    /// The palette IDs are the registry values stored in the palette, and the
    /// data longs are the packed palette indices.
    ///
    /// - For single-value palettes: `(vec![value], vec![])`.
    /// - For Linear/HashMap: palette entries + storage longs.
    /// - For Global: all values collected into a fresh palette + repacked storage.
    #[must_use]
    pub fn to_nbt_data(&self) -> (Vec<u32>, Vec<i64>) {
        match &self.data {
            PaletteData::Single(palette) => {
                (vec![palette.value().unwrap_or(0)], Vec::new())
            },
            PaletteData::Linear(palette, storage) => {
                let entries = palette.entries().to_vec();
                let longs: Vec<i64> = storage.raw().iter().map(|&v| v as i64).collect();
                (entries, longs)
            },
            PaletteData::HashMap(palette, storage) => {
                let entries = palette.entries().to_vec();
                let longs: Vec<i64> = storage.raw().iter().map(|&v| v as i64).collect();
                (entries, longs)
            },
            PaletteData::Global(storage) => {
                // Re-palette global data: collect all values, build a compact palette
                let size = self.strategy.size();
                let mut seen = Vec::new();
                let mut indices = Vec::with_capacity(size);
                for i in 0..size {
                    #[allow(clippy::cast_possible_truncation)]
                    let val = storage.get(i).unwrap_or(0) as u32;
                    let idx = match seen.iter().position(|&v| v == val) {
                        Some(pos) => pos,
                        None => {
                            seen.push(val);
                            seen.len() - 1
                        },
                    };
                    indices.push(idx);
                }

                let bits = bits_for_count(seen.len());
                let sb = self.strategy.storage_bits(bits);
                // Build packed longs
                if let Ok(mut new_storage) = BitStorage::new(sb, size) {
                    for (i, &idx) in indices.iter().enumerate() {
                        let _ = new_storage.set(i, idx as u64);
                    }
                    let longs: Vec<i64> = new_storage.raw().iter().map(|&v| v as i64).collect();
                    (seen, longs)
                } else {
                    // Fallback: shouldn't happen, but return safe empty
                    (vec![0], Vec::new())
                }
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_snapshots() {
        insta::assert_snapshot!(
            "out_of_bounds",
            format!(
                "{}",
                PalettedContainerError::OutOfBounds {
                    x: 17,
                    y: 0,
                    z: 3,
                    size: 16,
                }
            )
        );
        insta::assert_snapshot!(
            "invalid_bits_per_entry",
            format!("{}", PalettedContainerError::InvalidBitsPerEntry(99))
        );
        insta::assert_snapshot!(
            "insufficient_data",
            format!(
                "{}",
                PalettedContainerError::InsufficientData {
                    expected: 256,
                    actual: 10,
                }
            )
        );
        insta::assert_snapshot!(
            "malformed_varint",
            format!("{}", PalettedContainerError::MalformedVarInt)
        );
    }

    #[test]
    fn test_empty_container() {
        let c = PalettedContainer::empty(Strategy::BlockStates);
        assert_eq!(c.get(0, 0, 0).unwrap(), 0);
        assert_eq!(c.get(15, 15, 15).unwrap(), 0);
    }

    #[test]
    fn test_single_value_fill() {
        let c = PalettedContainer::new(Strategy::BlockStates, 42);
        for x in 0..16 {
            for z in 0..16 {
                assert_eq!(c.get(x, 0, z).unwrap(), 42);
            }
        }
    }

    #[test]
    fn test_set_triggers_upgrade_from_single() {
        let mut c = PalettedContainer::new(Strategy::BlockStates, 0);
        c.set(0, 0, 0, 1).unwrap();
        assert_eq!(c.get(0, 0, 0).unwrap(), 1);
        // Rest should still be 0
        assert_eq!(c.get(1, 0, 0).unwrap(), 0);
        assert_eq!(c.get(15, 15, 15).unwrap(), 0);
    }

    #[test]
    fn test_multiple_values() {
        let mut c = PalettedContainer::empty(Strategy::BlockStates);
        c.set(0, 0, 0, 1).unwrap();
        c.set(1, 0, 0, 2).unwrap();
        c.set(0, 1, 0, 3).unwrap();
        c.set(0, 0, 1, 4).unwrap();
        assert_eq!(c.get(0, 0, 0).unwrap(), 1);
        assert_eq!(c.get(1, 0, 0).unwrap(), 2);
        assert_eq!(c.get(0, 1, 0).unwrap(), 3);
        assert_eq!(c.get(0, 0, 1).unwrap(), 4);
    }

    #[test]
    fn test_upgrade_to_hashmap() {
        let mut c = PalettedContainer::empty(Strategy::BlockStates);
        // Fill with 17 distinct values to trigger HashMap palette (>16 = > 4 bits)
        for i in 0..17u32 {
            let x = (i % 16) as usize;
            let y = (i / 16) as usize;
            c.set(x, y, 0, i + 1).unwrap();
        }
        for i in 0..17u32 {
            let x = (i % 16) as usize;
            let y = (i / 16) as usize;
            assert_eq!(c.get(x, y, 0).unwrap(), i + 1);
        }
    }

    #[test]
    fn test_biome_container() {
        let mut c = PalettedContainer::empty(Strategy::Biomes);
        c.set(0, 0, 0, 1).unwrap();
        c.set(3, 3, 3, 5).unwrap();
        assert_eq!(c.get(0, 0, 0).unwrap(), 1);
        assert_eq!(c.get(3, 3, 3).unwrap(), 5);
        assert_eq!(c.get(1, 1, 1).unwrap(), 0);
    }

    #[test]
    fn test_out_of_bounds() {
        let c = PalettedContainer::empty(Strategy::BlockStates);
        assert!(c.get(16, 0, 0).is_err());
    }

    #[test]
    fn test_biome_out_of_bounds() {
        let c = PalettedContainer::empty(Strategy::Biomes);
        assert!(c.get(4, 0, 0).is_err());
    }

    #[test]
    fn test_serialize_roundtrip_single() {
        let c = PalettedContainer::new(Strategy::BlockStates, 42);
        let bytes = c.write_to_bytes();
        let mut cursor = bytes.as_slice();
        let c2 = PalettedContainer::read_from_bytes(Strategy::BlockStates, &mut cursor).unwrap();
        assert_eq!(c2.get(0, 0, 0).unwrap(), 42);
        assert_eq!(c2.get(15, 15, 15).unwrap(), 42);
    }

    #[test]
    fn test_serialize_roundtrip_linear() {
        let mut c = PalettedContainer::empty(Strategy::BlockStates);
        c.set(0, 0, 0, 1).unwrap();
        c.set(1, 0, 0, 2).unwrap();
        c.set(2, 0, 0, 3).unwrap();

        let bytes = c.write_to_bytes();
        let mut cursor = bytes.as_slice();
        let c2 = PalettedContainer::read_from_bytes(Strategy::BlockStates, &mut cursor).unwrap();
        assert_eq!(c2.get(0, 0, 0).unwrap(), 1);
        assert_eq!(c2.get(1, 0, 0).unwrap(), 2);
        assert_eq!(c2.get(2, 0, 0).unwrap(), 3);
        assert_eq!(c2.get(3, 0, 0).unwrap(), 0);
    }

    #[test]
    fn test_serialize_roundtrip_biome() {
        let mut c = PalettedContainer::empty(Strategy::Biomes);
        c.set(0, 0, 0, 10).unwrap();
        c.set(3, 3, 3, 20).unwrap();

        let bytes = c.write_to_bytes();
        let mut cursor = bytes.as_slice();
        let c2 = PalettedContainer::read_from_bytes(Strategy::Biomes, &mut cursor).unwrap();
        assert_eq!(c2.get(0, 0, 0).unwrap(), 10);
        assert_eq!(c2.get(3, 3, 3).unwrap(), 20);
    }

    #[test]
    fn test_count_non_zero() {
        let mut c = PalettedContainer::empty(Strategy::BlockStates);
        assert_eq!(c.count_non_zero(), 0);
        c.set(0, 0, 0, 1).unwrap();
        c.set(1, 0, 0, 2).unwrap();
        assert_eq!(c.count_non_zero(), 2);
    }

    #[test]
    fn test_bits_for_count() {
        assert_eq!(bits_for_count(0), 0);
        assert_eq!(bits_for_count(1), 0);
        assert_eq!(bits_for_count(2), 1);
        assert_eq!(bits_for_count(3), 2);
        assert_eq!(bits_for_count(4), 2);
        assert_eq!(bits_for_count(5), 3);
        assert_eq!(bits_for_count(16), 4);
        assert_eq!(bits_for_count(17), 5);
        assert_eq!(bits_for_count(256), 8);
        assert_eq!(bits_for_count(257), 9);
    }

    #[test]
    fn test_biome_global_palette_roundtrip() {
        // Biomes use Global palette at 4+ distinct values (4+ bits).
        // This test ensures deserialization correctly picks Global over Linear.
        let mut c = PalettedContainer::empty(Strategy::Biomes);
        // Insert 9 distinct biome IDs to force global palette (>3 bits → global)
        for i in 0..9u32 {
            let x = (i % 4) as usize;
            let y = (i / 4) as usize;
            c.set(x, y, 0, i + 1).unwrap();
        }

        let bytes = c.write_to_bytes();
        let mut cursor = bytes.as_slice();
        let c2 = PalettedContainer::read_from_bytes(Strategy::Biomes, &mut cursor).unwrap();
        for i in 0..9u32 {
            let x = (i % 4) as usize;
            let y = (i / 4) as usize;
            assert_eq!(c2.get(x, y, 0).unwrap(), i + 1);
        }
    }

    #[test]
    fn test_block_states_global_palette_roundtrip() {
        // Block states use Global palette at 9+ bits (>256 distinct values).
        // After the fix, Global palette uses global_palette_bits (15 for blocks).
        let mut c = PalettedContainer::empty(Strategy::BlockStates);
        // Insert 257 distinct values to trigger Global palette (>8 bits)
        for i in 0..257u32 {
            let x = (i % 16) as usize;
            let y = ((i / 16) % 16) as usize;
            let z = (i / 256) as usize;
            c.set(x, y, z, i + 1).unwrap();
        }

        assert_eq!(c.bits_per_entry(), 15); // global_palette_bits for blocks

        let bytes = c.write_to_bytes();
        let mut cursor = bytes.as_slice();
        let c2 = PalettedContainer::read_from_bytes(Strategy::BlockStates, &mut cursor).unwrap();
        for i in 0..257u32 {
            let x = (i % 16) as usize;
            let y = ((i / 16) % 16) as usize;
            let z = (i / 256) as usize;
            assert_eq!(c2.get(x, y, z).unwrap(), i + 1);
        }
    }

    #[test]
    fn test_get_and_set() {
        let mut c = PalettedContainer::empty(Strategy::BlockStates);
        let old = c.get_and_set(0, 0, 0, 42).unwrap();
        assert_eq!(old, 0);
        let old = c.get_and_set(0, 0, 0, 99).unwrap();
        assert_eq!(old, 42);
        // Same value returns it without modification
        let old = c.get_and_set(0, 0, 0, 99).unwrap();
        assert_eq!(old, 99);
    }

    #[test]
    fn test_bits_per_entry() {
        let c = PalettedContainer::empty(Strategy::BlockStates);
        assert_eq!(c.bits_per_entry(), 0); // SingleValue

        let mut c = PalettedContainer::empty(Strategy::BlockStates);
        c.set(0, 0, 0, 1).unwrap();
        assert_eq!(c.bits_per_entry(), 4); // Linear (clamped to 4 for blocks)

        let c = PalettedContainer::empty(Strategy::Biomes);
        assert_eq!(c.bits_per_entry(), 0); // SingleValue
    }

    // ── to_nbt_data / from_nbt_data roundtrip ──────────────────────

    #[test]
    fn test_to_nbt_data_single_value() {
        let c = PalettedContainer::new(Strategy::BlockStates, 42);
        let (palette, data) = c.to_nbt_data();
        assert_eq!(palette, vec![42]);
        assert!(data.is_empty());
    }

    #[test]
    fn test_to_nbt_data_roundtrip_linear() {
        let mut c = PalettedContainer::empty(Strategy::BlockStates);
        c.set(0, 0, 0, 1).unwrap();
        c.set(1, 0, 0, 2).unwrap();
        c.set(2, 0, 0, 3).unwrap();

        let (palette, data) = c.to_nbt_data();
        let c2 = PalettedContainer::from_nbt_data(Strategy::BlockStates, palette, &data).unwrap();

        assert_eq!(c2.get(0, 0, 0).unwrap(), 1);
        assert_eq!(c2.get(1, 0, 0).unwrap(), 2);
        assert_eq!(c2.get(2, 0, 0).unwrap(), 3);
        assert_eq!(c2.get(3, 0, 0).unwrap(), 0);
    }

    #[test]
    fn test_to_nbt_data_roundtrip_biome() {
        let mut c = PalettedContainer::empty(Strategy::Biomes);
        c.set(0, 0, 0, 10).unwrap();
        c.set(3, 3, 3, 20).unwrap();

        let (palette, data) = c.to_nbt_data();
        let c2 = PalettedContainer::from_nbt_data(Strategy::Biomes, palette, &data).unwrap();
        assert_eq!(c2.get(0, 0, 0).unwrap(), 10);
        assert_eq!(c2.get(3, 3, 3).unwrap(), 20);
    }

    #[test]
    fn test_to_nbt_data_roundtrip_global() {
        let mut c = PalettedContainer::empty(Strategy::BlockStates);
        // Insert 257 distinct values to trigger Global palette
        for i in 0..257u32 {
            let x = (i % 16) as usize;
            let y = ((i / 16) % 16) as usize;
            let z = (i / 256) as usize;
            c.set(x, y, z, i + 1).unwrap();
        }
        assert_eq!(c.bits_per_entry(), 15); // Global

        let (palette, data) = c.to_nbt_data();
        let c2 = PalettedContainer::from_nbt_data(Strategy::BlockStates, palette, &data).unwrap();
        for i in 0..257u32 {
            let x = (i % 16) as usize;
            let y = ((i / 16) % 16) as usize;
            let z = (i / 256) as usize;
            assert_eq!(c2.get(x, y, z).unwrap(), i + 1);
        }
    }
}
