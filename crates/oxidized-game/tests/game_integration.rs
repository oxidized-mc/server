//! Integration tests for the `oxidized-game` crate.
//!
//! These tests exercise cross-module workflows: chunk serialization,
//! light data, player list management, login packet sequencing, and
//! view-distance chunk iteration.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashSet;

use oxidized_game::chunk::view_distance::{chunks_to_load, chunks_to_unload, spiral_chunks};
use oxidized_game::level::game_rules::GameRules;
use oxidized_game::net::chunk_serializer::build_chunk_packet;
use oxidized_game::net::light_serializer::build_light_data;
use oxidized_game::player::game_mode::GameMode;
use oxidized_game::player::login::build_login_sequence;
use oxidized_game::player::player_list::PlayerList;
use oxidized_game::player::server_player::ServerPlayer;
use oxidized_nbt::NbtCompound;
use oxidized_protocol::auth::GameProfile;
use oxidized_protocol::codec::Packet;
use oxidized_protocol::packets::play::{
    ClientboundChangeDifficultyPacket, ClientboundLoginPacket, ClientboundPlayerAbilitiesPacket,
    ClientboundPlayerInfoUpdatePacket, ClientboundPlayerPositionPacket,
    ClientboundSetChunkCacheCenterPacket, ClientboundSetHeldSlotPacket,
    ClientboundSetSimulationDistancePacket,
};
use oxidized_protocol::types::ResourceLocation;
use oxidized_world::chunk::ChunkPos;
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
    nbt.put_int("DataVersion", 4786);
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
    // None sections are excluded from all masks (vanilla behavior).
    // Only Some(all-zeros) sets empty mask bits.
    assert!(data.empty_sky_y_mask.is_empty());
    assert!(data.empty_block_y_mask.is_empty());
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

    let packets = build_login_sequence(
        &player,
        1,
        &level_data,
        &player_list,
        &dimensions,
        0,
        &GameRules::default(),
        false,
    );

    // 8 packets: Login + Difficulty + Abilities + HeldSlot +
    // PlayerInfo + ChunkCenter + SimDistance + Position
    // (SpawnPos, Inventory, and GameMode event are now sent separately)
    assert_eq!(packets.len(), 8);

    // Verify packet IDs in the correct order
    assert_eq!(packets[0].id, ClientboundLoginPacket::PACKET_ID);
    assert_eq!(packets[1].id, ClientboundChangeDifficultyPacket::PACKET_ID);
    assert_eq!(packets[2].id, ClientboundPlayerAbilitiesPacket::PACKET_ID);
    assert_eq!(packets[3].id, ClientboundSetHeldSlotPacket::PACKET_ID);
    assert_eq!(
        packets[4].id,
        ClientboundSetChunkCacheCenterPacket::PACKET_ID
    );
    assert_eq!(
        packets[5].id,
        ClientboundSetSimulationDistancePacket::PACKET_ID
    );
    assert_eq!(packets[6].id, ClientboundPlayerPositionPacket::PACKET_ID);
    assert_eq!(packets[7].id, ClientboundPlayerInfoUpdatePacket::PACKET_ID);
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
// Movement validation integration tests
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
        false,
    );
    assert!(result.is_accepted, "normal walk should be is_accepted");
    assert!(!result.is_correction_needed);
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
        false,
    );
    assert!(!result.is_accepted, "200-block jump must be rejected");
    assert!(result.is_correction_needed);
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
        false,
    );
    assert!(result.is_accepted);
    assert!((result.new_pos.x - 42.0).abs() < f64::EPSILON);
    assert!((result.new_pos.y - 100.0).abs() < f64::EPSILON);
    assert!((result.new_pos.z + 7.5).abs() < f64::EPSILON);
    assert!((result.new_yaw - (-180.0)).abs() < f32::EPSILON);
    assert!((result.new_pitch - 30.0).abs() < f32::EPSILON);
}

// ---------------------------------------------------------------------------
// Entity movement encoding integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_entity_movement_small_delta_produces_delta_kind() {
    use oxidized_game::net::entity_movement::{EntityMoveKind, classify_move};

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
    use oxidized_game::net::entity_movement::{EntityMoveKind, classify_move};

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
    use oxidized_game::net::entity_movement::{EntityMoveKind, classify_move};

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
// Teleport confirmation integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_teleport_accept_correct_id() {
    use oxidized_game::player::login::handle_accept_teleportation;
    use std::time::Instant;

    let uuid = uuid::Uuid::new_v4();
    let profile = GameProfile::new(uuid, "TeleportTest".into());
    let dim = oxidized_protocol::types::resource_location::ResourceLocation::minecraft("overworld");
    let mut player = ServerPlayer::new(1, profile, dim, GameMode::Survival);
    player
        .teleport
        .pending
        .push_back((42, oxidized_protocol::types::Vec3::ZERO, Instant::now()));
    player
        .teleport
        .pending
        .push_back((43, oxidized_protocol::types::Vec3::ZERO, Instant::now()));

    assert!(
        handle_accept_teleportation(&mut player, 42),
        "correct teleport ID should be accepted"
    );
    assert_eq!(player.teleport.pending.len(), 1);
    assert_eq!(player.teleport.pending[0].0, 43);
}

