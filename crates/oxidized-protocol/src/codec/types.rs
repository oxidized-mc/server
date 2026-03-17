//! Minecraft wire type helpers for reading/writing protocol primitives.
//!
//! These operate on [`Bytes`] / [`BytesMut`] buffers and cover the
//! common types that aren't VarInt/VarLong (which live in [`super::varint`]).

use bytes::{Buf, BufMut, Bytes, BytesMut};
use thiserror::Error;

use super::varint;

/// Errors from reading typed protocol values.
#[derive(Debug, Error)]
pub enum TypeError {
    /// String exceeds the maximum allowed length.
    #[error("string too long: {len} chars (max {max})")]
    StringTooLong {
        /// Actual length in characters.
        len: usize,
        /// Maximum allowed length.
        max: usize,
    },

    /// Negative string length prefix.
    #[error("negative string length: {0}")]
    NegativeLength(i32),

    /// Not enough bytes remaining in the buffer.
    #[error("unexpected end of buffer (need {need}, have {have})")]
    UnexpectedEof {
        /// Bytes needed.
        need: usize,
        /// Bytes remaining.
        have: usize,
    },

    /// VarInt decoding failed.
    #[error("varint error: {0}")]
    VarInt(#[from] varint::VarIntError),

    /// Invalid UTF-8 in a string.
    #[error("invalid UTF-8: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
}

/// Reads a VarInt-length-prefixed UTF-8 string from `buf`.
///
/// # Errors
///
/// Returns [`TypeError`] if the string is too long, the buffer is truncated,
/// or the bytes are not valid UTF-8.
pub fn read_string(buf: &mut Bytes, max_chars: usize) -> Result<String, TypeError> {
    let len = varint::read_varint_buf(buf)?;
    if len < 0 {
        return Err(TypeError::NegativeLength(len));
    }
    let len = len as usize;
    if buf.remaining() < len {
        return Err(TypeError::UnexpectedEof {
            need: len,
            have: buf.remaining(),
        });
    }
    let raw = buf.split_to(len);
    let s = String::from_utf8(raw.to_vec())?;
    let char_count = s.chars().count();
    if char_count > max_chars {
        return Err(TypeError::StringTooLong {
            len: char_count,
            max: max_chars,
        });
    }
    Ok(s)
}

/// Writes a VarInt-length-prefixed UTF-8 string to `buf`.
pub fn write_string(buf: &mut BytesMut, s: &str) {
    varint::write_varint_buf(s.len() as i32, buf);
    buf.put_slice(s.as_bytes());
}

/// Reads a big-endian `u16` from `buf`.
///
/// # Errors
///
/// Returns [`TypeError::UnexpectedEof`] if fewer than 2 bytes remain.
pub fn read_u16(buf: &mut Bytes) -> Result<u16, TypeError> {
    if buf.remaining() < 2 {
        return Err(TypeError::UnexpectedEof {
            need: 2,
            have: buf.remaining(),
        });
    }
    Ok(buf.get_u16())
}

/// Writes a big-endian `u16` to `buf`.
pub fn write_u16(buf: &mut BytesMut, value: u16) {
    buf.put_u16(value);
}

/// Reads a big-endian `i64` from `buf`.
///
/// # Errors
///
/// Returns [`TypeError::UnexpectedEof`] if fewer than 8 bytes remain.
pub fn read_i64(buf: &mut Bytes) -> Result<i64, TypeError> {
    if buf.remaining() < 8 {
        return Err(TypeError::UnexpectedEof {
            need: 8,
            have: buf.remaining(),
        });
    }
    Ok(buf.get_i64())
}

/// Writes a big-endian `i64` to `buf`.
pub fn write_i64(buf: &mut BytesMut, value: i64) {
    buf.put_i64(value);
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_string_roundtrip() {
        let mut out = BytesMut::new();
        write_string(&mut out, "Hello, Minecraft!");
        let mut input = out.freeze();
        let s = read_string(&mut input, 255).unwrap();
        assert_eq!(s, "Hello, Minecraft!");
        assert!(!input.has_remaining());
    }

    #[test]
    fn test_string_empty() {
        let mut out = BytesMut::new();
        write_string(&mut out, "");
        let mut input = out.freeze();
        let s = read_string(&mut input, 255).unwrap();
        assert_eq!(s, "");
    }

    #[test]
    fn test_string_unicode() {
        let mut out = BytesMut::new();
        write_string(&mut out, "§aGreen §rNormal 🎮");
        let mut input = out.freeze();
        let s = read_string(&mut input, 255).unwrap();
        assert_eq!(s, "§aGreen §rNormal 🎮");
    }

    #[test]
    fn test_string_truncated_buffer() {
        // Write a string but truncate the buffer
        let mut out = BytesMut::new();
        write_string(&mut out, "Hello");
        let mut input = out.freeze().slice(..3); // only length prefix + partial data
        let err = read_string(&mut input, 255).unwrap_err();
        assert!(matches!(err, TypeError::UnexpectedEof { .. }));
    }

    #[test]
    fn test_string_exceeds_max_chars() {
        // 300 ASCII chars should be rejected when max is 255
        let long = "A".repeat(300);
        let mut out = BytesMut::new();
        write_string(&mut out, &long);
        let mut input = out.freeze();
        let err = read_string(&mut input, 255).unwrap_err();
        assert!(matches!(
            err,
            TypeError::StringTooLong { len: 300, max: 255 }
        ));
    }

    #[test]
    fn test_string_exactly_max_chars() {
        let exact = "B".repeat(255);
        let mut out = BytesMut::new();
        write_string(&mut out, &exact);
        let mut input = out.freeze();
        let s = read_string(&mut input, 255).unwrap();
        assert_eq!(s.len(), 255);
    }

    #[test]
    fn test_u16_roundtrip() {
        let mut out = BytesMut::new();
        write_u16(&mut out, 25565);
        let mut input = out.freeze();
        assert_eq!(read_u16(&mut input).unwrap(), 25565);
    }

    #[test]
    fn test_u16_eof() {
        let mut input = Bytes::from_static(&[0x01]);
        let err = read_u16(&mut input).unwrap_err();
        assert!(matches!(err, TypeError::UnexpectedEof { .. }));
    }

    #[test]
    fn test_i64_roundtrip() {
        let mut out = BytesMut::new();
        write_i64(&mut out, 1234567890123);
        let mut input = out.freeze();
        assert_eq!(read_i64(&mut input).unwrap(), 1234567890123);
    }

    #[test]
    fn test_i64_negative() {
        let mut out = BytesMut::new();
        write_i64(&mut out, -42);
        let mut input = out.freeze();
        assert_eq!(read_i64(&mut input).unwrap(), -42);
    }

    #[test]
    fn test_i64_eof() {
        let mut input = Bytes::from_static(&[0x01, 0x02, 0x03]);
        let err = read_i64(&mut input).unwrap_err();
        assert!(matches!(err, TypeError::UnexpectedEof { .. }));
    }
}
