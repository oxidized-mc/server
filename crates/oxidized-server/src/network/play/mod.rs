//! Play-state packet handling.
//!
//! Contains the main `select!` loop that drives keepalive, chat broadcast
//! relay, and serverbound packet dispatch for PLAY-state connections.
//!
//! # Future: Plugin-extensible packet dispatch
//!
//! The packet dispatch in the play loop currently uses a `match pkt.id`
//! block, which is efficient for the fixed set of vanilla packets. When
//! plugin support is added, this should migrate to a registry-based
//! approach (e.g., `HashMap<i32, Box<dyn PacketHandler>>`) so plugins
//! can register custom packet handlers without modifying this file.
//! The `EventBus` in `ServerContext` already provides the event-level
//! extensibility; the packet registry would add raw-packet-level hooks.

pub mod block_interaction;
pub mod chat;
pub mod commands;
pub mod helpers;
pub mod inventory;
pub mod movement;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use oxidized_game::chat::ChatRateLimiter;
use oxidized_game::chunk::chunk_tracker::PlayerChunkTracker;
use oxidized_game::player::{
    GameMode, ServerPlayer, build_login_sequence, handle_accept_teleportation,
};
use oxidized_protocol::auth;
use oxidized_protocol::codec::Packet;
use oxidized_protocol::connection::{Connection, ConnectionError};
use oxidized_protocol::packets::configuration::ClientInformation;
use oxidized_protocol::packets::play::{
    ClientboundAddEntityPacket, ClientboundChangeDifficultyPacket, ClientboundGameEventPacket,
    ClientboundInitializeBorderPacket, ClientboundKeepAlivePacket,
    ClientboundPlayerInfoRemovePacket, ClientboundPlayerInfoUpdatePacket,
    ClientboundRemoveEntitiesPacket, ClientboundSetChunkCacheRadiusPacket, GameEventType,
    PlayerCommandAction, PlayerInfoActions, PlayerInfoEntry, ServerboundAcceptTeleportationPacket,
    ServerboundChatCommandPacket, ServerboundChatCommandSignedPacket, ServerboundChatPacket,
    ServerboundChunkBatchReceivedPacket, ServerboundCommandSuggestionPacket,
    ServerboundKeepAlivePacket, ServerboundMovePlayerPosPacket, ServerboundMovePlayerPosRotPacket,
    ServerboundMovePlayerRotPacket, ServerboundMovePlayerStatusOnlyPacket,
    ServerboundPickItemFromBlockPacket, ServerboundPlayerActionPacket,
    ServerboundPlayerCommandPacket, ServerboundPlayerInputPacket, ServerboundSetCarriedItemPacket,
    ServerboundSetCreativeModeSlotPacket, ServerboundSignUpdatePacket, ServerboundUseItemOnPacket,
    ServerboundUseItemPacket,
};
use oxidized_protocol::types::resource_location::ResourceLocation;
use oxidized_world::chunk::ChunkPos;
use parking_lot::RwLock;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

use crate::network::{BroadcastMessage, LoginContext, MAX_SERVERBOUND_PLAY_ID, ServerContext};

use crate::network::helpers::decode_packet;

/// Shared context passed to all play-state packet handlers.
pub struct PlayContext<'a> {
    /// The connection to the client.
    pub conn: &'a mut Connection,
    /// The player entity (thread-safe via interior mutability).
    pub player: &'a Arc<RwLock<ServerPlayer>>,
    /// Shared server state.
    pub server_ctx: &'a Arc<ServerContext>,
    /// The player's display name.
    pub player_name: &'a str,
    /// The player's UUID.
    pub player_uuid: uuid::Uuid,
    /// The player's remote address.
    pub addr: SocketAddr,
    /// Tracks which chunks the player has loaded.
    pub chunk_tracker: &'a mut PlayerChunkTracker,
    /// Per-player chat rate limiter.
    pub rate_limiter: &'a mut ChatRateLimiter,
}

/// Entity type registry ID for players (`minecraft:player`).
const PLAYER_ENTITY_TYPE_ID: i32 = 155;

