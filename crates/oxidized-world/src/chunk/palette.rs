//! Palette types for mapping compact indices to actual values.
//!
//! Four strategies mirroring vanilla:
//! - [`SingleValuePalette`] — 0 bits, one value for the entire container
//! - [`LinearPalette`] — 1–4 bits (blocks) or 1–3 bits (biomes), array-backed
//! - [`HashMapPalette`] — 5–8 bits (blocks only), hash-backed for faster lookup
//! - [`GlobalPalette`] — 9+ bits (blocks) or 4+ bits (biomes), direct registry IDs

use std::collections::HashMap;

use thiserror::Error;

/// Errors from palette operations.
#[derive(Debug, Error)]
pub enum PaletteError {
    /// Palette is full and cannot accept more values.
    #[error("palette full: capacity {capacity}, tried to add value {value}")]
    Full {
        /// Current palette capacity.
        capacity: usize,
        /// Value that could not be added.
        value: u32,
    },
}

/// Result of adding a value to a palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteAddResult {
    /// The value was already present at this index.
    Existing(u32),
    /// The value was newly inserted at this index.
    New(u32),
    /// The palette is full and needs to be resized.
    NeedsResize,
}

// ── Single Value ───────────────────────────────────────────────────────────

/// A palette holding exactly one value. Bits-per-entry = 0.
///
/// The entire container is filled with this single value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingleValuePalette {
    value: Option<u32>,
}

impl SingleValuePalette {
    /// Creates a new empty single-value palette.
    #[must_use]
    pub fn new() -> Self {
        Self { value: None }
    }

    /// Creates a palette pre-filled with one value.
    #[must_use]
    pub fn with_value(value: u32) -> Self {
        Self { value: Some(value) }
    }

    /// Returns the stored value, if any.
    #[must_use]
    pub fn value(&self) -> Option<u32> {
        self.value
    }

    /// Returns the index for a value, or signals resize if different.
    pub fn index_or_insert(&mut self, value: u32) -> PaletteAddResult {
        match self.value {
            None => {
                self.value = Some(value);
                PaletteAddResult::New(0)
            },
            Some(v) if v == value => PaletteAddResult::Existing(0),
            Some(_) => PaletteAddResult::NeedsResize,
        }
    }

    /// Returns the value for palette index 0.
    #[must_use]
    pub fn value_for(&self, _index: u32) -> Option<u32> {
        self.value
    }

    /// Returns the number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.value.is_some())
    }

    /// Returns true if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.value.is_none()
    }

    /// Returns all values in palette order.
    #[must_use]
    pub fn entries(&self) -> Vec<u32> {
        self.value.into_iter().collect()
    }
}

impl Default for SingleValuePalette {
    fn default() -> Self {
        Self::new()
    }
}

// ── Linear Palette ─────────────────────────────────────────────────────────

/// Array-backed palette for small numbers of distinct values.
///
/// Used for 1–4 bits (blocks) or 1–3 bits (biomes). Lookup is O(n) but
/// n is at most 16 so this is cache-friendly and fast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearPalette {
    entries: Vec<u32>,
    max_size: usize,
}

impl LinearPalette {
    /// Creates a new linear palette with the given maximum capacity.
    #[must_use]
    pub fn new(bits: u8) -> Self {
        Self {
            entries: Vec::new(),
            max_size: 1 << bits,
        }
    }

    /// Creates from existing entries.
    #[must_use]
    pub fn from_entries(bits: u8, entries: Vec<u32>) -> Self {
        Self {
            entries,
            max_size: 1 << bits,
        }
    }

    /// Returns the palette index for a value, inserting if needed.
    pub fn index_or_insert(&mut self, value: u32) -> PaletteAddResult {
        // Linear search — fast for small palettes
        for (i, &v) in self.entries.iter().enumerate() {
            if v == value {
                #[allow(clippy::cast_possible_truncation)]
                return PaletteAddResult::Existing(i as u32);
            }
        }
        if self.entries.len() >= self.max_size {
            return PaletteAddResult::NeedsResize;
        }
        #[allow(clippy::cast_possible_truncation)]
        let idx = self.entries.len() as u32;
        self.entries.push(value);
        PaletteAddResult::New(idx)
    }

    /// Returns the global value for a palette index.
    #[must_use]
    pub fn value_for(&self, index: u32) -> Option<u32> {
        self.entries.get(index as usize).copied()
    }

    /// Returns the number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns all values in palette order.
    #[must_use]
    pub fn entries(&self) -> &[u32] {
        &self.entries
    }
}

// ── HashMap Palette ────────────────────────────────────────────────────────

/// Hash-backed palette for medium numbers of distinct values.
///
/// Used for 5–8 bits (blocks only). Provides O(1) lookup for `index_or_insert`
/// compared to `LinearPalette`'s O(n).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashMapPalette {
    /// Palette index → global value.
    id_to_value: Vec<u32>,
    /// Global value → palette index.
    value_to_id: HashMap<u32, u32>,
    max_size: usize,
}

