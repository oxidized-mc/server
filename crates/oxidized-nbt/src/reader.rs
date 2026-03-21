//! Binary NBT reader.
//!
//! Reads NBT data from any `impl Read` source using big-endian byte order,
//! Modified UTF-8 strings, and per-tag memory accounting that matches the
//! vanilla Minecraft server.

use std::io::Read;

use crate::accounter::NbtAccounter;
use crate::compound::NbtCompound;
use crate::error::{
    NbtError, TAG_BYTE, TAG_BYTE_ARRAY, TAG_COMPOUND, TAG_DOUBLE, TAG_END, TAG_FLOAT, TAG_INT,
    TAG_INT_ARRAY, TAG_LIST, TAG_LONG, TAG_LONG_ARRAY, TAG_SHORT, TAG_STRING,
};
use crate::list::NbtList;
use crate::mutf8::decode_modified_utf8;
use crate::tag::NbtTag;

// ── I/O helpers ─────────────────────────────────────────────────────────

fn map_eof(e: std::io::Error) -> NbtError {
    if e.kind() == std::io::ErrorKind::UnexpectedEof {
        NbtError::UnexpectedEnd
    } else {
        NbtError::Io(e)
    }
}

/// Generates a typed big-endian read helper for a fixed-size primitive.
macro_rules! read_primitive {
    ($fn_name:ident, $ty:ty, $size:literal) => {
        fn $fn_name<R: Read>(reader: &mut R) -> Result<$ty, NbtError> {
            let mut buf = [0u8; $size];
            reader.read_exact(&mut buf).map_err(map_eof)?;
            Ok(<$ty>::from_be_bytes(buf))
        }
    };
}

fn read_u8<R: Read>(reader: &mut R) -> Result<u8, NbtError> {
    let mut buf = [0u8; 1];
    reader.read_exact(&mut buf).map_err(map_eof)?;
    Ok(buf[0])
}

fn read_i8<R: Read>(reader: &mut R) -> Result<i8, NbtError> {
    Ok(read_u8(reader)? as i8)
}

read_primitive!(read_i16, i16, 2);
read_primitive!(read_u16, u16, 2);
read_primitive!(read_i32, i32, 4);
read_primitive!(read_i64, i64, 8);
read_primitive!(read_f32, f32, 4);
read_primitive!(read_f64, f64, 8);

/// Reads a Modified UTF-8 string (u16 length prefix + data).
fn read_string<R: Read>(reader: &mut R) -> Result<String, NbtError> {
    let len = read_u16(reader)? as usize;
    let mut data = vec![0u8; len];
    reader.read_exact(&mut data).map_err(map_eof)?;
    decode_modified_utf8(&data)
}

// ── Compound payload ────────────────────────────────────────────────────

fn read_compound_payload<R: Read>(
    reader: &mut R,
    accounter: &mut NbtAccounter,
) -> Result<NbtCompound, NbtError> {
    let mut compound = NbtCompound::new();
    loop {
        let type_id = read_u8(reader)?;
        if type_id == TAG_END {
            break;
        }
        if type_id > TAG_LONG_ARRAY {
            return Err(NbtError::InvalidTagType(type_id));
        }
        // 32 bytes per-entry overhead (key hash-map node)
        accounter.account_bytes(32)?;
        let name = read_string(reader)?;
        let tag = read_payload(reader, type_id, accounter)?;
        compound.put(name, tag);
    }
    Ok(compound)
}

// ── Public API ──────────────────────────────────────────────────────────

