//! Integration tests for the `oxidized-game` crate.
//!
//! These tests exercise cross-module workflows: chunk serialization,
//! light data, player list management, login packet sequencing, and
//! view-distance chunk iteration.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashSet;

use oxidized_game::chunk::view_distance::{chunks_to_load, chunks_to_unload, spiral_chunks};
use oxidized_game::net::chunk_serializer::build_chunk_packet;
use oxidized_game::net::light_serializer::build_light_data;
use oxidized_game::player::game_mode::GameMode;
use oxidized_game::player::login::build_login_sequence;
use oxidized_game::player::player_list::PlayerList;
use oxidized_game::player::server_player::ServerPlayer;
use oxidized_nbt::NbtCompound;
use oxidized_protocol::auth::GameProfile;
use oxidized_protocol::packets::play::{
    ClientboundGameEventPacket, ClientboundLoginPacket, ClientboundPlayerAbilitiesPacket,
    ClientboundPlayerInfoUpdatePacket, ClientboundPlayerPositionPacket,
    ClientboundSetChunkCacheCenterPacket, ClientboundSetDefaultSpawnPositionPacket,
    ClientboundSetSimulationDistancePacket,
};
use oxidized_protocol::types::ResourceLocation;
use oxidized_world::chunk::level_chunk::ChunkPos;
use oxidized_world::chunk::{DataLayer, LevelChunk};
use oxidized_world::storage::PrimaryLevelData;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_player(id: i32, name: &str) -> ServerPlayer {
    let uuid = Uuid::new_v4();
    let profile = GameProfile::new(uuid, name.into());
    ServerPlayer::new(
        id,
        profile,
        ResourceLocation::minecraft("overworld"),
        GameMode::Survival,
    )
}

fn make_level_data() -> PrimaryLevelData {
    let mut nbt = NbtCompound::new();
    nbt.put_string("LevelName", "TestWorld");
    nbt.put_int("DataVersion", 4782);
    nbt.put_int("GameType", 0);
    nbt.put_int("SpawnX", 100);
    nbt.put_int("SpawnY", 64);
    nbt.put_int("SpawnZ", -200);
    nbt.put_float("SpawnAngle", 90.0);
    nbt.put_int("SeaLevel", 63);
    PrimaryLevelData::from_nbt(&nbt).unwrap()
}

// ---------------------------------------------------------------------------
// Chunk packet tests
// ---------------------------------------------------------------------------

#[test]
fn test_build_chunk_packet_empty_chunk() {
    let chunk = LevelChunk::new(ChunkPos::new(3, -7));
    let pkt = build_chunk_packet(&chunk);

    assert_eq!(pkt.chunk_x, 3);
    assert_eq!(pkt.chunk_z, -7);
    // 24 sections serialized even when all-air → buffer must be non-empty
    assert!(!pkt.chunk_data.buffer.is_empty());
    // No heightmaps explicitly set, so heightmap entries are empty
    // (heightmap_data being "non-empty" refers to the buffer, not the entries)
}

#[test]
fn test_build_chunk_packet_with_blocks() {
    let empty_chunk = LevelChunk::new(ChunkPos::new(0, 0));
    let empty_pkt = build_chunk_packet(&empty_chunk);
    let empty_len = empty_pkt.chunk_data.buffer.len();

    let mut chunk = LevelChunk::new(ChunkPos::new(0, 0));
    // Stone = state ID 1.  y=64 → section index (64+64)/16 = 8
    chunk.set_block_state(0, 64, 0, 1).unwrap();
    chunk.set_block_state(1, 64, 1, 1).unwrap();
    chunk.set_block_state(2, 64, 2, 1).unwrap();

    let pkt = build_chunk_packet(&chunk);
    // A section with actual blocks uses a palette and therefore more bytes.
    assert!(
        pkt.chunk_data.buffer.len() > empty_len,
        "section_data with blocks ({}) should be larger than all-air ({})",
        pkt.chunk_data.buffer.len(),
        empty_len
    );
}

// ---------------------------------------------------------------------------
// Light data tests
// ---------------------------------------------------------------------------

#[test]
fn test_build_light_data_empty() {
    let sky: Vec<Option<DataLayer>> = vec![None; 26];
    let block: Vec<Option<DataLayer>> = vec![None; 26];

    let data = build_light_data(&sky, &block);

    // No non-zero light → sky/block masks should be empty (zero)
    assert!(data.sky_y_mask.is_empty());
    assert!(data.block_y_mask.is_empty());
    assert!(data.sky_updates.is_empty());
    assert!(data.block_updates.is_empty());
    // But the empty masks should be set for all 26 sections
    assert!(!data.empty_sky_y_mask.is_empty());
    assert!(!data.empty_block_y_mask.is_empty());
}

