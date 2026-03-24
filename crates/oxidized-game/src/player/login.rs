//! Login sequence builder and teleport confirmation.
//!
//! Builds the ordered list of PLAY-state packets that must be sent to a
//! newly joined player. The server crate is responsible for actually
//! writing these packets to the network — this module only produces the
//! encoded packet data.
//!
//! Also provides [`handle_accept_teleportation`] to process client-side
//! teleport confirmations.
//!
//! Mirrors `net.minecraft.server.network.ServerGamePacketListenerImpl.handleLogin`.

use bytes::BytesMut;
use sha2::{Digest, Sha256};

use oxidized_protocol::codec::Packet;
use oxidized_protocol::codec::slot::{ComponentPatchData, SlotData};
use oxidized_protocol::packets::play::{
    ClientboundChangeDifficultyPacket, ClientboundContainerSetContentPacket,
    ClientboundLoginPacket, ClientboundPlayerAbilitiesPacket, ClientboundPlayerInfoUpdatePacket,
    ClientboundPlayerPositionPacket, ClientboundSetChunkCacheCenterPacket,
    ClientboundSetDefaultSpawnPositionPacket, ClientboundSetHeldSlotPacket,
    ClientboundSetSimulationDistancePacket, CommonPlayerSpawnInfo, PlayerInfoActions,
    PlayerInfoEntry, RelativeFlags,
};
use oxidized_protocol::types::ResourceLocation;
use oxidized_protocol::types::block_pos::BlockPos;
use oxidized_world::storage::PrimaryLevelData;

use super::game_mode::GameMode;
use super::player_list::PlayerList;
use super::server_player::ServerPlayer;
use crate::inventory::ItemStack;
use crate::inventory::item_ids::item_name_to_id;
use crate::level::game_rules::{GameRuleKey, GameRules};
use crate::player::PlayerInventory;
use crate::player::inventory::PROTOCOL_SLOT_COUNT;

/// An encoded packet ready to be sent over the wire.
///
/// Contains the packet ID and the encoded body (without the packet ID
/// prefix — the connection layer prepends it during framing).
#[derive(Debug, Clone)]
pub struct EncodedPacket {
    /// Minecraft protocol packet ID.
    pub id: i32,
    /// Encoded packet body.
    pub body: BytesMut,
}

/// Hashes a world seed for client-side biome rendering.
///
/// Matches `BiomeManager.obfuscateSeed()` in vanilla: SHA-256 hash of
/// the 8-byte big-endian seed, interpreted as a signed `i64`.
fn obfuscate_seed(seed: i64) -> i64 {
    let hash = Sha256::digest(seed.to_be_bytes());
    i64::from_be_bytes(hash[..8].try_into().unwrap_or([0; 8]))
}

/// Builds the complete PLAY-state login packet sequence.
///
/// Returns the packets in the order mandated by
/// `PlayerList.placeNewPlayer` in vanilla:
///
/// 1. `ClientboundLoginPacket` — world metadata + entity ID
/// 2. `ClientboundChangeDifficultyPacket` — world difficulty
/// 3. `ClientboundPlayerAbilitiesPacket` — ability flags + speeds
/// 4. `ClientboundSetHeldSlotPacket` — selected hotbar slot
/// 5. `ClientboundSetChunkCacheCenterPacket` — chunk loading center
/// 6. `ClientboundSetSimulationDistancePacket` — simulation distance
/// 7. `ClientboundPlayerPositionPacket` — initial position + teleport ID
/// 8. `ClientboundPlayerInfoUpdatePacket` — all online players for tab list
///
/// The following packets are sent separately by the server crate at the
/// correct point in the join sequence:
/// - `ClientboundEntityEventPacket` — permission level (after this batch)
/// - `ClientboundCommandsPacket` — command tree
/// - `ClientboundInitializeBorderPacket` — world border
/// - `ClientboundSetTimePacket` — clock sync
/// - `ClientboundSetDefaultSpawnPositionPacket` — compass target
/// - Weather events (if raining)
/// - `ClientboundGameEventPacket(LevelChunksLoadStart)` — before chunks
/// - Chunk data
/// - `ClientboundContainerSetContentPacket` — inventory (sent last)
pub fn build_login_sequence(
    player: &ServerPlayer,
    teleport_id: i32,
    level_data: &PrimaryLevelData,
    all_players: &PlayerList,
    dimensions: &[ResourceLocation],
    dimension_type_id: i32,
    game_rules: &GameRules,
    is_flat: bool,
) -> Vec<EncodedPacket> {
    vec![
        build_login_packet(
            player,
            level_data,
            all_players,
            dimensions,
            dimension_type_id,
            game_rules,
            is_flat,
        ),
        build_difficulty_packet(level_data),
        build_abilities_packet(player),
        build_held_slot_packet(player),
        build_chunk_center_packet(player),
        build_simulation_distance_packet(player),
        build_position_packet(player, teleport_id),
        build_player_info_packet(all_players),
    ]
}

