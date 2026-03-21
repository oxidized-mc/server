//! Property-based tests for coordinate types and codec primitives.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use bytes::{Bytes, BytesMut};
use proptest::prelude::*;

use oxidized_protocol::codec::Packet;
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
// Chunk packet roundtrips
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

// ---------------------------------------------------------------------------
// Movement packet roundtrips
// ---------------------------------------------------------------------------

use oxidized_protocol::packets::play::{
    ClientboundEntityPositionSyncPacket, ClientboundMoveEntityPosPacket,
    ClientboundMoveEntityPosRotPacket, ClientboundMoveEntityRotPacket,
    ClientboundPlayerPositionPacket, ClientboundRotateHeadPacket, PlayerCommandAction, PlayerInput,
    RelativeFlags, ServerboundMovePlayerPacket, ServerboundPlayerCommandPacket,
    ServerboundPlayerInputPacket,
};

/// Builds raw bytes for `ServerboundMovePlayerPacket::decode_pos`.
fn build_move_pos_bytes(x: f64, y: f64, z: f64, flags: u8) -> Bytes {
    use bytes::BufMut;
    let mut buf = BytesMut::with_capacity(25);
    buf.put_f64(x);
    buf.put_f64(y);
    buf.put_f64(z);
    buf.put_u8(flags & 0x03);
    buf.freeze()
}

/// Builds raw bytes for `ServerboundMovePlayerPacket::decode_pos_rot`.
fn build_move_pos_rot_bytes(x: f64, y: f64, z: f64, yaw: f32, pitch: f32, flags: u8) -> Bytes {
    use bytes::BufMut;
    let mut buf = BytesMut::with_capacity(33);
    buf.put_f64(x);
    buf.put_f64(y);
    buf.put_f64(z);
    buf.put_f32(yaw);
    buf.put_f32(pitch);
    buf.put_u8(flags & 0x03);
    buf.freeze()
}

/// Builds raw bytes for `ServerboundMovePlayerPacket::decode_rot`.
fn build_move_rot_bytes(yaw: f32, pitch: f32, flags: u8) -> Bytes {
    use bytes::BufMut;
    let mut buf = BytesMut::with_capacity(9);
    buf.put_f32(yaw);
    buf.put_f32(pitch);
    buf.put_u8(flags & 0x03);
    buf.freeze()
}

/// Strategy for finite f64 values (no NaN/Inf).
fn finite_f64() -> impl Strategy<Value = f64> {
    prop::num::f64::NORMAL
        | prop::num::f64::POSITIVE
        | prop::num::f64::NEGATIVE
        | prop::num::f64::ZERO
}

/// Strategy for finite f32 values (no NaN/Inf).
fn finite_f32() -> impl Strategy<Value = f32> {
    prop::num::f32::NORMAL
        | prop::num::f32::POSITIVE
        | prop::num::f32::NEGATIVE
        | prop::num::f32::ZERO
}