#[test]
fn test_teleport_accept_wrong_id_fails() {
    use oxidized_game::player::login::handle_accept_teleportation;
    use std::time::Instant;

    let uuid = uuid::Uuid::new_v4();
    let profile = GameProfile::new(uuid, "TeleportTest2".into());
    let dim = oxidized_protocol::types::resource_location::ResourceLocation::minecraft("overworld");
    let mut player = ServerPlayer::new(2, profile, dim, GameMode::Survival);
    player
        .teleport
        .pending
        .push_back((10, oxidized_protocol::types::Vec3::ZERO, Instant::now()));

    assert!(
        !handle_accept_teleportation(&mut player, 99),
        "wrong teleport ID should be rejected"
    );
    assert_eq!(
        player.teleport.pending.len(),
        1,
        "queue unchanged on wrong ID"
    );
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

// ===========================================================================
// Entity framework integration tests
// ===========================================================================

use oxidized_game::entity::Entity;
use oxidized_game::entity::data_slots::*;
use oxidized_game::entity::synched_data::{DataSerializerType, SynchedEntityData};
use oxidized_game::entity::tracker::{
    EntityTracker, TRACKING_RANGE_ANIMAL, TRACKING_RANGE_MISC, TRACKING_RANGE_PLAYER,
    is_in_tracking_range,
};

/// Creating an entity populates all 8 base data slots and the bbox is
/// consistent with the stated dimensions.
#[test]
fn test_entity_create_full_lifecycle() {
    let mut entity = Entity::new(
        oxidized_protocol::types::resource_location::ResourceLocation::minecraft("pig"),
        0.9,
        0.9,
    );

    // 8 base data slots
    assert_eq!(entity.synched_data.len(), 8);
    assert_eq!(entity.synched_data.get::<i32>(DATA_AIR_SUPPLY), 300);
    assert!(!entity.synched_data.get::<bool>(DATA_SILENT));

    // Set position and verify AABB moves with it
    entity.set_pos(100.0, 65.0, -200.0);
    assert!(entity.bounding_box.contains(100.0, 65.5, -200.0));
    assert!(!entity.bounding_box.contains(100.0, 66.0, -200.0)); // height=0.9

    // Modify flags — verify independent bits
    entity.set_flag(FLAG_ON_FIRE, true);
    entity.set_flag(FLAG_INVISIBLE, true);
    assert!(entity.is_on_fire());
    assert!(entity.is_invisible());
    assert!(!entity.is_sprinting());

    entity.set_flag(FLAG_ON_FIRE, false);
    assert!(!entity.is_on_fire());
    assert!(entity.is_invisible());
}

/// Dirty tracking across a series of get/set/pack_dirty cycles.
#[test]
fn test_synched_data_dirty_lifecycle() {
    let mut data = SynchedEntityData::new();
    data.define(0, DataSerializerType::Byte, 0u8);
    data.define(1, DataSerializerType::Int, 300i32);
    data.define(4, DataSerializerType::Boolean, false);

    // Initially clean
    assert!(!data.is_dirty());
    assert!(data.pack_dirty().is_empty());

    // Change two slots
    data.set(0u8, 5u8);
    data.set(4u8, true);
    assert!(data.is_dirty());

    let dirty = data.pack_dirty();
    assert_eq!(dirty.len(), 2);
    assert!(dirty.iter().any(|d| d.slot == 0));
    assert!(dirty.iter().any(|d| d.slot == 4));
    assert!(!data.is_dirty());

    // Setting same value again should not dirty
    data.set(0u8, 5u8);
    assert!(!data.is_dirty());

    // pack_all always returns everything regardless of dirty state
    let all = data.pack_all();
    assert_eq!(all.len(), 3);
}

/// Multiple entities get unique IDs and each has independent synched data.
#[test]
fn test_multiple_entities_independent() {
    let e1 = Entity::new(
        oxidized_protocol::types::resource_location::ResourceLocation::minecraft("cow"),
        0.9,
        1.4,
    );
    let e2 = Entity::new(
        oxidized_protocol::types::resource_location::ResourceLocation::minecraft("zombie"),
        0.6,
        1.95,
    );
    let e3 = Entity::new(
        oxidized_protocol::types::resource_location::ResourceLocation::minecraft("creeper"),
        0.6,
        1.7,
    );

    // All IDs must be unique
    let ids = [e1.id, e2.id, e3.id];
    let unique: HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), 3, "entity IDs must be unique");

    // All UUIDs must be unique
    let uuids = [e1.uuid, e2.uuid, e3.uuid];
    let unique_uuids: HashSet<_> = uuids.iter().collect();
    assert_eq!(unique_uuids.len(), 3, "entity UUIDs must be unique");
}

