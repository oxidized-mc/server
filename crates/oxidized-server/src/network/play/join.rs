//! Player join sequence — login packets, tab-list broadcast, entity exchange.

use std::sync::Arc;
use std::time::Instant;

use oxidized_chat::Component;
use oxidized_codec::Packet;
use oxidized_mc_types::resource_location::ResourceLocation;
use oxidized_protocol::packets::configuration::ClientInformation;
use oxidized_protocol::packets::play::{
    ClientboundAddEntityPacket, ClientboundEntityEventPacket, ClientboundGameEventPacket,
    ClientboundInitializeBorderPacket, ClientboundPlayerInfoUpdatePacket,
    ClientboundRemoveEntitiesPacket, ClientboundSetEntityDataPacket, ClientboundSetEquipmentPacket,
    ClientboundSetTimePacket, ClientboundSystemChatPacket, ClockNetworkState, ClockUpdate,
    GameEventType, PlayerInfoActions, PlayerInfoEntry,
};
use oxidized_transport::connection::ConnectionError;
use oxidized_transport::handle::ConnectionHandle;
use oxidized_types::ChunkPos;
use parking_lot::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::network::{BroadcastMessage, ServerContext};
use oxidized_game::player::{
    GameMode, ServerPlayer, build_container_set_content_packet, build_login_sequence,
    build_spawn_position_packet,
};

use super::commands;
use super::helpers;
use super::{
    DATA_PLAYER_MODE_CUSTOMISATION, HAT_MASK, PLAYER_ENTITY_TYPE_ID, build_equipment_packet,
    pack_angle,
};

/// State returned by the join sequence, consumed by the play loop.
pub(super) struct JoinState {
    /// The player entity wrapped in thread-safe locks.
    pub player: Arc<RwLock<ServerPlayer>>,
    /// The player's display name (cached from profile).
    pub name: String,
    /// The player's UUID.
    pub uuid: Uuid,
    /// The player's assigned entity ID.
    pub entity_id: i32,
    /// The initial chunk X coordinate (block-based, for tracker init).
    pub initial_chunk_x: i32,
    /// The initial chunk Z coordinate (block-based, for tracker init).
    pub initial_chunk_z: i32,
    /// The player's effective view distance (capped to server max).
    pub view_distance: i32,
}