#[test]
fn test_build_light_data_with_sky_light() {
    let mut sky: Vec<Option<DataLayer>> = vec![None; 26];
    // Place non-zero sky light at indices 1 and 5
    sky[1] = Some(DataLayer::filled(15));
    sky[5] = Some(DataLayer::filled(8));

    let block: Vec<Option<DataLayer>> = vec![None; 26];
    let data = build_light_data(&sky, &block);

    // sky_y_mask should have bits 1 and 5 set
    assert_eq!(data.sky_y_mask.len(), 1);
    let mask = data.sky_y_mask[0];
    assert_ne!(mask & (1 << 1), 0, "bit 1 should be set");
    assert_ne!(mask & (1 << 5), 0, "bit 5 should be set");
    // Two sky update arrays
    assert_eq!(data.sky_updates.len(), 2);
    // Each is 2048 bytes
    assert_eq!(data.sky_updates[0].len(), 2048);
    assert_eq!(data.sky_updates[1].len(), 2048);
}

// ---------------------------------------------------------------------------
// Player list tests
// ---------------------------------------------------------------------------

#[test]
fn test_player_list_add_remove() {
    let mut list = PlayerList::new(20);
    assert_eq!(list.player_count(), 0);

    let player = make_player(list.next_entity_id(), "Alice");
    let uuid = player.uuid;
    list.add(player);
    assert_eq!(list.player_count(), 1);

    list.remove(&uuid);
    assert_eq!(list.player_count(), 0);
}

#[test]
fn test_player_list_entity_id_increments() {
    let list = PlayerList::new(20);
    let id1 = list.next_entity_id();
    let id2 = list.next_entity_id();
    let id3 = list.next_entity_id();

    // Each call returns a unique, incrementing value
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
    assert_ne!(id1, id2);
    assert_ne!(id2, id3);
}

// ---------------------------------------------------------------------------
// Login sequence test
// ---------------------------------------------------------------------------

#[test]
fn test_build_login_sequence() {
    let player = make_player(42, "Steve");
    let level_data = make_level_data();
    let mut player_list = PlayerList::new(20);
    player_list.add(make_player(1, "Alice"));
    let dimensions = vec![
        ResourceLocation::minecraft("overworld"),
        ResourceLocation::minecraft("the_nether"),
        ResourceLocation::minecraft("the_end"),
    ];

    let packets = build_login_sequence(&player, 1, &level_data, &player_list, &dimensions, 0);

    // Exactly 8 packets
    assert_eq!(packets.len(), 8);

    // Verify packet IDs in the correct order
    assert_eq!(packets[0].id, ClientboundLoginPacket::PACKET_ID);
    assert_eq!(packets[1].id, ClientboundPlayerAbilitiesPacket::PACKET_ID);
    assert_eq!(
        packets[2].id,
        ClientboundSetDefaultSpawnPositionPacket::PACKET_ID
    );
    assert_eq!(packets[3].id, ClientboundGameEventPacket::PACKET_ID);
    assert_eq!(packets[4].id, ClientboundPlayerInfoUpdatePacket::PACKET_ID);
    assert_eq!(
        packets[5].id,
        ClientboundSetChunkCacheCenterPacket::PACKET_ID
    );
    assert_eq!(
        packets[6].id,
        ClientboundSetSimulationDistancePacket::PACKET_ID
    );
    assert_eq!(packets[7].id, ClientboundPlayerPositionPacket::PACKET_ID);
}

// ---------------------------------------------------------------------------
// Spiral chunks tests
// ---------------------------------------------------------------------------

#[test]
fn test_spiral_chunks_count() {
    let center = ChunkPos::new(0, 0);

    let r1: Vec<_> = spiral_chunks(center, 1).collect();
    assert_eq!(r1.len(), 9, "radius=1 should yield 3×3=9 chunks");

    let r2: Vec<_> = spiral_chunks(center, 2).collect();
    assert_eq!(r2.len(), 25, "radius=2 should yield 5×5=25 chunks");
}

#[test]
fn test_spiral_chunks_center_first() {
    let center = ChunkPos::new(10, -5);
    let chunks: Vec<_> = spiral_chunks(center, 3).collect();
    assert_eq!(chunks[0], center, "first chunk yielded must be the center");
}

