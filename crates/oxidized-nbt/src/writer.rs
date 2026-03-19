//! Binary NBT writer.
//!
//! Writes NBT data to any `impl Write` sink using big-endian byte order
//! and Modified UTF-8 strings.

use std::io::Write;

use crate::compound::NbtCompound;
use crate::error::{NbtError, TAG_COMPOUND, TAG_END};
use crate::mutf8::encode_modified_utf8;
use crate::tag::NbtTag;

/// Writes a Modified UTF-8 string with its u16 length prefix.
fn write_mutf8_str<W: Write>(writer: &mut W, s: &str) -> Result<(), NbtError> {
    let encoded = encode_modified_utf8(s);
    let len = u16::try_from(encoded.len()).map_err(|_| {
        NbtError::InvalidFormat(format!(
            "string too long for modified UTF-8: {} bytes (max {})",
            encoded.len(),
            u16::MAX
        ))
    })?;
    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(&encoded)?;
    Ok(())
}

/// Maximum nesting depth for writing (matches reader and parser).
const WRITE_MAX_DEPTH: usize = crate::error::MAX_DEPTH;

/// Validates that writing at the given depth is within limits.
fn check_write_depth(depth: usize) -> Result<(), NbtError> {
    if depth > WRITE_MAX_DEPTH {
        Err(NbtError::DepthLimit {
            depth,
            max: WRITE_MAX_DEPTH,
        })
    } else {
        Ok(())
    }
}

/// Safely converts a `usize` length to `i32` for the NBT wire format.
fn len_to_i32(len: usize) -> Result<i32, NbtError> {
    i32::try_from(len)
        .map_err(|_| NbtError::InvalidFormat(format!("length {len} exceeds i32::MAX")))
}

/// Writes the payload for a single tag (no type ID, no name).
fn write_payload<W: Write>(writer: &mut W, tag: &NbtTag, depth: usize) -> Result<(), NbtError> {
    match tag {
        NbtTag::Byte(v) => writer.write_all(&v.to_be_bytes())?,
        NbtTag::Short(v) => writer.write_all(&v.to_be_bytes())?,
        NbtTag::Int(v) => writer.write_all(&v.to_be_bytes())?,
        NbtTag::Long(v) => writer.write_all(&v.to_be_bytes())?,
        NbtTag::Float(v) => writer.write_all(&v.to_be_bytes())?,
        NbtTag::Double(v) => writer.write_all(&v.to_be_bytes())?,

        NbtTag::ByteArray(arr) => {
            writer.write_all(&len_to_i32(arr.len())?.to_be_bytes())?;
            for &b in arr {
                writer.write_all(&[b as u8])?;
            }
        },

        NbtTag::String(s) => write_mutf8_str(writer, s)?,

        NbtTag::List(list) => {
            let next_depth = depth + 1;
            check_write_depth(next_depth)?;
            writer.write_all(&[list.element_type()])?;
            writer.write_all(&len_to_i32(list.len())?.to_be_bytes())?;
            for element in list.iter() {
                write_payload(writer, element, next_depth)?;
            }
        },

        NbtTag::Compound(compound) => {
            let next_depth = depth + 1;
            check_write_depth(next_depth)?;
            write_compound_payload(writer, compound, next_depth)?;
        },

        NbtTag::IntArray(arr) => {
            writer.write_all(&len_to_i32(arr.len())?.to_be_bytes())?;
            for &v in arr {
                writer.write_all(&v.to_be_bytes())?;
            }
        },

        NbtTag::LongArray(arr) => {
            writer.write_all(&len_to_i32(arr.len())?.to_be_bytes())?;
            for &v in arr {
                writer.write_all(&v.to_be_bytes())?;
            }
        },
    }
    Ok(())
}

/// Writes a compound's entries followed by a TAG_END marker.
fn write_compound_payload<W: Write>(
    writer: &mut W,
    compound: &NbtCompound,
    depth: usize,
) -> Result<(), NbtError> {
    for (name, tag) in compound.iter() {
        write_named_tag_inner(writer, name, tag, depth)?;
    }
    writer.write_all(&[TAG_END])?;
    Ok(())
}

/// Writes a single named tag: `[type_id: u8][name: mutf8][payload]`.
fn write_named_tag_inner<W: Write>(
    writer: &mut W,
    name: &str,
    tag: &NbtTag,
    depth: usize,
) -> Result<(), NbtError> {
    writer.write_all(&[tag.type_id()])?;
    write_mutf8_str(writer, name)?;
    write_payload(writer, tag, depth)?;
    Ok(())
}

/// Writes a single named tag: `[type_id: u8][name: mutf8][payload]`.
///
/// # Errors
///
/// Returns an error on I/O failure or if the name exceeds the u16 length
/// limit for Modified UTF-8.
pub fn write_named_tag<W: Write>(writer: &mut W, name: &str, tag: &NbtTag) -> Result<(), NbtError> {
    write_named_tag_inner(writer, name, tag, 0)
}

