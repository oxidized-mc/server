//! VarInt and VarLong encoding/decoding for the Minecraft wire protocol.
//!
//! VarInt uses a variable-length encoding (LEB128 variant) where each byte
//! contributes 7 data bits and uses bit 7 as a continuation flag. VarInt
//! encodes a 32-bit value in 1–5 bytes; VarLong encodes a 64-bit value
//! in 1–10 bytes.
//!
//! See [ADR-007](../../../../docs/adr/adr-007-packet-codec.md) for design rationale.

use std::io;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Maximum number of bytes a VarInt can occupy on the wire.
pub const VARINT_MAX_BYTES: usize = 5;

/// Maximum number of bytes a VarLong can occupy on the wire.
pub const VARLONG_MAX_BYTES: usize = 10;

/// Errors that can occur during VarInt/VarLong decoding.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum VarIntError {
    /// The encoded value exceeds the maximum byte length.
    #[error("VarInt is too large (exceeded {max_bytes} bytes)")]
    TooLarge {
        /// Maximum allowed bytes for this type.
        max_bytes: usize,
    },

    /// The buffer ended before the value was fully decoded.
    #[error("unexpected end of buffer")]
    UnexpectedEof,
}

// ---------------------------------------------------------------------------
// Synchronous encode / decode (buffer-based)
// ---------------------------------------------------------------------------

/// Encodes a 32-bit VarInt into `buf`, returning the number of bytes written (1–5).
pub fn encode_varint(mut value: i32, buf: &mut [u8; VARINT_MAX_BYTES]) -> usize {
    let mut i = 0;
    loop {
        let mut byte = (value & 0x7F) as u8;
        value = ((value as u32) >> 7) as i32;
        if value != 0 {
            byte |= 0x80;
        }
        buf[i] = byte;
        i += 1;
        if value == 0 {
            break;
        }
    }
    i
}

/// Decodes a VarInt from `buf`, returning `(value, bytes_consumed)`.
///
/// # Errors
///
/// Returns [`VarIntError::TooLarge`] if the value exceeds 5 bytes, or
/// [`VarIntError::UnexpectedEof`] if the buffer is too short.
pub fn decode_varint(buf: &[u8]) -> Result<(i32, usize), VarIntError> {
    let mut result: i32 = 0;
    let mut shift: u32 = 0;
    for (i, &byte) in buf.iter().enumerate() {
        result |= ((byte & 0x7F) as i32) << shift;
        if byte & 0x80 == 0 {
            return Ok((result, i + 1));
        }
        shift += 7;
        if shift >= 32 {
            return Err(VarIntError::TooLarge {
                max_bytes: VARINT_MAX_BYTES,
            });
        }
    }
    Err(VarIntError::UnexpectedEof)
}

/// Encodes a 64-bit VarLong into `buf`, returning the number of bytes written (1–10).
pub fn encode_varlong(mut value: i64, buf: &mut [u8; VARLONG_MAX_BYTES]) -> usize {
    let mut i = 0;
    loop {
        let mut byte = (value & 0x7F) as u8;
        value = ((value as u64) >> 7) as i64;
        if value != 0 {
            byte |= 0x80;
        }
        buf[i] = byte;
        i += 1;
        if value == 0 {
            break;
        }
    }
    i
}

/// Decodes a VarLong from `buf`, returning `(value, bytes_consumed)`.
///
/// # Errors
///
/// Returns [`VarIntError::TooLarge`] if the value exceeds 10 bytes, or
/// [`VarIntError::UnexpectedEof`] if the buffer is too short.
pub fn decode_varlong(buf: &[u8]) -> Result<(i64, usize), VarIntError> {
    let mut result: i64 = 0;
    let mut shift: u32 = 0;
    for (i, &byte) in buf.iter().enumerate() {
        result |= ((byte & 0x7F) as i64) << shift;
        if byte & 0x80 == 0 {
            return Ok((result, i + 1));
        }
        shift += 7;
        if shift >= 64 {
            return Err(VarIntError::TooLarge {
                max_bytes: VARLONG_MAX_BYTES,
            });
        }
    }
    Err(VarIntError::UnexpectedEof)
}

// ---------------------------------------------------------------------------
// Bytes / BytesMut helpers
// ---------------------------------------------------------------------------

/// Writes a VarInt to a [`BufMut`].
pub fn write_varint_buf(value: i32, buf: &mut BytesMut) {
    let mut tmp = [0u8; VARINT_MAX_BYTES];
    let len = encode_varint(value, &mut tmp);
    buf.put_slice(&tmp[..len]);
}

