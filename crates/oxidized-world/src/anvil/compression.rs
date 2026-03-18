//! Compression codecs used by the Anvil region file format.
//!
//! Each chunk in a region file is prefixed with a compression type byte.
//! The most common codec is Zlib (type 2), which is also the default for
//! writing.

use super::error::AnvilError;

/// Compression codec identifier stored before each chunk's compressed data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CompressionType {
    /// GZip compression (type 1). Legacy, rarely used.
    GZip = 1,
    /// Zlib/DEFLATE compression (type 2). The default and most common.
    Zlib = 2,
    /// No compression (type 3).
    None = 3,
    /// LZ4 block compression (type 4). Added in 24w04a.
    Lz4 = 4,
}

/// Bit flag indicating chunk data is stored in an external `.mcc` file.
pub const EXTERNAL_FLAG: u8 = 0x80;

impl CompressionType {
    /// Parses a compression type from the raw byte, ignoring the external flag.
    ///
    /// # Errors
    ///
    /// Returns [`AnvilError::UnknownCompression`] if the codec ID is not
    /// recognized.
    pub fn from_byte(b: u8) -> Result<Self, AnvilError> {
        match b & 0x7F {
            1 => Ok(Self::GZip),
            2 => Ok(Self::Zlib),
            3 => Ok(Self::None),
            4 => Ok(Self::Lz4),
            _ => Err(AnvilError::UnknownCompression(b)),
        }
    }

    /// Returns `true` if the external flag is set on the raw byte.
    #[must_use]
    pub fn is_external(b: u8) -> bool {
        b & EXTERNAL_FLAG != 0
    }
}

/// Decompresses chunk data according to the given codec.
///
/// # Errors
///
/// Returns [`AnvilError::Decompression`] if decompression fails.
pub fn decompress(data: &[u8], codec: CompressionType) -> Result<Vec<u8>, AnvilError> {
    use std::io::Read;
    match codec {
        CompressionType::GZip => {
            let mut decoder = flate2::read::GzDecoder::new(data);
            let mut out = Vec::new();
            decoder
                .read_to_end(&mut out)
                .map_err(|e| AnvilError::Decompression(e.to_string()))?;
            Ok(out)
        },
        CompressionType::Zlib => {
            let mut decoder = flate2::read::ZlibDecoder::new(data);
            let mut out = Vec::new();
            decoder
                .read_to_end(&mut out)
                .map_err(|e| AnvilError::Decompression(e.to_string()))?;
            Ok(out)
        },
        CompressionType::None => Ok(data.to_vec()),
        CompressionType::Lz4 => lz4_flex::decompress_size_prepended(data)
            .map_err(|e| AnvilError::Decompression(e.to_string())),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_type_from_byte() {
        assert_eq!(
            CompressionType::from_byte(1).unwrap(),
            CompressionType::GZip
        );
        assert_eq!(
            CompressionType::from_byte(2).unwrap(),
            CompressionType::Zlib
        );
        assert_eq!(
            CompressionType::from_byte(3).unwrap(),
            CompressionType::None
        );
        assert_eq!(CompressionType::from_byte(4).unwrap(), CompressionType::Lz4);
        assert!(CompressionType::from_byte(0).is_err());
        assert!(CompressionType::from_byte(5).is_err());
    }

    #[test]
    fn test_compression_type_from_byte_ignores_external_flag() {
        assert_eq!(
            CompressionType::from_byte(0x82).unwrap(),
            CompressionType::Zlib
        );
        assert_eq!(
            CompressionType::from_byte(0x81).unwrap(),
            CompressionType::GZip
        );
    }

    #[test]
    fn test_is_external() {
        assert!(!CompressionType::is_external(2));
        assert!(CompressionType::is_external(0x82));
        assert!(CompressionType::is_external(0x80));
    }

    #[test]
    fn test_zlib_roundtrip() {
        let original = b"hello world NBT data here";
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut encoder, original).unwrap();
        let compressed = encoder.finish().unwrap();
        let decompressed = decompress(&compressed, CompressionType::Zlib).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_gzip_roundtrip() {
        let original = b"gzip test data for chunks";
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut encoder, original).unwrap();
        let compressed = encoder.finish().unwrap();
        let decompressed = decompress(&compressed, CompressionType::GZip).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_none_passthrough() {
        let original = b"raw NBT bytes";
        let decompressed = decompress(original, CompressionType::None).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_lz4_roundtrip() {
        let original = b"lz4 test data for region files";
        let compressed = lz4_flex::compress_prepend_size(original);
        let decompressed = decompress(&compressed, CompressionType::Lz4).unwrap();
        assert_eq!(decompressed, original.as_slice());
    }
}
