//! Wire-format serialization helpers for [`PalettedContainer`](super::paletted_container::PalettedContainer).
//!
//! Contains VarInt encoding, long-array I/O, bit-storage reading, and
//! palette-tier construction from raw values or pre-read entries.

use super::bit_storage::BitStorage;
use super::palette::{HashMapPalette, LinearPalette, PaletteAddResult};
use super::paletted_container::{PaletteData, PalettedContainerError, Strategy};

/// Builds [`PaletteData`] by inserting raw values into the correct palette tier.
///
/// Used during palette upgrades where all container values need to be
/// re-indexed into a fresh palette of the appropriate tier.
pub(super) fn build_palette_data_from_values(
    strategy: Strategy,
    bits_needed: u8,
    values: &[u32],
) -> Result<PaletteData, PalettedContainerError> {
    let size = values.len();

    if bits_needed >= strategy.global_bits_threshold() {
        let global_bits = strategy.global_palette_bits();
        let mut storage = BitStorage::new(global_bits, size)?;
        for (i, &v) in values.iter().enumerate() {
            storage.set(i, u64::from(v))?;
        }
        return Ok(PaletteData::Global(storage));
    }

    let storage_bits = strategy.storage_bits(bits_needed);
    let use_hashmap = bits_needed >= strategy.hashmap_bits_threshold();

    if use_hashmap {
        let mut palette = HashMapPalette::new(storage_bits);
        let mut storage = BitStorage::new(storage_bits, size)?;
        for (i, &v) in values.iter().enumerate() {
            let idx = unwrap_palette_index(palette.index_or_insert(v), bits_needed)?;
            storage.set(i, u64::from(idx))?;
        }
        Ok(PaletteData::HashMap(palette, storage))
    } else {
        let mut palette = LinearPalette::new(storage_bits);
        let mut storage = BitStorage::new(storage_bits, size)?;
        for (i, &v) in values.iter().enumerate() {
            let idx = unwrap_palette_index(palette.index_or_insert(v), bits_needed)?;
            storage.set(i, u64::from(idx))?;
        }
        Ok(PaletteData::Linear(palette, storage))
    }
}

/// Builds [`PaletteData`] from pre-read palette entries and bit storage.
///
/// Selects between [`LinearPalette`] and [`HashMapPalette`] based on the
/// strategy's threshold for the given bits per entry.
pub(super) fn build_palette_data_from_entries(
    strategy: Strategy,
    bits_per_entry: u8,
    entries: Vec<u32>,
    storage: BitStorage,
) -> PaletteData {
    if bits_per_entry >= strategy.hashmap_bits_threshold() {
        let palette = HashMapPalette::from_entries(bits_per_entry, entries);
        PaletteData::HashMap(palette, storage)
    } else {
        let palette = LinearPalette::from_entries(bits_per_entry, entries);
        PaletteData::Linear(palette, storage)
    }
}

/// Extracts the palette index from a [`PaletteAddResult`], or returns an
/// error if the palette unexpectedly needs a resize.
fn unwrap_palette_index(result: PaletteAddResult, bits: u8) -> Result<u32, PalettedContainerError> {
    match result {
        PaletteAddResult::Existing(idx) | PaletteAddResult::New(idx) => Ok(idx),
        PaletteAddResult::NeedsResize => Err(PalettedContainerError::InvalidBitsPerEntry(bits)),
    }
}

/// Returns the minimum number of bits to represent `count` distinct values.
pub(super) fn bits_for_count(count: usize) -> u8 {
    if count <= 1 {
        return 0;
    }
    // ceil(log2(count))
    let bits = usize::BITS - (count - 1).leading_zeros();
    #[allow(clippy::cast_possible_truncation)]
    {
        bits as u8
    }
}

pub(super) fn write_varint(buf: &mut Vec<u8>, mut value: i32) {
    loop {
        #[allow(clippy::cast_sign_loss)]
        let byte = (value & 0x7F) as u8;
        value = ((value as u32) >> 7) as i32;
        if value == 0 {
            buf.push(byte);
            break;
        }
        buf.push(byte | 0x80);
    }
}

pub(super) fn write_longs(buf: &mut Vec<u8>, longs: &[u64]) {
    write_varint(buf, longs.len() as i32);
    for &l in longs {
        buf.extend_from_slice(&l.to_be_bytes());
    }
}

pub(super) fn read_u8(data: &mut &[u8]) -> Result<u8, PalettedContainerError> {
    if data.is_empty() {
        return Err(PalettedContainerError::InsufficientData {
            expected: 1,
            actual: 0,
        });
    }
    let b = data[0];
    *data = &data[1..];
    Ok(b)
}

pub(super) fn read_varint(data: &mut &[u8]) -> Result<i32, PalettedContainerError> {
    let mut result = 0i32;
    for i in 0..5 {
        if data.is_empty() {
            return Err(PalettedContainerError::InsufficientData {
                expected: 1,
                actual: 0,
            });
        }
        let byte = data[0];
        *data = &data[1..];
        result |= i32::from(byte & 0x7F) << (i * 7);
        if byte & 0x80 == 0 {
            return Ok(result);
        }
    }
    Err(PalettedContainerError::MalformedVarInt)
}

pub(super) fn read_bit_storage(
    bits: u8,
    size: usize,
    data: &mut &[u8],
) -> Result<BitStorage, PalettedContainerError> {
    let num_longs = read_varint(data)? as usize;
    let byte_len = num_longs * 8;
    if data.len() < byte_len {
        return Err(PalettedContainerError::InsufficientData {
            expected: byte_len,
            actual: data.len(),
        });
    }
    let mut longs = Vec::with_capacity(num_longs);
    for _ in 0..num_longs {
        let long = u64::from_be_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]);
        *data = &data[8..];
        longs.push(long);
    }
    Ok(BitStorage::from_raw(bits, size, longs)?)
}
