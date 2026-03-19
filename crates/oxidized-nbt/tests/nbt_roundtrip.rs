#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use std::io::Cursor;

use oxidized_nbt::{
    NbtAccounter, NbtCompound, NbtError, NbtList, NbtTag, format_snbt, format_snbt_pretty,
    parse_snbt, read_gzip, read_nbt, read_network_nbt, read_zlib, write_gzip, write_nbt,
    write_network_nbt, write_zlib,
};

// ── Helpers ─────────────────────────────────────────────────────────────

/// Builds a compound containing all 12 tag types.
fn all_types_compound() -> NbtCompound {
    let mut c = NbtCompound::new();
    c.put_byte("byte", 127);
    c.put_short("short", -32000);
    c.put_int("int", 1_000_000);
    c.put_long("long", 9_000_000_000i64);
    c.put_float("float", std::f32::consts::PI);
    c.put_double("double", std::f64::consts::E);
    c.put("byte_array", NbtTag::ByteArray(vec![1, 2, 3, -1]));
    c.put_string("string", "Hello, NBT!");

    let mut list = NbtList::new(3); // TAG_INT
    list.push(NbtTag::Int(10)).unwrap();
    list.push(NbtTag::Int(20)).unwrap();
    c.put("list", NbtTag::List(list));

    let mut inner = NbtCompound::new();
    inner.put_int("nested_val", 42);
    c.put("compound", NbtTag::Compound(inner));

    c.put("int_array", NbtTag::IntArray(vec![100, 200, 300]));
    c.put("long_array", NbtTag::LongArray(vec![1_000, 2_000, 3_000]));
    c
}

/// Write+read a compound through the binary (disk) format.
fn binary_roundtrip(compound: &NbtCompound) -> NbtCompound {
    let mut buf = Vec::new();
    write_nbt(&mut buf, compound).unwrap();
    let mut reader = buf.as_slice();
    let mut acc = NbtAccounter::unlimited();
    read_nbt(&mut reader, &mut acc).unwrap()
}

// ── Integration tests ───────────────────────────────────────────────────

#[test]
fn test_binary_roundtrip_complex_compound() {
    let original = all_types_compound();
    let result = binary_roundtrip(&original);

    assert_eq!(result.get_byte("byte"), Some(127));
    assert_eq!(result.get_short("short"), Some(-32000));
    assert_eq!(result.get_int("int"), Some(1_000_000));
    assert_eq!(result.get_long("long"), Some(9_000_000_000));
    assert_eq!(result.get_float("float"), Some(std::f32::consts::PI));
    assert_eq!(result.get_double("double"), Some(std::f64::consts::E));
    assert_eq!(
        result.get_byte_array("byte_array"),
        Some(&[1i8, 2, 3, -1][..])
    );
    assert_eq!(result.get_string("string"), Some("Hello, NBT!"));

    let list = result.get_list("list").unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list.get(0), Some(&NbtTag::Int(10)));
    assert_eq!(list.get(1), Some(&NbtTag::Int(20)));

    let inner = result.get_compound("compound").unwrap();
    assert_eq!(inner.get_int("nested_val"), Some(42));

    assert_eq!(
        result.get_int_array("int_array"),
        Some(&[100, 200, 300][..])
    );
    assert_eq!(
        result.get_long_array("long_array"),
        Some(&[1_000i64, 2_000, 3_000][..])
    );
}

#[test]
fn test_network_nbt_roundtrip() {
    let original = all_types_compound();

    // Write network format
    let mut net_buf = Vec::new();
    write_network_nbt(&mut net_buf, &original).unwrap();

    // Write disk format
    let mut disk_buf = Vec::new();
    write_nbt(&mut disk_buf, &original).unwrap();

    // Network format differs from disk (no root name bytes)
    assert_ne!(net_buf, disk_buf);
    // Network should be 2 bytes shorter (the empty root name u16 length)
    assert_eq!(net_buf.len() + 2, disk_buf.len());

    // Read back
    let mut reader = net_buf.as_slice();
    let mut acc = NbtAccounter::unlimited();
    let result = read_network_nbt(&mut reader, &mut acc).unwrap();
    assert_eq!(result, original);
}

#[test]
fn test_snbt_roundtrip_all_types() {
    let original = all_types_compound();
    let tag = NbtTag::Compound(original);

    let snbt1 = format_snbt(&tag);
    let parsed = parse_snbt(&snbt1).unwrap();
    let snbt2 = format_snbt(&parsed);

    assert_eq!(snbt1, snbt2);
}

#[test]
fn test_snbt_pretty_format() {
    let mut outer = NbtCompound::new();
    let mut inner = NbtCompound::new();
    inner.put_int("x", 1);
    inner.put_string("name", "test");
    outer.put("nested", NbtTag::Compound(inner));
    outer.put_int("top_level", 99);

    let tag = NbtTag::Compound(outer.clone());
    let pretty = format_snbt_pretty(&tag, 2);

    // Pretty format should contain newlines and indentation
    assert!(pretty.contains('\n'));
    assert!(pretty.contains("  "));

    // Should parse back to the same value
    let parsed = parse_snbt(&pretty).unwrap();
    assert_eq!(parsed, tag);
}

