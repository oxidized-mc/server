//! Nibble-packed light data layer (4 bits per block position).
//!
//! Each section stores 2048 bytes (16×16×16 ÷ 2) of light values in the
//! range 0–15. Used for both sky light and block light.
//!
//! Uses lazy allocation matching vanilla's `DataLayer`: no heap memory is
//! allocated until the first [`set()`](DataLayer::set) call that changes a
//! value from the default. This saves ~2 KB per empty section (typically
//! 10–15 per chunk = 20–30 KB). [`is_empty()`](DataLayer::is_empty) is O(1).

/// Size of a data layer in bytes (16 × 16 × 16 / 2).
pub const DATA_LAYER_SIZE: usize = 2048;

/// Pre-computed fill patterns for each nibble value 0–15.
///
/// `FILLED_DATA[v]` is a 2048-byte array where every nibble is `v`.
static FILLED_DATA: [[u8; DATA_LAYER_SIZE]; 16] = {
    let mut result = [[0u8; DATA_LAYER_SIZE]; 16];
    let mut v: usize = 0;
    while v < 16 {
        let byte = (v as u8) | ((v as u8) << 4);
        let mut i = 0;
        while i < DATA_LAYER_SIZE {
            result[v][i] = byte;
            i += 1;
        }
        v += 1;
    }
    result
};

/// A nibble-packed array of 4096 4-bit light values.
///
/// Matches vanilla's `DataLayer` format with lazy allocation: memory is not
/// allocated until the first write that differs from the default value.
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
#[derive(Debug, Clone)]
pub struct DataLayer {
    data: Option<Box<[u8; DATA_LAYER_SIZE]>>,
    default_value: u8,
}

