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
use oxidized_protocol::auth;
use oxidized_protocol::channel::{INBOUND_CHANNEL_CAPACITY, OUTBOUND_CHANNEL_CAPACITY};
use oxidized_protocol::chat::Component;
use oxidized_protocol::codec::Packet;
use oxidized_protocol::codec::slot::SlotData;
use oxidized_protocol::connection::{Connection, ConnectionError};
use oxidized_protocol::handle::ConnectionHandle;
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
use oxidized_world::chunk::ChunkPos;
use parking_lot::RwLock;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, info, warn};

use crate::network::{BroadcastMessage, LoginContext, MAX_SERVERBOUND_PLAY_ID, ServerContext};
use crate::network::reader::reader_loop;
use crate::network::writer::writer_loop;

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
}

/// Entity type registry ID for players (`minecraft:player`).
const PLAYER_ENTITY_TYPE_ID: i32 = 155;

/// Entity metadata slot for displayed skin parts (cape, jacket, sleeves, etc.).
///
/// Vanilla hierarchy: Entity(0–7), LivingEntity(8–14), Avatar(15–16), Player(17–20).
/// `DATA_PLAYER_MODE_CUSTOMISATION` is Avatar index 16.
const DATA_PLAYER_MODE_CUSTOMISATION: u8 = 16;

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
    let to_slot = |stack: &oxidized_game::inventory::ItemStack| -> Option<SlotData> {
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
/// Splits the [`Connection`] into reader/writer task pair (ADR-006),
/// creates bounded inbound/outbound channels, spawns the reader and
/// writer tasks, sends the join sequence through the outbound channel,
/// and enters the main play loop to process inbound packets.
///
/// # Errors
///
/// Returns a [`ConnectionError`] if any I/O or protocol step fails.
pub async fn handle_play_split(
    conn: Connection,
    profile: auth::GameProfile,
    client_info: ClientInformation,
    ctx: &LoginContext,
) -> Result<(), ConnectionError> {
    let server_ctx = &ctx.server_ctx;
    let addr = conn.remote_addr();

    // Split connection into reader/writer halves (ADR-006).
    let (reader, writer) = conn.into_split();

    // Create bounded channels (ADR-006: inbound=128, outbound=512).
    let (inbound_tx, mut inbound_rx) = mpsc::channel(INBOUND_CHANNEL_CAPACITY);
    let (outbound_tx, outbound_rx) = mpsc::channel(OUTBOUND_CHANNEL_CAPACITY);

    // Spawn reader and writer tasks.
    let reader_handle = tokio::spawn(reader_loop(reader, inbound_tx));
    let writer_handle = tokio::spawn(writer_loop(writer, outbound_rx));

    // Create the connection handle for sending packets.
    let conn_handle = ConnectionHandle::new(outbound_tx, addr);

    // Send join sequence through the outbound channel.
    let join_state =
        join::send_join_sequence(&conn_handle, profile, client_info, server_ctx).await?;
    let player_arc = join_state.player;
    let player_name = join_state.name;
    let uuid = join_state.uuid;
    let entity_id = join_state.entity_id;

    // Track which chunks the player has loaded.
    let initial_chunk =
        ChunkPos::from_block_coords(join_state.initial_chunk_x, join_state.initial_chunk_z);
    let mut chunk_tracker = PlayerChunkTracker::new(initial_chunk, join_state.view_distance);

    // Subscribe to broadcast channel.
    let mut broadcast_rx = server_ctx.broadcast_tx.subscribe();

    // Register a per-player kick channel for remote disconnect signaling.
    let (kick_tx, mut kick_rx) = tokio::sync::mpsc::channel::<String>(1);
    server_ctx.kick_channels.insert(uuid, kick_tx);

    // Per-player chat rate limiter.
    let mut rate_limiter = ChatRateLimiter::new();

    // Keepalive state — send a ping every 15 seconds, timeout after 30.
    const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
    const KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(30);
    // Vanilla resends unconfirmed teleports every 20 ticks (~1 second).
    const TELEPORT_RESEND_INTERVAL: Duration = Duration::from_secs(1);
    let mut keepalive_timer = tokio::time::interval(KEEPALIVE_INTERVAL);
    keepalive_timer.tick().await; // consume the immediate first tick
    let mut keepalive_pending = false;
    let mut keepalive_challenge: i64 = 0;
    let mut keepalive_sent_at = Instant::now();

    // Per-tick timer for chat rate limiter decay (50 ms = 1 game tick).
    let mut tick_timer = tokio::time::interval(Duration::from_millis(50));
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
    };

    // PLAY read loop — reads from inbound channel instead of socket (ADR-006).
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
                    let yaw = p.yaw;
                    let pitch = p.pitch;
                    let mut list = Vec::new();
                    for entry in p.pending_teleports.iter_mut() {
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
            // Read incoming packets from inbound channel (ADR-006: reader task dispatches here).
            inbound = inbound_rx.recv() => {
                match inbound {
                    Some(pkt) => {
                        match pkt.id {
                            ServerboundKeepAlivePacket::PACKET_ID => {
                                match keepalive::handle_keepalive(
                                    pkt.data, addr, &player_name,
                                    &mut keepalive_pending, keepalive_challenge, &keepalive_sent_at,
                                    &player_arc,
                                ) {
                                    keepalive::KeepaliveResult::Ok(ka_uuid, ka_latency) => {
                                        // Broadcast latency update to all players' tab lists.
                                        let latency_update = ClientboundPlayerInfoUpdatePacket {
                                            actions: PlayerInfoActions(PlayerInfoActions::UPDATE_LATENCY),
                                            entries: vec![PlayerInfoEntry {
                                                uuid: ka_uuid,
                                                latency: ka_latency,
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
                                    },
                                    keepalive::KeepaliveResult::Mismatch => {
                                        // Vanilla disconnects on wrong keepalive ID.
                                        info!(peer = %addr, name = %player_name, "Keepalive mismatch — disconnecting");
                                        break;
                                    },
                                    keepalive::KeepaliveResult::DecodeError => {},
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
                                    if !play_ctx.rate_limiter.try_acquire() {
                                        warn!(peer = %addr, name = %player_name, "Command rate-limited");
                                    } else {
                                        let (pos, rot) = {
                                            let p = play_ctx.player.read();
                                            ((p.pos.x, p.pos.y, p.pos.z), (p.yaw, p.pitch))
                                        };
                                        chat::handle_chat_command(
                                            play_ctx.conn_handle,
                                            &cmd_pkt.command,
                                            &player_name,
                                            uuid,
                                            pos, rot,
                                            // TODO: implement per-player ops (ops.json) instead of
                                            // giving all players the configured op level.
                                            server_ctx.op_permission_level as u32,
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
                                    if !play_ctx.rate_limiter.try_acquire() {
                                        warn!(peer = %addr, name = %player_name, "Command rate-limited (signed)");
                                    } else {
                                        let (pos, rot) = {
                                            let p = play_ctx.player.read();
                                            ((p.pos.x, p.pos.y, p.pos.z), (p.yaw, p.pitch))
                                        };
                                        chat::handle_chat_command(
                                            play_ctx.conn_handle,
                                            &cmd_pkt.command,
                                            &player_name,
                                            uuid,
                                            pos, rot,
                                            server_ctx.op_permission_level as u32,
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
                            ServerboundSwingPacket::PACKET_ID => {
                                if let Ok(swing) = decode_packet::<ServerboundSwingPacket>(
                                    pkt.data, addr, &player_name, "Swing",
                                ) {
                                    let entity_id = play_ctx.player.read().entity_id;
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
                            },
                            ServerboundPlayerAbilitiesPacket::PACKET_ID => {
                                if let Ok(abilities_pkt) = decode_packet::<ServerboundPlayerAbilitiesPacket>(
                                    pkt.data, addr, &player_name, "PlayerAbilities",
                                ) {
                                    let mut p = play_ctx.player.write();
                                    p.abilities.is_flying =
                                        abilities_pkt.is_flying() && p.abilities.can_fly;
                                    debug!(
                                        peer = %addr,
                                        name = %player_name,
                                        is_flying = p.abilities.is_flying,
                                        "Player abilities update",
                                    );
                                }
                            },
                            ServerboundClientInformationPlayPacket::PACKET_ID => {
                                if let Ok(info_pkt) = decode_packet::<ServerboundClientInformationPlayPacket>(
                                    pkt.data, addr, &player_name, "ClientInformation",
                                ) {
                                    let new_view_distance =
                                        i32::from(info_pkt.information.view_distance)
                                            .clamp(2, server_ctx.max_view_distance);
                                    let (old_view_distance, skin_changed) = {
                                        let mut p = play_ctx.player.write();
                                        let old_vd = p.view_distance;
                                        p.view_distance = new_view_distance;
                                        let old_skin = p.model_customisation;
                                        p.model_customisation =
                                            info_pkt.information.model_customisation;
                                        (old_vd, old_skin != p.model_customisation)
                                    };

                                    // Broadcast skin customisation change to others.
                                    if skin_changed {
                                        let skin = play_ctx.player.read().model_customisation;
                                        let skin_pkt =
                                            ClientboundSetEntityDataPacket::single_byte(
                                                entity_id,
                                                DATA_PLAYER_MODE_CUSTOMISATION,
                                                skin,
                                            );
                                        let encoded = skin_pkt.encode();
                                        server_ctx.broadcast(BroadcastMessage {
                                            packet_id:
                                                ClientboundSetEntityDataPacket::PACKET_ID,
                                            data: encoded.freeze(),
                                            exclude_entity: Some(entity_id),
                                            target_entity: None,
                                        });
                                    }

                                    if new_view_distance != old_view_distance {
                                        // Update the chunk tracker and send/forget chunks.
                                        let (to_load, to_unload) = play_ctx
                                            .chunk_tracker
                                            .update_view_distance(new_view_distance);
                                        let center = play_ctx.chunk_tracker.center;
                                        if !to_load.is_empty() || !to_unload.is_empty() {
                                            if let Err(e) = movement::send_chunk_updates(
                                                &mut play_ctx,
                                                center,
                                                &to_load,
                                                &to_unload,
                                            )
                                            .await
                                            {
                                                debug!(
                                                    peer = %addr,
                                                    name = %player_name,
                                                    error = %e,
                                                    "Failed to send view distance chunk updates",
                                                );
                                                break;
                                            }
                                        }
                                        debug!(
                                            peer = %addr,
                                            name = %player_name,
                                            old = old_view_distance,
                                            new = new_view_distance,
                                            loaded = to_load.len(),
                                            unloaded = to_unload.len(),
                                            "View distance changed",
                                        );
                                    }
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

    // Save player data to disk before removing from the list.
    {
        let nbt = player_arc.read().save_to_nbt();
        let playerdata_dir = server_ctx.storage.player_data_dir();
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
    {
        let leave_msg = ClientboundSystemChatPacket {
            content: Component::translatable(
                "multiplayer.player.left",
                vec![Component::text(player_name.clone())],
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
    }

    // Clean up — remove player from the list and broadcast removal to tab lists.
    server_ctx.kick_channels.remove(&uuid);
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
        server_ctx.broadcast(broadcast);
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
        server_ctx.broadcast(broadcast);
    }
    info!(peer = %addr, uuid = %uuid, name = %player_name, "Player removed from player list");

    // Drop outbound sender → writer task sees channel closed → exits.
    drop(conn_handle);

    // Wait for reader and writer tasks to finish.
    let _ = reader_handle.await;
    let _ = writer_handle.await;

    Ok(())
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
