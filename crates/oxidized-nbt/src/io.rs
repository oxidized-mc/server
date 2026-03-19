//! File and compression I/O helpers for NBT data.
//!
//! Provides convenience functions for reading/writing NBT compounds
//! with GZIP, zlib, or no compression, plus auto-detection of GZIP by
//! magic bytes.

use std::io::{Read, Write};
use std::path::Path;

use crate::accounter::NbtAccounter;
use crate::compound::NbtCompound;
use crate::error::NbtError;
use crate::reader::read_nbt;
use crate::writer::write_nbt;

/// Reads a root compound from GZIP-compressed data.
///
/// Uses the uncompressed disk quota (100 MiB).
///
/// # Examples
///
/// ```
/// use oxidized_nbt::{NbtCompound, read_gzip, write_gzip};
/// use std::io::Cursor;
///
/// let mut compound = NbtCompound::new();
/// compound.put_long("seed", 12345);
///
/// let mut compressed = Vec::new();
/// write_gzip(&mut compressed, &compound).unwrap();
///
/// let result = read_gzip(Cursor::new(&compressed)).unwrap();
/// assert_eq!(result.get_long("seed"), Some(12345));
/// ```
///
/// # Errors
///
/// Returns an error on decompression failure, invalid NBT, or quota violation.
pub fn read_gzip<R: Read>(reader: R) -> Result<NbtCompound, NbtError> {
    let mut decoder = flate2::read::GzDecoder::new(reader);
    let mut accounter = NbtAccounter::uncompressed_quota();
    read_nbt(&mut decoder, &mut accounter)
}

/// Reads a root compound from zlib-compressed data.
///
/// Uses the uncompressed disk quota (100 MiB).
///
/// # Errors
///
/// Returns an error on decompression failure, invalid NBT, or quota violation.
pub fn read_zlib(data: &[u8]) -> Result<NbtCompound, NbtError> {
    let mut decoder = flate2::read::ZlibDecoder::new(data);
    let mut accounter = NbtAccounter::uncompressed_quota();
    read_nbt(&mut decoder, &mut accounter)
}

/// Reads a root compound from uncompressed bytes.
///
/// Uses the uncompressed disk quota (100 MiB).
///
/// # Examples
///
/// ```
/// use oxidized_nbt::{NbtCompound, read_bytes, write_bytes};
///
/// let mut compound = NbtCompound::new();
/// compound.put_int("health", 20);
///
/// let data = write_bytes(&compound).unwrap();
/// let result = read_bytes(&data).unwrap();
/// assert_eq!(result.get_int("health"), Some(20));
/// ```
///
/// # Errors
///
/// Returns an error on invalid NBT or quota violation.
pub fn read_bytes(data: &[u8]) -> Result<NbtCompound, NbtError> {
    let mut cursor: &[u8] = data;
    let mut accounter = NbtAccounter::uncompressed_quota();
    read_nbt(&mut cursor, &mut accounter)
}

/// Reads a root compound from a file, auto-detecting GZIP compression
/// by the magic bytes `0x1F 0x8B`.
///
/// # Errors
///
/// Returns an error on I/O failure, invalid NBT, or quota violation.
pub fn read_file(path: &Path) -> Result<NbtCompound, NbtError> {
    let data = std::fs::read(path)?;
    if data.len() >= 2 && data[0] == 0x1F && data[1] == 0x8B {
        read_gzip(data.as_slice())
    } else {
        read_bytes(&data)
    }
}

/// Writes a root compound as GZIP-compressed data.
///
/// # Examples
///
/// ```
/// use oxidized_nbt::{NbtCompound, write_gzip};
///
/// let mut compound = NbtCompound::new();
/// compound.put_string("biome", "forest");
///
/// let mut compressed = Vec::new();
/// write_gzip(&mut compressed, &compound).unwrap();
/// // GZIP magic bytes
/// assert_eq!(compressed[0], 0x1F);
/// assert_eq!(compressed[1], 0x8B);
/// ```
///
/// # Errors
///
/// Returns an error on I/O or compression failure.
pub fn write_gzip<W: Write>(writer: W, compound: &NbtCompound) -> Result<(), NbtError> {
    let mut encoder = flate2::write::GzEncoder::new(writer, flate2::Compression::default());
    write_nbt(&mut encoder, compound)?;
    let _ = encoder.finish()?;
    Ok(())
}

