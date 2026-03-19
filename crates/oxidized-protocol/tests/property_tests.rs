//! Property-based tests for coordinate types and codec primitives.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use bytes::BytesMut;
use proptest::prelude::*;

use oxidized_protocol::codec::types;
use oxidized_protocol::types::block_pos::BlockPos;
use oxidized_protocol::types::chunk_pos::ChunkPos;
use oxidized_protocol::types::section_pos::SectionPos;

// ---------------------------------------------------------------------------
// BlockPos
// ---------------------------------------------------------------------------

proptest! {
    /// Pack → unpack roundtrip for every valid BlockPos.
    #[test]
    fn proptest_block_pos_pack_unpack(
        x in -33_554_432_i32..=33_554_431,
        y in -2048_i32..=2047,
        z in -33_554_432_i32..=33_554_431,
    ) {
        let pos = BlockPos::new(x, y, z);
        let roundtripped = BlockPos::from_long(pos.as_long());
        prop_assert_eq!(roundtripped, pos);
    }

    /// Wire (write → read) roundtrip for every valid BlockPos.
    #[test]
    fn proptest_block_pos_wire_roundtrip(
        x in -33_554_432_i32..=33_554_431,
        y in -2048_i32..=2047,
        z in -33_554_432_i32..=33_554_431,
    ) {
        let pos = BlockPos::new(x, y, z);
        let mut buf = BytesMut::new();
        pos.write(&mut buf);
        let mut data = buf.freeze();
        let decoded = BlockPos::read(&mut data).unwrap();
        prop_assert_eq!(decoded, pos);
    }
}

// ---------------------------------------------------------------------------
// ChunkPos
// ---------------------------------------------------------------------------

proptest! {
    /// Pack → unpack roundtrip for any i32 pair.
    #[test]
    fn proptest_chunk_pos_pack_unpack(x: i32, z: i32) {
        let pos = ChunkPos::new(x, z);
        let roundtripped = ChunkPos::from_long(pos.as_long());
        prop_assert_eq!(roundtripped, pos);
    }

    /// from_block_coords matches manual `>> 4`.
    #[test]
    fn proptest_chunk_pos_from_block_pos(bx: i32, bz: i32) {
        let chunk = ChunkPos::from_block_coords(bx, bz);
        prop_assert_eq!(chunk.x, bx >> 4);
        prop_assert_eq!(chunk.z, bz >> 4);
    }
}

// ---------------------------------------------------------------------------
// SectionPos
// ---------------------------------------------------------------------------

proptest! {
    /// Pack → unpack roundtrip for valid SectionPos ranges.
    ///
    /// X/Z: 22-bit signed → −2_097_152..=2_097_151
    /// Y:   20-bit signed → −524_288..=524_287
    #[test]
    fn proptest_section_pos_pack_unpack(
        x in -2_097_152_i32..=2_097_151,
        y in -524_288_i32..=524_287,
        z in -2_097_152_i32..=2_097_151,
    ) {
        let pos = SectionPos::new(x, y, z);
        let roundtripped = SectionPos::from_long(pos.as_long());
        prop_assert_eq!(roundtripped, pos);
    }
}

// ---------------------------------------------------------------------------
// Codec primitive roundtrips
// ---------------------------------------------------------------------------

proptest! {
    /// String write → read roundtrip (ASCII subset, max 100 chars).
    #[test]
    fn proptest_string_codec_roundtrip(s in "[a-zA-Z0-9 _]{0,100}") {
        let mut buf = BytesMut::new();
        types::write_string(&mut buf, &s);
        let mut data = buf.freeze();
        let decoded = types::read_string(&mut data, 100).unwrap();
        prop_assert_eq!(decoded, s);
    }

    #[test]
    fn proptest_i32_codec_roundtrip(value: i32) {
        let mut buf = BytesMut::new();
        types::write_i32(&mut buf, value);
        let mut data = buf.freeze();
        let decoded = types::read_i32(&mut data).unwrap();
        prop_assert_eq!(decoded, value);
    }

    #[test]
    fn proptest_i64_codec_roundtrip(value: i64) {
        let mut buf = BytesMut::new();
        types::write_i64(&mut buf, value);
        let mut data = buf.freeze();
        let decoded = types::read_i64(&mut data).unwrap();
        prop_assert_eq!(decoded, value);
    }

    #[test]
    fn proptest_u16_codec_roundtrip(value: u16) {
        let mut buf = BytesMut::new();
        types::write_u16(&mut buf, value);
        let mut data = buf.freeze();
        let decoded = types::read_u16(&mut data).unwrap();
        prop_assert_eq!(decoded, value);
    }

    #[test]
    fn proptest_bool_codec_roundtrip(value: bool) {
        let mut buf = BytesMut::new();
        types::write_bool(&mut buf, value);
        let mut data = buf.freeze();
        let decoded = types::read_bool(&mut data).unwrap();
        prop_assert_eq!(decoded, value);
    }

    /// UUID wire roundtrip via 16 random bytes.
    #[test]
    fn proptest_uuid_codec_roundtrip(raw in prop::array::uniform16(any::<u8>())) {
        let uuid = uuid::Uuid::from_bytes(raw);
        let mut buf = BytesMut::new();
        types::write_uuid(&mut buf, &uuid);
        let mut data = buf.freeze();
        let decoded = types::read_uuid(&mut data).unwrap();
        prop_assert_eq!(decoded, uuid);
    }
}
