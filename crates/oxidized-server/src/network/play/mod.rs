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
mod entity_tracking;
pub mod helpers;
pub mod inventory;
mod join;
mod keepalive;
pub mod mining;
pub mod movement;
pub mod pick_block;
pub mod placement;
pub mod sign_editing;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use oxidized_game::chat::ChatRateLimiter;
use oxidized_game::chunk::chunk_tracker::PlayerChunkTracker;
use oxidized_game::player::{ServerPlayer, handle_accept_teleportation};

use oxidized_chat::Component;
use oxidized_codec::Packet;
use oxidized_codec::slot::SlotData;
use oxidized_protocol::constants::MILLIS_PER_TICK;
use oxidized_protocol::packets::configuration::ClientInformation;
use oxidized_protocol::packets::play::{
    ClientboundAnimatePacket, ClientboundKeepAlivePacket, ClientboundPlayerInfoRemovePacket,
    ClientboundPlayerInfoUpdatePacket, ClientboundPlayerPositionPacket,
    ClientboundRemoveEntitiesPacket, ClientboundSetEntityDataPacket, ClientboundSetEquipmentPacket,
    ClientboundSystemChatPacket, PlayerInfoActions, PlayerInfoEntry, RelativeFlags,
    ServerboundAcceptTeleportationPacket, ServerboundChatCommandPacket,
    ServerboundChatCommandSignedPacket, ServerboundChatPacket, ServerboundChunkBatchReceivedPacket,
    ServerboundClientInformationPlayPacket, ServerboundCommandSuggestionPacket,
    ServerboundKeepAlivePacket, ServerboundMovePlayerPosPacket, ServerboundMovePlayerPosRotPacket,
    ServerboundMovePlayerRotPacket, ServerboundMovePlayerStatusOnlyPacket,
    ServerboundPickItemFromBlockPacket, ServerboundPlayerAbilitiesPacket,
    ServerboundPlayerActionPacket, ServerboundPlayerCommandPacket, ServerboundPlayerInputPacket,
    ServerboundSetCarriedItemPacket, ServerboundSetCreativeModeSlotPacket,
    ServerboundSignUpdatePacket, ServerboundSwingPacket, ServerboundUseItemOnPacket,
    ServerboundUseItemPacket, equipment_slot,
};
use oxidized_transport::connection::{Connection, ConnectionError};
use oxidized_transport::handle::ConnectionHandle;
use oxidized_types::ChunkPos;
use parking_lot::RwLock;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, info, warn};

use crate::network::reader::reader_loop;
use crate::network::writer::writer_loop;
use crate::network::{BroadcastMessage, LoginContext, MAX_SERVERBOUND_PLAY_ID, ServerContext};

use crate::network::helpers::decode_packet;

/// Shared context passed to all play-state packet handlers.
pub struct PlayContext<'a> {
    /// Handle to the connection's outbound channel (task pair model).
    pub conn_handle: &'a ConnectionHandle,
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
    /// Channel to send entity commands to the tick thread.
    pub entity_cmd_tx: &'a oxidized_game::entity::commands::EntityCommandSender,
}

/// Entity type registry ID for players (`minecraft:player`).
const PLAYER_ENTITY_TYPE_ID: i32 = 155;

/// Entity metadata slot for displayed skin parts (cape, jacket, sleeves, etc.).
///
/// Vanilla hierarchy: Entity(0–7), LivingEntity(8–14), Avatar(15–16), Player(17–20).
/// `DATA_PLAYER_MODE_CUSTOMISATION` is Avatar index 16.
const DATA_PLAYER_MODE_CUSTOMISATION: u8 = 16;

/// Bitmask for the hat model part (bit 6) in the model customisation byte.
const HAT_MASK: u8 = 1 << 6;