/// Reads a single named tag: `[type_id: u8][name: mutf8][payload]`.
///
/// If `type_id` is `TAG_END`, returns an empty name and `NbtTag::Byte(0)`
/// as a sentinel.
///
/// # Errors
///
/// Returns an error on I/O failure, invalid data, or resource limit violation.
pub fn read_named_tag<R: Read>(
    reader: &mut R,
    accounter: &mut NbtAccounter,
) -> Result<(String, NbtTag), NbtError> {
    let type_id = read_u8(reader)?;
    if type_id == TAG_END {
        return Ok((String::new(), NbtTag::Byte(0)));
    }
    if type_id > TAG_LONG_ARRAY {
        return Err(NbtError::InvalidTagType(type_id));
    }
    let name = read_string(reader)?;
    let tag = read_payload(reader, type_id, accounter)?;
    Ok((name, tag))
}

/// Reads a root compound in the unnamed-tag format used on disk and over
/// the network: `[TAG_COMPOUND][name: mutf8][compound payload]`.
///
/// The root name is read and discarded.
///
/// # Examples
///
/// ```
/// use oxidized_nbt::{NbtAccounter, NbtCompound, read_nbt, write_nbt};
///
/// let mut compound = NbtCompound::new();
/// compound.put_int("score", 100);
///
/// let mut buf = Vec::new();
/// write_nbt(&mut buf, &compound).unwrap();
///
/// let mut reader = buf.as_slice();
/// let mut acc = NbtAccounter::unlimited();
/// let result = read_nbt(&mut reader, &mut acc).unwrap();
/// assert_eq!(result.get_int("score"), Some(100));
/// ```
///
/// # Errors
///
/// Returns an error if the root tag is not a compound, or on any I/O or
/// accounting failure.
pub fn read_nbt<R: Read>(
    reader: &mut R,
    accounter: &mut NbtAccounter,
) -> Result<NbtCompound, NbtError> {
    let type_id = read_u8(reader)?;
    if type_id != TAG_COMPOUND {
        return Err(NbtError::InvalidFormat(format!(
            "expected root compound tag ({}), got {}",
            TAG_COMPOUND, type_id
        )));
    }
    // Read and discard the root name.
    let _name = read_string(reader)?;
    accounter.account_bytes(48)?;
    accounter.push_depth()?;
    let compound = read_compound_payload(reader, accounter)?;
    accounter.pop_depth();
    Ok(compound)
}

/// Reads a root compound from the network protocol format:
/// `[TAG_COMPOUND][compound payload]` — **no root name**.
///
/// This is the format used for Minecraft protocol packets (1.20.2+).
///
/// # Examples
///
/// ```
/// use oxidized_nbt::{NbtAccounter, NbtCompound, read_network_nbt, write_network_nbt};
///
/// let mut compound = NbtCompound::new();
/// compound.put_byte("op", 1);
///
/// let mut buf = Vec::new();
/// write_network_nbt(&mut buf, &compound).unwrap();
///
/// let mut reader = buf.as_slice();
/// let mut acc = NbtAccounter::default_quota();
/// let result = read_network_nbt(&mut reader, &mut acc).unwrap();
/// assert_eq!(result.get_byte("op"), Some(1));
/// ```
///
/// # Errors
///
/// Returns an error if the root tag is not a compound, or on any I/O or
/// accounting failure.
pub fn read_network_nbt<R: Read>(
    reader: &mut R,
    accounter: &mut NbtAccounter,
) -> Result<NbtCompound, NbtError> {
    let type_id = read_u8(reader)?;
    if type_id != TAG_COMPOUND {
        return Err(NbtError::InvalidFormat(format!(
            "expected root compound tag ({}), got {}",
            TAG_COMPOUND, type_id
        )));
    }
    accounter.account_bytes(48)?;
    accounter.push_depth()?;
    let compound = read_compound_payload(reader, accounter)?;
    accounter.pop_depth();
    Ok(compound)
}