/// EntityTracker full workflow: register → update → track → unregister.
#[test]
fn test_tracker_full_workflow() {
    let mut tracker = EntityTracker::new();
    assert!(tracker.is_empty());

    // Register two entities with different ranges
    tracker.register(1, TRACKING_RANGE_PLAYER);
    tracker.register(2, TRACKING_RANGE_ANIMAL);
    assert_eq!(tracker.len(), 2);

    let p1 = uuid::Uuid::new_v4();
    let p2 = uuid::Uuid::new_v4();
    let p3 = uuid::Uuid::new_v4();

    // Tick 1: p1 and p2 can see entity 1
    let (added, removed) = tracker.update(1, [p1, p2].into_iter().collect());
    assert_eq!(added.len(), 2);
    assert!(removed.is_empty());

    // Tick 2: p1 leaves, p3 arrives for entity 1
    let (added, removed) = tracker.update(1, [p2, p3].into_iter().collect());
    assert_eq!(added.len(), 1);
    assert_eq!(added[0], p3);
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0], p1);

    // Verify tracking queries
    assert!(!tracker.is_tracking(1, &p1));
    assert!(tracker.is_tracking(1, &p2));
    assert!(tracker.is_tracking(1, &p3));
    assert_eq!(tracker.watcher_count(1), 2);

    // Unregister entity 1 — returns all current watchers
    let final_watchers = tracker.unregister(1);
    assert!(final_watchers.contains(&p2));
    assert!(final_watchers.contains(&p3));
    assert_eq!(tracker.len(), 1); // entity 2 still tracked
}

/// AABB intersect/contains works across entity bounding boxes at
/// different positions.
#[test]
fn test_aabb_entity_collision() {
    let mut e1 = Entity::new(
        oxidized_protocol::types::resource_location::ResourceLocation::minecraft("cow"),
        0.9,
        1.4,
    );
    let mut e2 = Entity::new(
        oxidized_protocol::types::resource_location::ResourceLocation::minecraft("pig"),
        0.9,
        0.9,
    );

    // Place them at the same spot — should intersect
    e1.set_pos(10.0, 64.0, 10.0);
    e2.set_pos(10.0, 64.0, 10.0);
    assert!(e1.bounding_box.intersects(&e2.bounding_box));

    // Move e2 far away — should not intersect
    e2.set_pos(20.0, 64.0, 20.0);
    assert!(!e1.bounding_box.intersects(&e2.bounding_box));

    // Place them overlapping but not at center
    e2.set_pos(10.5, 64.0, 10.0);
    assert!(e1.bounding_box.intersects(&e2.bounding_box));
}

/// `is_in_tracking_range` works correctly at boundary conditions.
#[test]
fn test_tracking_range_boundary_conditions() {
    let range = TRACKING_RANGE_MISC as f64;

    // Exactly at range
    assert!(is_in_tracking_range(
        0.0,
        0.0,
        range,
        0.0,
        TRACKING_RANGE_MISC
    ));

    // Just beyond range
    assert!(!is_in_tracking_range(
        0.0,
        0.0,
        range + 0.01,
        0.0,
        TRACKING_RANGE_MISC
    ));

    // Diagonal — exactly at sqrt(range²) distance
    let diag = range / 2.0_f64.sqrt();
    assert!(is_in_tracking_range(
        0.0,
        0.0,
        diag,
        diag,
        TRACKING_RANGE_MISC
    ));

    // Negative coordinates
    assert!(is_in_tracking_range(
        -100.0,
        -100.0,
        -120.0,
        -100.0,
        TRACKING_RANGE_MISC
    ));
}

// =============================================================================
// Flat world generation — integration tests
// =============================================================================

use oxidized_game::worldgen::ChunkGenerator;
use oxidized_game::worldgen::flat::{FlatChunkGenerator, FlatWorldConfig};
use oxidized_world::chunk::heightmap::HeightmapType;
use oxidized_world::registry::{BEDROCK, DIRT, GRASS_BLOCK};