/// Converts a float angle (degrees) to the protocol's packed byte format.
///
/// The packed format maps 0–360° to 0–255, with proper wrapping for
/// negative angles and values above 360°.
pub(crate) fn pack_angle(degrees: f32) -> u8 {
    oxidized_game::net::entity_movement::pack_degrees(degrees)
}

/// Returns `true` if the packet ID is a movement packet.
fn is_movement_packet(id: i32) -> bool {
    id == ServerboundMovePlayerPosPacket::PACKET_ID
        || id == ServerboundMovePlayerPosRotPacket::PACKET_ID
        || id == ServerboundMovePlayerRotPacket::PACKET_ID
        || id == ServerboundMovePlayerStatusOnlyPacket::PACKET_ID
}

/// Builds a [`ClientboundSetEquipmentPacket`] for the given player.
///
/// Includes main hand, off hand, and all four armor slots. Empty slots
/// are included so the client clears any stale equipment.
fn build_equipment_packet(player: &ServerPlayer) -> ClientboundSetEquipmentPacket {
    use inventory::item_stack_to_slot_data;

    let inv = &player.inventory;
    let to_slot = |stack: &oxidized_inventory::ItemStack| -> Option<SlotData> {
        if stack.is_empty() {
            None
        } else {
            Some(item_stack_to_slot_data(stack))
        }
    };

    ClientboundSetEquipmentPacket {
        entity_id: player.entity_id,
        equipments: vec![
            (equipment_slot::MAIN_HAND, to_slot(inv.get_selected())),
            (equipment_slot::OFF_HAND, to_slot(inv.get_offhand())),
            (equipment_slot::FEET, to_slot(inv.get_armor(3))),
            (equipment_slot::LEGS, to_slot(inv.get_armor(2))),
            (equipment_slot::CHEST, to_slot(inv.get_armor(1))),
            (equipment_slot::HEAD, to_slot(inv.get_armor(0))),
        ],
    }
}

