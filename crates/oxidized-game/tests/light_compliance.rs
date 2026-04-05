//! Light packet compliance tests for the lighting engine.
//!
//! Verifies that `LightUpdateData` wire format matches vanilla,
//! BitSet mask encoding is correct, and serialize-deserialize roundtrips.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use bytes::{Bytes, BytesMut};
use oxidized_game::net::light_serializer::build_light_data;
use oxidized_protocol::packets::play::LightUpdateData;
use oxidized_world::chunk::DataLayer;

/// Helper: serialize `LightUpdateData` to bytes and back.
fn roundtrip(data: &LightUpdateData) -> LightUpdateData {
    let mut buf = BytesMut::new();
    data.write_to(&mut buf);
    let mut bytes = Bytes::from(buf);
    LightUpdateData::read_from(&mut bytes).expect("roundtrip decode failed")
}

// --- Wire format tests ---

#[test]
fn test_empty_light_data_wire_format() {
    let data = LightUpdateData::empty();
    let mut buf = BytesMut::new();
    data.write_to(&mut buf);
    let bytes = buf.freeze();

    // Empty: 4 BitSets (each VarInt(0)) + 2 arrays (each VarInt(0)) = 6 zero VarInts
    // Each VarInt(0) is a single byte 0x00
    assert_eq!(bytes.len(), 6, "empty LightUpdateData should be 6 bytes");
    assert!(bytes.iter().all(|&b| b == 0), "all bytes should be 0x00");
}

#[test]
fn test_single_sky_section_wire_format() {
    let mut sky: Vec<Option<DataLayer>> = vec![None; 26];
    sky[0] = Some(DataLayer::filled(15));
    let block: Vec<Option<DataLayer>> = vec![None; 26];
    let data = build_light_data(&sky, &block);

    let mut buf = BytesMut::new();
    data.write_to(&mut buf);
    let bytes = buf.freeze();

    // sky_y_mask: VarInt(1) + i64(1) = 1 + 8 = 9 bytes (bit 0 set)
    // block_y_mask: VarInt(0) = 1 byte
    // empty_sky_y_mask: VarInt(0) = 1 byte
    // empty_block_y_mask: VarInt(0) = 1 byte
    // sky_updates: VarInt(1) + VarInt(2048) + 2048 bytes = 1 + 2 + 2048 = 2051
    // block_updates: VarInt(0) = 1 byte
    // Total: 9 + 1 + 1 + 1 + 2051 + 1 = 2064
    assert_eq!(bytes.len(), 2064, "single sky section packet size mismatch");
}

#[test]
fn test_bitset_mask_encoding_bit_0() {
    let mut sky: Vec<Option<DataLayer>> = vec![None; 26];
    sky[0] = Some(DataLayer::filled(15));
    let block: Vec<Option<DataLayer>> = vec![None; 26];
    let data = build_light_data(&sky, &block);

    // sky_y_mask should have bit 0 set → i64 value = 1
    assert_eq!(data.sky_y_mask.len(), 1);
    assert_eq!(data.sky_y_mask[0], 1);
}

#[test]
fn test_bitset_mask_encoding_bit_25() {
    let mut sky: Vec<Option<DataLayer>> = vec![None; 26];
    sky[25] = Some(DataLayer::filled(15));
    let block: Vec<Option<DataLayer>> = vec![None; 26];
    let data = build_light_data(&sky, &block);

    // Bit 25 should be set → i64 value = 1 << 25
    assert_eq!(data.sky_y_mask.len(), 1);
    assert_eq!(data.sky_y_mask[0], 1i64 << 25);
}

#[test]
fn test_bitset_mask_encoding_multiple_bits() {
    let mut sky: Vec<Option<DataLayer>> = vec![None; 26];
    sky[0] = Some(DataLayer::filled(15));
    sky[5] = Some(DataLayer::filled(10));
    sky[25] = Some(DataLayer::filled(1));
    let block: Vec<Option<DataLayer>> = vec![None; 26];
    let data = build_light_data(&sky, &block);

    let expected = 1i64 | (1i64 << 5) | (1i64 << 25);
    assert_eq!(data.sky_y_mask[0], expected);
    assert_eq!(data.sky_updates.len(), 3);
}