/// Runs the complete player join sequence: creates the player, sends login
/// packets, broadcasts tab-list/entity data, and returns the state needed
/// for the play loop.
///
/// # Errors
///
/// Returns a [`ConnectionError`] if any I/O or protocol step fails.
pub(super) async fn send_join_sequence(
    conn_handle: &ConnectionHandle,
    profile: oxidized_auth::GameProfile,
    client_info: ClientInformation,
    server_ctx: &Arc<ServerContext>,
) -> Result<JoinState, ConnectionError> {
    let addr = conn_handle.remote_addr();

    let uuid = profile.uuid().ok_or_else(|| {
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Profile has invalid UUID",
        ))
    })?;

    // Vanilla: canPlayerLogin — reject if server is full (and this isn't a reconnect).
    let is_server_full = {
        let player_list = server_ctx.network.player_list.read();
        player_list.is_full() && !player_list.contains(&uuid)
    };
    if is_server_full {
        warn!(peer = %addr, uuid = %uuid, "Server full — rejecting login");
        return Err(
            crate::network::helpers::disconnect_handle(conn_handle, "Server is full!").await,
        );
    }

    // Create ServerPlayer with entity ID from the player list.
    let entity_id = server_ctx.network.player_list.read().next_entity_id();
    let game_mode = GameMode::from_id(server_ctx.world.level_data.read().settings.game_type);
    let dimension = ResourceLocation::from_string("minecraft:overworld").map_err(|e| {
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })?;

    let mut player = ServerPlayer::new(entity_id, profile, dimension, game_mode);

    // Apply client preferences, capped to server maximums.
    player.connection.view_distance =
        i32::from(client_info.view_distance).clamp(2, server_ctx.settings.max_view_distance);
    player.connection.simulation_distance = server_ctx.settings.max_simulation_distance;
    player.connection.model_customisation = client_info.model_customisation;

    // Try to load saved player data from playerdata/<uuid>.dat.
    let playerdata_path = format!("world/playerdata/{uuid}.dat");
    if std::path::Path::new(&playerdata_path).exists() {
        match oxidized_nbt::read_file(std::path::Path::new(&playerdata_path)) {
            Ok(nbt) => {
                player.load_from_nbt(&nbt);
                debug!(peer = %addr, uuid = %uuid, "Loaded player data from disk");
            },
            Err(e) => {
                warn!(peer = %addr, uuid = %uuid, error = %e, "Failed to load player data — using defaults");
            },
        }
    } else {
        // New player — spawn at world spawn, centered on the block (vanilla: +0.5 on X/Z).
        let (sx, sy, sz) = server_ctx.world.level_data.read().spawn_pos();
        player.movement.pos =
            oxidized_mc_types::Vec3::new(f64::from(sx) + 0.5, f64::from(sy), f64::from(sz) + 0.5);
        debug!(peer = %addr, uuid = %uuid, "New player — spawning at world spawn");
    }

    // Assign a teleport ID for the initial position packet.
    let teleport_id = player.teleport.next_id();
    player
        .teleport
        .pending
        .push_back((teleport_id, player.movement.pos, Instant::now()));

    let player_name = player.name.clone();
    let player_view_distance = player.connection.view_distance;
    let player_chunk_x = player.movement.pos.x.floor() as i32;
    let player_chunk_z = player.movement.pos.z.floor() as i32;

    // Build the login sequence.
    let dimension_type_id = oxidized_protocol::registry::get_registry_entry_index(
        "minecraft:dimension_type",
        "minecraft:overworld",
    )
    .map_err(|e| {
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("failed to resolve overworld dimension type ID: {e}"),
        ))
    })?;
    let is_flat = server_ctx.world.chunk_generator.generator_type() == "minecraft:flat";
    let packets = {
        let level_data = server_ctx.world.level_data.read();
        let player_list = server_ctx.network.player_list.read();
        let game_rules = server_ctx.world.game_rules.read();
        build_login_sequence(
            &player,
            teleport_id,
            &level_data,
            &player_list,
            &server_ctx.world.dimensions,
            dimension_type_id,
            &game_rules,
            is_flat,
        )
    };

    // Send all login packets before adding player to the list.
    // This prevents a "ghost" player entry if sending fails.
    for pkt in &packets {
        conn_handle
            .send_raw(pkt.id, bytes::Bytes::copy_from_slice(&pkt.body))
            .await?;
    }

    send_level_info(
        conn_handle,
        &player,
        &player_name,
        uuid,
        entity_id,
        server_ctx,
    )
    .await?;

    // Send initial chunk batch using the world generator.
    let chunk_center = ChunkPos::from_block_coords(player_chunk_x, player_chunk_z);
    let chunk_count =
        helpers::send_initial_chunks(conn_handle, chunk_center, player_view_distance, server_ctx)
            .await?;

    // Send inventory last, after chunks (vanilla: initInventoryMenu).
    {
        let inv_pkt = build_container_set_content_packet(&player);
        conn_handle
            .send_raw(inv_pkt.id, bytes::Bytes::copy_from_slice(&inv_pkt.body))
            .await?;
    }

    info!(
        peer = %addr,
        uuid = %uuid,
        chunks = chunk_count,
        "Initial chunk batch sent",
    );

    // Kick any existing session for this UUID before replacing it.
    // This fires the old session's kick_rx, causing a clean disconnect.
    if let Some(kick_tx) = server_ctx.network.kick_channels.get(&uuid) {
        let _ = kick_tx.try_send("You logged in from another location".to_string());
    }

    // Add player to the server player list (only after successful send).
    let (player_arc, old_player) = server_ctx.network.player_list.write().add(player);
    if let Some(old) = old_player {
        let old_p = old.read();
        let old_entity_id = old_p.entity_id;
        drop(old_p);
        warn!(
            peer = %addr,
            uuid = %uuid,
            name = %player_name,
            old_entity_id = old_entity_id,
            "Duplicate login — replacing existing player session",
        );
        // Remove old entity from all clients.
        let remove_entity = ClientboundRemoveEntitiesPacket {
            entity_ids: vec![old_entity_id],
        };
        let encoded = remove_entity.encode();
        server_ctx.broadcast(BroadcastMessage {
            packet_id: ClientboundRemoveEntitiesPacket::PACKET_ID,
            data: encoded.freeze(),
            exclude_entity: None,
            target_entity: None,
        });
    }

    broadcast_player_join(
        conn_handle,
        &player_arc,
        &player_name,
        uuid,
        entity_id,
        server_ctx,
    )
    .await?;

    info!(
        peer = %addr,
        uuid = %uuid,
        name = %player_name,
        entity_id = entity_id,
        packets = packets.len(),
        "PLAY login sequence sent",
    );

    Ok(JoinState {
        player: player_arc,
        name: player_name,
        uuid,
        entity_id,
        initial_chunk_x: player_chunk_x,
        initial_chunk_z: player_chunk_z,
        view_distance: player_view_distance,
    })
}

