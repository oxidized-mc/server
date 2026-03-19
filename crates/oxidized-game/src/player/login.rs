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

use oxidized_protocol::packets::play::{
    ClientboundGameEventPacket, ClientboundLoginPacket, ClientboundPlayerAbilitiesPacket,
    ClientboundPlayerInfoUpdatePacket, ClientboundPlayerPositionPacket,
    ClientboundSetChunkCacheCenterPacket, ClientboundSetDefaultSpawnPositionPacket,
    ClientboundSetSimulationDistancePacket, CommonPlayerSpawnInfo, GameEventType,
    PlayerInfoActions, PlayerInfoEntry, RelativeFlags,
};
use oxidized_protocol::types::block_pos::BlockPos;
use oxidized_protocol::types::ResourceLocation;
use oxidized_world::storage::PrimaryLevelData;

use super::game_mode::GameMode;
use super::player_list::PlayerList;
use super::server_player::ServerPlayer;

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

/// Builds the complete PLAY-state login packet sequence.
///
/// Returns the packets in the exact order mandated by
/// `ServerGamePacketListenerImpl.handleLogin`:
///
/// 1. `ClientboundLoginPacket` — world metadata + entity ID
/// 2. `ClientboundPlayerAbilitiesPacket` — ability flags + speeds
/// 3. `ClientboundSetDefaultSpawnPositionPacket` — compass target
/// 4. `ClientboundGameEventPacket` — game mode change event
/// 5. `ClientboundPlayerInfoUpdatePacket` — all online players for tab list
/// 6. `ClientboundSetChunkCacheCenterPacket` — chunk loading center
/// 7. `ClientboundSetSimulationDistancePacket` — simulation distance
/// 8. `ClientboundPlayerPositionPacket` — initial position + teleport ID
///
/// Chunk data (step 8 in vanilla) is handled separately by the chunk
/// streaming subsystem (Phase 13).
///
/// # Arguments
///
/// * `player` - The joining player (must have a teleport ID assigned via
///   [`ServerPlayer::next_teleport_id`] before calling).
/// * `teleport_id` - The teleport ID for the initial position packet.
/// * `level_data` - World metadata from `level.dat`.
/// * `all_players` - The server's player list (for the tab list packet).
/// * `dimensions` - All registered dimension identifiers.
/// * `dimension_type_id` - The protocol registry ID for the player's
///   current dimension type (e.g., 0 for overworld).
pub fn build_login_sequence(
    player: &ServerPlayer,
    teleport_id: i32,
    level_data: &PrimaryLevelData,
    all_players: &PlayerList,
    dimensions: &[ResourceLocation],
    dimension_type_id: i32,
) -> Vec<EncodedPacket> {
    let mut packets = Vec::with_capacity(8);

    // 1. ClientboundLoginPacket
    let login = ClientboundLoginPacket {
        player_id: player.entity_id,
        hardcore: level_data.hardcore,
        dimensions: dimensions.to_vec(),
        max_players: all_players.max_players() as i32,
        chunk_radius: player.view_distance,
        simulation_distance: player.simulation_distance,
        reduced_debug_info: false,
        show_death_screen: true,
        do_limited_crafting: false,
        common_spawn_info: CommonPlayerSpawnInfo {
            dimension_type_id,
            dimension: player.dimension.clone(),
            seed: 0, // hashed seed
            game_mode: player.game_mode.id() as u8,
            previous_game_mode: GameMode::nullable_id(player.previous_game_mode),
            is_debug: false,
            is_flat: false,
            last_death_location: None,
            portal_cooldown: 0,
            sea_level: level_data.sea_level,
        },
        enforces_secure_chat: false,
    };
    packets.push(EncodedPacket {
        id: ClientboundLoginPacket::PACKET_ID,
        body: login.encode(),
    });

    // 2. ClientboundPlayerAbilitiesPacket
    let abilities = ClientboundPlayerAbilitiesPacket {
        flags: player.abilities.flags_byte(),
        fly_speed: player.abilities.fly_speed,
        walk_speed: player.abilities.walk_speed,
    };
    packets.push(EncodedPacket {
        id: ClientboundPlayerAbilitiesPacket::PACKET_ID,
        body: abilities.encode(),
    });

    // 3. ClientboundSetDefaultSpawnPositionPacket
    let (sx, sy, sz) = level_data.spawn_pos();
    let spawn_block = BlockPos::new(sx, sy, sz);
    let spawn_pos = ClientboundSetDefaultSpawnPositionPacket {
        dimension: player.dimension.clone(),
        pos: spawn_block.as_long(),
        yaw: level_data.spawn_angle,
        pitch: 0.0,
    };
    packets.push(EncodedPacket {
        id: ClientboundSetDefaultSpawnPositionPacket::PACKET_ID,
        body: spawn_pos.encode(),
    });

    // 4. ClientboundGameEventPacket — CHANGE_GAME_MODE
    let game_event = ClientboundGameEventPacket {
        event: GameEventType::ChangeGameMode,
        param: player.game_mode.id() as f32,
    };
    packets.push(EncodedPacket {
        id: ClientboundGameEventPacket::PACKET_ID,
        body: game_event.encode(),
    });

    // 5. ClientboundPlayerInfoUpdatePacket — all online players
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
                listed: true,
                has_display_name: false,
                show_hat: false,
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
                | PlayerInfoActions::UPDATE_LATENCY,
        ),
        entries: info_entries,
    };
    packets.push(EncodedPacket {
        id: ClientboundPlayerInfoUpdatePacket::PACKET_ID,
        body: player_info.encode(),
    });

    // 6. ClientboundSetChunkCacheCenterPacket
    let chunk_center = ClientboundSetChunkCacheCenterPacket {
        chunk_x: player.chunk_x(),
        chunk_z: player.chunk_z(),
    };
    packets.push(EncodedPacket {
        id: ClientboundSetChunkCacheCenterPacket::PACKET_ID,
        body: chunk_center.encode(),
    });

    // 7. ClientboundSetSimulationDistancePacket
    let sim_dist = ClientboundSetSimulationDistancePacket {
        simulation_distance: player.simulation_distance,
    };
    packets.push(EncodedPacket {
        id: ClientboundSetSimulationDistancePacket::PACKET_ID,
        body: sim_dist.encode(),
    });

    // 8. ClientboundPlayerPositionPacket — initial position
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
    packets.push(EncodedPacket {
        id: ClientboundPlayerPositionPacket::PACKET_ID,
        body: position.encode(),
    });

    packets
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
    if let Some(&front) = player.pending_teleports.front() {
        if front == teleport_id {
            player.pending_teleports.pop_front();
            return true;
        }
    }
    false
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use oxidized_nbt::NbtCompound;
    use oxidized_protocol::auth::GameProfile;
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

        let packets = build_login_sequence(&player, 1, &level_data, &player_list, &dimensions, 0);
        assert_eq!(packets.len(), 8);
    }

    #[test]
    fn login_sequence_packet_order() {
        let player = make_player(42, "Steve");
        let level_data = make_level_data();
        let player_list = PlayerList::new(20);
        let dimensions = vec![ResourceLocation::minecraft("overworld")];

        let packets = build_login_sequence(&player, 1, &level_data, &player_list, &dimensions, 0);

        assert_eq!(packets[0].id, ClientboundLoginPacket::PACKET_ID);
        assert_eq!(packets[1].id, ClientboundPlayerAbilitiesPacket::PACKET_ID);
        assert_eq!(
            packets[2].id,
            ClientboundSetDefaultSpawnPositionPacket::PACKET_ID
        );
        assert_eq!(packets[3].id, ClientboundGameEventPacket::PACKET_ID);
        assert_eq!(
            packets[4].id,
            ClientboundPlayerInfoUpdatePacket::PACKET_ID
        );
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

    #[test]
    fn login_packet_contains_correct_entity_id() {
        let player = make_player(42, "Steve");
        let level_data = make_level_data();
        let player_list = PlayerList::new(20);
        let dimensions = vec![ResourceLocation::minecraft("overworld")];

        let packets = build_login_sequence(&player, 1, &level_data, &player_list, &dimensions, 0);
        let login =
            ClientboundLoginPacket::decode(packets[0].body.clone().freeze()).unwrap();

        assert_eq!(login.player_id, 42);
        assert!(!login.hardcore);
        assert_eq!(login.dimensions.len(), 1);
        assert_eq!(login.max_players, 20);
        assert_eq!(login.chunk_radius, 10);
        assert_eq!(login.simulation_distance, 10);
        assert_eq!(login.common_spawn_info.game_mode, 0);
        assert_eq!(login.common_spawn_info.previous_game_mode, -1);
        assert_eq!(login.common_spawn_info.sea_level, 63);
    }

    #[test]
    fn abilities_packet_matches_game_mode() {
        let player = make_player(1, "Creative");
        // Override to creative
        let mut player = player;
        player.game_mode = GameMode::Creative;
        player.abilities = super::super::abilities::PlayerAbilities::for_game_mode(GameMode::Creative);

        let level_data = make_level_data();
        let player_list = PlayerList::new(20);
        let dimensions = vec![ResourceLocation::minecraft("overworld")];

        let packets = build_login_sequence(&player, 1, &level_data, &player_list, &dimensions, 0);
        let abilities =
            ClientboundPlayerAbilitiesPacket::decode(packets[1].body.clone().freeze()).unwrap();

        // Creative: invulnerable(0x01) | can_fly(0x04) | instabuild(0x08) = 0x0D
        assert_eq!(abilities.flags, 0x0D);
        assert!((abilities.fly_speed - 0.05).abs() < f32::EPSILON);
        assert!((abilities.walk_speed - 0.1).abs() < f32::EPSILON);
    }

    #[test]
    fn spawn_position_uses_level_data() {
        let player = make_player(1, "Test");
        let level_data = make_level_data();
        let player_list = PlayerList::new(20);
        let dimensions = vec![ResourceLocation::minecraft("overworld")];

        let packets = build_login_sequence(&player, 1, &level_data, &player_list, &dimensions, 0);
        let spawn =
            ClientboundSetDefaultSpawnPositionPacket::decode(packets[2].body.clone().freeze())
                .unwrap();

        let pos = BlockPos::from_long(spawn.pos);
        assert_eq!(pos.x, 100);
        assert_eq!(pos.y, 64);
        assert_eq!(pos.z, -200);
        assert!((spawn.yaw - 90.0).abs() < f32::EPSILON);
    }

    #[test]
    fn game_event_sends_game_mode() {
        let player = make_player(1, "Test");
        let level_data = make_level_data();
        let player_list = PlayerList::new(20);
        let dimensions = vec![ResourceLocation::minecraft("overworld")];

        let packets = build_login_sequence(&player, 1, &level_data, &player_list, &dimensions, 0);
        let event =
            ClientboundGameEventPacket::decode(packets[3].body.clone().freeze()).unwrap();

        assert_eq!(event.event, GameEventType::ChangeGameMode);
        assert!((event.param - 0.0).abs() < f32::EPSILON); // Survival = 0
    }

    #[test]
    fn player_info_includes_all_online_players() {
        let level_data = make_level_data();
        let mut player_list = PlayerList::new(20);
        player_list.add(make_player(1, "Alice"));
        player_list.add(make_player(2, "Bob"));
        let dimensions = vec![ResourceLocation::minecraft("overworld")];

        let joining = make_player(3, "Charlie");
        let packets =
            build_login_sequence(&joining, 1, &level_data, &player_list, &dimensions, 0);
        let info =
            ClientboundPlayerInfoUpdatePacket::decode(packets[4].body.clone().freeze()).unwrap();

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

        let packets = build_login_sequence(&player, 1, &level_data, &player_list, &dimensions, 0);
        let center =
            ClientboundSetChunkCacheCenterPacket::decode(packets[5].body.clone().freeze()).unwrap();

        assert_eq!(center.chunk_x, 6);  // 100 >> 4 = 6
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

        let packets =
            build_login_sequence(&player, 42, &level_data, &player_list, &dimensions, 0);
        let pos =
            ClientboundPlayerPositionPacket::decode(packets[7].body.clone().freeze()).unwrap();

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
        let mut player = make_player(1, "Test");
        player.pending_teleports.push_back(1);
        player.pending_teleports.push_back(2);

        assert!(handle_accept_teleportation(&mut player, 1));
        assert_eq!(player.pending_teleports.len(), 1);
        assert_eq!(*player.pending_teleports.front().unwrap(), 2);
    }

    #[test]
    fn accept_teleportation_rejects_wrong_id() {
        let mut player = make_player(1, "Test");
        player.pending_teleports.push_back(1);

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
        let mut player = make_player(1, "Test");
        player.pending_teleports.push_back(1);
        player.pending_teleports.push_back(2);
        player.pending_teleports.push_back(3);

        assert!(handle_accept_teleportation(&mut player, 1));
        assert!(handle_accept_teleportation(&mut player, 2));
        assert!(handle_accept_teleportation(&mut player, 3));
        assert!(player.pending_teleports.is_empty());
    }

    #[test]
    fn hardcore_world_reflected_in_login_packet() {
        let player = make_player(1, "Test");
        let mut level_data = make_level_data();
        level_data.hardcore = true;
        let player_list = PlayerList::new(20);
        let dimensions = vec![ResourceLocation::minecraft("overworld")];

        let packets = build_login_sequence(&player, 1, &level_data, &player_list, &dimensions, 0);
        let login =
            ClientboundLoginPacket::decode(packets[0].body.clone().freeze()).unwrap();

        assert!(login.hardcore);
    }
}