#[test]
fn test_empty_section_uses_empty_mask() {
    let mut sky: Vec<Option<DataLayer>> = vec![None; 26];
    // All-zero layer → should go into empty_sky_y_mask, not sky_y_mask
    sky[3] = Some(DataLayer::new());
    let block: Vec<Option<DataLayer>> = vec![None; 26];
    let data = build_light_data(&sky, &block);

    // sky_y_mask should NOT have bit 3
    if !data.sky_y_mask.is_empty() {
        assert_eq!(data.sky_y_mask[0] & (1 << 3), 0);
    }
    // empty_sky_y_mask SHOULD have bit 3
    assert_eq!(data.empty_sky_y_mask.len(), 1);
    assert_eq!(data.empty_sky_y_mask[0] & (1 << 3), 1 << 3);
    // No actual data arrays
    assert!(data.sky_updates.is_empty());
}

#[test]
fn test_none_section_sets_no_mask_bits() {
    let sky: Vec<Option<DataLayer>> = vec![None; 26];
    let block: Vec<Option<DataLayer>> = vec![None; 26];
    let data = build_light_data(&sky, &block);

    assert!(data.sky_y_mask.is_empty());
    assert!(data.block_y_mask.is_empty());
    assert!(data.empty_sky_y_mask.is_empty());
    assert!(data.empty_block_y_mask.is_empty());
}

#[test]
fn test_light_data_array_is_2048_bytes() {
    let mut sky: Vec<Option<DataLayer>> = vec![None; 26];
    sky[0] = Some(DataLayer::filled(7));
    let block: Vec<Option<DataLayer>> = vec![None; 26];
    let data = build_light_data(&sky, &block);

    assert_eq!(data.sky_updates.len(), 1);
    assert_eq!(
        data.sky_updates[0].len(),
        2048,
        "each light section array must be 2048 bytes"
    );
}

// --- Roundtrip tests ---

#[test]
fn test_roundtrip_empty_light_data() {
    let original = LightUpdateData::empty();
    let decoded = roundtrip(&original);
    assert_eq!(original, decoded);
}

#[test]
fn test_roundtrip_single_sky_section() {
    let mut sky: Vec<Option<DataLayer>> = vec![None; 26];
    sky[0] = Some(DataLayer::filled(15));
    let block: Vec<Option<DataLayer>> = vec![None; 26];
    let original = build_light_data(&sky, &block);
    let decoded = roundtrip(&original);
    assert_eq!(original, decoded);
}

#[test]
fn test_roundtrip_mixed_sky_and_block() {
    let mut sky: Vec<Option<DataLayer>> = vec![None; 26];
    let mut block: Vec<Option<DataLayer>> = vec![None; 26];
    sky[0] = Some(DataLayer::filled(15));
    sky[10] = Some(DataLayer::new()); // empty → goes to empty mask
    block[5] = Some(DataLayer::filled(14));
    block[20] = Some(DataLayer::filled(1));

    let original = build_light_data(&sky, &block);
    let decoded = roundtrip(&original);
    assert_eq!(original, decoded);
}

#[test]
fn test_roundtrip_all_26_sections() {
    let sky: Vec<Option<DataLayer>> = (0..26).map(|_| Some(DataLayer::filled(15))).collect();
    let block: Vec<Option<DataLayer>> = (0..26).map(|_| Some(DataLayer::filled(8))).collect();
    let original = build_light_data(&sky, &block);
    let decoded = roundtrip(&original);
    assert_eq!(original, decoded);
}

#[test]
fn test_roundtrip_preserves_light_values() {
    let mut layer = DataLayer::new();
    layer.set(0, 0, 0, 14); // torch-level light
    layer.set(5, 10, 3, 8);
    layer.set(15, 15, 15, 1);

    let mut sky: Vec<Option<DataLayer>> = vec![None; 26];
    sky[12] = Some(layer);
    let block: Vec<Option<DataLayer>> = vec![None; 26];

    let original = build_light_data(&sky, &block);
    let decoded = roundtrip(&original);

    // Verify the actual light values survive roundtrip
    assert_eq!(original.sky_updates.len(), 1);
    assert_eq!(decoded.sky_updates.len(), 1);
    assert_eq!(original.sky_updates[0], decoded.sky_updates[0]);

    // Decode the nibbles and verify specific values
    let data = &decoded.sky_updates[0];
    let reconstructed = DataLayer::from_bytes(data).expect("valid 2048-byte array");
    assert_eq!(reconstructed.get(0, 0, 0), 14);
    assert_eq!(reconstructed.get(5, 10, 3), 8);
    assert_eq!(reconstructed.get(15, 15, 15), 1);
}
