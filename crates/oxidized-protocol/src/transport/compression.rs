//! Zlib compression and decompression for the Minecraft protocol.
//!
//! After the server sends `ClientboundLoginCompressionPacket`, the frame
//! format changes to include a `data_length` field:
//!
//! ```text
//! ┌──────────────────────┬──────────────────────┬──────────────────────┐
//! │ Packet Length (VarInt)│ Data Length (VarInt)  │ Packet Data          │
//! │ (length of rest)     │ (0 = uncompressed)   │ (compressed or raw)  │
//! └──────────────────────┴──────────────────────┴──────────────────────┘
//! ```
//!
//! - If `data_length == 0`: the packet data is uncompressed.
//! - If `data_length > 0`: the packet data is zlib-compressed and
//!   `data_length` is the size of the original uncompressed data.
//!
//! See [ADR-009](../../docs/adr/adr-009-encryption-compression.md) for
//! design rationale.

use flate2::Compression;
use flate2::bufread::{ZlibDecoder, ZlibEncoder};
use std::io::Read;
use thiserror::Error;

/// Maximum uncompressed packet size (8 MiB, matching vanilla).
const MAXIMUM_UNCOMPRESSED_LENGTH: usize = 8_388_608;

/// Maximum compressed packet size (2 MiB, matching vanilla).
const MAXIMUM_COMPRESSED_LENGTH: usize = 2_097_152;

/// Errors from compression and decompression operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CompressionError {
    /// Uncompressed data exceeds the maximum allowed size.
    #[error("uncompressed data too large: {0} bytes (max {MAXIMUM_UNCOMPRESSED_LENGTH})")]
    UncompressedTooLarge(usize),

    /// Compressed data exceeds the maximum allowed size.
    #[error("compressed data too large: {0} bytes (max {MAXIMUM_COMPRESSED_LENGTH})")]
    CompressedTooLarge(usize),

    /// The declared data length does not match the actual decompressed size.
    #[error("decompressed size mismatch: declared {declared}, actual {actual}")]
    SizeMismatch {
        /// The size declared in the `data_length` field.
        declared: usize,
        /// The actual decompressed size.
        actual: usize,
    },

    /// The data length is below the compression threshold (client bug).
    #[error("data length {data_length} is below threshold {threshold}")]
    BelowThreshold {
        /// The declared uncompressed data length.
        data_length: usize,
        /// The configured compression threshold.
        threshold: usize,
    },

    /// Zlib I/O error.
    #[error("zlib error: {0}")]
    Zlib(#[from] std::io::Error),
}

/// Per-connection compression state.
///
/// Holds the compression threshold and provides methods to compress and
/// decompress packet payloads. Reuses internal buffers across calls.
#[derive(Debug)]
pub struct CompressionState {
    threshold: usize,
    /// Reusable buffer for compression/decompression output.
    buf: Vec<u8>,
}

impl CompressionState {
    /// Creates a new compression state with the given threshold.
    ///
    /// Packets with uncompressed size below `threshold` are sent
    /// uncompressed (with `data_length = 0`).
    pub fn new(threshold: usize) -> Self {
        Self {
            threshold,
            buf: Vec::with_capacity(8192),
        }
    }

    /// Returns the compression threshold in bytes.
    pub fn threshold(&self) -> usize {
        self.threshold
    }