fn build_login_packet(
    player: &ServerPlayer,
    level_data: &PrimaryLevelData,
    all_players: &PlayerList,
    dimensions: &[ResourceLocation],
    dimension_type_id: i32,
    game_rules: &GameRules,
    is_flat: bool,
) -> EncodedPacket {
    let login = ClientboundLoginPacket {
        player_id: player.entity_id,
        is_hardcore: level_data.is_hardcore,
        dimensions: dimensions.to_vec(),
        max_players: all_players.max_players() as i32,
        chunk_radius: player.view_distance,
        simulation_distance: player.simulation_distance,
        has_reduced_debug_info: game_rules.get_bool(GameRuleKey::ReducedDebugInfo),
        is_showing_death_screen: !game_rules.get_bool(GameRuleKey::ImmediateRespawn),
        is_limited_crafting: game_rules.get_bool(GameRuleKey::LimitedCrafting),
        common_spawn_info: CommonPlayerSpawnInfo {
            dimension_type_id,
            dimension: player.dimension.clone(),
            seed: obfuscate_seed(level_data.world_seed),
            game_mode: player.game_mode.id() as u8,
            previous_game_mode: GameMode::nullable_id(player.previous_game_mode),
            is_debug: false,
            is_flat,
            last_death_location: player.last_death_location.clone(),
            portal_cooldown: 0,
            sea_level: level_data.sea_level,
        },
        is_secure_chat_enforced: false,
    };
    EncodedPacket {
        id: ClientboundLoginPacket::PACKET_ID,
        body: login.encode(),
    }
}

fn build_difficulty_packet(level_data: &PrimaryLevelData) -> EncodedPacket {
    let pkt = ClientboundChangeDifficultyPacket {
        difficulty: level_data.difficulty.clamp(0, 3) as u8,
        is_locked: level_data.is_difficulty_locked,
    };
    EncodedPacket {
        id: ClientboundChangeDifficultyPacket::PACKET_ID,
        body: pkt.encode(),
    }
}

fn build_abilities_packet(player: &ServerPlayer) -> EncodedPacket {
    let abilities = ClientboundPlayerAbilitiesPacket {
        flags: player.abilities.flags_byte(),
        fly_speed: player.abilities.fly_speed,
        walk_speed: player.abilities.walk_speed,
    };
    EncodedPacket {
        id: ClientboundPlayerAbilitiesPacket::PACKET_ID,
        body: abilities.encode(),
    }
}

/// Builds the spawn position packet for the join sequence.
///
/// Sent after the world border and time sync during login.
pub fn build_spawn_position_packet(
    player: &ServerPlayer,
    level_data: &PrimaryLevelData,
) -> EncodedPacket {
    let (sx, sy, sz) = level_data.spawn_pos();
    let spawn_block = BlockPos::new(sx, sy, sz);
    let spawn_pos = ClientboundSetDefaultSpawnPositionPacket {
        dimension: player.dimension.clone(),
        pos: spawn_block.as_long(),
        yaw: level_data.spawn_angle,
        pitch: 0.0,
    };
    EncodedPacket {
        id: ClientboundSetDefaultSpawnPositionPacket::PACKET_ID,
        body: spawn_pos.encode(),
    }
}