proptest! {
    /// ServerboundMovePlayerPacket Pos variant: bytes → decode → verify fields.
    #[test]
    fn proptest_move_player_pos_roundtrip(
        x in finite_f64(), y in finite_f64(), z in finite_f64(),
        flags in 0u8..=3,
    ) {
        let data = build_move_pos_bytes(x, y, z, flags);
        let pkt = ServerboundMovePlayerPacket::decode_pos(data).unwrap();
        prop_assert_eq!(pkt.x.unwrap().to_bits(), x.to_bits());
        prop_assert_eq!(pkt.y.unwrap().to_bits(), y.to_bits());
        prop_assert_eq!(pkt.z.unwrap().to_bits(), z.to_bits());
        prop_assert!(pkt.yaw.is_none());
        prop_assert!(pkt.pitch.is_none());
        prop_assert_eq!(pkt.on_ground, flags & 0x01 != 0);
        prop_assert_eq!(pkt.horizontal_collision, flags & 0x02 != 0);
        prop_assert!(pkt.has_pos());
        prop_assert!(!pkt.has_rot());
    }

    /// ServerboundMovePlayerPacket PosRot variant: bytes → decode → verify fields.
    #[test]
    fn proptest_move_player_pos_rot_roundtrip(
        x in finite_f64(), y in finite_f64(), z in finite_f64(),
        yaw in finite_f32(), pitch in finite_f32(),
        flags in 0u8..=3,
    ) {
        let data = build_move_pos_rot_bytes(x, y, z, yaw, pitch, flags);
        let pkt = ServerboundMovePlayerPacket::decode_pos_rot(data).unwrap();
        prop_assert_eq!(pkt.x.unwrap().to_bits(), x.to_bits());
        prop_assert_eq!(pkt.y.unwrap().to_bits(), y.to_bits());
        prop_assert_eq!(pkt.z.unwrap().to_bits(), z.to_bits());
        prop_assert_eq!(pkt.yaw.unwrap().to_bits(), yaw.to_bits());
        prop_assert_eq!(pkt.pitch.unwrap().to_bits(), pitch.to_bits());
        prop_assert_eq!(pkt.on_ground, flags & 0x01 != 0);
        prop_assert_eq!(pkt.horizontal_collision, flags & 0x02 != 0);
        prop_assert!(pkt.has_pos());
        prop_assert!(pkt.has_rot());
    }

    /// ServerboundMovePlayerPacket Rot variant: bytes → decode → verify fields.
    #[test]
    fn proptest_move_player_rot_roundtrip(
        yaw in finite_f32(), pitch in finite_f32(),
        flags in 0u8..=3,
    ) {
        let data = build_move_rot_bytes(yaw, pitch, flags);
        let pkt = ServerboundMovePlayerPacket::decode_rot(data).unwrap();
        prop_assert!(pkt.x.is_none());
        prop_assert_eq!(pkt.yaw.unwrap().to_bits(), yaw.to_bits());
        prop_assert_eq!(pkt.pitch.unwrap().to_bits(), pitch.to_bits());
        prop_assert_eq!(pkt.on_ground, flags & 0x01 != 0);
    }

    /// ServerboundMovePlayerPacket StatusOnly variant: byte → decode → verify flags.
    #[test]
    fn proptest_move_player_status_only_roundtrip(flags in 0u8..=3) {
        let data = Bytes::from(vec![flags]);
        let pkt = ServerboundMovePlayerPacket::decode_status_only(data).unwrap();
        prop_assert!(pkt.x.is_none());
        prop_assert!(pkt.yaw.is_none());
        prop_assert_eq!(pkt.on_ground, flags & 0x01 != 0);
        prop_assert_eq!(pkt.horizontal_collision, flags & 0x02 != 0);
    }

    /// ServerboundPlayerCommandPacket encode → decode roundtrip.
    #[test]
    fn proptest_player_command_roundtrip(
        entity_id: i32,
        action_id in 0i32..=6,
        data_val: i32,
    ) {
        let action = PlayerCommandAction::from_id(action_id).unwrap();
        let pkt = ServerboundPlayerCommandPacket { entity_id, action, data: data_val };
        let encoded = pkt.encode();
        let decoded = ServerboundPlayerCommandPacket::decode(encoded.freeze()).unwrap();
        prop_assert_eq!(decoded.entity_id, entity_id);
        prop_assert_eq!(decoded.action, action);
        prop_assert_eq!(decoded.data, data_val);
    }

    /// PlayerInput byte → struct → byte identity.
    #[test]
    fn proptest_player_input_byte_roundtrip(flags in 0u8..=0x7F) {
        let input = PlayerInput::from_byte(flags);
        let roundtripped = input.to_byte();
        prop_assert_eq!(roundtripped, flags);
    }

    /// ServerboundPlayerInputPacket encode → decode roundtrip.
    #[test]
    fn proptest_player_input_packet_roundtrip(flags in 0u8..=0x7F) {
        let pkt = ServerboundPlayerInputPacket {
            input: PlayerInput::from_byte(flags),
        };
        let encoded = pkt.encode();
        let decoded = ServerboundPlayerInputPacket::decode(encoded.freeze()).unwrap();
        prop_assert_eq!(decoded.input, pkt.input);
    }

    /// ClientboundPlayerPositionPacket encode → decode roundtrip.
    #[test]
    fn proptest_player_position_roundtrip(
        teleport_id: i32,
        x in finite_f64(), y in finite_f64(), z in finite_f64(),
        dx in finite_f64(), dy in finite_f64(), dz in finite_f64(),
        yaw in finite_f32(), pitch in finite_f32(),
        flags in 0i32..=0x1FF,
    ) {
        let pkt = ClientboundPlayerPositionPacket {
            teleport_id, x, y, z, dx, dy, dz, yaw, pitch,
            relative_flags: RelativeFlags(flags),
        };
        let encoded = pkt.encode();
        let decoded = ClientboundPlayerPositionPacket::decode(encoded.freeze()).unwrap();
        prop_assert_eq!(decoded.teleport_id, teleport_id);
        prop_assert_eq!(decoded.x.to_bits(), x.to_bits());
        prop_assert_eq!(decoded.y.to_bits(), y.to_bits());
        prop_assert_eq!(decoded.z.to_bits(), z.to_bits());
        prop_assert_eq!(decoded.dx.to_bits(), dx.to_bits());
        prop_assert_eq!(decoded.dy.to_bits(), dy.to_bits());
        prop_assert_eq!(decoded.dz.to_bits(), dz.to_bits());
        prop_assert_eq!(decoded.yaw.to_bits(), yaw.to_bits());
        prop_assert_eq!(decoded.pitch.to_bits(), pitch.to_bits());
        prop_assert_eq!(decoded.relative_flags, RelativeFlags(flags));
    }

    /// ClientboundMoveEntityPosPacket encode → decode roundtrip.
    #[test]
    fn proptest_move_entity_pos_roundtrip(
        entity_id: i32, dx: i16, dy: i16, dz: i16, on_ground: bool,
    ) {
        let pkt = ClientboundMoveEntityPosPacket { entity_id, dx, dy, dz, on_ground };
        let encoded = pkt.encode();
        let decoded = ClientboundMoveEntityPosPacket::decode(encoded.freeze()).unwrap();
        prop_assert_eq!(decoded, pkt);
    }

    /// ClientboundMoveEntityPosRotPacket encode → decode roundtrip.
    #[test]
    fn proptest_move_entity_pos_rot_roundtrip(
        entity_id: i32, dx: i16, dy: i16, dz: i16,
        yaw: u8, pitch: u8, on_ground: bool,
    ) {
        let pkt = ClientboundMoveEntityPosRotPacket {
            entity_id, dx, dy, dz, yaw, pitch, on_ground,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundMoveEntityPosRotPacket::decode(encoded.freeze()).unwrap();
        prop_assert_eq!(decoded, pkt);
    }

    /// ClientboundMoveEntityRotPacket encode → decode roundtrip.
    #[test]
    fn proptest_move_entity_rot_roundtrip(
        entity_id: i32, yaw: u8, pitch: u8, on_ground: bool,
    ) {
        let pkt = ClientboundMoveEntityRotPacket { entity_id, yaw, pitch, on_ground };
        let encoded = pkt.encode();
        let decoded = ClientboundMoveEntityRotPacket::decode(encoded.freeze()).unwrap();
        prop_assert_eq!(decoded, pkt);
    }

    /// ClientboundEntityPositionSyncPacket encode → decode roundtrip.
    #[test]
    fn proptest_entity_position_sync_roundtrip(
        entity_id: i32,
        x in finite_f64(), y in finite_f64(), z in finite_f64(),
        vx in finite_f64(), vy in finite_f64(), vz in finite_f64(),
        yaw in finite_f32(), pitch in finite_f32(),
        on_ground: bool,
    ) {
        let pkt = ClientboundEntityPositionSyncPacket {
            entity_id, x, y, z, vx, vy, vz, yaw, pitch, on_ground,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundEntityPositionSyncPacket::decode(encoded.freeze()).unwrap();
        prop_assert_eq!(decoded.entity_id, entity_id);
        prop_assert_eq!(decoded.x.to_bits(), x.to_bits());
        prop_assert_eq!(decoded.y.to_bits(), y.to_bits());
        prop_assert_eq!(decoded.z.to_bits(), z.to_bits());
        prop_assert_eq!(decoded.vx.to_bits(), vx.to_bits());
        prop_assert_eq!(decoded.vy.to_bits(), vy.to_bits());
        prop_assert_eq!(decoded.vz.to_bits(), vz.to_bits());
        prop_assert_eq!(decoded.yaw.to_bits(), yaw.to_bits());
        prop_assert_eq!(decoded.pitch.to_bits(), pitch.to_bits());
        prop_assert_eq!(decoded.on_ground, on_ground);
    }

    /// ClientboundRotateHeadPacket encode → decode roundtrip.
    #[test]
    fn proptest_rotate_head_roundtrip(entity_id: i32, head_yaw: u8) {
        let pkt = ClientboundRotateHeadPacket { entity_id, head_yaw };
        let encoded = pkt.encode();
        let decoded = ClientboundRotateHeadPacket::decode(encoded.freeze()).unwrap();
        prop_assert_eq!(decoded, pkt);
    }

    /// RelativeFlags: all 9 bits preserve through i32 roundtrip.
    #[test]
    fn proptest_relative_flags_bits(flags in 0i32..=0x1FF) {
        let rf = RelativeFlags(flags);
        for bit in 0..9 {
            prop_assert_eq!(rf.contains(1 << bit), flags & (1 << bit) != 0);
        }
    }
}

// ---------------------------------------------------------------------------
// Entity packet roundtrips
// ---------------------------------------------------------------------------

use oxidized_protocol::packets::play::{
    ClientboundAddEntityPacket, ClientboundRemoveEntitiesPacket, ClientboundSetEntityDataPacket,
};

proptest! {
    /// ClientboundAddEntityPacket encode → decode roundtrip (zero velocity).
    #[test]
    fn proptest_add_entity_roundtrip(
        entity_id: i32,
        entity_type: i32,
        x in finite_f64(), y in finite_f64(), z in finite_f64(),
        x_rot: u8, y_rot: u8, y_head_rot: u8,
        data_val: i32,
    ) {
        let uuid = uuid::Uuid::new_v4();
        let pkt = ClientboundAddEntityPacket {
            entity_id, uuid, entity_type,
            x, y, z,
            vx: 0.0, vy: 0.0, vz: 0.0,
            x_rot, y_rot, y_head_rot,
            data: data_val,
        };
        let buf = pkt.encode();
        let decoded = ClientboundAddEntityPacket::decode(buf.freeze()).unwrap();
        prop_assert_eq!(decoded.entity_id, entity_id);
        prop_assert_eq!(decoded.uuid, uuid);
        prop_assert_eq!(decoded.entity_type, entity_type);
        prop_assert_eq!(decoded.x.to_bits(), x.to_bits());
        prop_assert_eq!(decoded.y.to_bits(), y.to_bits());
        prop_assert_eq!(decoded.z.to_bits(), z.to_bits());
        prop_assert_eq!(decoded.x_rot, x_rot);
        prop_assert_eq!(decoded.y_rot, y_rot);
        prop_assert_eq!(decoded.y_head_rot, y_head_rot);
        prop_assert_eq!(decoded.data, data_val);
    }

    /// ClientboundRemoveEntitiesPacket encode → decode roundtrip.
    #[test]
    fn proptest_remove_entities_roundtrip(
        ids in prop::collection::vec(any::<i32>(), 0..20),
    ) {
        let pkt = ClientboundRemoveEntitiesPacket { entity_ids: ids.clone() };
        let buf = pkt.encode();
        let decoded = ClientboundRemoveEntitiesPacket::decode(buf.freeze()).unwrap();
        prop_assert_eq!(decoded.entity_ids, ids);
    }

    /// ClientboundSetEntityDataPacket single-byte entry encode → decode.
    #[test]
    fn proptest_set_entity_data_single_byte_roundtrip(
        entity_id: i32,
        slot in 0u8..=254,
        value: u8,
    ) {
        let pkt = ClientboundSetEntityDataPacket::single_byte(entity_id, slot, value);
        let buf = pkt.encode();
        let decoded = ClientboundSetEntityDataPacket::decode(buf.freeze()).unwrap();
        prop_assert_eq!(decoded.entity_id, entity_id);
        prop_assert_eq!(decoded.entries.len(), 1);
        prop_assert_eq!(decoded.entries[0].slot, slot);
        prop_assert_eq!(decoded.entries[0].serializer_type, 0); // Byte
        prop_assert_eq!(&decoded.entries[0].value_bytes, &vec![value]);
    }
}

// ---------------------------------------------------------------------------
// LpVec3 property tests
// ---------------------------------------------------------------------------

use oxidized_protocol::codec::lp_vec3;

proptest! {
    /// LpVec3 zero vector always encodes to a single byte.
    #[test]
    fn proptest_lpvec3_zero_is_single_byte(
        x in -1e-5f64..1e-5,
        y in -1e-5f64..1e-5,
        z in -1e-5f64..1e-5,
    ) {
        let mut buf = BytesMut::new();
        lp_vec3::write(&mut buf, x, y, z);
        prop_assert_eq!(buf.len(), 1, "near-zero vector should be 1 byte");
        prop_assert_eq!(buf[0], 0);
    }

    /// LpVec3 roundtrip: decoded values are within reasonable tolerance
    /// of the originals for typical game velocities.
    #[test]
    fn proptest_lpvec3_roundtrip_typical(
        x in -10.0f64..10.0,
        y in -10.0f64..10.0,
        z in -10.0f64..10.0,
    ) {
        let mut buf = BytesMut::new();
        lp_vec3::write(&mut buf, x, y, z);
        let mut data = buf.freeze();
        let (rx, ry, rz) = lp_vec3::read(&mut data).unwrap();

        let max_comp = x.abs().max(y.abs()).max(z.abs());
        if max_comp < 3.1e-5 {
            // Near-zero → decoded as exact zero
            prop_assert_eq!(rx, 0.0);
            prop_assert_eq!(ry, 0.0);
            prop_assert_eq!(rz, 0.0);
        } else {
            // Tolerance: proportional to scale (LpVec3 is 15-bit)
            let tol = max_comp * 0.01 + 0.001;
            prop_assert!((rx - x).abs() < tol,
                "x: {x} → {rx}, tol={tol}");
            prop_assert!((ry - y).abs() < tol,
                "y: {y} → {ry}, tol={tol}");
            prop_assert!((rz - z).abs() < tol,
                "z: {z} → {rz}, tol={tol}");
        }
    }

    /// LpVec3 NaN input is sanitized (decoded as zero vector).
    #[test]
    fn proptest_lpvec3_nan_sanitized(y in finite_f64(), z in finite_f64()) {
        let mut buf = BytesMut::new();
        lp_vec3::write(&mut buf, f64::NAN, y, z);
        let mut data = buf.freeze();
        let result = lp_vec3::read(&mut data);
        prop_assert!(result.is_ok(), "NaN-sanitized encode should decode successfully");
    }
}