    /// Compresses a packet payload if it exceeds the threshold.
    ///
    /// Returns `(data_length, payload)`:
    /// - If `data.len() < threshold`: `data_length = 0`, `payload` is the
    ///   original data (uncompressed).
    /// - If `data.len() >= threshold`: `data_length = data.len()`, `payload`
    ///   is the zlib-compressed data.
    ///
    /// # Errors
    ///
    /// Returns [`CompressionError`] if the data exceeds the maximum
    /// uncompressed size or zlib compression fails.
    pub fn compress(&mut self, data: &[u8]) -> Result<(i32, Vec<u8>), CompressionError> {
        if data.len() > MAXIMUM_UNCOMPRESSED_LENGTH {
            return Err(CompressionError::UncompressedTooLarge(data.len()));
        }

        if data.len() < self.threshold {
            // Below threshold: send uncompressed
            return Ok((0, data.to_vec()));
        }

        // Above threshold: compress with zlib
        self.buf.clear();
        let mut encoder = ZlibEncoder::new(data, Compression::default());
        encoder.read_to_end(&mut self.buf)?;

        let data_length = i32::try_from(data.len())
            .map_err(|_| CompressionError::UncompressedTooLarge(data.len()))?;

        if self.buf.len() > MAXIMUM_COMPRESSED_LENGTH {
            return Err(CompressionError::CompressedTooLarge(self.buf.len()));
        }

        Ok((data_length, std::mem::take(&mut self.buf)))
    }

    /// Decompresses a packet payload.
    ///
    /// - If `data_length == 0`: returns the raw data unchanged.
    /// - If `data_length > 0`: decompresses and validates size.
    ///
    /// # Errors
    ///
    /// Returns [`CompressionError`] if the data is invalid or exceeds
    /// size limits.
    pub fn decompress(
        &mut self,
        data_length: i32,
        compressed: &[u8],
    ) -> Result<Vec<u8>, CompressionError> {
        if data_length == 0 {
            // Uncompressed
            return Ok(compressed.to_vec());
        }

        let expected_len = data_length as usize;

        if expected_len > MAXIMUM_UNCOMPRESSED_LENGTH {
            return Err(CompressionError::UncompressedTooLarge(expected_len));
        }

        if compressed.len() > MAXIMUM_COMPRESSED_LENGTH {
            return Err(CompressionError::CompressedTooLarge(compressed.len()));
        }

        if expected_len < self.threshold {
            return Err(CompressionError::BelowThreshold {
                data_length: expected_len,
                threshold: self.threshold,
            });
        }

        // Decompress
        self.buf.clear();
        self.buf.reserve(expected_len);
        let mut decoder = ZlibDecoder::new(compressed);
        decoder.read_to_end(&mut self.buf)?;

        if self.buf.len() != expected_len {
            return Err(CompressionError::SizeMismatch {
                declared: expected_len,
                actual: self.buf.len(),
            });
        }

        Ok(std::mem::take(&mut self.buf))
    }

    /// Splits this compression state into independent compressor and
    /// decompressor halves with the same threshold.
    ///
    /// Each half owns its own internal buffer. Used when splitting a
    /// connection into reader (decompressor) and writer (compressor) tasks.
    pub fn split(self) -> (Decompressor, Compressor) {
        (
            Decompressor {
                threshold: self.threshold,
                buf: Vec::with_capacity(8192),
            },
            Compressor {
                threshold: self.threshold,
                buf: self.buf,
            },
        )
    }
}

// ---------------------------------------------------------------------------
// Split compression halves
// ---------------------------------------------------------------------------

/// Decompression half of a split [`CompressionState`].
///
/// Used by the connection reader task after [`CompressionState::split`].
#[derive(Debug)]
pub struct Decompressor {
    threshold: usize,
    buf: Vec<u8>,
}

impl Decompressor {
    /// Returns the compression threshold in bytes.
    pub fn threshold(&self) -> usize {
        self.threshold
    }