fn build_player_info_packet(all_players: &PlayerList) -> EncodedPacket {
    let info_entries: Vec<PlayerInfoEntry> = all_players
        .iter()
        .map(|p| {
            let p = p.read();
            PlayerInfoEntry {
                uuid: p.uuid,
                name: p.name.clone(),
                properties: p.profile.properties().to_vec(),
                game_mode: p.game_mode.id(),
                latency: 0,
                is_listed: true,
                has_display_name: false,
                display_name: None,
                is_hat_visible: false,
                list_order: 0,
            }
        })
        .collect();
    let player_info = ClientboundPlayerInfoUpdatePacket {
        actions: PlayerInfoActions(
            PlayerInfoActions::ADD_PLAYER
                | PlayerInfoActions::INITIALIZE_CHAT
                | PlayerInfoActions::UPDATE_GAME_MODE
                | PlayerInfoActions::UPDATE_LISTED
                | PlayerInfoActions::UPDATE_LATENCY
                | PlayerInfoActions::UPDATE_DISPLAY_NAME
                | PlayerInfoActions::UPDATE_LIST_ORDER
                | PlayerInfoActions::UPDATE_HAT,
        ),
        entries: info_entries,
    };
    EncodedPacket {
        id: ClientboundPlayerInfoUpdatePacket::PACKET_ID,
        body: player_info.encode(),
    }
}

/// Builds the inventory sync packet for the join sequence.
///
/// Sent after chunks have been delivered (last packet in vanilla join).
pub fn build_container_set_content_packet(player: &ServerPlayer) -> EncodedPacket {
    let items: Vec<Option<SlotData>> = (0..PROTOCOL_SLOT_COUNT as i16)
        .map(|proto_slot| {
            PlayerInventory::from_protocol_slot(proto_slot).and_then(|internal| {
                let stack = player.inventory.get(internal);
                if stack.is_empty() {
                    None
                } else {
                    Some(item_stack_to_slot_data(stack))
                }
            })
        })
        .collect();

    let pkt = ClientboundContainerSetContentPacket {
        container_id: 0,
        state_id: 0,
        items,
        carried_item: None,
    };
    EncodedPacket {
        id: ClientboundContainerSetContentPacket::PACKET_ID,
        body: pkt.encode(),
    }
}

fn build_held_slot_packet(player: &ServerPlayer) -> EncodedPacket {
    let pkt = ClientboundSetHeldSlotPacket {
        slot: player.inventory.selected_slot as i32,
    };
    EncodedPacket {
        id: ClientboundSetHeldSlotPacket::PACKET_ID,
        body: pkt.encode(),
    }
}

/// Converts a game [`ItemStack`] to a wire [`SlotData`] for the login
/// sequence. Uses the vanilla item registry to map names to protocol IDs.
fn item_stack_to_slot_data(stack: &ItemStack) -> SlotData {
    SlotData {
        count: stack.count,
        item_id: item_name_to_id(&stack.item.0),
        component_data: ComponentPatchData::default(),
    }
}

fn build_chunk_center_packet(player: &ServerPlayer) -> EncodedPacket {
    let chunk_center = ClientboundSetChunkCacheCenterPacket {
        chunk_x: player.chunk_x(),
        chunk_z: player.chunk_z(),
    };
    EncodedPacket {
        id: ClientboundSetChunkCacheCenterPacket::PACKET_ID,
        body: chunk_center.encode(),
    }
}