/// Sends level-info packets: permissions, commands, world border, time, spawn,
/// weather, and the chunk-load-start signal.
async fn send_level_info(
    conn_handle: &ConnectionHandle,
    player: &ServerPlayer,
    player_name: &str,
    uuid: Uuid,
    entity_id: i32,
    server_ctx: &Arc<ServerContext>,
) -> Result<(), ConnectionError> {
    let addr = conn_handle.remote_addr();

    // Send EntityEvent with the player's permission level (vanilla sends
    // this via sendPlayerPermissionLevel — event IDs 24–28).
    {
        let perm_level = server_ctx.ops.get_permission_level(&uuid).clamp(0, 4) as u8;
        conn_handle
            .send_packet(&ClientboundEntityEventPacket {
                entity_id,
                event_id: ClientboundEntityEventPacket::PERMISSION_LEVEL_BASE + perm_level,
            })
            .await?;
    }

    // Send the command tree so the client can offer tab-completion.
    {
        let cmd_source = commands::make_command_source(player_name, uuid, player, server_ctx);
        let tree = server_ctx.commands.serialize_tree(&cmd_source);
        let cmd_pkt = commands::commands_packet_from_tree(&tree);
        conn_handle.send_packet(&cmd_pkt).await?;
    }

    // Send default world border state (vanilla: sendLevelInfo).
    conn_handle
        .send_packet(&ClientboundInitializeBorderPacket {
            new_center_x: 0.0,
            new_center_z: 0.0,
            old_size: 59_999_968.0,
            new_size: 59_999_968.0,
            lerp_time: 0,
            new_absolute_max_size: 29_999_984,
            warning_blocks: 5,
            warning_time: 15,
        })
        .await?;

    // Send time sync with overworld clock data (vanilla: sendLevelInfo).
    {
        let time_pkt = {
            let ld = server_ctx.world.level_data.read();
            ClientboundSetTimePacket {
                game_time: ld.time.game_time,
                clock_updates: vec![ClockUpdate {
                    clock_id: ClientboundSetTimePacket::OVERWORLD_CLOCK_ID,
                    state: ClockNetworkState {
                        total_ticks: ld.time.day_time,
                        partial_tick: 0.0,
                        rate: 1.0,
                    },
                }],
            }
        };
        conn_handle.send_packet(&time_pkt).await?;
    }

    // Send spawn position (vanilla: sendLevelInfo → respawnData).
    {
        let spawn_pkt = {
            let ld = server_ctx.world.level_data.read();
            build_spawn_position_packet(player, &ld)
        };
        conn_handle
            .send_raw(spawn_pkt.id, bytes::Bytes::copy_from_slice(&spawn_pkt.body))
            .await?;
    }

    // Send weather state if it is currently raining (vanilla: sendLevelInfo).
    let (is_raining, is_thundering) = {
        let ld = server_ctx.world.level_data.read();
        (ld.weather.is_raining, ld.weather.is_thundering)
    };
    if is_raining {
        conn_handle
            .send_packet(&ClientboundGameEventPacket {
                event: GameEventType::StartRaining,
                param: 0.0,
            })
            .await?;
        conn_handle
            .send_packet(&ClientboundGameEventPacket {
                event: GameEventType::RainLevelChange,
                param: 1.0,
            })
            .await?;
        if is_thundering {
            conn_handle
                .send_packet(&ClientboundGameEventPacket {
                    event: GameEventType::ThunderLevelChange,
                    param: 1.0,
                })
                .await?;
        }
    }

    // Signal the client that chunk loading is about to begin (must be
    // BEFORE chunks are sent — vanilla ordering requirement).
    conn_handle
        .send_packet(&ClientboundGameEventPacket {
            event: GameEventType::LevelChunksLoadStart,
            param: 0.0,
        })
        .await?;

    debug!(peer = %addr, uuid = %uuid, "Level info sent");
    Ok(())
}