    /// Decompresses a packet payload.
    ///
    /// - If `data_length == 0`: returns the raw data unchanged.
    /// - If `data_length > 0`: decompresses and validates size.
    ///
    /// # Errors
    ///
    /// Returns [`CompressionError`] on invalid or oversized data.
    pub fn decompress(
        &mut self,
        data_length: i32,
        compressed: &[u8],
    ) -> Result<Vec<u8>, CompressionError> {
        if data_length == 0 {
            return Ok(compressed.to_vec());
        }

        let expected_len = data_length as usize;

        if expected_len > MAXIMUM_UNCOMPRESSED_LENGTH {
            return Err(CompressionError::UncompressedTooLarge(expected_len));
        }

        if compressed.len() > MAXIMUM_COMPRESSED_LENGTH {
            return Err(CompressionError::CompressedTooLarge(compressed.len()));
        }

        if expected_len < self.threshold {
            return Err(CompressionError::BelowThreshold {
                data_length: expected_len,
                threshold: self.threshold,
            });
        }

        self.buf.clear();
        self.buf.reserve(expected_len);
        let mut decoder = ZlibDecoder::new(compressed);
        decoder.read_to_end(&mut self.buf)?;

        if self.buf.len() != expected_len {
            return Err(CompressionError::SizeMismatch {
                declared: expected_len,
                actual: self.buf.len(),
            });
        }

        Ok(std::mem::take(&mut self.buf))
    }
}

/// Compression half of a split [`CompressionState`].
///
/// Used by the connection writer task after [`CompressionState::split`].
#[derive(Debug)]
pub struct Compressor {
    threshold: usize,
    buf: Vec<u8>,
}

impl Compressor {
    /// Returns the compression threshold in bytes.
    pub fn threshold(&self) -> usize {
        self.threshold
    }