// ---------------------------------------------------------------------------
// Chunks to load / unload tests
// ---------------------------------------------------------------------------

#[test]
fn test_chunks_to_load_unload_disjoint() {
    let old_center = ChunkPos::new(0, 0);
    let new_center = ChunkPos::new(1, 0);
    let radius = 2;

    let to_load = chunks_to_load(old_center, new_center, radius);
    let to_unload = chunks_to_unload(old_center, new_center, radius);

    let load_set: HashSet<_> = to_load.iter().collect();
    let unload_set: HashSet<_> = to_unload.iter().collect();

    // The two sets must be completely disjoint
    assert!(
        load_set.is_disjoint(&unload_set),
        "chunks_to_load and chunks_to_unload must not overlap"
    );

    // chunks_to_load should contain chunks in new view but not old view
    let old_view: HashSet<_> = spiral_chunks(old_center, radius).collect();
    let new_view: HashSet<_> = spiral_chunks(new_center, radius).collect();
    for pos in &to_load {
        assert!(new_view.contains(pos), "{pos:?} loaded but not in new view");
        assert!(
            !old_view.contains(pos),
            "{pos:?} loaded but was already in old view"
        );
    }

    // chunks_to_unload should contain chunks in old view but not new view
    for pos in &to_unload {
        assert!(
            old_view.contains(pos),
            "{pos:?} unloaded but was not in old view"
        );
        assert!(
            !new_view.contains(pos),
            "{pos:?} unloaded but is still in new view"
        );
    }
}

#[test]
fn test_chunks_to_load_no_movement() {
    let center = ChunkPos::new(5, 5);
    let radius = 2;

    let to_load = chunks_to_load(center, center, radius);
    let to_unload = chunks_to_unload(center, center, radius);

    assert!(to_load.is_empty(), "no movement → nothing to load");
    assert!(to_unload.is_empty(), "no movement → nothing to unload");
}

// ---------------------------------------------------------------------------
// Full chunk packet lifecycle tests
// ---------------------------------------------------------------------------

#[test]
fn test_full_chunk_packet_with_blocks_heightmaps_light() {
    use oxidized_world::chunk::heightmap::{Heightmap, HeightmapType};
    use oxidized_world::chunk::level_chunk::OVERWORLD_HEIGHT;

    let mut chunk = LevelChunk::new(ChunkPos::new(10, -20));

    // Place some blocks
    chunk.set_block_state(0, 64, 0, 1).unwrap(); // stone
    chunk.set_block_state(7, 65, 7, 2).unwrap(); // granite

    // Set client heightmaps
    let hm = Heightmap::new(HeightmapType::MotionBlocking, OVERWORLD_HEIGHT).unwrap();
    chunk.set_heightmap(hm);
    let hm2 = Heightmap::new(HeightmapType::WorldSurface, OVERWORLD_HEIGHT).unwrap();
    chunk.set_heightmap(hm2);

    let pkt = build_chunk_packet(&chunk);

    // Verify coordinates
    assert_eq!(pkt.chunk_x, 10);
    assert_eq!(pkt.chunk_z, -20);

    // Verify heightmaps present
    assert_eq!(pkt.chunk_data.heightmaps.len(), 2);

    // Verify section buffer is non-empty and larger than empty chunk
    let empty_len = build_chunk_packet(&LevelChunk::new(ChunkPos::new(0, 0)))
        .chunk_data
        .buffer
        .len();
    assert!(pkt.chunk_data.buffer.len() > empty_len);

    // Verify the packet encodes without panicking
    let encoded = pkt.encode();
    assert!(encoded.len() > 8);
    // Check coordinates in wire format
    assert_eq!(i32::from_be_bytes(encoded[0..4].try_into().unwrap()), 10);
    assert_eq!(i32::from_be_bytes(encoded[4..8].try_into().unwrap()), -20);
}

#[test]
fn test_build_light_data_block_light_only() {
    let sky: Vec<Option<DataLayer>> = vec![None; 26];
    let mut block: Vec<Option<DataLayer>> = vec![None; 26];
    block[10] = Some(DataLayer::filled(14));

    let data = build_light_data(&sky, &block);

    // Block light should have bit 10 set
    assert_eq!(data.block_y_mask.len(), 1);
    assert_ne!(data.block_y_mask[0] & (1 << 10), 0);
    assert_eq!(data.block_updates.len(), 1);
    assert_eq!(data.block_updates[0].len(), 2048);

    // Sky should be all empty
    assert!(data.sky_y_mask.is_empty());
    assert!(data.sky_updates.is_empty());
}

