//! Modified UTF-8 encoding and decoding.
//!
//! Java's Modified UTF-8 differs from standard UTF-8 in two ways:
//! - Null bytes (U+0000) are encoded as `[0xC0, 0x80]` instead of `[0x00]`.
//! - Supplementary characters (> U+FFFF) are encoded as a surrogate pair
//!   using two 3-byte CESU-8 sequences rather than a single 4-byte UTF-8 sequence.

use crate::error::NbtError;

/// Encodes a Rust `&str` into Modified UTF-8 bytes.
pub fn encode_modified_utf8(s: &str) -> Vec<u8> {
    let mut result = Vec::with_capacity(s.len());
    for ch in s.chars() {
        let code = ch as u32;
        if code == 0 {
            // Null byte: encoded as overlong 2-byte sequence
            result.push(0xC0);
            result.push(0x80);
        } else if code <= 0x7F {
            result.push(code as u8);
        } else if code <= 0x7FF {
            result.push(0xC0 | (code >> 6) as u8);
            result.push(0x80 | (code & 0x3F) as u8);
        } else if code <= 0xFFFF {
            result.push(0xE0 | (code >> 12) as u8);
            result.push(0x80 | ((code >> 6) & 0x3F) as u8);
            result.push(0x80 | (code & 0x3F) as u8);
        } else {
            // Supplementary character: encode as surrogate pair (CESU-8)
            let adjusted = code - 0x10000;
            let high = 0xD800 + (adjusted >> 10);
            let low = 0xDC00 + (adjusted & 0x3FF);
            // High surrogate (3 bytes)
            result.push(0xE0 | (high >> 12) as u8);
            result.push(0x80 | ((high >> 6) & 0x3F) as u8);
            result.push(0x80 | (high & 0x3F) as u8);
            // Low surrogate (3 bytes)
            result.push(0xE0 | (low >> 12) as u8);
            result.push(0x80 | ((low >> 6) & 0x3F) as u8);
            result.push(0x80 | (low & 0x3F) as u8);
        }
    }
    result
}

