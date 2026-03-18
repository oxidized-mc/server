//! Anvil region file format (`.mca`) reader.
//!
//! A region file stores up to 1024 chunks (32×32) in a sector-based layout.
//! The first 8 KiB is a header containing offset and timestamp tables;
//! chunk data follows in 4 KiB sectors.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::compression::{decompress, CompressionType};
use super::error::AnvilError;

/// Size of one sector in bytes (4 KiB).
pub const SECTOR_BYTES: usize = 4096;

/// Number of chunks along one axis of a region (32).
pub const REGION_SIZE: usize = 32;

/// Number of chunk slots in a region (32 × 32 = 1024).
pub const SECTOR_INTS: usize = REGION_SIZE * REGION_SIZE;

/// Total header size: offset table (4096 bytes) + timestamp table (4096 bytes).
pub const HEADER_BYTES: usize = SECTOR_BYTES * 2;

/// Minimum valid payload length (1 byte for compression type).
const MIN_PAYLOAD_LEN: usize = 1;

/// Maximum allowed chunk payload size (16 MiB safety limit).
const MAX_CHUNK_SIZE: usize = 16 * 1024 * 1024;

/// A parsed entry from the region file offset table.
///
/// Each 4-byte entry encodes the starting sector (3 bytes, big-endian)
/// and sector count (1 byte). A zero entry means the chunk is absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OffsetEntry {
    /// Starting sector index. Valid chunks start at sector ≥ 2 (after header).
    pub sector_number: u32,
    /// Number of consecutive 4 KiB sectors used by this chunk.
    pub sector_count: u8,
}

impl OffsetEntry {
    /// Returns `true` if this entry indicates a chunk is present.
    #[must_use]
    pub fn is_present(self) -> bool {
        self.sector_number != 0 || self.sector_count != 0
    }

    /// Parses an offset entry from a raw big-endian `u32`.
    #[must_use]
    pub fn from_u32(raw: u32) -> Self {
        Self {
            sector_number: raw >> 8,
            sector_count: (raw & 0xFF) as u8,
        }
    }

    /// Packs this entry back into a big-endian `u32`.
    #[must_use]
    pub fn to_u32(self) -> u32 {
        (self.sector_number << 8) | self.sector_count as u32
    }
}

/// An open Anvil region file (`.mca`).
///
/// Reads the 8 KiB header on open and provides methods to read individual
/// chunk data. Chunk data is decompressed before being returned.
pub struct RegionFile {
    reader: BufReader<File>,
    offsets: [OffsetEntry; SECTOR_INTS],
    timestamps: [u32; SECTOR_INTS],
    path: PathBuf,
    file_len: u64,
}

impl RegionFile {
    /// Opens and parses a region file at the given path.
    ///
    /// Reads and validates the 8 KiB header.
    ///
    /// # Errors
    ///
    /// Returns [`AnvilError::Io`] on file open/read failure, or
    /// [`AnvilError::InvalidHeader`] if the file is too small.
    pub fn open(path: &Path) -> Result<Self, AnvilError> {
        let file = File::open(path).map_err(|e| AnvilError::io(path, e))?;
        let file_len = file.metadata().map_err(|e| AnvilError::io(path, e))?.len();

        if file_len < HEADER_BYTES as u64 {
            return Err(AnvilError::InvalidHeader(format!(
                "file too small: {} bytes (need at least {})",
                file_len, HEADER_BYTES
            )));
        }

        let mut region = Self {
            reader: BufReader::new(file),
            offsets: [OffsetEntry {
                sector_number: 0,
                sector_count: 0,
            }; SECTOR_INTS],
            timestamps: [0u32; SECTOR_INTS],
            path: path.to_path_buf(),
            file_len,
        };
        region.read_header()?;
        Ok(region)
    }

    /// Returns the local chunk index (0–1023) for the given chunk coordinates.
    ///
    /// Handles negative coordinates correctly by wrapping into 0..32.
    #[must_use]
    pub fn chunk_index(chunk_x: i32, chunk_z: i32) -> usize {
        let lx = ((chunk_x % 32) + 32) as usize % 32;
        let lz = ((chunk_z % 32) + 32) as usize % 32;
        lz * 32 + lx
    }