/// Writes a root compound as zlib-compressed bytes.
///
/// # Errors
///
/// Returns an error on compression failure.
pub fn write_zlib(compound: &NbtCompound) -> Result<Vec<u8>, NbtError> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    write_nbt(&mut encoder, compound)?;
    Ok(encoder.finish()?)
}

/// Writes a root compound as uncompressed bytes.
///
/// # Examples
///
/// ```
/// use oxidized_nbt::{NbtCompound, write_bytes};
///
/// let mut compound = NbtCompound::new();
/// compound.put_string("msg", "hello");
///
/// let bytes = write_bytes(&compound).unwrap();
/// assert!(!bytes.is_empty());
/// ```
///
/// # Errors
///
/// Returns an error on I/O failure (unlikely with `Vec<u8>`).
pub fn write_bytes(compound: &NbtCompound) -> Result<Vec<u8>, NbtError> {
    let mut buf = Vec::new();
    write_nbt(&mut buf, compound)?;
    Ok(buf)
}

/// Writes a root compound to a GZIP-compressed file.
///
/// # Errors
///
/// Returns an error on I/O or compression failure.
pub fn write_file(path: &Path, compound: &NbtCompound) -> Result<(), NbtError> {
    let file = std::fs::File::create(path)?;
    let buf_writer = std::io::BufWriter::new(file);
    write_gzip(buf_writer, compound)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn sample_compound() -> NbtCompound {
        let mut c = NbtCompound::new();
        c.put_string("name", "Test");
        c.put_int("version", 42);
        c.put_long("seed", 987654321);
        c
    }

    #[test]
    fn test_bytes_roundtrip() {
        let compound = sample_compound();
        let data = write_bytes(&compound).unwrap();
        let result = read_bytes(&data).unwrap();
        assert_eq!(compound, result);
    }

    #[test]
    fn test_gzip_roundtrip() {
        let compound = sample_compound();
        let mut compressed = Vec::new();
        write_gzip(&mut compressed, &compound).unwrap();

        // Verify GZIP magic bytes
        assert!(compressed.len() >= 2);
        assert_eq!(compressed[0], 0x1F);
        assert_eq!(compressed[1], 0x8B);

        let result = read_gzip(compressed.as_slice()).unwrap();
        assert_eq!(compound, result);
    }

    #[test]
    fn test_zlib_roundtrip() {
        let compound = sample_compound();
        let compressed = write_zlib(&compound).unwrap();
        let result = read_zlib(&compressed).unwrap();
        assert_eq!(compound, result);
    }

    #[test]
    fn test_file_roundtrip() {
        let compound = sample_compound();
        let dir = std::env::temp_dir();
        let path = dir.join("oxidized_nbt_test_file_roundtrip.dat");

        write_file(&path, &compound).unwrap();
        let result = read_file(&path).unwrap();
        assert_eq!(compound, result);

        // Cleanup
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_read_file_auto_detects_gzip() {
        let compound = sample_compound();
        let dir = std::env::temp_dir();
        let path = dir.join("oxidized_nbt_test_autodetect.dat");

        // write_file uses GZIP
        write_file(&path, &compound).unwrap();

        // read_file should auto-detect GZIP and decompress
        let result = read_file(&path).unwrap();
        assert_eq!(compound, result);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_read_file_uncompressed() {
        let compound = sample_compound();
        let dir = std::env::temp_dir();
        let path = dir.join("oxidized_nbt_test_uncompressed.dat");

        // Write raw bytes directly
        let data = write_bytes(&compound).unwrap();
        std::fs::write(&path, &data).unwrap();

        // read_file should detect non-GZIP and read uncompressed
        let result = read_file(&path).unwrap();
        assert_eq!(compound, result);

        let _ = std::fs::remove_file(&path);
    }
}
