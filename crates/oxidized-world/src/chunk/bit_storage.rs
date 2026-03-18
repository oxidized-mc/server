//! Compact bit-packed storage for arrays of small integers.
//!
//! Packs N values of `bits` width into an array of `u64` longs, matching
//! the vanilla `SimpleBitStorage` format used on the wire.

use thiserror::Error;

/// Errors from [`BitStorage`] operations.
#[derive(Debug, Error)]
pub enum BitStorageError {
    /// The requested bits-per-entry is out of range.
    #[error("bits per entry must be 1..=32, got {0}")]
    InvalidBits(u8),

    /// Index out of bounds.
    #[error("index {index} out of bounds for size {size}")]
    OutOfBounds {
        /// The requested index.
        index: usize,
        /// The container size.
        size: usize,
    },

    /// Value exceeds the maximum for the configured bit width.
    #[error("value {value} exceeds max {max} for {bits}-bit storage")]
    ValueTooLarge {
        /// The value that was too large.
        value: u64,
        /// Maximum allowed value.
        max: u64,
        /// Configured bits per entry.
        bits: u8,
    },

    /// Raw data length does not match expected size.
    #[error("expected {expected} longs, got {actual}")]
    DataLengthMismatch {
        /// Expected number of longs.
        expected: usize,
        /// Actual number of longs.
        actual: usize,
    },
}

/// Compact bit-packed storage matching vanilla's `SimpleBitStorage`.
///
/// Packs `size` values of `bits` width into a `Vec<u64>`. Values do NOT
/// span across long boundaries — unused high bits in each long are wasted.
///
/// # Wire format
///
/// ```text
/// [VarInt: number of longs] [long₀] [long₁] … [longₙ]
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitStorage {
    data: Vec<u64>,
    bits: u8,
    size: usize,
    mask: u64,
    values_per_long: usize,
}

impl BitStorage {
    /// Creates a new zero-filled storage.
    ///
    /// # Errors
    ///
    /// Returns [`BitStorageError::InvalidBits`] if `bits` is 0 or > 32.
    pub fn new(bits: u8, size: usize) -> Result<Self, BitStorageError> {
        if bits == 0 || bits > 32 {
            return Err(BitStorageError::InvalidBits(bits));
        }
        let values_per_long = 64 / bits as usize;
        let num_longs = size.div_ceil(values_per_long);
        Ok(Self {
            data: vec![0u64; num_longs],
            bits,
            size,
            mask: (1u64 << bits) - 1,
            values_per_long,
        })
    }

    /// Creates storage from existing raw data.
    ///
    /// # Errors
    ///
    /// Returns an error if `bits` is invalid or `data` length doesn't match.
    pub fn from_raw(bits: u8, size: usize, data: Vec<u64>) -> Result<Self, BitStorageError> {
        if bits == 0 || bits > 32 {
            return Err(BitStorageError::InvalidBits(bits));
        }
        let values_per_long = 64 / bits as usize;
        let expected_longs = size.div_ceil(values_per_long);
        if data.len() != expected_longs {
            return Err(BitStorageError::DataLengthMismatch {
                expected: expected_longs,
                actual: data.len(),
            });
        }
        Ok(Self {
            data,
            bits,
            size,
            mask: (1u64 << bits) - 1,
            values_per_long,
        })
    }

    /// Returns the value at the given index.
    ///
    /// # Errors
    ///
    /// Returns [`BitStorageError::OutOfBounds`] if `index >= size`.
    pub fn get(&self, index: usize) -> Result<u64, BitStorageError> {
        if index >= self.size {
            return Err(BitStorageError::OutOfBounds {
                index,
                size: self.size,
            });
        }
        let long_index = index / self.values_per_long;
        let bit_offset = (index % self.values_per_long) * self.bits as usize;
        Ok((self.data[long_index] >> bit_offset) & self.mask)
    }