    /// Reads and decompresses chunk data, or returns `None` if the chunk
    /// is not present in this region.
    ///
    /// # Errors
    ///
    /// Returns an error on I/O failure, unknown compression, or
    /// decompression failure.
    pub fn read_chunk_data(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<Option<Vec<u8>>, AnvilError> {
        let idx = Self::chunk_index(chunk_x, chunk_z);
        let entry = self.offsets[idx];
        if !entry.is_present() {
            return Ok(None);
        }

        // Validate sector bounds
        let byte_offset = entry.sector_number as u64 * SECTOR_BYTES as u64;
        let max_byte = byte_offset + entry.sector_count as u64 * SECTOR_BYTES as u64;
        if byte_offset < HEADER_BYTES as u64 || max_byte > self.file_len {
            tracing::warn!(
                chunk_x,
                chunk_z,
                sector = entry.sector_number,
                count = entry.sector_count,
                file_len = self.file_len,
                "invalid sector range, skipping chunk"
            );
            return Ok(None);
        }

        self.reader
            .seek(SeekFrom::Start(byte_offset))
            .map_err(|e| AnvilError::io(&self.path, e))?;

        // Read 4-byte payload length (big-endian)
        let mut len_buf = [0u8; 4];
        self.reader
            .read_exact(&mut len_buf)
            .map_err(|e| AnvilError::io(&self.path, e))?;
        let payload_len = u32::from_be_bytes(len_buf) as usize;

        if payload_len < MIN_PAYLOAD_LEN {
            tracing::warn!(chunk_x, chunk_z, payload_len, "empty chunk payload");
            return Ok(None);
        }

        if payload_len > MAX_CHUNK_SIZE {
            return Err(AnvilError::CorruptedChunk {
                chunk_x,
                chunk_z,
                reason: format!("payload too large: {payload_len} bytes"),
            });
        }

        // Validate payload doesn't exceed allocated sectors
        let max_payload = entry.sector_count as usize * SECTOR_BYTES - 4; // subtract length prefix
        if payload_len > max_payload {
            tracing::warn!(
                chunk_x,
                chunk_z,
                payload_len,
                max_payload,
                "payload length exceeds allocated sectors, skipping chunk"
            );
            return Ok(None);
        }

        // Read 1-byte compression type
        let mut codec_byte = [0u8; 1];
        self.reader
            .read_exact(&mut codec_byte)
            .map_err(|e| AnvilError::io(&self.path, e))?;

        if CompressionType::is_external(codec_byte[0]) {
            // External chunks (.mcc files) not yet supported
            tracing::warn!(
                chunk_x,
                chunk_z,
                "external chunk storage (.mcc) not supported, skipping"
            );
            return Ok(None);
        }

        let codec = CompressionType::from_byte(codec_byte[0])?;

        // Read compressed data (payload_len - 1 because the codec byte is included)
        let compressed_len = payload_len - 1;
        let mut compressed = vec![0u8; compressed_len];
        self.reader
            .read_exact(&mut compressed)
            .map_err(|e| AnvilError::io(&self.path, e))?;

        decompress(&compressed, codec).map(Some)
    }

    /// Returns `true` if the chunk at the given coordinates exists.
    #[must_use]
    pub fn has_chunk(&self, chunk_x: i32, chunk_z: i32) -> bool {
        let idx = Self::chunk_index(chunk_x, chunk_z);
        self.offsets[idx].is_present()
    }

    /// Returns the timestamp for the given chunk, or 0 if not present.
    #[must_use]
    pub fn chunk_timestamp(&self, chunk_x: i32, chunk_z: i32) -> u32 {
        let idx = Self::chunk_index(chunk_x, chunk_z);
        self.timestamps[idx]
    }

    /// Returns the file path this region was opened from.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads and parses the 8 KiB header, sanitizing invalid entries.
    fn read_header(&mut self) -> Result<(), AnvilError> {
        self.reader
            .seek(SeekFrom::Start(0))
            .map_err(|e| AnvilError::io(&self.path, e))?;

        let file_sectors = self.file_len / SECTOR_BYTES as u64;

        // Read offset table (4096 bytes = 1024 × 4-byte entries)
        let mut buf = [0u8; 4];
        for (i, offset) in self.offsets.iter_mut().enumerate() {
            self.reader
                .read_exact(&mut buf)
                .map_err(|e| AnvilError::io(&self.path, e))?;
            let entry = OffsetEntry::from_u32(u32::from_be_bytes(buf));

            // Validate entry: zero out if sector overlaps header,
            // sector_count is zero, or sectors extend beyond file.
            // Stricter than Java (which only checks start offset > file size)
            // — we verify the entire sector range fits.
            if entry.is_present() {
                let end_sector = entry.sector_number as u64 + entry.sector_count as u64;
                if entry.sector_number < 2 || entry.sector_count == 0 || end_sector > file_sectors {
                    tracing::warn!(
                        slot = i,
                        sector = entry.sector_number,
                        count = entry.sector_count,
                        file_sectors,
                        path = %self.path.display(),
                        "invalid offset entry in region header, zeroing"
                    );
                    *offset = OffsetEntry {
                        sector_number: 0,
                        sector_count: 0,
                    };
                    continue;
                }
            }
            *offset = entry;
        }

        // Read timestamp table (4096 bytes = 1024 × 4-byte entries)
        for ts in &mut self.timestamps {
            self.reader
                .read_exact(&mut buf)
                .map_err(|e| AnvilError::io(&self.path, e))?;
            *ts = u32::from_be_bytes(buf);
        }

        Ok(())
    }
}

impl std::fmt::Debug for RegionFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegionFile")
            .field("path", &self.path)
            .field("file_len", &self.file_len)
            .finish()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ── OffsetEntry ────────────────────────────────────────────────