/// Broadcasts the new player to all existing players' tab-lists and entities,
/// and sends all existing players' entities to the joining player.
async fn broadcast_player_join(
    conn_handle: &ConnectionHandle,
    player_arc: &Arc<RwLock<ServerPlayer>>,
    player_name: &str,
    uuid: Uuid,
    entity_id: i32,
    server_ctx: &Arc<ServerContext>,
) -> Result<(), ConnectionError> {
    // Send the joining player their own tab list entry (the login sequence
    // was built before the player was added to the list, so it's missing).
    let self_info = build_player_info_packet(player_arc, player_name, uuid);
    conn_handle.send_packet(&self_info).await?;

    // Send the joining player their own entity data (skin parts + cape
    // visibility). Vanilla default for DATA_PLAYER_MODE_CUSTOMISATION is 0
    // (all parts hidden), so the client won't render outer layers or capes
    // unless we send the actual value.
    let skin = player_arc.read().connection.model_customisation;
    let self_entity_data = ClientboundSetEntityDataPacket::single_byte(
        entity_id,
        DATA_PLAYER_MODE_CUSTOMISATION,
        skin,
    );
    conn_handle.send_packet(&self_entity_data).await?;

    // Broadcast the new player to all existing players' tab lists.
    broadcast_player_info(server_ctx, player_arc, player_name, uuid, entity_id);

    // Broadcast the new player's entity, equipment, and skin to all existing
    // players, and send all existing players' entities to the joining player.
    broadcast_and_collect_entities(conn_handle, player_arc, uuid, entity_id, server_ctx).await?;

    // Broadcast "Player joined the game" system message (vanilla yellow text).
    let join_msg = ClientboundSystemChatPacket {
        content: Component::translatable(
            "multiplayer.player.joined",
            vec![Component::text(player_name)],
        ),
        is_overlay: false,
    };
    let encoded = join_msg.encode();
    server_ctx.broadcast(BroadcastMessage {
        packet_id: ClientboundSystemChatPacket::PACKET_ID,
        data: encoded.freeze(),
        exclude_entity: None,
        target_entity: None,
    });

    Ok(())
}

/// Builds the full `PlayerInfoUpdate` packet for the joining player.
fn build_player_info_packet(
    player_arc: &Arc<RwLock<ServerPlayer>>,
    player_name: &str,
    uuid: Uuid,
) -> ClientboundPlayerInfoUpdatePacket {
    let p = player_arc.read();
    ClientboundPlayerInfoUpdatePacket {
        actions: all_info_actions(),
        entries: vec![PlayerInfoEntry {
            uuid,
            name: player_name.to_owned(),
            properties: p.profile.properties().to_vec(),
            game_mode: p.game_mode as i32,
            latency: 0,
            is_listed: true,
            has_display_name: false,
            display_name: None,
            is_hat_visible: (p.connection.model_customisation & HAT_MASK) != 0,
            list_order: 0,
        }],
    }
}

/// Returns the combined action flags used for join-time player info packets.
fn all_info_actions() -> PlayerInfoActions {
    PlayerInfoActions(
        PlayerInfoActions::ADD_PLAYER
            | PlayerInfoActions::INITIALIZE_CHAT
            | PlayerInfoActions::UPDATE_GAME_MODE
            | PlayerInfoActions::UPDATE_LISTED
            | PlayerInfoActions::UPDATE_LATENCY
            | PlayerInfoActions::UPDATE_DISPLAY_NAME
            | PlayerInfoActions::UPDATE_LIST_ORDER
            | PlayerInfoActions::UPDATE_HAT,
    )
}

/// Broadcasts the joining player's tab-list entry to all existing players.
fn broadcast_player_info(
    server_ctx: &Arc<ServerContext>,
    player_arc: &Arc<RwLock<ServerPlayer>>,
    player_name: &str,
    uuid: Uuid,
    entity_id: i32,
) {
    let join_info = build_player_info_packet(player_arc, player_name, uuid);
    let encoded = join_info.encode();
    server_ctx.broadcast(BroadcastMessage {
        packet_id: ClientboundPlayerInfoUpdatePacket::PACKET_ID,
        data: encoded.freeze(),
        exclude_entity: Some(entity_id),
        target_entity: None,
    });
}