#[test]
fn test_build_light_data_mixed_sky_block() {
    let mut sky: Vec<Option<DataLayer>> = vec![None; 26];
    let mut block: Vec<Option<DataLayer>> = vec![None; 26];
    sky[0] = Some(DataLayer::filled(15));
    sky[3] = Some(DataLayer::filled(8));
    block[1] = Some(DataLayer::filled(12));
    block[3] = Some(DataLayer::filled(5));

    let data = build_light_data(&sky, &block);

    // Sky: bits 0 and 3
    assert_eq!(data.sky_updates.len(), 2);
    // Block: bits 1 and 3
    assert_eq!(data.block_updates.len(), 2);

    // Verify masks are disjoint with their empty counterparts
    if !data.sky_y_mask.is_empty() && !data.empty_sky_y_mask.is_empty() {
        assert_eq!(data.sky_y_mask[0] & data.empty_sky_y_mask[0], 0);
    }
    if !data.block_y_mask.is_empty() && !data.empty_block_y_mask.is_empty() {
        assert_eq!(data.block_y_mask[0] & data.empty_block_y_mask[0], 0);
    }
}

#[test]
fn test_chunk_tracker_large_jump() {
    use oxidized_game::chunk::chunk_tracker::PlayerChunkTracker;

    let mut tracker = PlayerChunkTracker::new(ChunkPos::new(0, 0), 2);
    assert_eq!(tracker.loaded_count(), 25);

    // Jump far away — no overlap
    let (to_load, to_unload) = tracker.update_center(ChunkPos::new(1000, 1000));
    assert_eq!(to_unload.len(), 25, "all old chunks should be unloaded");
    assert_eq!(to_load.len(), 25, "all new chunks should be loaded");
    assert_eq!(
        tracker.loaded_count(),
        25,
        "total loaded should be constant"
    );

    // Verify none of the old chunks are still loaded
    assert!(!tracker.is_loaded(&ChunkPos::new(0, 0)));
    // Verify new center is loaded
    assert!(tracker.is_loaded(&ChunkPos::new(1000, 1000)));
}

// ---------------------------------------------------------------------------
// Phase 14 — Movement validation integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_movement_validation_normal_walk() {
    use oxidized_game::player::movement::validate_movement;
    use oxidized_protocol::types::Vec3;

    // Simulate a normal walking step: 0.1 blocks forward
    let current = Vec3::new(100.0, 64.0, -50.0);
    let result = validate_movement(
        current,
        90.0,
        0.0,
        Some(100.1),
        Some(64.0),
        Some(-50.0),
        Some(91.0),
        Some(0.0),
    );
    assert!(result.accepted, "normal walk should be accepted");
    assert!(!result.needs_correction);
    assert!((result.new_pos.x - 100.1).abs() < f64::EPSILON);
    assert!((result.new_yaw - 91.0).abs() < f32::EPSILON);
}

#[test]
fn test_movement_validation_too_fast_triggers_correction() {
    use oxidized_game::player::movement::validate_movement;
    use oxidized_protocol::types::Vec3;

    // Attempt to teleport 200 blocks — must be rejected
    let result = validate_movement(
        Vec3::new(0.0, 64.0, 0.0),
        0.0,
        0.0,
        Some(200.0),
        Some(64.0),
        Some(0.0),
        None,
        None,
    );
    assert!(!result.accepted, "200-block jump must be rejected");
    assert!(result.needs_correction);
}

#[test]
fn test_movement_validation_preserves_unchanged_fields() {
    use oxidized_game::player::movement::validate_movement;
    use oxidized_protocol::types::Vec3;

    // Rotation-only update: position should not change
    let current = Vec3::new(42.0, 100.0, -7.5);
    let result = validate_movement(
        current,
        45.0,
        -10.0,
        None,
        None,
        None,
        Some(180.0),
        Some(30.0),
    );
    assert!(result.accepted);
    assert!((result.new_pos.x - 42.0).abs() < f64::EPSILON);
    assert!((result.new_pos.y - 100.0).abs() < f64::EPSILON);
    assert!((result.new_pos.z + 7.5).abs() < f64::EPSILON);
    assert!((result.new_yaw - 180.0).abs() < f32::EPSILON);
    assert!((result.new_pitch - 30.0).abs() < f32::EPSILON);
}