/// Writes a root compound in the unnamed-tag format:
/// `[TAG_COMPOUND][empty name][compound payload]`.
///
/// This is the format used for disk files (level.dat, etc.).
///
/// # Examples
///
/// ```
/// use oxidized_nbt::{NbtCompound, write_nbt};
///
/// let mut compound = NbtCompound::new();
/// compound.put_string("level", "plains");
///
/// let mut buf = Vec::new();
/// write_nbt(&mut buf, &compound).unwrap();
/// assert!(!buf.is_empty());
/// ```
///
/// # Errors
///
/// Returns an error on I/O failure.
pub fn write_nbt<W: Write>(writer: &mut W, compound: &NbtCompound) -> Result<(), NbtError> {
    writer.write_all(&[TAG_COMPOUND])?;
    // Empty root name
    writer.write_all(&0u16.to_be_bytes())?;
    write_compound_payload(writer, compound, 1)?;
    Ok(())
}

/// Writes a root compound in the network protocol format:
/// `[TAG_COMPOUND][compound payload]` — **no root name**.
///
/// This is the format used for Minecraft protocol packets (1.20.2+).
///
/// # Examples
///
/// ```
/// use oxidized_nbt::{NbtCompound, write_network_nbt};
///
/// let mut compound = NbtCompound::new();
/// compound.put_int("entity_id", 7);
///
/// let mut buf = Vec::new();
/// write_network_nbt(&mut buf, &compound).unwrap();
/// assert!(!buf.is_empty());
/// ```
///
/// # Errors
///
/// Returns an error on I/O failure.
pub fn write_network_nbt<W: Write>(writer: &mut W, compound: &NbtCompound) -> Result<(), NbtError> {
    writer.write_all(&[TAG_COMPOUND])?;
    write_compound_payload(writer, compound, 1)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::error::{TAG_BYTE, TAG_INT};
    use crate::list::NbtList;

    #[test]
    fn test_write_empty_compound() {
        let compound = NbtCompound::new();
        let mut buf = Vec::new();
        write_nbt(&mut buf, &compound).unwrap();

        // TAG_COMPOUND(10) + name_len(0,0) + TAG_END(0)
        assert_eq!(buf, vec![TAG_COMPOUND, 0, 0, TAG_END]);
    }

    #[test]
    fn test_write_byte_named_tag() {
        let mut buf = Vec::new();
        write_named_tag(&mut buf, "b", &NbtTag::Byte(42)).unwrap();

        // type(1) + name_len(0,1) + 'b' + value(42)
        assert_eq!(buf, vec![TAG_BYTE, 0, 1, b'b', 42]);
    }

    #[test]
    fn test_write_compound_with_entries() {
        let mut compound = NbtCompound::new();
        compound.put_byte("x", 1);

        let mut buf = Vec::new();
        write_nbt(&mut buf, &compound).unwrap();

        // TAG_COMPOUND + name_len(0,0) + [entry: type(1) + name_len(0,1) + 'x' + value(1)] + TAG_END
        assert_eq!(
            buf,
            vec![TAG_COMPOUND, 0, 0, TAG_BYTE, 0, 1, b'x', 1, TAG_END]
        );
    }

    #[test]
    fn test_write_list_of_ints() {
        let mut list = NbtList::new(TAG_INT);
        list.push(NbtTag::Int(1)).unwrap();
        list.push(NbtTag::Int(2)).unwrap();

        let mut buf = Vec::new();
        write_named_tag(&mut buf, "nums", &NbtTag::List(list)).unwrap();

        // Verify we can parse: type_id=9, name="nums", element_type=3, count=2, values
        assert_eq!(buf[0], crate::error::TAG_LIST);
        // Name length = 4
        assert_eq!(u16::from_be_bytes([buf[1], buf[2]]), 4);
    }

    #[test]
    fn test_write_big_endian_int() {
        let mut buf = Vec::new();
        write_named_tag(&mut buf, "v", &NbtTag::Int(0x01020304)).unwrap();

        // After type(3) + name_len(0,1) + 'v' = 4 bytes, the payload starts:
        assert_eq!(&buf[4..8], &[0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_write_then_read_roundtrip() {
        let mut root = NbtCompound::new();
        root.put_string("msg", "hello");
        root.put_int("num", 42);

        let mut buf = Vec::new();
        write_nbt(&mut buf, &root).unwrap();

        let mut reader = buf.as_slice();
        let mut acc = crate::accounter::NbtAccounter::unlimited();
        let result = crate::reader::read_nbt(&mut reader, &mut acc).unwrap();

        assert_eq!(result.get_string("msg"), Some("hello"));
        assert_eq!(result.get_int("num"), Some(42));
    }

    #[test]
    fn test_write_depth_limit() {
        // Build a compound nested beyond MAX_DEPTH.
        let mut compound = NbtCompound::new();
        compound.put_byte("leaf", 1);
        for _ in 0..crate::error::MAX_DEPTH {
            let mut outer = NbtCompound::new();
            outer.put("n", NbtTag::Compound(compound));
            compound = outer;
        }

        let mut buf = Vec::new();
        let result = write_nbt(&mut buf, &compound);
        assert!(result.is_err(), "should reject deeply nested compound");
    }
}