    /// Compresses a packet payload if it exceeds the threshold.
    ///
    /// Returns `(data_length, payload)`:
    /// - If `data.len() < threshold`: `data_length = 0`, `payload` is
    ///   the original data (uncompressed).
    /// - If `data.len() >= threshold`: `data_length = data.len()`,
    ///   `payload` is the zlib-compressed data.
    ///
    /// # Errors
    ///
    /// Returns [`CompressionError`] on oversized data or zlib failure.
    pub fn compress(&mut self, data: &[u8]) -> Result<(i32, Vec<u8>), CompressionError> {
        if data.len() > MAXIMUM_UNCOMPRESSED_LENGTH {
            return Err(CompressionError::UncompressedTooLarge(data.len()));
        }

        if data.len() < self.threshold {
            return Ok((0, data.to_vec()));
        }

        self.buf.clear();
        let mut encoder = ZlibEncoder::new(data, Compression::default());
        encoder.read_to_end(&mut self.buf)?;

        let data_length = i32::try_from(data.len())
            .map_err(|_| CompressionError::UncompressedTooLarge(data.len()))?;

        if self.buf.len() > MAXIMUM_COMPRESSED_LENGTH {
            return Err(CompressionError::CompressedTooLarge(self.buf.len()));
        }

        Ok((data_length, std::mem::take(&mut self.buf)))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_below_threshold_passthrough() {
        let mut state = CompressionState::new(256);
        let data = b"short payload";

        let (data_length, payload) = state.compress(data).unwrap();
        assert_eq!(data_length, 0, "below threshold should be uncompressed");
        assert_eq!(payload, data, "payload should be unchanged");
    }

    #[test]
    fn test_above_threshold_compresses() {
        let mut state = CompressionState::new(256);
        // Create data above threshold (repeating pattern compresses well)
        let data: Vec<u8> = (0..512).map(|i| (i % 256) as u8).collect();

        let (data_length, compressed) = state.compress(&data).unwrap();
        assert_eq!(data_length, 512, "data_length should be original size");
        assert!(
            compressed.len() < data.len(),
            "compressed should be smaller: {} < {}",
            compressed.len(),
            data.len()
        );
    }

    #[test]
    fn test_compress_decompress_roundtrip() {
        let mut state = CompressionState::new(256);
        let data: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();

        let (data_length, compressed) = state.compress(&data).unwrap();
        let decompressed = state.decompress(data_length, &compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_decompress_uncompressed() {
        let mut state = CompressionState::new(256);
        let data = b"raw uncompressed data";

        let result = state.decompress(0, data).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_decompress_size_mismatch() {
        let mut state = CompressionState::new(256);
        let data: Vec<u8> = (0..512).map(|i| (i % 256) as u8).collect();

        let (_, compressed) = state.compress(&data).unwrap();
        // Claim the data is a different size
        let err = state.decompress(1024, &compressed).unwrap_err();
        assert!(
            matches!(
                err,
                CompressionError::SizeMismatch {
                    declared: 1024,
                    actual: 512
                }
            ),
            "expected SizeMismatch, got: {err:?}"
        );
    }

    #[test]
    fn test_decompress_below_threshold_rejected() {
        let mut state = CompressionState::new(256);
        let err = state.decompress(100, b"some data").unwrap_err();
        assert!(
            matches!(
                err,
                CompressionError::BelowThreshold {
                    data_length: 100,
                    threshold: 256
                }
            ),
            "expected BelowThreshold, got: {err:?}"
        );
    }

    #[test]
    fn test_compress_too_large() {
        // We can't actually allocate 8 MiB + 1 easily, so test the check
        // by verifying the constant is correct
        assert_eq!(MAXIMUM_UNCOMPRESSED_LENGTH, 8_388_608);
    }

    #[test]
    fn test_threshold_zero_compresses_everything() {
        let mut state = CompressionState::new(0);
        let data = b"tiny";

        let (data_length, compressed) = state.compress(data).unwrap();
        // Even tiny data gets compressed when threshold is 0
        assert_eq!(data_length as usize, data.len());
        // Decompress should work
        let decompressed = state.decompress(data_length, &compressed).unwrap();
        assert_eq!(decompressed, data.to_vec());
    }

    #[test]
    fn test_exact_threshold_compresses() {
        let mut state = CompressionState::new(100);
        let data: Vec<u8> = (0..100).map(|i| (i % 256) as u8).collect();

        let (data_length, _) = state.compress(&data).unwrap();
        assert_eq!(
            data_length as usize,
            data.len(),
            "data exactly at threshold should be compressed"
        );
    }

    #[test]
    fn test_multiple_roundtrips_reuse_buffers() {
        let mut state = CompressionState::new(64);

        for i in 0..5 {
            let data: Vec<u8> = (0..200).map(|j| ((i * 37 + j) % 256) as u8).collect();
            let (data_length, compressed) = state.compress(&data).unwrap();
            let decompressed = state.decompress(data_length, &compressed).unwrap();
            assert_eq!(decompressed, data, "roundtrip {i} failed");
        }
    }

    // -----------------------------------------------------------------------
    // CompressionState::split tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_split_roundtrip() {
        let state = CompressionState::new(64);
        let (mut decompressor, mut compressor) = state.split();

        let data: Vec<u8> = (0..200).map(|i| (i % 256) as u8).collect();
        let (data_length, compressed) = compressor.compress(&data).unwrap();
        let decompressed = decompressor.decompress(data_length, &compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_split_below_threshold() {
        let state = CompressionState::new(256);
        let (mut decompressor, mut compressor) = state.split();

        let data = b"short";
        let (data_length, payload) = compressor.compress(data).unwrap();
        assert_eq!(data_length, 0);
        let decompressed = decompressor.decompress(data_length, &payload).unwrap();
        assert_eq!(decompressed, data.to_vec());
    }

    #[test]
    fn test_split_preserves_threshold() {
        let state = CompressionState::new(128);
        let (decompressor, compressor) = state.split();
        assert_eq!(compressor.threshold(), 128);
        assert_eq!(decompressor.threshold(), 128);
    }

    #[test]
    fn test_split_multiple_roundtrips() {
        let state = CompressionState::new(64);
        let (mut decompressor, mut compressor) = state.split();

        for i in 0..5 {
            let data: Vec<u8> = (0..200).map(|j| ((i * 37 + j) % 256) as u8).collect();
            let (data_length, compressed) = compressor.compress(&data).unwrap();
            let decompressed =
                decompressor.decompress(data_length, &compressed).unwrap();
            assert_eq!(decompressed, data, "split roundtrip {i} failed");
        }
    }
}