/// Reads a VarInt from a [`Buf`], advancing the cursor.
///
/// # Errors
///
/// Returns [`VarIntError`] on malformed or truncated input.
pub fn read_varint_buf(buf: &mut Bytes) -> Result<i32, VarIntError> {
    let mut result: i32 = 0;
    let mut shift: u32 = 0;
    loop {
        if !buf.has_remaining() {
            return Err(VarIntError::UnexpectedEof);
        }
        let byte = buf.get_u8();
        result |= ((byte & 0x7F) as i32) << shift;
        if byte & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift >= 32 {
            return Err(VarIntError::TooLarge {
                max_bytes: VARINT_MAX_BYTES,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Async helpers (tokio)
// ---------------------------------------------------------------------------

/// Reads a VarInt from an async reader, one byte at a time.
///
/// # Errors
///
/// Returns [`io::Error`] on I/O failure or malformed VarInt.
pub async fn read_varint_async(reader: &mut (impl AsyncRead + Unpin)) -> Result<i32, io::Error> {
    let mut result: i32 = 0;
    let mut shift: u32 = 0;
    loop {
        let byte = reader.read_u8().await?;
        result |= ((byte & 0x7F) as i32) << shift;
        if byte & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift >= 32 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                VarIntError::TooLarge {
                    max_bytes: VARINT_MAX_BYTES,
                },
            ));
        }
    }
}

/// Writes a VarInt to an async writer.
///
/// # Errors
///
/// Returns [`io::Error`] on I/O failure.
pub async fn write_varint_async(
    writer: &mut (impl AsyncWrite + Unpin),
    value: i32,
) -> Result<(), io::Error> {
    let mut tmp = [0u8; VARINT_MAX_BYTES];
    let len = encode_varint(value, &mut tmp);
    writer.write_all(&tmp[..len]).await?;
    Ok(())
}

/// Returns the number of bytes needed to encode `value` as a VarInt.
pub const fn varint_size(value: i32) -> usize {
    let value = value as u32;
    match value {
        0..=0x7F => 1,
        0x80..=0x3FFF => 2,
        0x4000..=0x1F_FFFF => 3,
        0x20_0000..=0x0FFF_FFFF => 4,
        _ => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // VarInt encode
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_varint_zero() {
        let mut buf = [0u8; VARINT_MAX_BYTES];
        let len = encode_varint(0, &mut buf);
        assert_eq!(&buf[..len], &[0x00]);
    }

    #[test]
    fn test_encode_varint_one() {
        let mut buf = [0u8; VARINT_MAX_BYTES];
        let len = encode_varint(1, &mut buf);
        assert_eq!(&buf[..len], &[0x01]);
    }

    #[test]
    fn test_encode_varint_127() {
        let mut buf = [0u8; VARINT_MAX_BYTES];
        let len = encode_varint(127, &mut buf);
        assert_eq!(&buf[..len], &[0x7F]);
    }

    #[test]
    fn test_encode_varint_128() {
        let mut buf = [0u8; VARINT_MAX_BYTES];
        let len = encode_varint(128, &mut buf);
        assert_eq!(&buf[..len], &[0x80, 0x01]);
    }

    #[test]
    fn test_encode_varint_255() {
        let mut buf = [0u8; VARINT_MAX_BYTES];
        let len = encode_varint(255, &mut buf);
        assert_eq!(&buf[..len], &[0xFF, 0x01]);
    }

    #[test]
    fn test_encode_varint_300() {
        let mut buf = [0u8; VARINT_MAX_BYTES];
        let len = encode_varint(300, &mut buf);
        assert_eq!(&buf[..len], &[0xAC, 0x02]);
    }

    #[test]
    fn test_encode_varint_25565() {
        let mut buf = [0u8; VARINT_MAX_BYTES];
        let len = encode_varint(25565, &mut buf);
        assert_eq!(&buf[..len], &[0xDD, 0xC7, 0x01]);
    }

    #[test]
    fn test_encode_varint_max() {
        let mut buf = [0u8; VARINT_MAX_BYTES];
        let len = encode_varint(i32::MAX, &mut buf);
        assert_eq!(&buf[..len], &[0xFF, 0xFF, 0xFF, 0xFF, 0x07]);
    }

    #[test]
    fn test_encode_varint_negative_one() {
        let mut buf = [0u8; VARINT_MAX_BYTES];
        let len = encode_varint(-1, &mut buf);
        assert_eq!(&buf[..len], &[0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
    }

    #[test]
    fn test_encode_varint_min() {
        let mut buf = [0u8; VARINT_MAX_BYTES];
        let len = encode_varint(i32::MIN, &mut buf);
        assert_eq!(&buf[..len], &[0x80, 0x80, 0x80, 0x80, 0x08]);
    }

    // -----------------------------------------------------------------------
    // VarInt decode
    // -----------------------------------------------------------------------

    #[test]
    fn test_decode_varint_zero() {
        let (val, len) = decode_varint(&[0x00]).unwrap();
        assert_eq!(val, 0);
        assert_eq!(len, 1);
    }

    #[test]
    fn test_decode_varint_300() {
        let (val, len) = decode_varint(&[0xAC, 0x02]).unwrap();
        assert_eq!(val, 300);
        assert_eq!(len, 2);
    }

    #[test]
    fn test_decode_varint_negative_one() {
        let (val, len) = decode_varint(&[0xFF, 0xFF, 0xFF, 0xFF, 0x0F]).unwrap();
        assert_eq!(val, -1);
        assert_eq!(len, 5);
    }

    #[test]
    fn test_decode_varint_empty_buffer() {
        let err = decode_varint(&[]).unwrap_err();
        assert_eq!(err, VarIntError::UnexpectedEof);
    }

    #[test]
    fn test_decode_varint_too_large() {
        // 6 continuation bytes — exceeds 5-byte limit
        let err = decode_varint(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x01]).unwrap_err();
        assert!(matches!(err, VarIntError::TooLarge { .. }));
    }

    #[test]
    fn test_decode_varint_with_trailing_data() {
        // Valid VarInt followed by extra bytes — should only consume the VarInt
        let (val, len) = decode_varint(&[0xAC, 0x02, 0xFF, 0xFF]).unwrap();
        assert_eq!(val, 300);
        assert_eq!(len, 2);
    }

    // -----------------------------------------------------------------------
    // VarInt roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn test_varint_roundtrip_edge_cases() {
        let values = [0, 1, -1, 127, 128, 255, 256, 25565, i32::MAX, i32::MIN];
        for &v in &values {
            let mut buf = [0u8; VARINT_MAX_BYTES];
            let len = encode_varint(v, &mut buf);
            let (decoded, consumed) = decode_varint(&buf[..len]).unwrap();
            assert_eq!(decoded, v, "roundtrip failed for {v}");
            assert_eq!(consumed, len, "consumed mismatch for {v}");
        }
    }

    // -----------------------------------------------------------------------
    // VarLong encode / decode
    // -----------------------------------------------------------------------

    #[test]
    fn test_varlong_roundtrip_edge_cases() {
        let values = [0i64, 1, -1, 127, 128, i32::MAX as i64, i64::MAX, i64::MIN];
        for &v in &values {
            let mut buf = [0u8; VARLONG_MAX_BYTES];
            let len = encode_varlong(v, &mut buf);
            let (decoded, consumed) = decode_varlong(&buf[..len]).unwrap();
            assert_eq!(decoded, v, "roundtrip failed for {v}");
            assert_eq!(consumed, len, "consumed mismatch for {v}");
        }
    }

    #[test]
    fn test_decode_varlong_too_large() {
        let bad = [0x80u8; 11]; // 11 continuation bytes
        let err = decode_varlong(&bad).unwrap_err();
        assert!(matches!(err, VarIntError::TooLarge { .. }));
    }

    // -----------------------------------------------------------------------
    // Bytes / BytesMut helpers
    // -----------------------------------------------------------------------

    #[test]
    fn test_buf_roundtrip() {
        let mut out = BytesMut::new();
        write_varint_buf(25565, &mut out);
        let mut input = out.freeze();
        let val = read_varint_buf(&mut input).unwrap();
        assert_eq!(val, 25565);
        assert!(!input.has_remaining());
    }

    // -----------------------------------------------------------------------
    // Async read/write
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_async_varint_roundtrip() {
        let mut buf = Vec::new();
        write_varint_async(&mut buf, 300).await.unwrap();

        let mut cursor = io::Cursor::new(buf);
        let val = read_varint_async(&mut cursor).await.unwrap();
        assert_eq!(val, 300);
    }

    #[tokio::test]
    async fn test_async_varint_negative() {
        let mut buf = Vec::new();
        write_varint_async(&mut buf, -1).await.unwrap();

        let mut cursor = io::Cursor::new(buf);
        let val = read_varint_async(&mut cursor).await.unwrap();
        assert_eq!(val, -1);
    }

    // -----------------------------------------------------------------------
    // varint_size
    // -----------------------------------------------------------------------

    #[test]
    fn test_varint_size() {
        assert_eq!(varint_size(0), 1);
        assert_eq!(varint_size(127), 1);
        assert_eq!(varint_size(128), 2);
        assert_eq!(varint_size(255), 2);
        assert_eq!(varint_size(25565), 3);
        assert_eq!(varint_size(i32::MAX), 5);
        assert_eq!(varint_size(-1), 5);
    }

    // -----------------------------------------------------------------------
    // Property-based tests
    // -----------------------------------------------------------------------

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_varint_roundtrip_proptest(value: i32) {
            let mut buf = [0u8; VARINT_MAX_BYTES];
            let len = encode_varint(value, &mut buf);
            let (decoded, consumed) = decode_varint(&buf[..len]).unwrap();
            prop_assert_eq!(decoded, value);
            prop_assert_eq!(consumed, len);
        }

        #[test]
        fn test_varlong_roundtrip_proptest(value: i64) {
            let mut buf = [0u8; VARLONG_MAX_BYTES];
            let len = encode_varlong(value, &mut buf);
            let (decoded, consumed) = decode_varlong(&buf[..len]).unwrap();
            prop_assert_eq!(decoded, value);
            prop_assert_eq!(consumed, len);
        }

        #[test]
        fn test_varint_size_matches_encode(value: i32) {
            let mut buf = [0u8; VARINT_MAX_BYTES];
            let len = encode_varint(value, &mut buf);
            prop_assert_eq!(varint_size(value), len);
        }
    }
}