    /// Sets the value at the given index.
    ///
    /// # Errors
    ///
    /// Returns an error if `index` is out of bounds or `value` exceeds the bit width.
    pub fn set(&mut self, index: usize, value: u64) -> Result<(), BitStorageError> {
        if index >= self.size {
            return Err(BitStorageError::OutOfBounds {
                index,
                size: self.size,
            });
        }
        if value > self.mask {
            return Err(BitStorageError::ValueTooLarge {
                value,
                max: self.mask,
                bits: self.bits,
            });
        }
        let long_index = index / self.values_per_long;
        let bit_offset = (index % self.values_per_long) * self.bits as usize;
        self.data[long_index] &= !(self.mask << bit_offset);
        self.data[long_index] |= value << bit_offset;
        Ok(())
    }

    /// Sets the value at the given index and returns the previous value.
    ///
    /// # Errors
    ///
    /// Returns an error if `index` is out of bounds or `value` exceeds the bit width.
    pub fn get_and_set(&mut self, index: usize, value: u64) -> Result<u64, BitStorageError> {
        if index >= self.size {
            return Err(BitStorageError::OutOfBounds {
                index,
                size: self.size,
            });
        }
        if value > self.mask {
            return Err(BitStorageError::ValueTooLarge {
                value,
                max: self.mask,
                bits: self.bits,
            });
        }
        let long_index = index / self.values_per_long;
        let bit_offset = (index % self.values_per_long) * self.bits as usize;
        let old = (self.data[long_index] >> bit_offset) & self.mask;
        self.data[long_index] &= !(self.mask << bit_offset);
        self.data[long_index] |= value << bit_offset;
        Ok(old)
    }

    /// Returns the bits per entry.
    #[must_use]
    pub fn bits(&self) -> u8 {
        self.bits
    }

    /// Returns the number of values stored.
    #[must_use]
    pub fn size(&self) -> usize {
        self.size
    }

    /// Returns a reference to the raw packed data.
    #[must_use]
    pub fn raw(&self) -> &[u64] {
        &self.data
    }

    /// Consumes the storage and returns the raw packed data.
    #[must_use]
    pub fn into_raw(self) -> Vec<u64> {
        self.data
    }

    /// Returns the number of u64 longs in the backing array.
    #[must_use]
    pub fn num_longs(&self) -> usize {
        self.data.len()
    }

    /// Iterates over all values.
    pub fn iter(&self) -> BitStorageIter<'_> {
        BitStorageIter {
            storage: self,
            index: 0,
        }
    }
}

/// Iterator over all values in a [`BitStorage`].
pub struct BitStorageIter<'a> {
    storage: &'a BitStorage,
    index: usize,
}