/// Handles the PLAY-state login sequence for a newly joined player.
///
/// Splits the [`Connection`] into a per-connection reader/writer task pair,
/// creates bounded inbound/outbound channels, spawns the reader and
/// writer tasks, sends the join sequence through the outbound channel,
/// and enters the main play loop to process inbound packets.
///
/// # Errors
///
/// Returns a [`ConnectionError`] if any I/O or protocol step fails.
pub async fn handle_play_split(
    conn: Connection,
    profile: oxidized_auth::GameProfile,
    client_info: ClientInformation,
    ctx: &LoginContext,
) -> Result<(), ConnectionError> {
    let server_ctx = &ctx.server_ctx;
    let addr = conn.remote_addr();

    // Split connection into reader/writer halves.
    let (reader, writer) = conn.into_split();

    // Create bounded inbound/outbound channels with configurable capacity.
    let (inbound_tx, mut inbound_rx) = mpsc::channel(server_ctx.settings.inbound_channel_capacity);
    let (outbound_tx, outbound_rx) = mpsc::channel(server_ctx.settings.outbound_channel_capacity);

    // Spawn reader and writer tasks.
    let reader_handle = tokio::spawn(reader_loop(reader, inbound_tx));
    let write_timeout = Duration::from_secs(server_ctx.settings.timeouts.write_timeout_secs);
    let writer_handle = tokio::spawn(writer_loop(writer, outbound_rx, write_timeout));

    // Create the connection handle for sending packets.
    let conn_handle = ConnectionHandle::new(outbound_tx, addr);

    // Send join sequence through the outbound channel.
    let join_state =
        join::send_join_sequence(&conn_handle, profile, client_info, server_ctx).await?;
    let player_arc = join_state.player;
    let player_name = join_state.name;
    let uuid = join_state.uuid;
    let entity_id = join_state.entity_id;

    // Send SpawnPlayer command to the tick thread's ECS world via the entity command channel.
    {
        use oxidized_game::entity::commands::EntityCommand;
        use oxidized_game::entity::components::{ExperienceData, SpawnData};
        use oxidized_mc_types::BlockPos;

        let p = player_arc.read();
        let _ = ctx.entity_cmd_tx.try_send(EntityCommand::SpawnPlayer {
            network_id: entity_id,
            uuid,
            profile: p.profile.clone(),
            position: glam::DVec3::new(p.movement.pos.x, p.movement.pos.y, p.movement.pos.z),
            rotation: (p.movement.yaw, p.movement.pitch),
            game_mode: p.game_mode,
            inventory: Box::new(p.inventory.clone()),
            health: p.combat.health,
            food_level: p.combat.food_level,
            experience: ExperienceData {
                level: p.experience.xp_level,
                progress: p.experience.xp_progress,
                total: p.experience.xp_total,
            },
            spawn_data: SpawnData {
                dimension: p.spawn.dimension.clone(),
                spawn_pos: BlockPos::new(
                    p.spawn.spawn_pos.x,
                    p.spawn.spawn_pos.y,
                    p.spawn.spawn_pos.z,
                ),
                spawn_angle: p.spawn.spawn_angle,
            },
        });
    }

    // Track which chunks the player has loaded.
    let initial_chunk =
        ChunkPos::from_block_coords(join_state.initial_chunk_x, join_state.initial_chunk_z);
    let mut chunk_tracker = PlayerChunkTracker::new(initial_chunk, join_state.view_distance);

    // Subscribe to broadcast channel.
    let mut broadcast_rx = server_ctx.network.broadcast_tx.subscribe();

    // Register a per-player kick channel for remote disconnect signaling.
    let (kick_tx, mut kick_rx) = tokio::sync::mpsc::channel::<String>(1);
    server_ctx.network.kick_channels.insert(uuid, kick_tx);

    // Per-player chat rate limiter.
    let mut rate_limiter = ChatRateLimiter::new();

    // Keepalive state — configurable ping interval and timeout.
    let keepalive_interval =
        Duration::from_secs(server_ctx.settings.timeouts.keepalive_interval_secs);
    let keepalive_timeout =
        Duration::from_secs(server_ctx.settings.timeouts.keepalive_timeout_secs);
    // Vanilla resends unconfirmed teleports every 20 ticks (~1 second).
    const TELEPORT_RESEND_INTERVAL: Duration = Duration::from_secs(1);
    let mut keepalive_timer = tokio::time::interval(keepalive_interval);
    keepalive_timer.tick().await; // consume the immediate first tick
    let mut keepalive_pending = false;
    let mut keepalive_challenge: i64 = 0;
    let mut keepalive_sent_at = Instant::now();

    // Per-tick timer for chat rate limiter decay (50 ms = 1 game tick).
    let mut tick_timer = tokio::time::interval(Duration::from_millis(MILLIS_PER_TICK));
    tick_timer.tick().await;

    // Build the play context for handler dispatch.
    let mut play_ctx = PlayContext {
        conn_handle: &conn_handle,
        player: &player_arc,
        server_ctx,
        player_name: &player_name,
        player_uuid: uuid,
        addr,
        chunk_tracker: &mut chunk_tracker,
        rate_limiter: &mut rate_limiter,
        entity_cmd_tx: &ctx.entity_cmd_tx,
    };

    // PLAY read loop — reads from the bounded inbound channel fed by the reader task.
    loop {
        tokio::select! {
            // Kick signal from another task (e.g., duplicate login replacement).
            reason = kick_rx.recv() => {
                let reason = reason.unwrap_or_else(|| "Kicked".to_string());
                info!(peer = %addr, name = %player_name, %reason, "Player kicked");
                let _ = crate::network::helpers::disconnect_handle(
                    play_ctx.conn_handle, &reason,
                ).await;
                break;
            },
            // Keepalive timer — send a ping every 15 seconds.
            _ = keepalive_timer.tick() => {
                if keepalive_pending {
                    let elapsed = keepalive_sent_at.elapsed();
                    if elapsed >= keepalive_timeout {
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
                    if let Err(e) = play_ctx.conn_handle.send_packet(&pkt).await {
                        debug!(peer = %addr, name = %player_name, error = %e, "Failed to send keepalive");
                        break;
                    }
                }
            },
            // Per-tick decay for chat rate limiter + teleport resend.
            _ = tick_timer.tick() => {
                play_ctx.rate_limiter.tick();

                // Resend unconfirmed teleports older than 1 second (vanilla: 20 ticks).
                let resend_list: Vec<(i32, f64, f64, f64, f32, f32)> = {
                    let now = Instant::now();
                    let mut p = play_ctx.player.write();
                    let yaw = p.movement.yaw;
                    let pitch = p.movement.pitch;
                    let mut list = Vec::new();
                    for entry in p.teleport.pending.iter_mut() {
                        if now.duration_since(entry.2) >= TELEPORT_RESEND_INTERVAL {
                            list.push((entry.0, entry.1.x, entry.1.y, entry.1.z, yaw, pitch));
                            entry.2 = now;
                        }
                    }
                    list
                };
                let mut resend_failed = false;
                for (tid, x, y, z, yaw, pitch) in &resend_list {
                    let pkt = ClientboundPlayerPositionPacket {
                        teleport_id: *tid,
                        x: *x,
                        y: *y,
                        z: *z,
                        dx: 0.0,
                        dy: 0.0,
                        dz: 0.0,
                        yaw: *yaw,
                        pitch: *pitch,
                        relative_flags: RelativeFlags::empty(),
                    };
                    if let Err(e) = play_ctx.conn_handle.send_packet(&pkt).await {
                        debug!(peer = %addr, name = %player_name, error = %e, "Failed to resend teleport");
                        resend_failed = true;
                        break;
                    }
                    debug!(peer = %addr, name = %player_name, teleport_id = tid, "Resent unconfirmed teleport");
                }
                if resend_failed {
                    break;
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
                        // Queue on outbound channel — writer batches with other packets.
                        if conn_handle.try_send_raw(msg.packet_id, msg.data).is_err() {
                            warn!(peer = %addr, name = %player_name, "Outbound channel full on broadcast");
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
            // Read incoming packets dispatched by the reader task via the inbound channel.
            inbound = inbound_rx.recv() => {
                match inbound {
                    Some(pkt) => {
                        match pkt.id {
                            ServerboundKeepAlivePacket::PACKET_ID => {
                                match handle_keepalive_response(
                                    pkt.data, addr, &player_name,
                                    &mut keepalive_pending, keepalive_challenge, &keepalive_sent_at,
                                    &player_arc, server_ctx,
                                ) {
                                    KeepaliveAction::Continue => {},
                                    KeepaliveAction::Disconnect => break,
                                }
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
                                entity_tracking::handle_player_command(&play_ctx, pkt.data);
                            },
                            ServerboundPlayerInputPacket::PACKET_ID => {
                                entity_tracking::handle_player_input(&play_ctx, pkt.data);
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
                                    dispatch_chat_command(
                                        &mut play_ctx, &cmd_pkt.command, &player_name, uuid, server_ctx,
                                    ).await;
                                }
                            },
                            ServerboundChatCommandSignedPacket::PACKET_ID => {
                                if let Ok(cmd_pkt) = decode_packet::<ServerboundChatCommandSignedPacket>(
                                    pkt.data,
                                    addr, &player_name, "ChatCommandSigned",
                                ) {
                                    dispatch_chat_command(
                                        &mut play_ctx, &cmd_pkt.command, &player_name, uuid, server_ctx,
                                    ).await;
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
                            ServerboundSwingPacket::PACKET_ID => {
                                handle_swing_packet(
                                    pkt.data, &play_ctx, server_ctx,
                                );
                            },
                            ServerboundPlayerAbilitiesPacket::PACKET_ID => {
                                handle_abilities_packet(pkt.data, &play_ctx);
                            },
                            ServerboundClientInformationPlayPacket::PACKET_ID => {
                                if handle_client_information_packet(
                                    pkt.data, &mut play_ctx, entity_id, server_ctx,
                                ).await.is_err() {
                                    break;
                                }
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
                    // Inbound channel closed — reader task exited (client disconnect or error).
                    None => {
                        info!(peer = %addr, name = %player_name, "Player disconnected (reader closed)");
                        break;
                    },
                }
            },
        }
    }

    // Player disconnected or was kicked — clean up.
    // Send DespawnPlayer command to the tick thread's ECS world.
    let _ = ctx
        .entity_cmd_tx
        .try_send(oxidized_game::entity::commands::EntityCommand::DespawnPlayer { uuid });

    cleanup_disconnected_player(
        &conn_handle,
        &player_arc,
        &player_name,
        uuid,
        entity_id,
        addr,
        server_ctx,
    )
    .await;

    // Drop outbound sender → writer task sees channel closed → exits.
    drop(conn_handle);

    // Wait for reader and writer tasks to finish.
    let _ = reader_handle.await;
    let _ = writer_handle.await;

    Ok(())
}

/// Result of processing a keepalive response.
enum KeepaliveAction {
    /// Keepalive accepted or decode error (continue loop).
    Continue,
    /// Keepalive mismatch (disconnect).
    Disconnect,
}

/// Processes a keepalive response and broadcasts the latency update.
fn handle_keepalive_response(
    data: bytes::Bytes,
    addr: SocketAddr,
    player_name: &str,
    keepalive_pending: &mut bool,
    keepalive_challenge: i64,
    keepalive_sent_at: &Instant,
    player: &Arc<RwLock<ServerPlayer>>,
    server_ctx: &Arc<ServerContext>,
) -> KeepaliveAction {
    match keepalive::handle_keepalive(
        data,
        addr,
        player_name,
        keepalive_pending,
        keepalive_challenge,
        keepalive_sent_at,
        player,
    ) {
        keepalive::KeepaliveResult::Ok(uuid, latency) => {
            let latency_update = ClientboundPlayerInfoUpdatePacket {
                actions: PlayerInfoActions(PlayerInfoActions::UPDATE_LATENCY),
                entries: vec![PlayerInfoEntry {
                    uuid,
                    latency,
                    ..Default::default()
                }],
            };
            let encoded = latency_update.encode();
            server_ctx.broadcast(BroadcastMessage {
                packet_id: ClientboundPlayerInfoUpdatePacket::PACKET_ID,
                data: encoded.into(),
                exclude_entity: None,
                target_entity: None,
            });
            KeepaliveAction::Continue
        },
        keepalive::KeepaliveResult::Mismatch => {
            info!(peer = %addr, name = %player_name, "Keepalive mismatch — disconnecting");
            KeepaliveAction::Disconnect
        },
        keepalive::KeepaliveResult::DecodeError => KeepaliveAction::Continue,
    }
}

/// Rate-limits and dispatches a chat command (shared by signed and unsigned variants).
async fn dispatch_chat_command(
    play_ctx: &mut PlayContext<'_>,
    command: &str,
    player_name: &str,
    uuid: uuid::Uuid,
    server_ctx: &Arc<ServerContext>,
) {
    if !play_ctx.rate_limiter.try_acquire() {
        warn!(peer = %play_ctx.addr, name = %player_name, "Command rate-limited");
        return;
    }
    let (pos, rot) = {
        let p = play_ctx.player.read();
        (
            (p.movement.pos.x, p.movement.pos.y, p.movement.pos.z),
            (p.movement.yaw, p.movement.pitch),
        )
    };
    let perm_level = server_ctx.ops.get_permission_level(&uuid).clamp(0, 4) as u32;
    chat::handle_chat_command(
        play_ctx.conn_handle,
        command,
        player_name,
        uuid,
        pos,
        rot,
        perm_level,
        server_ctx,
    )
    .await;
}

/// Decodes and broadcasts a swing animation to other players.
fn handle_swing_packet(data: bytes::Bytes, ctx: &PlayContext<'_>, server_ctx: &Arc<ServerContext>) {
    let Ok(swing) =
        decode_packet::<ServerboundSwingPacket>(data, ctx.addr, ctx.player_name, "Swing")
    else {
        return;
    };
    let entity_id = ctx.player.read().entity_id;
    let action = if swing.hand == 1 { 3u8 } else { 0u8 };
    let animate = ClientboundAnimatePacket { entity_id, action };
    let encoded = animate.encode();
    server_ctx.broadcast(BroadcastMessage {
        packet_id: ClientboundAnimatePacket::PACKET_ID,
        data: encoded.into(),
        exclude_entity: Some(entity_id),
        target_entity: None,
    });
}

/// Updates the player's flying state from a client abilities packet.
fn handle_abilities_packet(data: bytes::Bytes, ctx: &PlayContext<'_>) {
    let Ok(pkt) = decode_packet::<ServerboundPlayerAbilitiesPacket>(
        data,
        ctx.addr,
        ctx.player_name,
        "PlayerAbilities",
    ) else {
        return;
    };
    let mut p = ctx.player.write();
    p.abilities.is_flying = pkt.is_flying() && p.abilities.can_fly;
    debug!(
        peer = %ctx.addr,
        name = %ctx.player_name,
        is_flying = p.abilities.is_flying,
        "Player abilities update",
    );
}

/// Handles client information updates (view distance, skin customisation).
///
/// Returns `Err(())` if sending chunk updates failed and the caller should break.
async fn handle_client_information_packet(
    data: bytes::Bytes,
    play_ctx: &mut PlayContext<'_>,
    entity_id: i32,
    server_ctx: &Arc<ServerContext>,
) -> Result<(), ()> {
    let Ok(info_pkt) = decode_packet::<ServerboundClientInformationPlayPacket>(
        data,
        play_ctx.addr,
        play_ctx.player_name,
        "ClientInformation",
    ) else {
        return Ok(());
    };

    let new_view_distance = i32::from(info_pkt.information.view_distance)
        .clamp(2, server_ctx.settings.max_view_distance);
    let (old_view_distance, skin_changed, hat_changed) = {
        let mut p = play_ctx.player.write();
        let old_vd = p.connection.view_distance;
        p.connection.view_distance = new_view_distance;
        let old_skin = p.connection.model_customisation;
        p.connection.model_customisation = info_pkt.information.model_customisation;
        let new_skin = p.connection.model_customisation;
        (
            old_vd,
            old_skin != new_skin,
            (old_skin & HAT_MASK) != (new_skin & HAT_MASK),
        )
    };

    if skin_changed {
        let skin = play_ctx.player.read().connection.model_customisation;
        let skin_pkt = ClientboundSetEntityDataPacket::single_byte(
            entity_id,
            DATA_PLAYER_MODE_CUSTOMISATION,
            skin,
        );
        let encoded = skin_pkt.encode();
        server_ctx.broadcast(BroadcastMessage {
            packet_id: ClientboundSetEntityDataPacket::PACKET_ID,
            data: encoded.freeze(),
            exclude_entity: Some(entity_id),
            target_entity: None,
        });

        // Broadcast hat visibility update to tab list (vanilla parity).
        if hat_changed {
            let hat_update = ClientboundPlayerInfoUpdatePacket {
                actions: PlayerInfoActions(PlayerInfoActions::UPDATE_HAT),
                entries: vec![PlayerInfoEntry {
                    uuid: play_ctx.player_uuid,
                    is_hat_visible: (skin & HAT_MASK) != 0,
                    ..Default::default()
                }],
            };
            let encoded = hat_update.encode();
            server_ctx.broadcast(BroadcastMessage {
                packet_id: ClientboundPlayerInfoUpdatePacket::PACKET_ID,
                data: encoded.into(),
                exclude_entity: None,
                target_entity: None,
            });
        }
    }

    if new_view_distance != old_view_distance {
        let (to_load, to_unload) = play_ctx
            .chunk_tracker
            .update_view_distance(new_view_distance);
        let center = play_ctx.chunk_tracker.center;
        if !to_load.is_empty() || !to_unload.is_empty() {
            if let Err(e) =
                movement::send_chunk_updates(play_ctx, center, &to_load, &to_unload).await
            {
                debug!(
                    peer = %play_ctx.addr,
                    name = %play_ctx.player_name,
                    error = %e,
                    "Failed to send view distance chunk updates",
                );
                return Err(());
            }
        }
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            old = old_view_distance,
            new = new_view_distance,
            loaded = to_load.len(),
            unloaded = to_unload.len(),
            "View distance changed",
        );
    }

    Ok(())
}

/// Saves player data, broadcasts leave messages, and removes the player
/// from all tracking structures.
async fn cleanup_disconnected_player(
    conn_handle: &ConnectionHandle,
    player: &Arc<RwLock<ServerPlayer>>,
    player_name: &str,
    uuid: uuid::Uuid,
    entity_id: i32,
    addr: SocketAddr,
    server_ctx: &Arc<ServerContext>,
) {
    // Save player data to disk.
    {
        let nbt = player.read().save_to_nbt();
        let playerdata_dir = server_ctx.world.storage.player_data_dir();
        let player_uuid = uuid;
        if let Err(e) = tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&playerdata_dir)?;
            let path = playerdata_dir.join(format!("{player_uuid}.dat"));
            oxidized_nbt::write_file(&path, &nbt)
        })
        .await
        {
            warn!(peer = %addr, uuid = %uuid, error = %e, "Failed to save player data");
        }
    }

    // Broadcast "Player left the game" system message.
    let leave_msg = ClientboundSystemChatPacket {
        content: Component::translatable(
            "multiplayer.player.left",
            vec![Component::text(player_name.to_owned())],
        ),
        is_overlay: false,
    };
    let encoded = leave_msg.encode();
    server_ctx.broadcast(BroadcastMessage {
        packet_id: ClientboundSystemChatPacket::PACKET_ID,
        data: encoded.freeze(),
        exclude_entity: None,
        target_entity: None,
    });

    // Remove player from tracking structures.
    server_ctx.network.kick_channels.remove(&uuid);
    server_ctx.network.player_list.write().remove(&uuid);

    // Broadcast tab-list removal.
    let remove_pkt = ClientboundPlayerInfoRemovePacket { uuids: vec![uuid] };
    let encoded = remove_pkt.encode();
    server_ctx.broadcast(BroadcastMessage {
        packet_id: ClientboundPlayerInfoRemovePacket::PACKET_ID,
        data: encoded.freeze(),
        exclude_entity: None,
        target_entity: None,
    });

    // Broadcast entity removal so other players stop rendering this player.
    let remove_entity = ClientboundRemoveEntitiesPacket {
        entity_ids: vec![entity_id],
    };
    let encoded = remove_entity.encode();
    server_ctx.broadcast(BroadcastMessage {
        packet_id: ClientboundRemoveEntitiesPacket::PACKET_ID,
        data: encoded.freeze(),
        exclude_entity: None,
        target_entity: None,
    });

    // Suppress unused-variable warning — conn_handle is dropped by caller.
    let _ = conn_handle;
    info!(peer = %addr, uuid = %uuid, name = %player_name, "Player removed from player list");
}
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
            pending = p.teleport.pending.len(),
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
            ctx.player.write().connection.chunk_send_rate = rate.clamp(0.1, 100.0);
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