impl HashMapPalette {
    /// Creates a new hash-backed palette.
    #[must_use]
    pub fn new(bits: u8) -> Self {
        Self {
            id_to_value: Vec::new(),
            value_to_id: HashMap::new(),
            max_size: 1 << bits,
        }
    }

    /// Creates from existing entries.
    #[must_use]
    pub fn from_entries(bits: u8, entries: Vec<u32>) -> Self {
        let mut value_to_id = HashMap::with_capacity(entries.len());
        #[allow(clippy::cast_possible_truncation)]
        for (i, &v) in entries.iter().enumerate() {
            value_to_id.insert(v, i as u32);
        }
        Self {
            id_to_value: entries,
            value_to_id,
            max_size: 1 << bits,
        }
    }

    /// Returns the palette index for a value, inserting if needed.
    pub fn index_or_insert(&mut self, value: u32) -> PaletteAddResult {
        if let Some(&idx) = self.value_to_id.get(&value) {
            return PaletteAddResult::Existing(idx);
        }
        if self.id_to_value.len() >= self.max_size {
            return PaletteAddResult::NeedsResize;
        }
        #[allow(clippy::cast_possible_truncation)]
        let idx = self.id_to_value.len() as u32;
        self.id_to_value.push(value);
        self.value_to_id.insert(value, idx);
        PaletteAddResult::New(idx)
    }

    /// Returns the global value for a palette index.
    #[must_use]
    pub fn value_for(&self, index: u32) -> Option<u32> {
        self.id_to_value.get(index as usize).copied()
    }

    /// Returns the number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.id_to_value.len()
    }

    /// Returns true if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.id_to_value.is_empty()
    }

    /// Returns all values in palette order.
    #[must_use]
    pub fn entries(&self) -> &[u32] {
        &self.id_to_value
    }
}

// ── Global Palette ─────────────────────────────────────────────────────────

/// Direct registry ID palette — no mapping, values are stored as-is.
///
/// Used when there are too many distinct values for a local palette.
/// Block states: 9+ bits (up to ~15 bits for ~30k states).
/// Biomes: 4+ bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalPalette;

impl GlobalPalette {
    /// Returns the palette index — identity mapping (value == index).
    #[must_use]
    pub fn index_for(value: u32) -> u32 {
        value
    }

    /// Returns the value — identity mapping (index == value).
    #[must_use]
    pub fn value_for(index: u32) -> u32 {
        index
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_single_value_empty() {
        let p = SingleValuePalette::new();
        assert!(p.is_empty());
        assert_eq!(p.len(), 0);
    }

    #[test]
    fn test_single_value_insert_and_lookup() {
        let mut p = SingleValuePalette::new();
        assert_eq!(p.index_or_insert(42), PaletteAddResult::New(0));
        assert_eq!(p.index_or_insert(42), PaletteAddResult::Existing(0));
        assert_eq!(p.value_for(0), Some(42));
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn test_single_value_needs_resize() {
        let mut p = SingleValuePalette::with_value(1);
        assert_eq!(p.index_or_insert(2), PaletteAddResult::NeedsResize);
    }

    #[test]
    fn test_linear_insert_and_lookup() {
        let mut p = LinearPalette::new(2); // Max 4 entries
        assert_eq!(p.index_or_insert(10), PaletteAddResult::New(0));
        assert_eq!(p.index_or_insert(20), PaletteAddResult::New(1));
        assert_eq!(p.index_or_insert(10), PaletteAddResult::Existing(0));
        assert_eq!(p.value_for(0), Some(10));
        assert_eq!(p.value_for(1), Some(20));
    }

    #[test]
    fn test_linear_full() {
        let mut p = LinearPalette::new(1); // Max 2 entries
        p.index_or_insert(1);
        p.index_or_insert(2);
        assert_eq!(p.index_or_insert(3), PaletteAddResult::NeedsResize);
    }

    #[test]
    fn test_hashmap_insert_and_lookup() {
        let mut p = HashMapPalette::new(3); // Max 8 entries
        assert_eq!(p.index_or_insert(100), PaletteAddResult::New(0));
        assert_eq!(p.index_or_insert(200), PaletteAddResult::New(1));
        assert_eq!(p.index_or_insert(100), PaletteAddResult::Existing(0));
        assert_eq!(p.value_for(0), Some(100));
        assert_eq!(p.value_for(1), Some(200));
    }

    #[test]
    fn test_hashmap_from_entries() {
        let p = HashMapPalette::from_entries(4, vec![5, 10, 15]);
        assert_eq!(p.value_for(0), Some(5));
        assert_eq!(p.value_for(2), Some(15));
        assert_eq!(p.len(), 3);
    }

    #[test]
    fn test_global_identity() {
        assert_eq!(GlobalPalette::index_for(12345), 12345);
        assert_eq!(GlobalPalette::value_for(12345), 12345);
    }
}