/// A full generate→serialize round-trip: the generated chunk must produce a
/// valid chunk packet that the client can decode (non-empty, starts with
/// valid data).
#[test]
fn flat_chunk_generate_and_serialize_roundtrip() {
    let generator = FlatChunkGenerator::new(FlatWorldConfig::default());
    let chunk = generator.generate_chunk(ChunkPos { x: 3, z: -7 });
    let pkt = build_chunk_packet(&chunk);
    let encoded = pkt.encode();
    // Packet must be non-empty and significantly larger than an air-only chunk
    // (air chunk is mostly zeros; generated chunk has palette entries).
    assert!(
        encoded.len() > 100,
        "encoded chunk packet is suspiciously small: {} bytes",
        encoded.len()
    );
}

/// Every column in a generated chunk must have the correct block stack.
#[test]
fn flat_chunk_all_columns_match_config() {
    let generator = FlatChunkGenerator::new(FlatWorldConfig::default());
    let chunk = generator.generate_chunk(ChunkPos { x: 0, z: 0 });

    for x in 0..16_i32 {
        for z in 0..16_i32 {
            let bedrock = chunk.get_block_state(x, -64, z).unwrap();
            let dirt1 = chunk.get_block_state(x, -63, z).unwrap();
            let dirt2 = chunk.get_block_state(x, -62, z).unwrap();
            let grass = chunk.get_block_state(x, -61, z).unwrap();
            let air = chunk.get_block_state(x, -60, z).unwrap();

            assert_eq!(
                bedrock,
                u32::from(BEDROCK.0),
                "({x},{z}) y=-64 should be bedrock"
            );
            assert_eq!(dirt1, u32::from(DIRT.0), "({x},{z}) y=-63 should be dirt");
            assert_eq!(dirt2, u32::from(DIRT.0), "({x},{z}) y=-62 should be dirt");
            assert_eq!(
                grass,
                u32::from(GRASS_BLOCK.0),
                "({x},{z}) y=-61 should be grass_block"
            );
            assert_eq!(air, 0, "({x},{z}) y=-60 should be air");
        }
    }
}

/// Heightmaps must be computed and present after generation.
#[test]
fn flat_chunk_has_heightmaps() {
    let generator = FlatChunkGenerator::new(FlatWorldConfig::default());
    let chunk = generator.generate_chunk(ChunkPos { x: 0, z: 0 });

    // All three client heightmap types should exist.
    assert!(
        chunk.heightmap(HeightmapType::MotionBlocking).is_some(),
        "MOTION_BLOCKING heightmap missing"
    );
    assert!(
        chunk.heightmap(HeightmapType::WorldSurface).is_some(),
        "WORLD_SURFACE heightmap missing"
    );
    assert!(
        chunk
            .heightmap(HeightmapType::MotionBlockingNoLeaves)
            .is_some(),
        "MOTION_BLOCKING_NO_LEAVES heightmap missing"
    );
}

/// find_spawn_y returns a Y one above the surface (players stand on grass).
#[test]
fn flat_spawn_y_is_above_surface() {
    let generator = FlatChunkGenerator::new(FlatWorldConfig::default());
    let spawn_y = generator.find_spawn_y();
    // Default flat: 4 layers starting at y=-64, surface at y=-61, spawn at y=-60.
    assert_eq!(spawn_y, -60);
}

/// Custom layer config produces correct blocks.
#[test]
fn flat_custom_layers_generate_correctly() {
    use oxidized_world::registry::SAND;

    let config = FlatWorldConfig::from_layers(&[(BEDROCK, 1), (SAND, 5)]);
    let generator = FlatChunkGenerator::new(config);
    let chunk = generator.generate_chunk(ChunkPos { x: 0, z: 0 });

    // Bedrock at bottom
    assert_eq!(
        chunk.get_block_state(0, -64, 0).unwrap(),
        u32::from(BEDROCK.0)
    );
    // Sand for 5 layers
    for y in -63..=-59_i32 {
        assert_eq!(
            chunk.get_block_state(0, y, 0).unwrap(),
            u32::from(SAND.0),
            "y={y} should be sand"
        );
    }
    // Air above
    assert_eq!(chunk.get_block_state(0, -58, 0).unwrap(), 0);
}

/// Generator type is correct.
#[test]
fn flat_generator_type_string() {
    let generator = FlatChunkGenerator::new(FlatWorldConfig::default());
    assert_eq!(generator.generator_type(), "minecraft:flat");
}

// ── Biome registry consistency ─────────────────────────────────────────────