    #[test]
    fn test_offset_entry_from_u32() {
        // sector_number=2, sector_count=1 → raw = (2 << 8) | 1 = 513
        let entry = OffsetEntry::from_u32(513);
        assert_eq!(entry.sector_number, 2);
        assert_eq!(entry.sector_count, 1);
        assert!(entry.is_present());
    }

    #[test]
    fn test_offset_entry_zero_is_absent() {
        let entry = OffsetEntry::from_u32(0);
        assert!(!entry.is_present());
        assert_eq!(entry.sector_number, 0);
        assert_eq!(entry.sector_count, 0);
    }

    #[test]
    fn test_offset_entry_roundtrip() {
        let entry = OffsetEntry {
            sector_number: 42,
            sector_count: 3,
        };
        let raw = entry.to_u32();
        let back = OffsetEntry::from_u32(raw);
        assert_eq!(back, entry);
    }

    #[test]
    fn test_offset_entry_max_values() {
        // Max sector_number fits in 24 bits: 0xFFFFFF
        let entry = OffsetEntry {
            sector_number: 0xFF_FFFF,
            sector_count: 255,
        };
        let raw = entry.to_u32();
        let back = OffsetEntry::from_u32(raw);
        assert_eq!(back, entry);
    }

    // ── chunk_index ────────────────────────────────────────────────

    #[test]
    fn test_chunk_index_corners() {
        assert_eq!(RegionFile::chunk_index(0, 0), 0);
        assert_eq!(RegionFile::chunk_index(31, 0), 31);
        assert_eq!(RegionFile::chunk_index(0, 31), 992);
        assert_eq!(RegionFile::chunk_index(31, 31), 1023);
    }

    #[test]
    fn test_chunk_index_negative_coords() {
        // Chunk (-1, -1) local = (31, 31)
        assert_eq!(RegionFile::chunk_index(-1, -1), 1023);
        // Chunk (-32, -32) local = (0, 0)
        assert_eq!(RegionFile::chunk_index(-32, -32), 0);
    }

    #[test]
    fn test_chunk_index_wraps_correctly() {
        // Chunk (32, 0) wraps to local (0, 0) in a different region
        assert_eq!(RegionFile::chunk_index(32, 0), 0);
        // Chunk (33, 1) wraps to local (1, 1)
        assert_eq!(RegionFile::chunk_index(33, 1), 33);
    }

    // ── RegionFile with synthetic data ─────────────────────────────