fn build_simulation_distance_packet(player: &ServerPlayer) -> EncodedPacket {
    let sim_dist = ClientboundSetSimulationDistancePacket {
        simulation_distance: player.simulation_distance,
    };
    EncodedPacket {
        id: ClientboundSetSimulationDistancePacket::PACKET_ID,
        body: sim_dist.encode(),
    }
}

fn build_position_packet(player: &ServerPlayer, teleport_id: i32) -> EncodedPacket {
    let position = ClientboundPlayerPositionPacket {
        teleport_id,
        x: player.pos.x,
        y: player.pos.y,
        z: player.pos.z,
        dx: 0.0,
        dy: 0.0,
        dz: 0.0,
        yaw: player.yaw,
        pitch: player.pitch,
        relative_flags: RelativeFlags::empty(),
    };
    EncodedPacket {
        id: ClientboundPlayerPositionPacket::PACKET_ID,
        body: position.encode(),
    }
}

/// Processes a client's teleport confirmation.
///
/// When the client sends `ServerboundAcceptTeleportationPacket`, this
/// function removes the matching teleport ID from the player's pending
/// queue. The player is not considered "fully in world" until their
/// initial teleport is acknowledged.
///
/// # Returns
///
/// `true` if the teleport ID matched and was removed, `false` otherwise
/// (unexpected or duplicate confirmation).
pub fn handle_accept_teleportation(player: &mut ServerPlayer, teleport_id: i32) -> bool {
    if let Some(idx) = player
        .pending_teleports
        .iter()
        .position(|&(id, _, _)| id == teleport_id)
    {
        // Remove this entry and all entries before it (they're implicitly confirmed).
        for _ in 0..=idx {
            player.pending_teleports.pop_front();
        }
        return true;
    }
    false
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use oxidized_nbt::NbtCompound;
    use oxidized_protocol::auth::GameProfile;
    use oxidized_protocol::types::Vec3;
    use uuid::Uuid;

    use super::*;

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

    #[test]
    fn login_sequence_produces_eight_packets() {
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
        assert_eq!(packets.len(), 8);
    }

    #[test]
    fn login_sequence_packet_order() {
        let player = make_player(42, "Steve");
        let level_data = make_level_data();
        let player_list = PlayerList::new(20);
        let dimensions = vec![ResourceLocation::minecraft("overworld")];

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

    #[test]
    fn login_packet_contains_correct_entity_id() {
        let player = make_player(42, "Steve");
        let level_data = make_level_data();
        let player_list = PlayerList::new(20);
        let dimensions = vec![ResourceLocation::minecraft("overworld")];

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
        let login = ClientboundLoginPacket::decode(packets[0].body.clone().freeze()).unwrap();

        assert_eq!(login.player_id, 42);
        assert!(!login.is_hardcore);
        assert_eq!(login.dimensions.len(), 1);
        assert_eq!(login.max_players, 20);
        assert_eq!(login.chunk_radius, 10);
        assert_eq!(login.simulation_distance, 10);
        assert_eq!(login.common_spawn_info.game_mode, 0);
        assert_eq!(login.common_spawn_info.previous_game_mode, -1);
        assert_eq!(login.common_spawn_info.sea_level, 63);
        assert!(!login.common_spawn_info.is_flat);
    }

    #[test]
    fn abilities_packet_matches_game_mode() {
        let player = make_player(1, "Creative");
        // Override to creative
        let mut player = player;
        player.game_mode = GameMode::Creative;
        player.abilities =
            super::super::abilities::PlayerAbilities::for_game_mode(GameMode::Creative);

        let level_data = make_level_data();
        let player_list = PlayerList::new(20);
        let dimensions = vec![ResourceLocation::minecraft("overworld")];

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
        let abilities =
            ClientboundPlayerAbilitiesPacket::decode(packets[2].body.clone().freeze()).unwrap();

        // Creative: invulnerable(0x01) | can_fly(0x04) | instabuild(0x08) = 0x0D
        assert_eq!(abilities.flags, 0x0D);
        assert!((abilities.fly_speed - 0.05).abs() < f32::EPSILON);
        assert!((abilities.walk_speed - 0.1).abs() < f32::EPSILON);
    }

    #[test]
    fn spawn_position_packet_uses_level_data() {
        let player = make_player(1, "Test");
        let level_data = make_level_data();

        let pkt = build_spawn_position_packet(&player, &level_data);
        let spawn = ClientboundSetDefaultSpawnPositionPacket::decode(pkt.body.freeze()).unwrap();

        let pos = BlockPos::from_long(spawn.pos);
        assert_eq!(pos.x, 100);
        assert_eq!(pos.y, 64);
        assert_eq!(pos.z, -200);
        assert!((spawn.yaw - 90.0).abs() < f32::EPSILON);
    }

    #[test]
    fn is_flat_flag_propagated_to_login_packet() {
        let player = make_player(1, "Test");
        let level_data = make_level_data();
        let player_list = PlayerList::new(20);
        let dimensions = vec![ResourceLocation::minecraft("overworld")];

        let packets = build_login_sequence(
            &player,
            1,
            &level_data,
            &player_list,
            &dimensions,
            0,
            &GameRules::default(),
            true,
        );
        let login = ClientboundLoginPacket::decode(packets[0].body.clone().freeze()).unwrap();
        assert!(login.common_spawn_info.is_flat);
    }

    #[test]
    fn player_info_includes_all_online_players() {
        let level_data = make_level_data();
        let mut player_list = PlayerList::new(20);
        player_list.add(make_player(1, "Alice"));
        player_list.add(make_player(2, "Bob"));
        let dimensions = vec![ResourceLocation::minecraft("overworld")];

        let joining = make_player(3, "Charlie");
        let packets = build_login_sequence(
            &joining,
            1,
            &level_data,
            &player_list,
            &dimensions,
            0,
            &GameRules::default(),
            false,
        );
        let info =
            ClientboundPlayerInfoUpdatePacket::decode(packets[7].body.clone().freeze()).unwrap();

        // Should contain Alice and Bob (the players in the list).
        assert_eq!(info.entries.len(), 2);
        let names: Vec<&str> = info.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Alice"));
        assert!(names.contains(&"Bob"));
    }

    #[test]
    fn chunk_cache_center_matches_player_position() {
        let mut player = make_player(1, "Test");
        player.pos = oxidized_protocol::types::Vec3::new(100.0, 64.0, -200.0);

        let level_data = make_level_data();
        let player_list = PlayerList::new(20);
        let dimensions = vec![ResourceLocation::minecraft("overworld")];

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
        let center =
            ClientboundSetChunkCacheCenterPacket::decode(packets[4].body.clone().freeze()).unwrap();

        assert_eq!(center.chunk_x, 6); // 100 >> 4 = 6
        assert_eq!(center.chunk_z, -13); // -200 >> 4 = -13
    }

    #[test]
    fn position_packet_uses_player_pos_and_teleport_id() {
        let mut player = make_player(1, "Test");
        player.pos = oxidized_protocol::types::Vec3::new(50.5, 70.0, -100.25);
        player.yaw = 90.0;
        player.pitch = -15.0;

        let level_data = make_level_data();
        let player_list = PlayerList::new(20);
        let dimensions = vec![ResourceLocation::minecraft("overworld")];

        let packets = build_login_sequence(
            &player,
            42,
            &level_data,
            &player_list,
            &dimensions,
            0,
            &GameRules::default(),
            false,
        );
        let pos =
            ClientboundPlayerPositionPacket::decode(packets[6].body.clone().freeze()).unwrap();

        assert_eq!(pos.teleport_id, 42);
        assert!((pos.x - 50.5).abs() < 0.001);
        assert!((pos.y - 70.0).abs() < 0.001);
        assert!((pos.z + 100.25).abs() < 0.001);
        assert!((pos.yaw - 90.0).abs() < 0.001);
        assert!((pos.pitch + 15.0).abs() < 0.001);
        assert_eq!(pos.relative_flags, RelativeFlags::empty());
    }

    #[test]
    fn accept_teleportation_removes_matching_id() {
        use std::time::Instant;
        let mut player = make_player(1, "Test");
        player
            .pending_teleports
            .push_back((1, Vec3::ZERO, Instant::now()));
        player
            .pending_teleports
            .push_back((2, Vec3::ZERO, Instant::now()));

        assert!(handle_accept_teleportation(&mut player, 1));
        assert_eq!(player.pending_teleports.len(), 1);
        assert_eq!(player.pending_teleports.front().unwrap().0, 2);
    }

    #[test]
    fn accept_teleportation_rejects_wrong_id() {
        use std::time::Instant;
        let mut player = make_player(1, "Test");
        player
            .pending_teleports
            .push_back((1, Vec3::ZERO, Instant::now()));

        assert!(!handle_accept_teleportation(&mut player, 99));
        assert_eq!(player.pending_teleports.len(), 1);
    }

    #[test]
    fn accept_teleportation_empty_queue() {
        let mut player = make_player(1, "Test");
        assert!(!handle_accept_teleportation(&mut player, 1));
    }

    #[test]
    fn accept_teleportation_clears_all_sequential() {
        use std::time::Instant;
        let mut player = make_player(1, "Test");
        player
            .pending_teleports
            .push_back((1, Vec3::ZERO, Instant::now()));
        player
            .pending_teleports
            .push_back((2, Vec3::ZERO, Instant::now()));
        player
            .pending_teleports
            .push_back((3, Vec3::ZERO, Instant::now()));

        assert!(handle_accept_teleportation(&mut player, 1));
        assert!(handle_accept_teleportation(&mut player, 2));
        assert!(handle_accept_teleportation(&mut player, 3));
        assert!(player.pending_teleports.is_empty());
    }

    #[test]
    fn hardcore_world_reflected_in_login_packet() {
        let player = make_player(1, "Test");
        let mut level_data = make_level_data();
        level_data.is_hardcore = true;
        let player_list = PlayerList::new(20);
        let dimensions = vec![ResourceLocation::minecraft("overworld")];

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
        let login = ClientboundLoginPacket::decode(packets[0].body.clone().freeze()).unwrap();

        assert!(login.is_hardcore);
    }

    #[test]
    fn obfuscate_seed_deterministic() {
        // Same seed always produces same hash
        let hash1 = obfuscate_seed(12345);
        let hash2 = obfuscate_seed(12345);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn obfuscate_seed_different_for_different_seeds() {
        assert_ne!(obfuscate_seed(0), obfuscate_seed(1));
        assert_ne!(obfuscate_seed(12345), obfuscate_seed(-12345));
    }

    #[test]
    fn obfuscate_seed_zero_input_nonzero_output() {
        // SHA-256 of 0 should not be 0
        assert_ne!(obfuscate_seed(0), 0);
    }

    #[test]
    fn login_packet_contains_hashed_seed() {
        let mut level_data = make_level_data();
        level_data.world_seed = 42;
        let player = make_player(1, "Test");
        let player_list = PlayerList::new(20);
        let dimensions = vec![ResourceLocation::minecraft("overworld")];

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
        let login = ClientboundLoginPacket::decode(packets[0].body.clone().freeze()).unwrap();
        assert_eq!(login.common_spawn_info.seed, obfuscate_seed(42));
        assert_ne!(login.common_spawn_info.seed, 0);
    }

    #[test]
    fn difficulty_packet_respects_lock() {
        let mut level_data = make_level_data();
        level_data.is_difficulty_locked = true;
        let player = make_player(1, "Test");
        let player_list = PlayerList::new(20);
        let dimensions = vec![ResourceLocation::minecraft("overworld")];

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
        let diff =
            ClientboundChangeDifficultyPacket::decode(packets[1].body.clone().freeze()).unwrap();
        assert!(diff.is_locked);
    }
}