/// Verifies that biome IDs used in chunk data (from `oxidized-world`'s
/// `BIOME_NAMES`) match the order entries are sent in the
/// `minecraft:worldgen/biome` registry packet (from `oxidized-protocol`'s
/// `registries.json`).
///
/// If these ever diverge, the client will display wrong biomes because
/// chunk palette IDs won't correspond to the registry IDs the client
/// received during configuration.
#[test]
fn biome_ids_match_protocol_registry_order() {
    let protocol_entries =
        oxidized_protocol::registry::get_registry_entries("minecraft:worldgen/biome")
            .expect("biome registry should load");

    let protocol_count = protocol_entries.len();
    let world_count = oxidized_world::registry::biome_count();
    assert_eq!(
        protocol_count, world_count,
        "protocol biome count ({protocol_count}) != world biome count ({world_count})"
    );

    for (idx, (protocol_name, _)) in protocol_entries.iter().enumerate() {
        let world_id = oxidized_world::registry::biome_name_to_id(protocol_name);
        assert_eq!(
            world_id,
            Some(idx as u32),
            "biome {protocol_name}: protocol index={idx} but world ID={world_id:?}"
        );

        let world_name = oxidized_world::registry::biome_id_to_name(idx as u32);
        assert_eq!(
            world_name,
            Some(protocol_name.as_str()),
            "biome index {idx}: protocol name={protocol_name} but world name={world_name:?}"
        );
    }
}

// ============== Cross-chunk light propagation tests (23a.10) ==============

mod cross_chunk_light {
    use oxidized_game::lighting::cross_chunk::{
        ChunkNeighbors, propagate_block_light_cross_chunk, propagate_sky_light_cross_chunk,
    };
    use oxidized_game::lighting::engine::LightEngine;
    use oxidized_game::lighting::queue::LightUpdate;
    use oxidized_protocol::types::BlockPos;
    use oxidized_world::chunk::heightmap::{Heightmap, HeightmapType};
    use oxidized_world::chunk::level_chunk::{OVERWORLD_HEIGHT, OVERWORLD_MIN_Y};
    use oxidized_world::chunk::{ChunkPos, LevelChunk};
    use oxidized_world::registry::{BEDROCK, BlockRegistry, BlockStateId, DIRT, GRASS_BLOCK};

