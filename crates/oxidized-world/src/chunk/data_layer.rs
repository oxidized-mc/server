//! Nibble-packed light data layer (4 bits per block position).
//!
//! Each section stores 2048 bytes (16×16×16 ÷ 2) of light values in the
//! range 0–15. Used for both sky light and block light.

/// Size of a data layer in bytes (16 × 16 × 16 / 2).
pub const DATA_LAYER_SIZE: usize = 2048;

/// A nibble-packed array of 4096 4-bit light values.
///
/// Matches vanilla's `DataLayer` format.
///
/// # Examples
///
/// ```
/// use oxidized_world::chunk::DataLayer;
///
/// let mut layer = DataLayer::new();
/// assert!(layer.is_empty());
///
/// layer.set(0, 0, 0, 15); // max light level
/// assert_eq!(layer.get(0, 0, 0), 15);
/// assert!(!layer.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataLayer {
    data: Vec<u8>,
}

impl DataLayer {
    /// Creates a new data layer filled with zeros (no light).
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: vec![0u8; DATA_LAYER_SIZE],
        }
    }

    /// Creates a data layer filled with a uniform value (0–15).
    #[must_use]
    pub fn filled(value: u8) -> Self {
        let nibble = value & 0x0F;
        let byte = nibble | (nibble << 4);
        Self {
            data: vec![byte; DATA_LAYER_SIZE],
        }
    }

    /// Creates a data layer from raw bytes.
    ///
    /// Returns `None` if the slice is not exactly 2048 bytes.
    #[must_use]
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() != DATA_LAYER_SIZE {
            return None;
        }
        Some(Self {
            data: data.to_vec(),
        })
    }

    /// Computes the flat index for a 16³ section coordinate.
    const fn section_index(x: usize, y: usize, z: usize) -> usize {
        (y << 8) | (z << 4) | x
    }

    /// Returns the light value at `(x, y, z)` within the section (0–15 each).
    ///
    /// # Panics
    ///
    /// Panics if any coordinate is >= 16.
    #[must_use]
    pub fn get(&self, x: usize, y: usize, z: usize) -> u8 {
        assert!(
            x < 16 && y < 16 && z < 16,
            "DataLayer::get coordinate out of bounds: ({x}, {y}, {z}), expected 0..16"
        );
        let index = Self::section_index(x, y, z);
        let byte_pos = index >> 1;
        if index & 1 == 0 {
            self.data[byte_pos] & 0x0F
        } else {
            (self.data[byte_pos] >> 4) & 0x0F
        }
    }

    /// Sets the light value at `(x, y, z)` within the section.
    ///
    /// # Panics
    ///
    /// Panics if any coordinate is >= 16 or `value` > 15.
    pub fn set(&mut self, x: usize, y: usize, z: usize, value: u8) {
        assert!(
            x < 16 && y < 16 && z < 16,
            "DataLayer::set coordinate out of bounds: ({x}, {y}, {z}), expected 0..16"
        );
        assert!(
            value <= 15,
            "DataLayer::set value out of range: {value}, expected 0..=15"
        );
        let index = Self::section_index(x, y, z);
        let byte_pos = index >> 1;
        if index & 1 == 0 {
            self.data[byte_pos] = (self.data[byte_pos] & 0xF0) | (value & 0x0F);
        } else {
            self.data[byte_pos] = (self.data[byte_pos] & 0x0F) | ((value & 0x0F) << 4);
        }
    }

    /// Returns the raw bytes (2048 bytes).
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Consumes and returns the raw bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    /// Returns true if all values are zero.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.iter().all(|&b| b == 0)
    }
}