/// Decodes Modified UTF-8 bytes into a Rust [`String`].
///
/// # Errors
///
/// Returns [`NbtError::InvalidUtf8`] if the data contains invalid sequences,
/// bare null bytes, lone surrogates, or 4-byte sequences.
pub fn decode_modified_utf8(data: &[u8]) -> Result<String, NbtError> {
    let mut result = String::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if b == 0 {
            // Bare null byte is invalid in modified UTF-8
            return Err(NbtError::InvalidUtf8);
        } else if b & 0x80 == 0 {
            // 1-byte ASCII (0x01–0x7F)
            result.push(b as char);
            i += 1;
        } else if b & 0xE0 == 0xC0 {
            // 2-byte sequence
            if i + 1 >= data.len() {
                return Err(NbtError::InvalidUtf8);
            }
            let b2 = data[i + 1];
            if b2 & 0xC0 != 0x80 {
                return Err(NbtError::InvalidUtf8);
            }
            let code = u32::from(b & 0x1F) << 6 | u32::from(b2 & 0x3F);
            if code == 0 {
                // [0xC0, 0x80] → U+0000 (null)
                result.push('\0');
            } else {
                result.push(char::from_u32(code).ok_or(NbtError::InvalidUtf8)?);
            }
            i += 2;
        } else if b & 0xF0 == 0xE0 {
            // 3-byte sequence
            if i + 2 >= data.len() {
                return Err(NbtError::InvalidUtf8);
            }
            let b2 = data[i + 1];
            let b3 = data[i + 2];
            if b2 & 0xC0 != 0x80 || b3 & 0xC0 != 0x80 {
                return Err(NbtError::InvalidUtf8);
            }
            let code = u32::from(b & 0x0F) << 12 | u32::from(b2 & 0x3F) << 6 | u32::from(b3 & 0x3F);

            if (0xD800..=0xDBFF).contains(&code) {
                // High surrogate — expect low surrogate immediately after
                if i + 5 >= data.len() {
                    return Err(NbtError::InvalidUtf8);
                }
                let b4 = data[i + 3];
                let b5 = data[i + 4];
                let b6 = data[i + 5];
                if b4 & 0xF0 != 0xE0 || b5 & 0xC0 != 0x80 || b6 & 0xC0 != 0x80 {
                    return Err(NbtError::InvalidUtf8);
                }
                let low =
                    u32::from(b4 & 0x0F) << 12 | u32::from(b5 & 0x3F) << 6 | u32::from(b6 & 0x3F);
                if !(0xDC00..=0xDFFF).contains(&low) {
                    return Err(NbtError::InvalidUtf8);
                }
                let supplementary = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                result.push(char::from_u32(supplementary).ok_or(NbtError::InvalidUtf8)?);
                i += 6;
            } else if (0xDC00..=0xDFFF).contains(&code) {
                // Lone low surrogate is invalid
                return Err(NbtError::InvalidUtf8);
            } else {
                result.push(char::from_u32(code).ok_or(NbtError::InvalidUtf8)?);
                i += 3;
            }
        } else {
            // 4-byte sequences are not valid in modified UTF-8
            return Err(NbtError::InvalidUtf8);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_empty_string_roundtrip() {
        let encoded = encode_modified_utf8("");
        assert!(encoded.is_empty());
        let decoded = decode_modified_utf8(&encoded).unwrap();
        assert_eq!(decoded, "");
    }

    #[test]
    fn test_ascii_roundtrip() {
        let s = "Hello, NBT world!";
        let encoded = encode_modified_utf8(s);
        assert_eq!(encoded, s.as_bytes());
        let decoded = decode_modified_utf8(&encoded).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn test_null_byte_encoding() {
        let s = "\0";
        let encoded = encode_modified_utf8(s);
        assert_eq!(encoded, &[0xC0, 0x80]);
    }

    #[test]
    fn test_null_byte_decoding() {
        let decoded = decode_modified_utf8(&[0xC0, 0x80]).unwrap();
        assert_eq!(decoded, "\0");
    }

    #[test]
    fn test_null_byte_roundtrip() {
        let s = "before\0after";
        let encoded = encode_modified_utf8(s);
        let decoded = decode_modified_utf8(&encoded).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn test_bmp_characters_roundtrip() {
        // CJK characters (BMP, 3-byte in both UTF-8 and MUTF-8)
        let s = "你好世界";
        let encoded = encode_modified_utf8(s);
        let decoded = decode_modified_utf8(&encoded).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn test_supplementary_character_roundtrip() {
        // 🎮 = U+1F3AE (supplementary, requires surrogate pair in MUTF-8)
        let s = "🎮";
        let encoded = encode_modified_utf8(s);
        // Supplementary → 6 bytes (two 3-byte surrogate halves)
        assert_eq!(encoded.len(), 6);
        let decoded = decode_modified_utf8(&encoded).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn test_mixed_content_roundtrip() {
        let s = "Hello\0世界🎮!";
        let encoded = encode_modified_utf8(s);
        let decoded = decode_modified_utf8(&encoded).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn test_bare_null_byte_is_invalid() {
        let result = decode_modified_utf8(&[0x00]);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_continuation_byte() {
        // Start of 2-byte sequence followed by non-continuation byte
        let result = decode_modified_utf8(&[0xC2, 0x00]);
        assert!(result.is_err());
    }

    #[test]
    fn test_truncated_two_byte_sequence() {
        let result = decode_modified_utf8(&[0xC2]);
        assert!(result.is_err());
    }

    #[test]
    fn test_truncated_three_byte_sequence() {
        let result = decode_modified_utf8(&[0xE0, 0x80]);
        assert!(result.is_err());
    }

    #[test]
    fn test_four_byte_utf8_is_invalid() {
        // 4-byte UTF-8 lead byte is not valid in modified UTF-8
        let result = decode_modified_utf8(&[0xF0, 0x9F, 0x8E, 0xAE]);
        assert!(result.is_err());
    }

    #[test]
    fn test_lone_low_surrogate_is_invalid() {
        // Encode a lone low surrogate (0xDC00) as a 3-byte sequence
        let low: u32 = 0xDC00;
        let data = [
            0xE0 | (low >> 12) as u8,
            0x80 | ((low >> 6) & 0x3F) as u8,
            0x80 | (low & 0x3F) as u8,
        ];
        let result = decode_modified_utf8(&data);
        assert!(result.is_err());
    }
}