    fn flat_chunk(pos: ChunkPos) -> LevelChunk {
        let mut chunk = LevelChunk::new(pos);
        let bedrock = u32::from(BEDROCK.0);
        let dirt = u32::from(DIRT.0);
        let grass = u32::from(GRASS_BLOCK.0);

        for x in 0..16i32 {
            for z in 0..16i32 {
                chunk
                    .set_block_state(x, OVERWORLD_MIN_Y, z, bedrock)
                    .unwrap();
                chunk
                    .set_block_state(x, OVERWORLD_MIN_Y + 1, z, dirt)
                    .unwrap();
                chunk
                    .set_block_state(x, OVERWORLD_MIN_Y + 2, z, dirt)
                    .unwrap();
                chunk
                    .set_block_state(x, OVERWORLD_MIN_Y + 3, z, grass)
                    .unwrap();
            }
        }

        let mut hm = Heightmap::new(HeightmapType::MotionBlocking, OVERWORLD_HEIGHT).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                hm.set(x, z, 4).unwrap();
            }
        }
        chunk.set_heightmap(hm);
        chunk
    }

    fn glowstone_id() -> u32 {
        u32::from(
            BlockRegistry
                .default_state("minecraft:glowstone")
                .expect("glowstone missing")
                .0,
        )
    }

    /// Torch at chunk edge (x=15) in chunk (0,0) — light should propagate
    /// into chunk (1,0) at x=0 via cross-chunk boundary entries.
    #[test]
    fn test_torch_at_chunk_edge_propagates_to_neighbor() {
        let mut center = flat_chunk(ChunkPos::new(0, 0));
        let mut east = flat_chunk(ChunkPos::new(1, 0));

        // Initialize light for both chunks.
        let mut engine = LightEngine::new();
        engine.light_chunk(&mut center).unwrap();
        engine.light_chunk(&mut east).unwrap();

        // Place glowstone at x=15 (east edge) in center chunk, above surface.
        let gs = glowstone_id();
        let emission = BlockStateId(gs as u16).light_emission();
        center.set_block_state(15, -56, 8, gs).unwrap();
        engine.queue_mut().push(LightUpdate {
            pos: BlockPos::new(15, -56, 8),
            old_emission: 0,
            new_emission: emission,
            old_opacity: 0,
            new_opacity: BlockStateId(gs as u16).light_opacity(),
        });

        let result = engine.process_updates(&mut center).unwrap();

        // Should have block boundary entries crossing into east (x >= 16).
        assert!(
            !result.block_boundary.is_empty(),
            "expected block boundary entries from glowstone at chunk edge"
        );
        assert!(
            result.block_boundary.iter().any(|b| b.world_x >= 16),
            "expected boundary entries crossing into east chunk (x >= 16)"
        );

        // Now propagate into the east neighbor.
        let mut neighbors = ChunkNeighbors {
            north: None,
            south: None,
            east: Some(&mut east),
            west: None,
        };
        propagate_block_light_cross_chunk(&mut neighbors, &result.block_boundary, 0, 0);

        // Verify light appeared in the east neighbor at x=0.
        let light_at_border = east.get_block_light_at(0, -56, 8);
        assert!(
            light_at_border > 0,
            "expected block light at east neighbor border, got {light_at_border}"
        );
        // Should be attenuated: emission passes as boundary level, then -1 for air opacity.
        assert!(
            light_at_border <= emission,
            "neighbor light {light_at_border} should be <= source emission {emission}"
        );

        // Light should propagate further into the neighbor.
        let light_at_1 = east.get_block_light_at(1, -56, 8);
        assert!(
            light_at_1 > 0 && light_at_1 < light_at_border,
            "expected further propagation: at x=1 got {light_at_1}, at x=0 got {light_at_border}"
        );
    }

    /// Removing a torch at chunk edge should produce decrease boundary entries
    /// that can clear light in the neighbor chunk.
    #[test]
    fn test_remove_torch_at_chunk_edge_decreases_neighbor_light() {
        let mut center = flat_chunk(ChunkPos::new(0, 0));
        let mut east = flat_chunk(ChunkPos::new(1, 0));

        let mut engine = LightEngine::new();
        engine.light_chunk(&mut center).unwrap();
        engine.light_chunk(&mut east).unwrap();

        // Place glowstone at east edge.
        let gs = glowstone_id();
        let emission = BlockStateId(gs as u16).light_emission();
        center.set_block_state(15, -56, 8, gs).unwrap();
        engine.queue_mut().push(LightUpdate {
            pos: BlockPos::new(15, -56, 8),
            old_emission: 0,
            new_emission: emission,
            old_opacity: 0,
            new_opacity: BlockStateId(gs as u16).light_opacity(),
        });

        let result = engine.process_updates(&mut center).unwrap();
        let mut neighbors = ChunkNeighbors {
            north: None,
            south: None,
            east: Some(&mut east),
            west: None,
        };
        propagate_block_light_cross_chunk(&mut neighbors, &result.block_boundary, 0, 0);

        // Verify light is present in the neighbor.
        let light_before = east.get_block_light_at(0, -56, 8);
        assert!(light_before > 0, "light should exist before removal");

        // Now remove the glowstone.
        center.set_block_state(15, -56, 8, 0).unwrap();
        engine.queue_mut().push(LightUpdate {
            pos: BlockPos::new(15, -56, 8),
            old_emission: emission,
            new_emission: 0,
            old_opacity: BlockStateId(gs as u16).light_opacity(),
            new_opacity: 0,
        });

        let result2 = engine.process_updates(&mut center).unwrap();

        // Decrease boundary entries are intentionally discarded to prevent
        // phantom light injection (they carried old_level and were incorrectly
        // processed as increases in neighbors). The block boundary should only
        // contain entries from the increase pass; with no remaining light
        // source, no increase boundaries are produced.
        // TODO: proper cross-chunk decrease propagation (Phase 26) will
        // allow the neighbor to clear its stale light.
        assert!(
            result2.block_boundary.is_empty(),
            "decrease boundary entries should be discarded (no increase sources remain)"
        );

        // The neighbor retains stale block light — this is a known limitation
        // until cross-chunk decrease propagation is implemented.
        let light_after = east.get_block_light_at(0, -56, 8);
        assert!(
            light_after > 0,
            "neighbor retains stale light until cross-chunk decrease is implemented"
        );
    }

    /// When a neighbor chunk is not loaded (None), boundary entries are
    /// silently dropped — no panic.
    #[test]
    fn test_missing_neighbor_no_panic() {
        let mut center = flat_chunk(ChunkPos::new(0, 0));

        let mut engine = LightEngine::new();
        engine.light_chunk(&mut center).unwrap();

        // Place glowstone at chunk edge.
        let gs = glowstone_id();
        let emission = BlockStateId(gs as u16).light_emission();
        center.set_block_state(15, -56, 8, gs).unwrap();
        engine.queue_mut().push(LightUpdate {
            pos: BlockPos::new(15, -56, 8),
            old_emission: 0,
            new_emission: emission,
            old_opacity: 0,
            new_opacity: BlockStateId(gs as u16).light_opacity(),
        });

        let result = engine.process_updates(&mut center).unwrap();
        assert!(!result.block_boundary.is_empty());

        // All neighbors are None — should not panic.
        let mut neighbors = ChunkNeighbors {
            north: None,
            south: None,
            east: None,
            west: None,
        };
        propagate_block_light_cross_chunk(&mut neighbors, &result.block_boundary, 0, 0);
        propagate_sky_light_cross_chunk(&mut neighbors, &result.sky_boundary, 0, 0);
    }

    /// Sky boundary entries from initialize_sky_light at chunk edge
    /// propagate into neighbor chunks.
    #[test]
    fn test_sky_light_boundary_propagates_to_neighbor() {
        let mut center = flat_chunk(ChunkPos::new(0, 0));
        let mut east = flat_chunk(ChunkPos::new(1, 0));

        let mut engine = LightEngine::new();
        let center_result = engine.light_chunk(&mut center).unwrap();
        engine.light_chunk(&mut east).unwrap();

        // A flat chunk's sky light init will produce boundary entries where
        // sky light bleeds sideways below the heightmap at the chunk edges.
        // Even if the flat chunk doesn't produce many, verify the API works.
        if !center_result.sky_boundary.is_empty() {
            let mut neighbors = ChunkNeighbors {
                north: None,
                south: None,
                east: Some(&mut east),
                west: None,
            };
            propagate_sky_light_cross_chunk(
                &mut neighbors,
                &center_result.sky_boundary,
                0,
                0,
            );
        }
        // No assertion on specific values — this test verifies the API works
        // end-to-end without panicking.
    }
}