/// Reads a length-prefixed typed array (IntArray / LongArray).
///
/// Validates the length, accounts for memory, and reads `len` elements
/// using the provided reader function.
fn read_typed_array<R: Read, T>(
    reader: &mut R,
    accounter: &mut NbtAccounter,
    elem_size: usize,
    type_name: &str,
    read_elem: fn(&mut R) -> Result<T, NbtError>,
) -> Result<Vec<T>, NbtError> {
    let len = read_i32(reader)?;
    if len < 0 {
        return Err(NbtError::InvalidFormat(format!(
            "negative {type_name} array length: {len}"
        )));
    }
    let len = len as usize;
    accounter.account_bytes(
        elem_size
            .checked_mul(len)
            .and_then(|v| v.checked_add(24))
            .ok_or_else(|| NbtError::InvalidFormat(format!("{type_name} array size overflow")))?,
    )?;
    let mut arr = Vec::with_capacity(len);
    for _ in 0..len {
        arr.push(read_elem(reader)?);
    }
    Ok(arr)
}

/// Reads the payload for a specific tag type.
///
/// The `type_id` must be in the range `TAG_BYTE..=TAG_LONG_ARRAY`.
/// Memory and depth are tracked through `accounter`.
///
/// # Errors
///
/// Returns an error for invalid types, I/O failures, or limit violations.
pub fn read_payload<R: Read>(
    reader: &mut R,
    type_id: u8,
    accounter: &mut NbtAccounter,
) -> Result<NbtTag, NbtError> {
    match type_id {
        TAG_END => Err(NbtError::InvalidFormat(
            "unexpected TAG_END in payload".into(),
        )),

        TAG_BYTE => {
            accounter.account_bytes(9)?;
            Ok(NbtTag::Byte(read_i8(reader)?))
        },

        TAG_SHORT => {
            accounter.account_bytes(10)?;
            Ok(NbtTag::Short(read_i16(reader)?))
        },

        TAG_INT => {
            accounter.account_bytes(12)?;
            Ok(NbtTag::Int(read_i32(reader)?))
        },

        TAG_LONG => {
            accounter.account_bytes(16)?;
            Ok(NbtTag::Long(read_i64(reader)?))
        },

        TAG_FLOAT => {
            accounter.account_bytes(12)?;
            Ok(NbtTag::Float(read_f32(reader)?))
        },

        TAG_DOUBLE => {
            accounter.account_bytes(16)?;
            Ok(NbtTag::Double(read_f64(reader)?))
        },

        TAG_BYTE_ARRAY => {
            let len = read_i32(reader)?;
            if len < 0 {
                return Err(NbtError::InvalidFormat(format!(
                    "negative byte array length: {len}"
                )));
            }
            let len = len as usize;
            accounter.account_bytes(
                len.checked_add(24)
                    .ok_or_else(|| NbtError::InvalidFormat("byte array size overflow".into()))?,
            )?;
            let mut data = vec![0u8; len];
            reader.read_exact(&mut data).map_err(map_eof)?;
            let arr: Vec<i8> = data.into_iter().map(|b| b as i8).collect();
            Ok(NbtTag::ByteArray(arr))
        },

        TAG_STRING => {
            let str_len = read_u16(reader)? as usize;
            accounter.account_bytes(
                (2_usize)
                    .checked_mul(str_len)
                    .and_then(|v| v.checked_add(36))
                    .ok_or_else(|| NbtError::InvalidFormat("string size overflow".into()))?,
            )?;
            let mut data = vec![0u8; str_len];
            reader.read_exact(&mut data).map_err(map_eof)?;
            let s = decode_modified_utf8(&data)?;
            Ok(NbtTag::String(s))
        },

        TAG_LIST => {
            let elem_type = read_u8(reader)?;
            let count = read_i32(reader)?;
            if count < 0 {
                return Err(NbtError::InvalidFormat(format!(
                    "negative list count: {count}"
                )));
            }
            let count = count as usize;
            if count > 0 && elem_type == TAG_END {
                return Err(NbtError::InvalidFormat(
                    "non-empty list with TAG_END element type".into(),
                ));
            }
            if elem_type > TAG_LONG_ARRAY && elem_type != TAG_END {
                return Err(NbtError::InvalidTagType(elem_type));
            }
            accounter.account_bytes(
                (4_usize)
                    .checked_mul(count)
                    .and_then(|v| v.checked_add(36))
                    .ok_or_else(|| NbtError::InvalidFormat("list size overflow".into()))?,
            )?;
            accounter.push_depth()?;
            let mut list = NbtList::new(elem_type);
            for _ in 0..count {
                let tag = read_payload(reader, elem_type, accounter)?;
                // Type is guaranteed to match — the element type comes from
                // the list header and we read with that same type.
                list.push(tag)?;
            }
            accounter.pop_depth();
            Ok(NbtTag::List(list))
        },

        TAG_COMPOUND => {
            accounter.account_bytes(48)?;
            accounter.push_depth()?;
            let compound = read_compound_payload(reader, accounter)?;
            accounter.pop_depth();
            Ok(NbtTag::Compound(compound))
        },

        TAG_INT_ARRAY => {
            let arr = read_typed_array(reader, accounter, 4, "int", read_i32)?;
            Ok(NbtTag::IntArray(arr))
        },

        TAG_LONG_ARRAY => {
            let arr = read_typed_array(reader, accounter, 8, "long", read_i64)?;
            Ok(NbtTag::LongArray(arr))
        },

        _ => Err(NbtError::InvalidTagType(type_id)),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::error::MAX_DEPTH;
    use crate::writer::{write_named_tag, write_nbt};

    /// Helper: write an NbtCompound to bytes and read it back.
    fn roundtrip_compound(compound: &NbtCompound) -> NbtCompound {
        let mut buf = Vec::new();
        write_nbt(&mut buf, compound).unwrap();
        let mut reader = buf.as_slice();
        let mut acc = NbtAccounter::unlimited();
        read_nbt(&mut reader, &mut acc).unwrap()
    }

    /// Helper: write a named tag to bytes and read it back.
    fn roundtrip_named(name: &str, tag: &NbtTag) -> (String, NbtTag) {
        let mut buf = Vec::new();
        write_named_tag(&mut buf, name, tag).unwrap();
        let mut reader = buf.as_slice();
        let mut acc = NbtAccounter::unlimited();
        read_named_tag(&mut reader, &mut acc).unwrap()
    }

    #[test]
    fn test_roundtrip_byte() {
        let (name, tag) = roundtrip_named("b", &NbtTag::Byte(-7));
        assert_eq!(name, "b");
        assert_eq!(tag, NbtTag::Byte(-7));
    }

    #[test]
    fn test_roundtrip_short() {
        let (name, tag) = roundtrip_named("s", &NbtTag::Short(12345));
        assert_eq!(name, "s");
        assert_eq!(tag, NbtTag::Short(12345));
    }

    #[test]
    fn test_roundtrip_int() {
        let (name, tag) = roundtrip_named("i", &NbtTag::Int(-100_000));
        assert_eq!(name, "i");
        assert_eq!(tag, NbtTag::Int(-100_000));
    }

    #[test]
    fn test_roundtrip_long() {
        let (name, tag) = roundtrip_named("l", &NbtTag::Long(i64::MIN));
        assert_eq!(name, "l");
        assert_eq!(tag, NbtTag::Long(i64::MIN));
    }

    #[test]
    fn test_roundtrip_float() {
        let (_, tag) = roundtrip_named("f", &NbtTag::Float(1.5));
        assert_eq!(tag, NbtTag::Float(1.5));
    }

    #[test]
    fn test_roundtrip_double() {
        let (_, tag) = roundtrip_named("d", &NbtTag::Double(std::f64::consts::PI));
        assert_eq!(tag, NbtTag::Double(std::f64::consts::PI));
    }

    #[test]
    fn test_roundtrip_string() {
        let (_, tag) = roundtrip_named("str", &NbtTag::String("Hello NBT! 🎮".into()));
        assert_eq!(tag, NbtTag::String("Hello NBT! 🎮".into()));
    }

    #[test]
    fn test_roundtrip_byte_array() {
        let arr = vec![1i8, -2, 3, 127, -128];
        let (_, tag) = roundtrip_named("ba", &NbtTag::ByteArray(arr.clone()));
        assert_eq!(tag, NbtTag::ByteArray(arr));
    }

    #[test]
    fn test_roundtrip_int_array() {
        let arr = vec![10, -20, i32::MAX, i32::MIN];
        let (_, tag) = roundtrip_named("ia", &NbtTag::IntArray(arr.clone()));
        assert_eq!(tag, NbtTag::IntArray(arr));
    }

    #[test]
    fn test_roundtrip_long_array() {
        let arr = vec![100i64, -200, i64::MAX, i64::MIN];
        let (_, tag) = roundtrip_named("la", &NbtTag::LongArray(arr.clone()));
        assert_eq!(tag, NbtTag::LongArray(arr));
    }

    #[test]
    fn test_roundtrip_empty_compound() {
        let c = NbtCompound::new();
        let result = roundtrip_compound(&c);
        assert!(result.is_empty());
    }

    #[test]
    fn test_roundtrip_nested_compound() {
        let mut inner = NbtCompound::new();
        inner.put_int("x", 1);
        inner.put_string("name", "test");

        let mut outer = NbtCompound::new();
        outer.put("child", inner);
        outer.put_byte("flag", 1);

        let result = roundtrip_compound(&outer);
        assert_eq!(result.get_byte("flag"), Some(1));
        let child = result.get_compound("child").unwrap();
        assert_eq!(child.get_int("x"), Some(1));
        assert_eq!(child.get_string("name"), Some("test"));
    }

    #[test]
    fn test_roundtrip_list_of_ints() {
        let mut list = NbtList::new(TAG_INT);
        list.push(NbtTag::Int(10)).unwrap();
        list.push(NbtTag::Int(20)).unwrap();
        list.push(NbtTag::Int(30)).unwrap();

        let (_, tag) = roundtrip_named("nums", &NbtTag::List(list));
        let result = tag.as_list().unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result.element_type(), TAG_INT);
        assert_eq!(result.get(0), Some(&NbtTag::Int(10)));
        assert_eq!(result.get(2), Some(&NbtTag::Int(30)));
    }

    #[test]
    fn test_roundtrip_list_of_compounds() {
        let mut list = NbtList::new(TAG_COMPOUND);
        let mut c1 = NbtCompound::new();
        c1.put_int("id", 1);
        let mut c2 = NbtCompound::new();
        c2.put_int("id", 2);
        list.push(NbtTag::Compound(c1)).unwrap();
        list.push(NbtTag::Compound(c2)).unwrap();

        let (_, tag) = roundtrip_named("entries", &NbtTag::List(list));
        let result = tag.as_list().unwrap();
        assert_eq!(result.len(), 2);
        let ids: Vec<i32> = result.compounds().filter_map(|c| c.get_int("id")).collect();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn test_roundtrip_empty_list() {
        let list = NbtList::empty();
        let (_, tag) = roundtrip_named("empty", &NbtTag::List(list));
        let result = tag.as_list().unwrap();
        assert!(result.is_empty());
        assert_eq!(result.element_type(), TAG_END);
    }

    #[test]
    fn test_roundtrip_complex_structure() {
        let mut root = NbtCompound::new();
        root.put_string("name", "Test World");
        root.put_int("version", 19133);
        root.put_long("seed", 123456789);
        root.put_byte("hardcore", 0);
        root.put_double("spawn_x", 100.5);
        root.put_float("rain", 0.75);
        root.put("data", NbtTag::ByteArray(vec![1, 2, 3, 4]));
        root.put("ids", NbtTag::IntArray(vec![100, 200, 300]));
        root.put("timestamps", NbtTag::LongArray(vec![1_000_000, 2_000_000]));

        let mut list = NbtList::new(TAG_COMPOUND);
        let mut entry = NbtCompound::new();
        entry.put_string("item", "diamond");
        entry.put_byte("count", 64);
        list.push(NbtTag::Compound(entry)).unwrap();
        root.put("inventory", list);

        let result = roundtrip_compound(&root);
        assert_eq!(result.get_string("name"), Some("Test World"));
        assert_eq!(result.get_int("version"), Some(19133));
        assert_eq!(result.get_long("seed"), Some(123456789));
        assert_eq!(result.get_byte("hardcore"), Some(0));
        assert_eq!(result.get_double("spawn_x"), Some(100.5));
        assert_eq!(result.get_float("rain"), Some(0.75));
        assert_eq!(result.get_byte_array("data"), Some(&[1i8, 2, 3, 4][..]));
        assert_eq!(result.get_int_array("ids"), Some(&[100, 200, 300][..]));
        assert_eq!(
            result.get_long_array("timestamps"),
            Some(&[1_000_000i64, 2_000_000][..])
        );

        let inv = result.get_list("inventory").unwrap();
        assert_eq!(inv.len(), 1);
        let item = inv.compounds().next().unwrap();
        assert_eq!(item.get_string("item"), Some("diamond"));
        assert_eq!(item.get_byte("count"), Some(64));
    }

    #[test]
    fn test_accounter_rejects_oversized_data() {
        let mut compound = NbtCompound::new();
        compound.put_string("key", "a]".repeat(500));

        let mut buf = Vec::new();
        write_nbt(&mut buf, &compound).unwrap();

        let mut reader = buf.as_slice();
        let mut acc = NbtAccounter::new(10);
        let result = read_nbt(&mut reader, &mut acc);
        assert!(result.is_err());
    }

    #[test]
    fn test_depth_limit_enforcement() {
        // Build binary data that exceeds MAX_DEPTH directly, bypassing the
        // writer's own depth checks.
        let mut buf = Vec::new();
        // Root: TAG_COMPOUND + empty name
        buf.push(TAG_COMPOUND);
        buf.extend_from_slice(&0u16.to_be_bytes());
        // Nest MAX_DEPTH + 1 compounds (each named "n")
        for _ in 0..=MAX_DEPTH {
            // TAG_COMPOUND + name "n"
            buf.push(TAG_COMPOUND);
            buf.extend_from_slice(&1u16.to_be_bytes());
            buf.push(b'n');
        }
        // Leaf byte at the deepest level
        buf.push(TAG_BYTE);
        buf.extend_from_slice(&1u16.to_be_bytes());
        buf.push(b'v');
        buf.push(1);
        // Close all compounds
        buf.extend(std::iter::repeat_n(TAG_END, MAX_DEPTH + 2));

        let mut reader = buf.as_slice();
        let mut acc = NbtAccounter::unlimited();
        let result = read_nbt(&mut reader, &mut acc);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_named_tag_end_sentinel() {
        let data = [TAG_END];
        let mut reader = data.as_slice();
        let mut acc = NbtAccounter::unlimited();
        let (name, tag) = read_named_tag(&mut reader, &mut acc).unwrap();
        assert_eq!(name, "");
        assert_eq!(tag, NbtTag::Byte(0));
    }

    #[test]
    fn test_invalid_tag_type_rejected() {
        // A tag with type 99 (invalid)
        let data = [99u8, 0, 0];
        let mut reader = data.as_slice();
        let mut acc = NbtAccounter::unlimited();
        let result = read_named_tag(&mut reader, &mut acc);
        assert!(result.is_err());
    }

    #[test]
    fn test_unexpected_end_on_empty_input() {
        let data: &[u8] = &[];
        let mut reader = data;
        let mut acc = NbtAccounter::unlimited();
        let result = read_named_tag(&mut reader, &mut acc);
        assert!(result.is_err());
    }
}
