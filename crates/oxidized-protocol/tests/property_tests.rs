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

// ---------------------------------------------------------------------------
// Phase 13 — Chunk packet roundtrips
// ---------------------------------------------------------------------------

use oxidized_protocol::packets::play::{
    ClientboundChunkBatchFinishedPacket, ClientboundForgetLevelChunkPacket,
    ClientboundSetChunkCacheCenterPacket, ClientboundSetChunkCacheRadiusPacket,
    ServerboundChunkBatchReceivedPacket,
};

proptest! {
    /// ForgetLevelChunk encode → decode roundtrip for any chunk coordinates.
    #[test]
    fn proptest_forget_level_chunk_roundtrip(x: i32, z: i32) {
        let pkt = ClientboundForgetLevelChunkPacket { chunk_x: x, chunk_z: z };
        let encoded = pkt.encode();
        let decoded = ClientboundForgetLevelChunkPacket::decode(encoded.freeze()).unwrap();
        prop_assert_eq!(decoded.chunk_x, x);
        prop_assert_eq!(decoded.chunk_z, z);
    }

    /// ChunkBatchFinished encode → decode roundtrip for any VarInt batch size.
    #[test]
    fn proptest_chunk_batch_finished_roundtrip(batch_size: i32) {
        let pkt = ClientboundChunkBatchFinishedPacket { batch_size };
        let encoded = pkt.encode();
        let decoded = ClientboundChunkBatchFinishedPacket::decode(encoded.freeze()).unwrap();
        prop_assert_eq!(decoded.batch_size, batch_size);
    }

    /// ChunkBatchReceived encode → decode roundtrip for finite f32 values.
    #[test]
    fn proptest_chunk_batch_received_roundtrip(
        rate in prop::num::f32::NORMAL | prop::num::f32::POSITIVE | prop::num::f32::NEGATIVE | prop::num::f32::ZERO
    ) {
        let pkt = ServerboundChunkBatchReceivedPacket { desired_chunks_per_tick: rate };
        let encoded = pkt.encode();
        let decoded = ServerboundChunkBatchReceivedPacket::decode(encoded.freeze()).unwrap();
        prop_assert_eq!(decoded.desired_chunks_per_tick.to_bits(), rate.to_bits());
    }

    /// SetChunkCacheCenter encode → decode roundtrip for any coordinates.
    #[test]
    fn proptest_set_chunk_cache_center_roundtrip(x: i32, z: i32) {
        let pkt = ClientboundSetChunkCacheCenterPacket { chunk_x: x, chunk_z: z };
        let encoded = pkt.encode();
        let decoded = ClientboundSetChunkCacheCenterPacket::decode(encoded.freeze()).unwrap();
        prop_assert_eq!(decoded.chunk_x, x);
        prop_assert_eq!(decoded.chunk_z, z);
    }

    /// SetChunkCacheRadius encode → decode roundtrip for any radius.
    #[test]
    fn proptest_set_chunk_cache_radius_roundtrip(radius: i32) {
        let pkt = ClientboundSetChunkCacheRadiusPacket { radius };
        let encoded = pkt.encode();
        let decoded = ClientboundSetChunkCacheRadiusPacket::decode(encoded.freeze()).unwrap();
        prop_assert_eq!(decoded.radius, radius);
    }

    /// LevelChunkWithLight: encoded bytes always start with chunk coordinates.
    #[test]
    fn proptest_level_chunk_with_light_coordinates(x: i32, z: i32) {
        use oxidized_protocol::packets::play::{
            ChunkPacketData, ClientboundLevelChunkWithLightPacket, LightUpdateData,
        };
        let pkt = ClientboundLevelChunkWithLightPacket {
            chunk_x: x,
            chunk_z: z,
            chunk_data: ChunkPacketData {
                heightmaps: vec![],
                buffer: vec![],
            },
            light_data: LightUpdateData::empty(),
        };
        let encoded = pkt.encode();
        let decoded_x = i32::from_be_bytes(encoded[0..4].try_into().unwrap());
        let decoded_z = i32::from_be_bytes(encoded[4..8].try_into().unwrap());
        prop_assert_eq!(decoded_x, x);
        prop_assert_eq!(decoded_z, z);
    }
}