    #[test]
    fn test_region_file_open_and_read_chunk() {
        use std::io::Write;

        let dir = std::env::temp_dir();
        let path = dir.join("oxidized_test_region.mca");

        // Build a synthetic region file with one chunk at slot 0
        let mut file_data = vec![0u8; HEADER_BYTES];

        // Slot 0: sector_number=2, sector_count=1
        let raw: u32 = (2 << 8) | 1;
        file_data[0..4].copy_from_slice(&raw.to_be_bytes());

        // Timestamp for slot 0
        let ts: u32 = 1_700_000_000;
        file_data[SECTOR_BYTES..SECTOR_BYTES + 4].copy_from_slice(&ts.to_be_bytes());

        // Chunk data at sector 2: compress some test bytes with zlib
        let test_nbt = b"test NBT payload data";
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(test_nbt).unwrap();
        let compressed = encoder.finish().unwrap();

        // Pad file_data to sector 2 start
        file_data.resize(SECTOR_BYTES * 2, 0);

        // Write chunk header: 4-byte length + 1-byte codec
        let payload_len = (compressed.len() + 1) as u32;
        file_data.extend_from_slice(&payload_len.to_be_bytes());
        file_data.push(2); // Zlib
        file_data.extend_from_slice(&compressed);

        // Pad to sector boundary
        let total = file_data.len();
        let padded = total.div_ceil(SECTOR_BYTES) * SECTOR_BYTES;
        file_data.resize(padded, 0);

        std::fs::write(&path, &file_data).unwrap();

        // Read it back
        let mut region = RegionFile::open(&path).unwrap();
        assert!(region.has_chunk(0, 0));
        assert!(!region.has_chunk(1, 0));
        assert_eq!(region.chunk_timestamp(0, 0), 1_700_000_000);

        let data = region.read_chunk_data(0, 0).unwrap().unwrap();
        assert_eq!(data, test_nbt);

        // Non-existent chunk returns None
        assert!(region.read_chunk_data(1, 0).unwrap().is_none());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_region_file_too_small() {
        let dir = std::env::temp_dir();
        let path = dir.join("oxidized_test_region_small.mca");
        std::fs::write(&path, [0u8; 100]).unwrap();

        let result = RegionFile::open(&path);
        assert!(result.is_err());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_region_file_invalid_sector_skipped() {
        let dir = std::env::temp_dir();
        let path = dir.join("oxidized_test_region_bad_sector.mca");

        // Build a file that's only 2 sectors (header) but has an offset
        // pointing to sector 10 (beyond file).
        let mut file_data = vec![0u8; HEADER_BYTES];
        let raw: u32 = (10 << 8) | 1;
        file_data[0..4].copy_from_slice(&raw.to_be_bytes());

        std::fs::write(&path, &file_data).unwrap();

        let mut region = RegionFile::open(&path).unwrap();
        // Should return None (sanitized during header read) rather than error
        let result = region.read_chunk_data(0, 0).unwrap();
        assert!(result.is_none());
        // The entry should have been zeroed during header parse
        assert!(!region.has_chunk(0, 0));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_region_file_sector_count_zero_sanitized() {
        let dir = std::env::temp_dir();
        let path = dir.join("oxidized_test_region_zero_count.mca");

        // Build a file with 3 sectors (header + 1 data sector)
        let mut file_data = vec![0u8; SECTOR_BYTES * 3];

        // Slot 0: sector_number=2, sector_count=0 — invalid
        let raw: u32 = 2 << 8; // sector_count = 0
        file_data[0..4].copy_from_slice(&raw.to_be_bytes());

        std::fs::write(&path, &file_data).unwrap();

        let region = RegionFile::open(&path).unwrap();
        // Entry should be sanitized to absent during header read
        assert!(!region.has_chunk(0, 0));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_region_file_sector_number_overlaps_header_sanitized() {
        let dir = std::env::temp_dir();
        let path = dir.join("oxidized_test_region_header_overlap.mca");

        // Build a file with enough sectors
        let mut file_data = vec![0u8; SECTOR_BYTES * 4];

        // Slot 0: sector_number=1 (overlaps header), sector_count=1
        let raw: u32 = (1 << 8) | 1;
        file_data[0..4].copy_from_slice(&raw.to_be_bytes());

        // Slot 1: sector_number=0 (also invalid), sector_count=1
        let raw2: u32 = (0 << 8) | 1;
        file_data[4..8].copy_from_slice(&raw2.to_be_bytes());

        std::fs::write(&path, &file_data).unwrap();

        let region = RegionFile::open(&path).unwrap();
        // Both should be sanitized
        assert!(!region.has_chunk(0, 0));
        assert!(!region.has_chunk(1, 0));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_region_file_payload_exceeds_sectors() {
        let dir = std::env::temp_dir();
        let path = dir.join("oxidized_test_region_payload_overflow.mca");

        // Build a region with 1 sector of data for chunk at slot 0
        let mut file_data = vec![0u8; HEADER_BYTES];

        // Slot 0: sector_number=2, sector_count=1 (4096 bytes max)
        let raw: u32 = (2 << 8) | 1;
        file_data[0..4].copy_from_slice(&raw.to_be_bytes());

        // Pad to sector 2
        file_data.resize(SECTOR_BYTES * 2, 0);

        // Write a payload_len that claims to be larger than 1 sector
        // max_payload = 1 * 4096 - 4 = 4092 bytes
        let fake_payload_len: u32 = 4093; // exceeds max_payload
        file_data.extend_from_slice(&fake_payload_len.to_be_bytes());
        file_data.push(2); // Zlib codec byte

        // Pad to 3 sectors to not trigger file-length validation
        file_data.resize(SECTOR_BYTES * 3, 0);

        std::fs::write(&path, &file_data).unwrap();

        let mut region = RegionFile::open(&path).unwrap();
        // Should return None due to payload exceeding sector allocation
        let result = region.read_chunk_data(0, 0).unwrap();
        assert!(result.is_none());

        let _ = std::fs::remove_file(&path);
    }
}