// ---------------------------------------------------------------------------
// Phase 14 — Entity movement encoding integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_entity_movement_small_delta_produces_delta_kind() {
    use oxidized_game::net::entity_movement::{classify_move, EntityMoveKind};

    // Moving 1 block on each axis — should use delta encoding
    let kind = classify_move(0.0, 64.0, 0.0, 1.0, 65.0, 1.0);
    match kind {
        EntityMoveKind::Delta { dx, dy, dz } => {
            assert_eq!(dx, 4096, "1 block = 4096 units");
            assert_eq!(dy, 4096);
            assert_eq!(dz, 4096);
        },
        _ => panic!("Expected Delta for 1-block move"),
    }
}

#[test]
fn test_entity_movement_large_teleport_produces_sync_kind() {
    use oxidized_game::net::entity_movement::{classify_move, EntityMoveKind};

    // Moving 100 blocks — must use full sync
    let kind = classify_move(0.0, 64.0, 0.0, 100.0, 64.0, 0.0);
    match kind {
        EntityMoveKind::Sync { x, y, z } => {
            assert!((x - 100.0).abs() < f64::EPSILON);
            assert!((y - 64.0).abs() < f64::EPSILON);
            assert!(z.abs() < f64::EPSILON);
        },
        _ => panic!("Expected Sync for 100-block teleport"),
    }
}

#[test]
fn test_entity_movement_boundary_at_eight_blocks() {
    use oxidized_game::net::entity_movement::{classify_move, EntityMoveKind};

    // 7.999 blocks — should fit delta
    let kind = classify_move(0.0, 0.0, 0.0, 7.999, 0.0, 0.0);
    assert!(
        matches!(kind, EntityMoveKind::Delta { .. }),
        "7.999 blocks should use delta"
    );

    // 8.001 blocks — should not fit
    let kind = classify_move(0.0, 0.0, 0.0, 8.001, 0.0, 0.0);
    assert!(
        matches!(kind, EntityMoveKind::Sync { .. }),
        "8.001 blocks should use sync"
    );
}

#[test]
fn test_degree_packing_known_angles() {
    use oxidized_game::net::entity_movement::{pack_degrees, unpack_degrees};

    // Verify known angles survive roundtrip within tolerance
    for &angle in &[0.0f32, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0] {
        let packed = pack_degrees(angle);
        let unpacked = unpack_degrees(packed);
        assert!(
            (unpacked - angle).abs() < 1.41,
            "angle {angle}° → packed {packed} → unpacked {unpacked}°"
        );
    }
}

// ---------------------------------------------------------------------------
// Phase 14 — Teleport confirmation integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_teleport_accept_correct_id() {
    use oxidized_game::player::login::handle_accept_teleportation;

    let uuid = uuid::Uuid::new_v4();
    let profile = GameProfile::new(uuid, "TeleportTest".into());
    let dim = oxidized_protocol::types::resource_location::ResourceLocation::minecraft("overworld");
    let mut player = ServerPlayer::new(1, profile, dim, GameMode::Survival);
    player.pending_teleports.push_back(42);
    player.pending_teleports.push_back(43);

    assert!(
        handle_accept_teleportation(&mut player, 42),
        "correct teleport ID should be accepted"
    );
    assert_eq!(player.pending_teleports.len(), 1);
    assert_eq!(player.pending_teleports[0], 43);
}

#[test]
fn test_teleport_accept_wrong_id_fails() {
    use oxidized_game::player::login::handle_accept_teleportation;

    let uuid = uuid::Uuid::new_v4();
    let profile = GameProfile::new(uuid, "TeleportTest2".into());
    let dim = oxidized_protocol::types::resource_location::ResourceLocation::minecraft("overworld");
    let mut player = ServerPlayer::new(2, profile, dim, GameMode::Survival);
    player.pending_teleports.push_back(10);

    assert!(
        !handle_accept_teleportation(&mut player, 99),
        "wrong teleport ID should be rejected"
    );
    assert_eq!(player.pending_teleports.len(), 1, "queue unchanged on wrong ID");
}

#[test]
fn test_teleport_accept_empty_queue_fails() {
    use oxidized_game::player::login::handle_accept_teleportation;

    let uuid = uuid::Uuid::new_v4();
    let profile = GameProfile::new(uuid, "TeleportTest3".into());
    let dim = oxidized_protocol::types::resource_location::ResourceLocation::minecraft("overworld");
    let mut player = ServerPlayer::new(3, profile, dim, GameMode::Survival);

    assert!(
        !handle_accept_teleportation(&mut player, 1),
        "empty queue should reject any ID"
    );
}