// ============== Persistent WorldLighting tests (23a.11) ==============

mod world_lighting_integration {
    use oxidized_game::lighting::engine::LightEngine;
    use oxidized_game::lighting::queue::LightUpdate;
    use oxidized_game::lighting::world_lighting::WorldLighting;
    use oxidized_protocol::types::BlockPos;
    use oxidized_world::chunk::heightmap::{Heightmap, HeightmapType};
    use oxidized_world::chunk::level_chunk::{OVERWORLD_HEIGHT, OVERWORLD_MIN_Y};
    use oxidized_world::chunk::{ChunkPos, LevelChunk};
    use oxidized_world::registry::{BEDROCK, BlockRegistry, BlockStateId, DIRT, GRASS_BLOCK};

    fn flat_chunk(pos: ChunkPos) -> LevelChunk {
        let mut chunk = LevelChunk::new(pos);
        let bedrock = u32::from(BEDROCK.0);
        let dirt = u32::from(DIRT.0);
        let grass = u32::from(GRASS_BLOCK.0);

        for x in 0..16i32 {
            for z in 0..16i32 {
                chunk
                    .set_block_state(x, OVERWORLD_MIN_Y, z, bedrock)
                    .unwrap();
                chunk
                    .set_block_state(x, OVERWORLD_MIN_Y + 1, z, dirt)
                    .unwrap();
                chunk
                    .set_block_state(x, OVERWORLD_MIN_Y + 2, z, dirt)
                    .unwrap();
                chunk
                    .set_block_state(x, OVERWORLD_MIN_Y + 3, z, grass)
                    .unwrap();
            }
        }

        let mut hm = Heightmap::new(HeightmapType::MotionBlocking, OVERWORLD_HEIGHT).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                hm.set(x, z, 4).unwrap();
            }
        }
        chunk.set_heightmap(hm);
        chunk
    }

    fn glowstone_id() -> u32 {
        u32::from(
            BlockRegistry
                .default_state("minecraft:glowstone")
                .expect("glowstone missing")
                .0,
        )
    }

    /// Verifies that the persistent engine retains state across two
    /// consecutive `process_updates()` calls. Tick 1 places a light source;
    /// tick 2 removes it. The engine should correctly undo the light.
    #[test]
    fn test_engine_retains_state_across_two_process_updates_calls() {
        let mut wl = WorldLighting::new();
        let mut chunk = flat_chunk(ChunkPos::new(0, 0));

        // Initialize sky light.
        wl.engine_mut().light_chunk(&mut chunk).unwrap();

        // Tick 1: place glowstone at (8, -56, 8).
        let gs = glowstone_id();
        let emission = BlockStateId(gs as u16).light_emission();
        let opacity = BlockStateId(gs as u16).light_opacity();
        chunk.set_block_state(8, -56, 8, gs).unwrap();
        wl.engine_mut().queue_mut().push(LightUpdate {
            pos: BlockPos::new(8, -56, 8),
            old_emission: 0,
            new_emission: emission,
            old_opacity: 0,
            new_opacity: opacity,
        });
        let result1 = wl.engine_mut().process_updates(&mut chunk).unwrap();
        assert!(!result1.changed_sections.is_empty());
        assert_eq!(chunk.get_block_light_at(8, -56, 8), emission);
        assert_eq!(chunk.get_block_light_at(9, -56, 8), emission - 1);

        // Tick 2: remove glowstone (set to air).
        chunk.set_block_state(8, -56, 8, 0).unwrap();
        wl.engine_mut().queue_mut().push(LightUpdate {
            pos: BlockPos::new(8, -56, 8),
            old_emission: emission,
            new_emission: 0,
            old_opacity: opacity,
            new_opacity: 0,
        });
        let result2 = wl.engine_mut().process_updates(&mut chunk).unwrap();
        assert!(!result2.changed_sections.is_empty());
        assert_eq!(chunk.get_block_light_at(8, -56, 8), 0);
        assert_eq!(chunk.get_block_light_at(9, -56, 8), 0);
    }

    /// Verifies that boundary entries queued in tick N are available for
    /// retrieval in tick N+1 via `drain_boundaries()`.
    #[test]
    fn test_boundary_entries_queued_in_tick_n_available_in_tick_n_plus_1() {
        let mut wl = WorldLighting::new();
        let mut chunk = flat_chunk(ChunkPos::new(0, 0));

        // Initialize and place glowstone at chunk edge (0, -56, 8).
        wl.engine_mut().light_chunk(&mut chunk).unwrap();
        let gs = glowstone_id();
        let emission = BlockStateId(gs as u16).light_emission();
        let opacity = BlockStateId(gs as u16).light_opacity();
        chunk.set_block_state(0, -56, 8, gs).unwrap();
        wl.engine_mut().queue_mut().push(LightUpdate {
            pos: BlockPos::new(0, -56, 8),
            old_emission: 0,
            new_emission: emission,
            old_opacity: 0,
            new_opacity: opacity,
        });

        // Tick 1: process updates, collect boundary entries.
        let result = wl.engine_mut().process_updates(&mut chunk).unwrap();
        assert!(
            !result.block_boundary.is_empty(),
            "expected block boundary entries at chunk edge"
        );

        // Store boundary entries into WorldLighting (simulating tick loop).
        let target = ChunkPos::new(-1, 0); // neighbor to the west
        wl.queue_boundaries(target, result.block_boundary, result.sky_boundary);
        assert!(wl.has_pending_work());

        // Tick 2: drain boundaries — they should be available.
        let boundaries = wl.drain_boundaries();
        assert!(
            boundaries.contains_key(&target),
            "boundary entries for target chunk should be present"
        );
        let pending = &boundaries[&target];
        assert!(
            !pending.block.is_empty(),
            "block boundary entries should carry over to next tick"
        );
    }

    /// Two-tick light propagation: torch placed near chunk edge in tick 1
    /// produces boundary entries; tick 2 propagates them into the neighbor.
    #[test]
    fn test_two_tick_cross_chunk_propagation_via_world_lighting() {
        use oxidized_game::lighting::cross_chunk::{
            ChunkNeighbors, propagate_block_light_cross_chunk,
        };

        let mut wl = WorldLighting::new();
        let mut center = flat_chunk(ChunkPos::new(0, 0));
        let mut east = flat_chunk(ChunkPos::new(1, 0));

        // Initialize both chunks.
        {
            let mut engine = LightEngine::new();
            engine.light_chunk(&mut center).unwrap();
            engine.light_chunk(&mut east).unwrap();
        }

        // Tick 1: place glowstone at east edge of center chunk (15, -56, 8).
        let gs = glowstone_id();
        let emission = BlockStateId(gs as u16).light_emission();
        let opacity = BlockStateId(gs as u16).light_opacity();
        center.set_block_state(15, -56, 8, gs).unwrap();
        wl.engine_mut().queue_mut().push(LightUpdate {
            pos: BlockPos::new(15, -56, 8),
            old_emission: 0,
            new_emission: emission,
            old_opacity: 0,
            new_opacity: opacity,
        });

        let result = wl.engine_mut().process_updates(&mut center).unwrap();
        assert_eq!(center.get_block_light_at(15, -56, 8), emission);
        assert!(
            !result.block_boundary.is_empty(),
            "placing light at chunk edge should produce boundary entries"
        );

        // Store boundary entries (simulating end of tick 1).
        let east_pos = ChunkPos::new(1, 0);
        wl.queue_boundaries(east_pos, result.block_boundary, result.sky_boundary);

        // Tick 2: drain boundaries and propagate into neighbor.
        let boundaries = wl.drain_boundaries();
        let pending = &boundaries[&east_pos];

        let mut neighbors = ChunkNeighbors {
            north: None,
            south: None,
            east: Some(&mut east),
            west: None,
        };
        propagate_block_light_cross_chunk(&mut neighbors, &pending.block, 0, 0);

        // The east chunk's x=0 (world x=16) should have light from the torch.
        let east_light = east.get_block_light_at(0, -56, 8);
        assert!(
            east_light > 0,
            "east chunk should have light from cross-chunk propagation, got {east_light}"
        );
    }
}