#[test]
fn test_gzip_roundtrip() {
    let original = all_types_compound();
    let mut compressed = Vec::new();
    write_gzip(&mut compressed, &original).unwrap();

    let result = read_gzip(Cursor::new(compressed)).unwrap();
    assert_eq!(result, original);
}

#[test]
fn test_zlib_roundtrip() {
    let original = all_types_compound();
    let compressed = write_zlib(&original).unwrap();
    let result = read_zlib(&compressed).unwrap();
    assert_eq!(result, original);
}

#[test]
fn test_nested_compounds_deep() {
    // Build 50 levels of nesting: each has key "inner" -> Compound
    let mut compound = NbtCompound::new();
    compound.put_int("leaf", 12345);

    for _ in 0..50 {
        let mut outer = NbtCompound::new();
        outer.put("inner", NbtTag::Compound(compound));
        compound = outer;
    }

    let result = binary_roundtrip(&compound);

    // Walk 50 levels to reach the leaf
    let mut current = &result;
    for _ in 0..50 {
        current = current
            .get_compound("inner")
            .expect("missing 'inner' compound");
    }
    assert_eq!(current.get_int("leaf"), Some(12345));
}

#[test]
fn test_list_type_enforcement() {
    let mut list = NbtList::new(3); // TAG_INT
    list.push(NbtTag::Int(1)).unwrap();

    let err = list.push(NbtTag::String("oops".into())).unwrap_err();
    assert!(
        matches!(
            err,
            NbtError::ListTypeMismatch {
                expected: 3,
                got: 8
            }
        ),
        "expected ListTypeMismatch, got: {err:?}"
    );
}

#[test]
fn test_empty_compound_roundtrip() {
    let original = NbtCompound::new();
    let result = binary_roundtrip(&original);
    assert!(result.is_empty());
    assert_eq!(result, original);
}

#[test]
fn test_large_arrays_roundtrip() {
    let mut compound = NbtCompound::new();

    let byte_arr: Vec<i8> = (0..10_000).map(|i| (i % 128) as i8).collect();
    let int_arr: Vec<i32> = (0..10_000).map(|i| i * 7).collect();
    let long_arr: Vec<i64> = (0..10_000).map(|i| i64::from(i) * 13).collect();

    compound.put("bytes", NbtTag::ByteArray(byte_arr.clone()));
    compound.put("ints", NbtTag::IntArray(int_arr.clone()));
    compound.put("longs", NbtTag::LongArray(long_arr.clone()));

    let result = binary_roundtrip(&compound);

    assert_eq!(result.get_byte_array("bytes").unwrap(), byte_arr.as_slice());
    assert_eq!(result.get_int_array("ints").unwrap(), int_arr.as_slice());
    assert_eq!(result.get_long_array("longs").unwrap(), long_arr.as_slice());
}

// ── Property-based tests (proptest) ─────────────────────────────────────

use proptest::prelude::*;

/// Wrap a single tag in a compound under key "v", write_nbt, read_nbt, extract.
fn binary_roundtrip_tag(tag: NbtTag) -> NbtTag {
    let mut c = NbtCompound::new();
    c.put("v", tag);
    let result = binary_roundtrip(&c);
    result.get("v").expect("missing key 'v'").clone()
}

proptest! {
    #[test]
    fn proptest_byte_roundtrip(val: i8) {
        let result = binary_roundtrip_tag(NbtTag::Byte(val));
        prop_assert_eq!(result, NbtTag::Byte(val));
    }

    #[test]
    fn proptest_short_roundtrip(val: i16) {
        let result = binary_roundtrip_tag(NbtTag::Short(val));
        prop_assert_eq!(result, NbtTag::Short(val));
    }

    #[test]
    fn proptest_int_roundtrip(val: i32) {
        let result = binary_roundtrip_tag(NbtTag::Int(val));
        prop_assert_eq!(result, NbtTag::Int(val));
    }

    #[test]
    fn proptest_long_roundtrip(val: i64) {
        let result = binary_roundtrip_tag(NbtTag::Long(val));
        prop_assert_eq!(result, NbtTag::Long(val));
    }

    #[test]
    fn proptest_float_roundtrip(val in proptest::num::f32::NORMAL) {
        let result = binary_roundtrip_tag(NbtTag::Float(val));
        prop_assert_eq!(result, NbtTag::Float(val));
    }

    #[test]
    fn proptest_double_roundtrip(val in proptest::num::f64::NORMAL) {
        let result = binary_roundtrip_tag(NbtTag::Double(val));
        prop_assert_eq!(result, NbtTag::Double(val));
    }

    #[test]
    fn proptest_string_roundtrip(val in "[\\PC]{0,100}") {
        let result = binary_roundtrip_tag(NbtTag::String(val.clone()));
        prop_assert_eq!(result, NbtTag::String(val));
    }

    #[test]
    fn proptest_snbt_primitive_roundtrip(val: i32) {
        let tag = NbtTag::Int(val);
        let snbt = format_snbt(&tag);
        let parsed = parse_snbt(&snbt).unwrap();
        prop_assert_eq!(parsed, tag);
    }
}