/// Converts a float angle (degrees) to the protocol's packed byte format.
///
/// The packed format maps 0–360° to 0–255.
fn pack_angle(degrees: f32) -> u8 {
    ((degrees / 360.0) * 256.0) as u8
}

/// Returns `true` if the packet ID is a movement packet.
fn is_movement_packet(id: i32) -> bool {
    id == ServerboundMovePlayerPosPacket::PACKET_ID
        || id == ServerboundMovePlayerPosRotPacket::PACKET_ID
        || id == ServerboundMovePlayerRotPacket::PACKET_ID
        || id == ServerboundMovePlayerStatusOnlyPacket::PACKET_ID
}

/// Handles the PLAY-state login sequence for a newly joined player.
///
/// Creates a [`ServerPlayer`], loads saved player data (if available),
/// builds the 10-packet login sequence via [`build_login_sequence`], and
/// sends them to the client. Then enters the main play loop to process
/// incoming PLAY packets.
///
/// # Errors
///
/// Returns a [`ConnectionError`] if any I/O or protocol step fails.
pub async fn handle_play_entry(
    conn: &mut Connection,
    profile: auth::GameProfile,
    client_info: ClientInformation,
    ctx: &LoginContext,
) -> Result<(), ConnectionError> {
    let addr = conn.remote_addr();
    let server_ctx = &ctx.server_ctx;

    let uuid = profile.uuid().ok_or_else(|| {
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Profile has invalid UUID",
        ))
    })?;

    // Create ServerPlayer with entity ID from the player list.
    let entity_id = server_ctx.player_list.read().next_entity_id();
    let game_mode = GameMode::from_id(server_ctx.level_data.read().game_type);
    let dimension = ResourceLocation::from_string("minecraft:overworld").map_err(|e| {
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })?;

    let mut player = ServerPlayer::new(entity_id, profile, dimension, game_mode);

    // Apply client preferences, capped to server maximums.
    player.view_distance = i32::from(client_info.view_distance).min(server_ctx.max_view_distance);
    player.simulation_distance =
        i32::from(client_info.view_distance).min(server_ctx.max_simulation_distance);

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
        // New player — spawn at world spawn.
        let (sx, sy, sz) = server_ctx.level_data.read().spawn_pos();
        player.pos =
            oxidized_protocol::types::Vec3::new(f64::from(sx), f64::from(sy), f64::from(sz));
        debug!(peer = %addr, uuid = %uuid, "New player — spawning at world spawn");
    }

    // Assign a teleport ID for the initial position packet.
    let teleport_id = player.next_teleport_id();
    player.pending_teleports.push_back(teleport_id);

    let player_name = player.name.clone();
    let player_view_distance = player.view_distance;
    let player_chunk_x = player.pos.x.floor() as i32;
    let player_chunk_z = player.pos.z.floor() as i32;

    // Build the 10-packet login sequence.
    let dimension_type_id = 0; // overworld = 0 in registry order
    let packets = {
        let level_data = server_ctx.level_data.read();
        let player_list = server_ctx.player_list.read();
        let game_rules = server_ctx.game_rules.read();
        build_login_sequence(
            &player,
            teleport_id,
            &level_data,
            &player_list,
            &server_ctx.dimensions,
            dimension_type_id,
            &game_rules,
        )
    };

    // Send all login packets before adding player to the list.
    // This prevents a "ghost" player entry if sending fails.
    for pkt in &packets {
        conn.send_raw(pkt.id, &pkt.body).await?;
    }
    conn.flush().await?;

    // Send difficulty to the client.
    let difficulty_pkt = {
        let ld = server_ctx.level_data.read();
        ClientboundChangeDifficultyPacket {
            difficulty: ld.difficulty.clamp(0, 3) as u8,
            locked: false,
        }
    };
    conn.send_packet(&difficulty_pkt).await?;

    // Send weather state if it is currently raining.
    let (is_raining, is_thundering) = {
        let ld = server_ctx.level_data.read();
        (ld.is_raining, ld.is_thundering)
    };
    if is_raining {
        conn.send_packet(&ClientboundGameEventPacket {
            event: GameEventType::StartRaining,
            param: 0.0,
        })
        .await?;
        conn.send_packet(&ClientboundGameEventPacket {
            event: GameEventType::RainLevelChange,
            param: 1.0,
        })
        .await?;
        if is_thundering {
            conn.send_packet(&ClientboundGameEventPacket {
                event: GameEventType::ThunderLevelChange,
                param: 1.0,
            })
            .await?;
        }
    }

    // Send default world border state.
    conn.send_packet(&ClientboundInitializeBorderPacket {
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

    // Send chunk cache radius to the client.
    let cache_radius = ClientboundSetChunkCacheRadiusPacket {
        radius: player_view_distance,
    };
    conn.send_packet(&cache_radius).await?;

    // Send the command tree so the client can offer tab-completion.
    {
        let cmd_source = commands::make_command_source(&player_name, uuid, &player, server_ctx);
        let tree = server_ctx.commands.serialize_tree(&cmd_source);
        let cmd_pkt = commands::commands_packet_from_tree(&tree);
        conn.send_packet(&cmd_pkt).await?;
    }

    // Signal the client that chunk loading is about to begin (must be
    // BEFORE chunks are sent — vanilla ordering requirement).
    conn.send_packet(&ClientboundGameEventPacket {
        event: GameEventType::LevelChunksLoadStart,
        param: 0.0,
    })
    .await?;

    // Send initial chunk batch using the world generator.
    let chunk_center = ChunkPos::from_block_coords(player_chunk_x, player_chunk_z);
    let chunk_count = helpers::send_initial_chunks(
        conn,
        chunk_center,
        player_view_distance,
        &server_ctx.chunks,
        server_ctx.chunk_generator.as_ref(),
    )
    .await?;

    info!(
        peer = %addr,
        uuid = %uuid,
        chunks = chunk_count,
        "Initial chunk batch sent",
    );

    // Add player to the server player list (only after successful send).
    let player_arc = server_ctx.player_list.write().add(player);

    // Send the joining player their own tab list entry (the login sequence
    // was built before the player was added to the list, so it's missing).
    {
        let self_info = {
            let p = player_arc.read();
            ClientboundPlayerInfoUpdatePacket {
                actions: PlayerInfoActions(
                    PlayerInfoActions::ADD_PLAYER
                        | PlayerInfoActions::INITIALIZE_CHAT
                        | PlayerInfoActions::UPDATE_GAME_MODE
                        | PlayerInfoActions::UPDATE_LISTED
                        | PlayerInfoActions::UPDATE_LATENCY,
                ),
                entries: vec![PlayerInfoEntry {
                    uuid,
                    name: player_name.clone(),
                    properties: p.profile.properties().to_vec(),
                    game_mode: p.game_mode as i32,
                    latency: 0,
                    listed: true,
                    has_display_name: false,
                    show_hat: false,
                    list_order: 0,
                }],
            }
        };
        conn.send_packet(&self_info).await?;
    }

    // Broadcast the new player to all existing players' tab lists.
    {
        let p = player_arc.read();
        let join_info = ClientboundPlayerInfoUpdatePacket {
            actions: PlayerInfoActions(
                PlayerInfoActions::ADD_PLAYER
                    | PlayerInfoActions::INITIALIZE_CHAT
                    | PlayerInfoActions::UPDATE_GAME_MODE
                    | PlayerInfoActions::UPDATE_LISTED
                    | PlayerInfoActions::UPDATE_LATENCY,
            ),
            entries: vec![PlayerInfoEntry {
                uuid,
                name: player_name.clone(),
                properties: p.profile.properties().to_vec(),
                game_mode: p.game_mode as i32,
                latency: 0,
                listed: true,
                has_display_name: false,
                show_hat: false,
                list_order: 0,
            }],
        };
        drop(p);
        let encoded = join_info.encode();
        let broadcast = BroadcastMessage {
            packet_id: ClientboundPlayerInfoUpdatePacket::PACKET_ID,
            data: encoded.freeze(),
            exclude_entity: Some(entity_id),
            target_entity: None,
        };
        let _ = server_ctx.broadcast_tx.send(broadcast);
    }

    // Broadcast the new player's entity to all existing players, and send
    // all existing players' entities to the joining player.
    {
        let add_entity = {
            let p = player_arc.read();
            ClientboundAddEntityPacket {
                entity_id,
                uuid,
                entity_type: PLAYER_ENTITY_TYPE_ID,
                x: p.pos.x,
                y: p.pos.y,
                z: p.pos.z,
                vx: 0.0,
                vy: 0.0,
                vz: 0.0,
                x_rot: pack_angle(p.pitch),
                y_rot: pack_angle(p.yaw),
                y_head_rot: pack_angle(p.yaw),
                data: 0,
            }
        };

        // Broadcast new player entity to existing players.
        let encoded = add_entity.encode();
        let broadcast = BroadcastMessage {
            packet_id: ClientboundAddEntityPacket::PACKET_ID,
            data: encoded.freeze(),
            exclude_entity: Some(entity_id),
            target_entity: None,
        };
        let _ = server_ctx.broadcast_tx.send(broadcast);

        // Collect existing players' entity packets (no locks held across await).
        let other_entities: Vec<ClientboundAddEntityPacket> = {
            let player_list = server_ctx.player_list.read();
            player_list
                .iter()
                .filter_map(|other_arc| {
                    let other = other_arc.read();
                    if other.entity_id == entity_id {
                        return None;
                    }
                    Some(ClientboundAddEntityPacket {
                        entity_id: other.entity_id,
                        uuid: other.uuid,
                        entity_type: PLAYER_ENTITY_TYPE_ID,
                        x: other.pos.x,
                        y: other.pos.y,
                        z: other.pos.z,
                        vx: 0.0,
                        vy: 0.0,
                        vz: 0.0,
                        x_rot: pack_angle(other.pitch),
                        y_rot: pack_angle(other.yaw),
                        y_head_rot: pack_angle(other.yaw),
                        data: 0,
                    })
                })
                .collect()
        };

        // Send existing player entities to the joining player.
        for pkt in &other_entities {
            conn.send_packet(pkt).await?;
        }
    }

    // Track which chunks the player has loaded.
    let initial_chunk = ChunkPos::from_block_coords(player_chunk_x, player_chunk_z);
    let mut chunk_tracker = PlayerChunkTracker::new(initial_chunk, player_view_distance);

    // Subscribe to broadcast channel.
    let mut broadcast_rx = server_ctx.broadcast_tx.subscribe();

    // Per-player chat rate limiter.
    let mut rate_limiter = ChatRateLimiter::new();

    info!(
        peer = %addr,
        uuid = %uuid,
        name = %player_name,
        entity_id = entity_id,
        packets = packets.len(),
        "PLAY login sequence sent",
    );

    // Keepalive state — send a ping every 15 seconds, timeout after 30.
    const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
    const KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(30);
    let mut keepalive_timer = tokio::time::interval(KEEPALIVE_INTERVAL);
    keepalive_timer.tick().await; // consume the immediate first tick
    let mut keepalive_pending = false;
    let mut keepalive_challenge: i64 = 0;
    let mut keepalive_sent_at = Instant::now();

    // Build the play context for handler dispatch.
    let mut play_ctx = PlayContext {
        conn,
        player: &player_arc,
        server_ctx,
        player_name: &player_name,
        player_uuid: uuid,
        addr,
        chunk_tracker: &mut chunk_tracker,
        rate_limiter: &mut rate_limiter,
    };

    // PLAY read loop — dispatches packets to handler functions.
    loop {
        tokio::select! {
            // Keepalive timer — send a ping every 15 seconds.
            _ = keepalive_timer.tick() => {
                if keepalive_pending {
                    let elapsed = keepalive_sent_at.elapsed();
                    if elapsed >= KEEPALIVE_TIMEOUT {
                        info!(peer = %addr, name = %player_name, "Keepalive timeout — disconnecting");
                        break;
                    }
                } else {
                    keepalive_challenge = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64;
                    keepalive_pending = true;
                    keepalive_sent_at = Instant::now();
                    let pkt = ClientboundKeepAlivePacket { id: keepalive_challenge };
                    if let Err(e) = play_ctx.conn.send_packet(&pkt).await {
                        debug!(peer = %addr, name = %player_name, error = %e, "Failed to send keepalive");
                        break;
                    }
                }
            },
            // Receive broadcast packets from other systems (chat, block updates, etc.).
            broadcast_result = broadcast_rx.recv() => {
                match broadcast_result {
                    Ok(msg) => {
                        let my_entity_id = play_ctx.player.read().entity_id;
                        // Skip if this broadcast targets a different player.
                        if let Some(target_id) = msg.target_entity {
                            if target_id != my_entity_id {
                                continue;
                            }
                        }
                        // Skip if this broadcast excludes the current player.
                        if let Some(exclude_id) = msg.exclude_entity {
                            if exclude_id == my_entity_id {
                                continue;
                            }
                        }
                        if let Err(e) = play_ctx.conn.send_raw(msg.packet_id, &msg.data).await {
                            debug!(peer = %addr, name = %player_name, error = %e, "Failed to send broadcast");
                            break;
                        }
                        if let Err(e) = play_ctx.conn.flush().await {
                            debug!(peer = %addr, name = %player_name, error = %e, "Failed to flush broadcast");
                            break;
                        }
                    },
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        debug!(peer = %addr, name = %player_name, missed = n, "Broadcast lagged — dropped messages");
                    },
                    Err(broadcast::error::RecvError::Closed) => {
                        debug!(peer = %addr, name = %player_name, "Broadcast channel closed");
                        break;
                    },
                }
            },
            // Read incoming packets from this client.
            packet_result = play_ctx.conn.read_raw_packet() => {
                match packet_result {
                    Ok(pkt) => {
                        match pkt.id {
                            ServerboundKeepAlivePacket::PACKET_ID => {
                                handle_keepalive(
                                    pkt.data, addr, &player_name,
                                    &mut keepalive_pending, keepalive_challenge, &keepalive_sent_at,
                                );
                            },
                            id if is_movement_packet(id) => {
                                movement::handle_movement(&mut play_ctx, id, pkt.data).await?;
                            },
                            ServerboundAcceptTeleportationPacket::PACKET_ID => {
                                handle_accept_teleport(
                                    &play_ctx, pkt.data,
                                );
                            },
                            ServerboundChunkBatchReceivedPacket::PACKET_ID => {
                                handle_chunk_batch_ack(&play_ctx, pkt.data);
                            },
                            ServerboundPlayerCommandPacket::PACKET_ID => {
                                handle_player_command(&play_ctx, pkt.data);
                            },
                            ServerboundPlayerInputPacket::PACKET_ID => {
                                handle_player_input(&play_ctx, pkt.data);
                            },
                            ServerboundChatPacket::PACKET_ID => {
                                if let Ok(chat_pkt) = decode_packet::<ServerboundChatPacket>(
                                    pkt.data,
                                    addr, &player_name, "Chat",
                                ) {
                                    let msg = chat_pkt.message.clone();
                                    chat::handle_chat(&mut play_ctx, &msg).await?;
                                }
                            },
                            ServerboundChatCommandPacket::PACKET_ID => {
                                if let Ok(cmd_pkt) = decode_packet::<ServerboundChatCommandPacket>(
                                    pkt.data,
                                    addr, &player_name, "ChatCommand",
                                ) {
                                    if !play_ctx.rate_limiter.try_acquire(std::time::Instant::now()) {
                                        warn!(peer = %addr, name = %player_name, "Command rate-limited");
                                    } else {
                                        let (pos, rot) = {
                                            let p = play_ctx.player.read();
                                            ((p.pos.x, p.pos.y, p.pos.z), (p.yaw, p.pitch))
                                        };
                                        chat::handle_chat_command(
                                            play_ctx.conn,
                                            &cmd_pkt.command,
                                            &player_name,
                                            uuid,
                                            pos, rot,
                                            4,
                                            server_ctx,
                                        ).await;
                                    }
                                }
                            },
                            ServerboundChatCommandSignedPacket::PACKET_ID => {
                                if let Ok(cmd_pkt) = decode_packet::<ServerboundChatCommandSignedPacket>(
                                    pkt.data,
                                    addr, &player_name, "ChatCommandSigned",
                                ) {
                                    if !play_ctx.rate_limiter.try_acquire(std::time::Instant::now()) {
                                        warn!(peer = %addr, name = %player_name, "Command rate-limited (signed)");
                                    } else {
                                        let (pos, rot) = {
                                            let p = play_ctx.player.read();
                                            ((p.pos.x, p.pos.y, p.pos.z), (p.yaw, p.pitch))
                                        };
                                        chat::handle_chat_command(
                                            play_ctx.conn,
                                            &cmd_pkt.command,
                                            &player_name,
                                            uuid,
                                            pos, rot,
                                            4,
                                            server_ctx,
                                        ).await;
                                    }
                                }
                            },
                            ServerboundCommandSuggestionPacket::PACKET_ID => {
                                commands::handle_command_suggestion(&mut play_ctx, pkt.data).await?;
                            },
                            ServerboundSetCarriedItemPacket::PACKET_ID => {
                                inventory::handle_set_carried_item(&mut play_ctx, pkt.data).await?;
                            },
                            ServerboundSetCreativeModeSlotPacket::PACKET_ID => {
                                inventory::handle_set_creative_mode_slot(&mut play_ctx, pkt.data).await?;
                            },
                            ServerboundPlayerActionPacket::PACKET_ID => {
                                block_interaction::handle_player_action(&mut play_ctx, pkt.data).await?;
                            },
                            ServerboundUseItemOnPacket::PACKET_ID => {
                                block_interaction::handle_use_item_on(&mut play_ctx, pkt.data).await?;
                            },
                            ServerboundUseItemPacket::PACKET_ID => {
                                block_interaction::handle_use_item(&mut play_ctx, pkt.data).await?;
                            },
                            ServerboundSignUpdatePacket::PACKET_ID => {
                                block_interaction::handle_sign_update(&mut play_ctx, pkt.data).await?;
                            },
                            ServerboundPickItemFromBlockPacket::PACKET_ID => {
                                block_interaction::handle_pick_item_from_block(&mut play_ctx, pkt.data).await?;
                            },
                            unknown if !(0..=MAX_SERVERBOUND_PLAY_ID).contains(&unknown) => {
                                warn!(
                                    peer = %addr,
                                    name = %player_name,
                                    packet_id = format_args!("0x{:02X}", unknown),
                                    size = pkt.data.len(),
                                    "Unknown or unregistered packet from client",
                                );
                            },
                            _unhandled => {
                                debug!(
                                    peer = %addr,
                                    name = %player_name,
                                    packet_id = format_args!("0x{:02X}", pkt.id),
                                    size = pkt.data.len(),
                                    "PLAY packet (unhandled)",
                                );
                            },
                        }
                    },
                    Err(ConnectionError::Io(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                        info!(peer = %addr, name = %player_name, "Player disconnected");
                        break;
                    },
                    Err(e) => {
                        warn!(peer = %addr, name = %player_name, error = %e, "PLAY connection error");
                        break;
                    },
                }
            },
        }
    }

    // Clean up — remove player from the list and broadcast removal to tab lists.
    server_ctx.player_list.write().remove(&uuid);
    {
        let remove_pkt = ClientboundPlayerInfoRemovePacket { uuids: vec![uuid] };
        let encoded = remove_pkt.encode();
        let broadcast = BroadcastMessage {
            packet_id: ClientboundPlayerInfoRemovePacket::PACKET_ID,
            data: encoded.freeze(),
            exclude_entity: None,
            target_entity: None,
        };
        let _ = server_ctx.broadcast_tx.send(broadcast);
    }
    // Broadcast entity removal so other players stop rendering this player.
    {
        let remove_entity = ClientboundRemoveEntitiesPacket {
            entity_ids: vec![entity_id],
        };
        let encoded = remove_entity.encode();
        let broadcast = BroadcastMessage {
            packet_id: ClientboundRemoveEntitiesPacket::PACKET_ID,
            data: encoded.freeze(),
            exclude_entity: None,
            target_entity: None,
        };
        let _ = server_ctx.broadcast_tx.send(broadcast);
    }
    info!(peer = %addr, uuid = %uuid, name = %player_name, "Player removed from player list");

    Ok(())
}

/// Handles a keepalive response from the client.
fn handle_keepalive(
    data: bytes::Bytes,
    addr: SocketAddr,
    player_name: &str,
    keepalive_pending: &mut bool,
    keepalive_challenge: i64,
    keepalive_sent_at: &Instant,
) {
    if let Ok(ka) =
        decode_packet::<ServerboundKeepAlivePacket>(data, addr, player_name, "KeepAlive")
    {
        if *keepalive_pending && ka.id == keepalive_challenge {
            *keepalive_pending = false;
            let latency = keepalive_sent_at.elapsed().as_millis();
            debug!(peer = %addr, name = %player_name, latency_ms = latency, "Keepalive response");
        } else {
            debug!(
                peer = %addr,
                name = %player_name,
                expected = keepalive_challenge,
                got = ka.id,
                pending = *keepalive_pending,
                "Unexpected keepalive response",
            );
        }
    }
}

/// Handles a teleport confirmation from the client.
fn handle_accept_teleport(ctx: &PlayContext<'_>, data: bytes::Bytes) {
    if let Ok(ack) = decode_packet::<ServerboundAcceptTeleportationPacket>(
        data,
        ctx.addr,
        ctx.player_name,
        "AcceptTeleportation",
    ) {
        let mut p = ctx.player.write();
        let accepted = handle_accept_teleportation(&mut p, ack.teleport_id);
        debug!(
            peer = %ctx.addr,
            name = %ctx.player_name,
            teleport_id = ack.teleport_id,
            accepted = accepted,
            pending = p.pending_teleports.len(),
            "Teleport confirmation",
        );
    }
}

/// Handles a chunk batch acknowledgement from the client.
fn handle_chunk_batch_ack(ctx: &PlayContext<'_>, data: bytes::Bytes) {
    if let Ok(batch_ack) = decode_packet::<ServerboundChunkBatchReceivedPacket>(
        data,
        ctx.addr,
        ctx.player_name,
        "ChunkBatchReceived",
    ) {
        let rate = batch_ack.desired_chunks_per_tick;
        if rate.is_finite() && rate > 0.0 {
            ctx.player.write().chunk_send_rate = rate.clamp(0.1, 100.0);
            debug!(peer = %ctx.addr, name = %ctx.player_name, rate, "Chunk batch rate update");
        } else {
            debug!(
                peer = %ctx.addr,
                name = %ctx.player_name,
                invalid_rate = rate,
                "Ignored invalid chunk send rate",
            );
        }
    }
}

/// Handles player command packets (sprint, sneak, etc.).
fn handle_player_command(ctx: &PlayContext<'_>, data: bytes::Bytes) {
    if let Ok(cmd) = decode_packet::<ServerboundPlayerCommandPacket>(
        data,
        ctx.addr,
        ctx.player_name,
        "PlayerCommand",
    ) {
        match cmd.action {
            PlayerCommandAction::StartSprinting => {
                ctx.player.write().sprinting = true;
                debug!(peer = %ctx.addr, name = %ctx.player_name, "Player started sprinting");
            },
            PlayerCommandAction::StopSprinting => {
                ctx.player.write().sprinting = false;
                debug!(peer = %ctx.addr, name = %ctx.player_name, "Player stopped sprinting");
            },
            _ => {
                debug!(
                    peer = %ctx.addr,
                    name = %ctx.player_name,
                    action = ?cmd.action,
                    "Player command (unhandled action)",
                );
            },
        }
    }
}

/// Handles player input packets (shift, sprint flags).
fn handle_player_input(ctx: &PlayContext<'_>, data: bytes::Bytes) {
    if let Ok(input_pkt) = decode_packet::<ServerboundPlayerInputPacket>(
        data,
        ctx.addr,
        ctx.player_name,
        "PlayerInput",
    ) {
        let mut p = ctx.player.write();
        p.sneaking = input_pkt.input.shift;
        p.sprinting = input_pkt.input.sprint;
    }
}