impl Iterator for BitStorageIter<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.storage.size {
            return None;
        }
        let long_index = self.index / self.storage.values_per_long;
        let bit_offset = (self.index % self.storage.values_per_long) * self.storage.bits as usize;
        self.index += 1;
        Some((self.storage.data[long_index] >> bit_offset) & self.storage.mask)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.storage.size - self.index;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for BitStorageIter<'_> {}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_new_zero_filled() {
        let bs = BitStorage::new(4, 16).unwrap();
        assert_eq!(bs.bits(), 4);
        assert_eq!(bs.size(), 16);
        assert_eq!(bs.num_longs(), 1); // 16 values * 4 bits = 64 bits = 1 long
        for i in 0..16 {
            assert_eq!(bs.get(i).unwrap(), 0);
        }
    }

    #[test]
    fn test_set_and_get() {
        let mut bs = BitStorage::new(4, 16).unwrap();
        bs.set(0, 5).unwrap();
        bs.set(1, 15).unwrap();
        bs.set(15, 9).unwrap();
        assert_eq!(bs.get(0).unwrap(), 5);
        assert_eq!(bs.get(1).unwrap(), 15);
        assert_eq!(bs.get(15).unwrap(), 9);
        assert_eq!(bs.get(2).unwrap(), 0);
    }

    #[test]
    fn test_values_dont_span_longs() {
        // 5-bit values: 64/5 = 12 values per long, 4 bits wasted
        let mut bs = BitStorage::new(5, 24).unwrap();
        assert_eq!(bs.num_longs(), 2);
        bs.set(11, 31).unwrap(); // Last value in first long
        bs.set(12, 17).unwrap(); // First value in second long
        assert_eq!(bs.get(11).unwrap(), 31);
        assert_eq!(bs.get(12).unwrap(), 17);
    }

    #[test]
    fn test_1bit_storage() {
        let mut bs = BitStorage::new(1, 64).unwrap();
        assert_eq!(bs.num_longs(), 1);
        bs.set(0, 1).unwrap();
        bs.set(63, 1).unwrap();
        assert_eq!(bs.get(0).unwrap(), 1);
        assert_eq!(bs.get(1).unwrap(), 0);
        assert_eq!(bs.get(63).unwrap(), 1);
    }

    #[test]
    fn test_block_states_4096_entries() {
        // Typical 4-bit palette for block states
        let mut bs = BitStorage::new(4, 4096).unwrap();
        assert_eq!(bs.num_longs(), 256); // 4096/16 = 256
        bs.set(0, 1).unwrap();
        bs.set(4095, 15).unwrap();
        assert_eq!(bs.get(0).unwrap(), 1);
        assert_eq!(bs.get(4095).unwrap(), 15);
    }

    #[test]
    fn test_biome_storage() {
        // Biomes: 64 entries, typically 1-3 bits
        let bs = BitStorage::new(2, 64).unwrap();
        assert_eq!(bs.num_longs(), 2); // 64/32 = 2
    }

    #[test]
    fn test_from_raw_roundtrip() {
        let mut bs = BitStorage::new(4, 16).unwrap();
        for i in 0..16u64 {
            bs.set(i as usize, i).unwrap();
        }
        let raw = bs.raw().to_vec();
        let bs2 = BitStorage::from_raw(4, 16, raw).unwrap();
        for i in 0..16 {
            assert_eq!(bs2.get(i).unwrap(), i as u64);
        }
    }

    #[test]
    fn test_iter() {
        let mut bs = BitStorage::new(4, 4).unwrap();
        bs.set(0, 1).unwrap();
        bs.set(1, 2).unwrap();
        bs.set(2, 3).unwrap();
        bs.set(3, 4).unwrap();
        let values: Vec<u64> = bs.iter().collect();
        assert_eq!(values, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_out_of_bounds() {
        let bs = BitStorage::new(4, 16).unwrap();
        assert!(bs.get(16).is_err());
    }

    #[test]
    fn test_value_too_large() {
        let mut bs = BitStorage::new(4, 16).unwrap();
        assert!(bs.set(0, 16).is_err()); // Max is 15 for 4 bits
        assert!(bs.set(0, 15).is_ok());
    }

    #[test]
    fn test_invalid_bits() {
        assert!(BitStorage::new(0, 16).is_err());
        assert!(BitStorage::new(33, 16).is_err());
    }

    #[test]
    fn test_from_raw_length_mismatch() {
        assert!(BitStorage::from_raw(4, 16, vec![0u64; 2]).is_err());
    }

    #[test]
    fn test_overwrite_value() {
        let mut bs = BitStorage::new(8, 4).unwrap();
        bs.set(1, 255).unwrap();
        assert_eq!(bs.get(1).unwrap(), 255);
        bs.set(1, 42).unwrap();
        assert_eq!(bs.get(1).unwrap(), 42);
    }

    #[test]
    fn test_get_and_set() {
        let mut bs = BitStorage::new(4, 16).unwrap();
        let old = bs.get_and_set(0, 5).unwrap();
        assert_eq!(old, 0);
        let old = bs.get_and_set(0, 10).unwrap();
        assert_eq!(old, 5);
        assert_eq!(bs.get(0).unwrap(), 10);
        // Other values unchanged
        assert_eq!(bs.get(1).unwrap(), 0);
    }
}