/// Broadcasts the new player's entity/equipment/skin to existing players, and
/// sends all existing players' entity data to the joining player.
async fn broadcast_and_collect_entities(
    conn_handle: &ConnectionHandle,
    player_arc: &Arc<RwLock<ServerPlayer>>,
    uuid: Uuid,
    entity_id: i32,
    server_ctx: &Arc<ServerContext>,
) -> Result<(), ConnectionError> {
    // Broadcast new player entity to existing players.
    let add_entity = {
        let p = player_arc.read();
        ClientboundAddEntityPacket {
            entity_id,
            uuid,
            entity_type: PLAYER_ENTITY_TYPE_ID,
            x: p.movement.pos.x,
            y: p.movement.pos.y,
            z: p.movement.pos.z,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            x_rot: pack_angle(p.movement.pitch),
            y_rot: pack_angle(p.movement.yaw),
            y_head_rot: pack_angle(p.movement.yaw),
            data: 0,
        }
    };
    let encoded = add_entity.encode();
    server_ctx.broadcast(BroadcastMessage {
        packet_id: ClientboundAddEntityPacket::PACKET_ID,
        data: encoded.freeze(),
        exclude_entity: Some(entity_id),
        target_entity: None,
    });

    // Broadcast new player's equipment to existing players.
    let equip_pkt = {
        let p = player_arc.read();
        build_equipment_packet(&p)
    };
    let encoded = equip_pkt.encode();
    server_ctx.broadcast(BroadcastMessage {
        packet_id: ClientboundSetEquipmentPacket::PACKET_ID,
        data: encoded.freeze(),
        exclude_entity: Some(entity_id),
        target_entity: None,
    });

    // Broadcast new player's skin customisation to existing players.
    let skin = player_arc.read().connection.model_customisation;
    let pkt = ClientboundSetEntityDataPacket::single_byte(
        entity_id,
        DATA_PLAYER_MODE_CUSTOMISATION,
        skin,
    );
    let encoded = pkt.encode();
    server_ctx.broadcast(BroadcastMessage {
        packet_id: ClientboundSetEntityDataPacket::PACKET_ID,
        data: encoded.freeze(),
        exclude_entity: Some(entity_id),
        target_entity: None,
    });

    // Collect existing players' entity + equipment + skin packets (no locks
    // held across await).
    let other_entities = collect_existing_player_data(server_ctx, entity_id);

    // Send existing player entities + equipment + skin to the joining player.
    for (add_pkt, equip_pkt, skin_pkt) in &other_entities {
        conn_handle.send_packet(add_pkt).await?;
        conn_handle.send_packet(equip_pkt).await?;
        conn_handle.send_packet(skin_pkt).await?;
    }

    Ok(())
}

/// Collects entity, equipment, and skin data for all players except
/// `exclude_entity_id`, without holding locks across await points.
fn collect_existing_player_data(
    server_ctx: &Arc<ServerContext>,
    exclude_entity_id: i32,
) -> Vec<(
    ClientboundAddEntityPacket,
    ClientboundSetEquipmentPacket,
    ClientboundSetEntityDataPacket,
)> {
    let player_list = server_ctx.network.player_list.read();
    player_list
        .iter()
        .filter_map(|other_arc| {
            let other = other_arc.read();
            if other.entity_id == exclude_entity_id {
                return None;
            }
            let add = ClientboundAddEntityPacket {
                entity_id: other.entity_id,
                uuid: other.uuid,
                entity_type: PLAYER_ENTITY_TYPE_ID,
                x: other.movement.pos.x,
                y: other.movement.pos.y,
                z: other.movement.pos.z,
                vx: 0.0,
                vy: 0.0,
                vz: 0.0,
                x_rot: pack_angle(other.movement.pitch),
                y_rot: pack_angle(other.movement.yaw),
                y_head_rot: pack_angle(other.movement.yaw),
                data: 0,
            };
            let equip = build_equipment_packet(&other);
            let skin = ClientboundSetEntityDataPacket::single_byte(
                other.entity_id,
                DATA_PLAYER_MODE_CUSTOMISATION,
                other.connection.model_customisation,
            );
            Some((add, equip, skin))
        })
        .collect()
}