impl PartialEq for DataLayer {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl Eq for DataLayer {}

impl DataLayer {
    /// Creates a new data layer filled with zeros (no light).
    ///
    /// Does **not** allocate heap memory. The first [`set()`](Self::set) call
    /// with a non-zero value triggers allocation.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: None,
            default_value: 0,
        }
    }

    /// Creates a data layer filled with a uniform value (0–15).
    ///
    /// Does **not** allocate heap memory. The filled value is stored as a
    /// compact default and expanded on the first heterogeneous
    /// [`set()`](Self::set).
    #[must_use]
    pub fn filled(value: u8) -> Self {
        Self {
            data: None,
            default_value: value & 0x0F,
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
        let mut arr = Box::new([0u8; DATA_LAYER_SIZE]);
        arr.copy_from_slice(data);
        Some(Self {
            data: Some(arr),
            default_value: 0,
        })
    }

    /// Computes the flat index for a 16³ section coordinate.
    const fn section_index(x: usize, y: usize, z: usize) -> usize {
        (y << 8) | (z << 4) | x
    }

    /// Ensures the backing array is allocated, filling it from
    /// [`default_value`] if this is the first materialization.
    fn materialize(&mut self) -> &mut [u8; DATA_LAYER_SIZE] {
        self.data.get_or_insert_with(|| {
            let byte = self.default_value | (self.default_value << 4);
            Box::new([byte; DATA_LAYER_SIZE])
        })
    }

    /// Returns the light value at `(x, y, z)` within the section (0–15 each).
    ///
    /// Returns the default fill value if the backing array has not been
    /// allocated.
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
        match &self.data {
            None => self.default_value,
            Some(data) => {
                let index = Self::section_index(x, y, z);
                let byte_pos = index >> 1;
                if index & 1 == 0 {
                    data[byte_pos] & 0x0F
                } else {
                    (data[byte_pos] >> 4) & 0x0F
                }
            },
        }
    }

    /// Sets the light value at `(x, y, z)` within the section.
    ///
    /// Allocates the backing array on the first call that changes a value
    /// from the default.
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
        // Skip materialization when writing the default value to a lazy layer.
        if self.data.is_none() && value == self.default_value {
            return;
        }
        let data = self.materialize();
        let index = Self::section_index(x, y, z);
        let byte_pos = index >> 1;
        if index & 1 == 0 {
            data[byte_pos] = (data[byte_pos] & 0xF0) | (value & 0x0F);
        } else {
            data[byte_pos] = (data[byte_pos] & 0x0F) | ((value & 0x0F) << 4);
        }
    }

    /// Sets all values to `value` and de-allocates the backing array.
    ///
    /// After this call the layer stores only the fill value, freeing 2 KB
    /// of heap memory. Matches vanilla's `DataLayer.fill()`.
    pub fn fill(&mut self, value: u8) {
        self.default_value = value & 0x0F;
        self.data = None;
    }

    /// Returns the raw bytes (2048 bytes).
    ///
    /// If the backing array has not been allocated, returns a reference to a
    /// pre-computed static array matching the default fill value.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        match &self.data {
            Some(data) => data.as_slice(),
            None => &FILLED_DATA[self.default_value as usize],
        }
    }

    /// Consumes and returns the raw bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        match self.data {
            Some(data) => {
                let boxed_slice: Box<[u8]> = data;
                boxed_slice.into_vec()
            },
            None => FILLED_DATA[self.default_value as usize].to_vec(),
        }
    }

    /// Returns `true` if all values are zero (no light).
    ///
    /// This is an O(1) check when the layer has not been materialized.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match &self.data {
            None => self.default_value == 0,
            Some(data) => data.iter().all(|&b| b == 0),
        }
    }

    /// Returns `true` if all values are known to be the same without
    /// scanning the backing array (i.e. the layer is still in its lazy
    /// state).
    #[must_use]
    pub fn is_definitely_homogeneous(&self) -> bool {
        self.data.is_none()
    }

    /// Returns `true` if all values are known to equal `value` without
    /// scanning the backing array.
    #[must_use]
    pub fn is_definitely_filled_with(&self, value: u8) -> bool {
        self.data.is_none() && self.default_value == (value & 0x0F)
    }

    /// Returns `true` if the backing array has been allocated.
    #[must_use]
    pub fn is_materialized(&self) -> bool {
        self.data.is_some()
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
    fn test_new_does_not_allocate() {
        let dl = DataLayer::new();
        assert!(!dl.is_materialized());
    }

    #[test]
    fn test_filled_does_not_allocate() {
        let dl = DataLayer::filled(15);
        assert!(!dl.is_materialized());
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
    fn test_set_materializes_on_non_default_value() {
        let mut dl = DataLayer::new();
        assert!(!dl.is_materialized());
        dl.set(5, 5, 5, 7);
        assert!(dl.is_materialized());
    }

    #[test]
    fn test_set_does_not_materialize_on_default_value() {
        let mut dl = DataLayer::new();
        dl.set(5, 5, 5, 0);
        assert!(!dl.is_materialized());
    }

    #[test]
    fn test_set_on_filled_layer_materializes() {
        let mut dl = DataLayer::filled(15);
        dl.set(5, 5, 5, 10);
        assert!(dl.is_materialized());
        assert_eq!(dl.get(5, 5, 5), 10);
        // Other positions retain the fill value.
        assert_eq!(dl.get(0, 0, 0), 15);
        assert_eq!(dl.get(15, 15, 15), 15);
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
        assert!(dl.is_materialized());
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
    fn test_is_empty_before_any_set() {
        let dl = DataLayer::new();
        assert!(dl.is_empty());
        assert!(!dl.is_materialized());
    }

    #[test]
    fn test_get_returns_zero_before_any_set() {
        let dl = DataLayer::new();
        assert_eq!(dl.get(0, 0, 0), 0);
        assert_eq!(dl.get(8, 8, 8), 0);
        assert_eq!(dl.get(15, 15, 15), 0);
    }

    #[test]
    fn test_filled_15_is_not_empty() {
        let dl = DataLayer::filled(15);
        assert!(!dl.is_empty());
    }

    #[test]
    fn test_fill_deallocates() {
        let mut dl = DataLayer::new();
        dl.set(5, 5, 5, 10);
        assert!(dl.is_materialized());
        dl.fill(0);
        assert!(!dl.is_materialized());
        assert!(dl.is_empty());
        assert_eq!(dl.get(5, 5, 5), 0);
    }

    #[test]
    fn test_fill_with_nonzero() {
        let mut dl = DataLayer::new();
        dl.set(3, 3, 3, 5);
        dl.fill(15);
        assert!(!dl.is_materialized());
        assert!(!dl.is_empty());
        assert_eq!(dl.get(0, 0, 0), 15);
        assert_eq!(dl.get(3, 3, 3), 15);
    }

    #[test]
    fn test_is_definitely_homogeneous() {
        let dl = DataLayer::new();
        assert!(dl.is_definitely_homogeneous());

        let dl = DataLayer::filled(15);
        assert!(dl.is_definitely_homogeneous());

        let mut dl = DataLayer::new();
        dl.set(0, 0, 0, 1);
        assert!(!dl.is_definitely_homogeneous());
    }

    #[test]
    fn test_is_definitely_filled_with() {
        let dl = DataLayer::new();
        assert!(dl.is_definitely_filled_with(0));
        assert!(!dl.is_definitely_filled_with(15));

        let dl = DataLayer::filled(15);
        assert!(dl.is_definitely_filled_with(15));
        assert!(!dl.is_definitely_filled_with(0));
    }

    #[test]
    fn test_equality_materialized_vs_lazy() {
        let lazy = DataLayer::new();
        let materialized = DataLayer::from_bytes(&[0u8; DATA_LAYER_SIZE]).unwrap();
        assert_eq!(lazy, materialized);
    }

    #[test]
    fn test_equality_filled_vs_from_bytes() {
        let filled = DataLayer::filled(15);
        let from_bytes = DataLayer::from_bytes(&[0xFF; DATA_LAYER_SIZE]).unwrap();
        assert_eq!(filled, from_bytes);
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

        /// fill(v) then get returns v for all coordinates.
        #[test]
        fn proptest_fill_then_get(
            x in 0usize..16,
            y in 0usize..16,
            z in 0usize..16,
            fill_value in 0u8..=15,
        ) {
            let mut dl = DataLayer::new();
            dl.set(8, 8, 8, 5); // materialize
            dl.fill(fill_value);
            prop_assert_eq!(dl.get(x, y, z), fill_value);
            prop_assert!(!dl.is_materialized());
        }
    }
}