impl Default for DataLayer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_new_is_all_zero() {
        let dl = DataLayer::new();
        for x in 0..16 {
            for y in 0..16 {
                for z in 0..16 {
                    assert_eq!(dl.get(x, y, z), 0);
                }
            }
        }
    }

    #[test]
    fn test_filled() {
        let dl = DataLayer::filled(15);
        assert_eq!(dl.get(0, 0, 0), 15);
        assert_eq!(dl.get(15, 15, 15), 15);
    }

    #[test]
    fn test_set_and_get() {
        let mut dl = DataLayer::new();
        dl.set(0, 0, 0, 10);
        dl.set(1, 0, 0, 5);
        dl.set(15, 15, 15, 15);
        assert_eq!(dl.get(0, 0, 0), 10);
        assert_eq!(dl.get(1, 0, 0), 5);
        assert_eq!(dl.get(15, 15, 15), 15);
        // Unchanged positions
        assert_eq!(dl.get(2, 0, 0), 0);
    }

    #[test]
    fn test_nibble_independence() {
        let mut dl = DataLayer::new();
        // Set even-indexed nibble
        dl.set(0, 0, 0, 12);
        // Set odd-indexed nibble in same byte
        dl.set(1, 0, 0, 7);
        assert_eq!(dl.get(0, 0, 0), 12);
        assert_eq!(dl.get(1, 0, 0), 7);
    }

    #[test]
    fn test_from_bytes() {
        let mut raw = vec![0u8; DATA_LAYER_SIZE];
        raw[0] = 0xAB; // nibble[0]=0xB, nibble[1]=0xA
        let dl = DataLayer::from_bytes(&raw).unwrap();
        assert_eq!(dl.get(0, 0, 0), 0xB);
        assert_eq!(dl.get(1, 0, 0), 0xA);
    }

    #[test]
    fn test_from_bytes_wrong_size() {
        assert!(DataLayer::from_bytes(&[0u8; 100]).is_none());
    }

    #[test]
    fn test_is_empty() {
        assert!(DataLayer::new().is_empty());
        let mut dl = DataLayer::new();
        dl.set(5, 5, 5, 1);
        assert!(!dl.is_empty());
    }

    #[test]
    fn test_roundtrip_bytes() {
        let mut dl = DataLayer::new();
        dl.set(3, 7, 11, 14);
        let bytes = dl.as_bytes().to_vec();
        let dl2 = DataLayer::from_bytes(&bytes).unwrap();
        assert_eq!(dl2.get(3, 7, 11), 14);
    }

    #[test]
    fn test_filled_15_byte_pattern() {
        let dl = DataLayer::filled(15);
        // Every byte should be 0xFF (both nibbles = 15)
        assert!(dl.as_bytes().iter().all(|&b| b == 0xFF));
        assert_eq!(dl.as_bytes().len(), DATA_LAYER_SIZE);
        insta::assert_snapshot!(
            "filled_15_first_16_bytes",
            format!("{:02X?}", &dl.as_bytes()[..16])
        );
    }

    #[test]
    fn test_filled_0_byte_pattern() {
        let dl = DataLayer::filled(0);
        assert!(dl.as_bytes().iter().all(|&b| b == 0x00));
    }

    #[test]
    fn test_filled_arbitrary_value() {
        for v in 0..=15 {
            let dl = DataLayer::filled(v);
            let expected_byte = v | (v << 4);
            assert!(
                dl.as_bytes().iter().all(|&b| b == expected_byte),
                "filled({v}) should produce byte {expected_byte:#04X}"
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// get(x,y,z) returns the value previously set(x,y,z) for all valid coordinates.
        #[test]
        fn proptest_get_returns_set_value(
            x in 0usize..16,
            y in 0usize..16,
            z in 0usize..16,
            value in 0u8..=15,
        ) {
            let mut dl = DataLayer::new();
            dl.set(x, y, z, value);
            prop_assert_eq!(dl.get(x, y, z), value);
        }

        /// Setting one nibble never corrupts its adjacent nibble (same byte).
        #[test]
        fn proptest_adjacent_nibbles_independent(
            y in 0usize..16,
            z in 0usize..16,
            even_x in (0usize..8).prop_map(|x| x * 2),
            val_even in 0u8..=15,
            val_odd in 0u8..=15,
        ) {
            let mut dl = DataLayer::new();
            let odd_x = even_x + 1;

            // Set both nibbles in the same byte
            dl.set(even_x, y, z, val_even);
            dl.set(odd_x, y, z, val_odd);

            prop_assert_eq!(dl.get(even_x, y, z), val_even,
                "even nibble corrupted at ({}, {}, {})", even_x, y, z);
            prop_assert_eq!(dl.get(odd_x, y, z), val_odd,
                "odd nibble corrupted at ({}, {}, {})", odd_x, y, z);

            // Set even again — odd must survive
            dl.set(even_x, y, z, val_even ^ 0x0F);
            prop_assert_eq!(dl.get(odd_x, y, z), val_odd,
                "odd nibble corrupted after re-setting even at ({}, {}, {})", even_x, y, z);
        }

        /// from_bytes(layer.as_bytes()) roundtrips perfectly.
        #[test]
        fn proptest_from_bytes_roundtrip(
            bytes in proptest::collection::vec(any::<u8>(), DATA_LAYER_SIZE),
        ) {
            let dl = DataLayer::from_bytes(&bytes).unwrap();
            prop_assert_eq!(dl.as_bytes(), bytes.as_slice());

            // Also verify all 4096 nibble positions survive roundtrip
            let dl2 = DataLayer::from_bytes(dl.as_bytes()).unwrap();
            for x in 0..16 {
                for y in 0..16 {
                    for z in 0..16 {
                        prop_assert_eq!(dl.get(x, y, z), dl2.get(x, y, z));
                    }
                }
            }
        }
    }
}
