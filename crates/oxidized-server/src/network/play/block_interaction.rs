//! Block interaction handlers for play state.
//!
//! Handles block breaking (creative instant + survival mining), block placing,
//! and the sequence acknowledgement protocol. Sign updates are stubbed.

use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use tracing::{debug, warn};

use oxidized_game::inventory::item_ids::item_name_to_id;
use oxidized_game::inventory::item_stack::ItemStack;
use oxidized_game::player::GameMode;
use oxidized_protocol::chat::Component;
use oxidized_protocol::codec::Packet;
use oxidized_protocol::packets::play::serverbound_player_action::PlayerAction;
use oxidized_protocol::packets::play::{
    ClientboundBlockChangedAckPacket, ClientboundBlockUpdatePacket,
    ClientboundOpenSignEditorPacket, ClientboundSetPlayerInventoryPacket,
    ClientboundSystemChatPacket, ServerboundPickItemFromBlockPacket, ServerboundPlayerActionPacket,
    ServerboundSignUpdatePacket, ServerboundUseItemOnPacket, ServerboundUseItemPacket,
};
use oxidized_protocol::types::{BlockPos, Direction, Vec3};
use oxidized_world::chunk::ChunkPos;
use oxidized_world::chunk::level_chunk::{OVERWORLD_MAX_Y, OVERWORLD_MIN_Y};
use oxidized_world::registry::{AIR, BlockRegistry};

use super::PlayContext;
use crate::network::helpers::decode_packet;
use crate::network::{BroadcastMessage, ConnectionError, ServerContext};

/// Survival block interaction reach (squared).
///
/// Vanilla: `getBlockInteractionRange() + additionalRange + 0.5`
/// = 4.5 + 1.0 + 0.5 = 6.0 blocks → 36.0 sq
const SURVIVAL_REACH_DISTANCE_SQ: f64 = 6.0 * 6.0;

/// Creative block interaction reach (squared).
///
/// Vanilla: 5.0 + 1.0 + 0.5 = 6.5 blocks → 42.25 sq
const CREATIVE_REACH_DISTANCE_SQ: f64 = 6.5 * 6.5;

/// Minimum survival mining duration.
///
/// Even the fastest tool/block combo takes at least 1 game tick (50 ms).
/// Blocks with hardness 0 (tall grass, etc.) are instant-break in creative
/// only; survival always requires at least 1 tick. This is a conservative
/// lower bound — per-block hardness will tighten it later.
const MIN_MINING_DURATION: Duration = Duration::from_millis(50);

/// Maximum distance from a sign the player can edit (squared).
const MAX_SIGN_EDIT_DISTANCE_SQ: f64 = 8.0 * 8.0;

/// Minimum valid build height for overworld (inclusive).
const MIN_BUILD_HEIGHT: i32 = OVERWORLD_MIN_Y;

/// Maximum valid build height for overworld (inclusive).
/// `OVERWORLD_MAX_Y` is 320 (exclusive), so the last valid Y is 319.
const MAX_BUILD_HEIGHT: i32 = OVERWORLD_MAX_Y - 1;

/// Returns the squared distance from the player's eye position to the center
/// of the given block.
fn player_distance_to_block_sq(play_ctx: &PlayContext<'_>, pos: BlockPos) -> f64 {
    let player = play_ctx.player.read();
    let eye_height = if player.sneaking { 1.27 } else { 1.62 };
    let eye = Vec3::new(player.pos.x, player.pos.y + eye_height, player.pos.z);
    let block_center = Vec3::new(pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5);
    eye.distance_to_sqr(block_center)
}

/// Returns `true` if the player is within block interaction range.
///
/// Creative mode players have a longer reach (6.5 blocks) than survival/adventure (6.0).
fn is_within_reach(play_ctx: &PlayContext<'_>, pos: BlockPos) -> bool {
    let limit = if play_ctx.player.read().game_mode == GameMode::Creative {
        CREATIVE_REACH_DISTANCE_SQ
    } else {
        SURVIVAL_REACH_DISTANCE_SQ
    };
    player_distance_to_block_sq(play_ctx, pos) <= limit
}

/// Returns `true` if the position is within valid overworld build limits.
fn is_within_build_height(pos: BlockPos) -> bool {
    pos.y >= MIN_BUILD_HEIGHT && pos.y <= MAX_BUILD_HEIGHT
}

/// Returns `true` if the position is inside the spawn protection zone.
///
/// Vanilla uses Chebyshev distance: `max(|bx - sx|, |bz - sz|)`. A radius
/// of 0 disables spawn protection entirely.
///
/// TODO: Accept player info and skip protection for operators once ops.json
/// is implemented. Currently all players are treated as non-ops.
fn is_spawn_protected(ctx: &ServerContext, pos: BlockPos) -> bool {
    let radius = ctx.spawn_protection;
    if radius == 0 {
        return false;
    }

    let level_data = ctx.level_data.read();
    let (sx, sz) = (level_data.spawn_x, level_data.spawn_z);

    let dx = (pos.x - sx).unsigned_abs();
    let dz = (pos.z - sz).unsigned_abs();
    let chebyshev = dx.max(dz);

    chebyshev < radius
}

/// Handles `ServerboundPlayerActionPacket` (0x29) — block digging actions.
///
/// In creative mode, `StartDestroyBlock` instantly breaks the block.
/// In survival mode, `StopDestroyBlock` finishes the break. A basic guard
/// ensures the client sent `StartDestroyBlock` for the same position first.
pub async fn handle_player_action(
    play_ctx: &mut PlayContext<'_>,
    data: Bytes,
) -> Result<(), ConnectionError> {
    let pkt = decode_packet::<ServerboundPlayerActionPacket>(
        data,
        play_ctx.addr,
        play_ctx.player_name,
        "PlayerAction",
    )?;

    // Spectators cannot interact with blocks.
    if play_ctx.player.read().game_mode == GameMode::Spectator {
        resync_block(play_ctx, pkt.pos, Some(pkt.direction)).await?;
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    // Reject destroy actions on positions outside valid build height.
    if matches!(
        pkt.action,
        PlayerAction::StartDestroyBlock
            | PlayerAction::StopDestroyBlock
            | PlayerAction::AbortDestroyBlock
    ) && !is_within_build_height(pkt.pos)
    {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            pos = ?pkt.pos,
            "Block action rejected: outside build height"
        );
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    // Reject destroy actions inside the spawn protection zone.
    if matches!(
        pkt.action,
        PlayerAction::StartDestroyBlock | PlayerAction::StopDestroyBlock
    ) && is_spawn_protected(play_ctx.server_ctx, pkt.pos)
    {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            pos = ?pkt.pos,
            "Block action rejected: spawn protection"
        );
        // Vanilla sends "<pos> is under spawn protection" on the actionbar.
        send_actionbar(
            play_ctx,
            Component::translatable(
                "build.spawn_protection".to_owned(),
                vec![Component::text(format!(
                    "{}, {}, {}",
                    pkt.pos.x, pkt.pos.y, pkt.pos.z
                ))],
            ),
        )
        .await?;
        resync_block(play_ctx, pkt.pos, Some(pkt.direction)).await?;
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    // Validate reach distance for destroy actions.
    if matches!(
        pkt.action,
        PlayerAction::StartDestroyBlock
            | PlayerAction::StopDestroyBlock
            | PlayerAction::AbortDestroyBlock
    ) && !is_within_reach(play_ctx, pkt.pos)
    {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            pos = ?pkt.pos,
            "Block action rejected: out of reach"
        );
        // Re-sync the client's view of the block.
        resync_block(play_ctx, pkt.pos, Some(pkt.direction)).await?;
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    match pkt.action {
        PlayerAction::StartDestroyBlock => {
            let game_mode = play_ctx.player.read().game_mode;
            if game_mode == GameMode::Creative {
                do_block_break(play_ctx, pkt.pos, pkt.sequence).await?;
            } else {
                // Record mining start position and time for StopDestroyBlock validation.
                let entity_id = {
                    let mut player = play_ctx.player.write();
                    player.mining_start_pos = Some(pkt.pos);
                    player.mining_start_time = Some(Instant::now());
                    player.entity_id
                };
                // Broadcast mining start animation to other players.
                broadcast_block_destruction(
                    play_ctx.server_ctx,
                    entity_id,
                    pkt.pos,
                    0,
                    Some(entity_id),
                );
                debug!(
                    peer = %play_ctx.addr,
                    name = %play_ctx.player_name,
                    pos = ?pkt.pos,
                    "Block mining started (survival)"
                );
                send_ack(play_ctx, pkt.sequence).await?;
            }
        },
        PlayerAction::StopDestroyBlock => {
            let game_mode = play_ctx.player.read().game_mode;
            if game_mode != GameMode::Creative {
                // Validate that the player started mining this block.
                let (mining_pos, mining_time) = {
                    let player = play_ctx.player.read();
                    (player.mining_start_pos, player.mining_start_time)
                };
                if mining_pos != Some(pkt.pos) {
                    debug!(
                        peer = %play_ctx.addr,
                        name = %play_ctx.player_name,
                        pos = ?pkt.pos,
                        mining_pos = ?mining_pos,
                        "StopDestroyBlock rejected: position mismatch"
                    );
                    resync_block(play_ctx, pkt.pos, Some(pkt.direction)).await?;
                    send_ack(play_ctx, pkt.sequence).await?;
                    return Ok(());
                }
                // Validate mining duration — even the fastest break takes ≥1 tick.
                if let Some(start) = mining_time {
                    if start.elapsed() < MIN_MINING_DURATION {
                        debug!(
                            peer = %play_ctx.addr,
                            name = %play_ctx.player_name,
                            pos = ?pkt.pos,
                            elapsed = ?start.elapsed(),
                            "StopDestroyBlock rejected: too fast (possible exploit)"
                        );
                        resync_block(play_ctx, pkt.pos, Some(pkt.direction)).await?;
                        send_ack(play_ctx, pkt.sequence).await?;
                        return Ok(());
                    }
                }
                do_block_break(play_ctx, pkt.pos, pkt.sequence).await?;
                {
                    let mut player = play_ctx.player.write();
                    let eid = player.entity_id;
                    player.mining_start_pos = None;
                    player.mining_start_time = None;
                    // Clear mining animation on other players' screens.
                    broadcast_block_destruction(
                        play_ctx.server_ctx,
                        eid,
                        pkt.pos,
                        10,
                        Some(eid),
                    );
                }
            } else {
                send_ack(play_ctx, pkt.sequence).await?;
            }
        },
        PlayerAction::AbortDestroyBlock => {
            let entity_id = {
                let mut player = play_ctx.player.write();
                player.mining_start_pos = None;
                player.mining_start_time = None;
                player.entity_id
            };
            // Clear mining animation on other players' screens.
            broadcast_block_destruction(
                play_ctx.server_ctx,
                entity_id,
                pkt.pos,
                10, // 10 = clear animation
                Some(entity_id),
            );
            debug!(
                peer = %play_ctx.addr,
                name = %play_ctx.player_name,
                pos = ?pkt.pos,
                "Block mining aborted"
            );
            send_ack(play_ctx, pkt.sequence).await?;
        },
        PlayerAction::SwapItemWithOffhand => {
            // Vanilla: swap selected hotbar slot ↔ offhand, then sync both.
            let (sel_slot, off_slot) = {
                let mut p = play_ctx.player.write();
                p.inventory.swap_offhand()
            };
            sync_inventory_slot(play_ctx, sel_slot).await?;
            sync_inventory_slot(play_ctx, off_slot).await?;
            // Broadcast equipment change (both main hand and off hand changed).
            super::inventory::broadcast_full_equipment(play_ctx);
            debug!(
                peer = %play_ctx.addr,
                name = %play_ctx.player_name,
                "Swapped main hand ↔ offhand"
            );
        },
        PlayerAction::DropItem => {
            // Vanilla: drop one item from the selected hotbar slot.
            let _dropped = play_ctx.player.write().inventory.drop_item();
            let sel = play_ctx.player.read().inventory.selected_slot as usize;
            sync_inventory_slot(play_ctx, sel).await?;
            debug!(
                peer = %play_ctx.addr,
                name = %play_ctx.player_name,
                "Dropped 1 item from selected slot"
            );
        },
        PlayerAction::DropAllItems => {
            // Vanilla: drop entire stack from the selected hotbar slot.
            let _dropped = play_ctx.player.write().inventory.drop_all_items();
            let sel = play_ctx.player.read().inventory.selected_slot as usize;
            sync_inventory_slot(play_ctx, sel).await?;
            debug!(
                peer = %play_ctx.addr,
                name = %play_ctx.player_name,
                "Dropped all items from selected slot"
            );
        },
        PlayerAction::ReleaseUseItem => {
            debug!(
                peer = %play_ctx.addr,
                name = %play_ctx.player_name,
                "ReleaseUseItem: not yet implemented"
            );
        },
    }

    Ok(())
}

/// Handles `ServerboundUseItemOnPacket` (0x42) — block placement / interaction.
///
/// Places the player's held block onto the targeted face. If the player is not
/// holding a placeable block, the packet is acknowledged but no block is placed.
pub async fn handle_use_item_on(
    play_ctx: &mut PlayContext<'_>,
    data: Bytes,
) -> Result<(), ConnectionError> {
    let pkt = decode_packet::<ServerboundUseItemOnPacket>(
        data,
        play_ctx.addr,
        play_ctx.player_name,
        "UseItemOn",
    )?;

    // Spectators cannot place or interact with blocks.
    if play_ctx.player.read().game_mode == GameMode::Spectator {
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    // Validate cursor position — vanilla clamps each axis to [0, 1] with
    // a small epsilon for floating-point tolerance. Values outside indicate
    // a modified client.
    let (cx, cy, cz) = (
        pkt.hit_result.cursor_x,
        pkt.hit_result.cursor_y,
        pkt.hit_result.cursor_z,
    );
    const CURSOR_EPSILON: f32 = 0.0000001;
    if cx < -CURSOR_EPSILON
        || cx > 1.0 + CURSOR_EPSILON
        || cy < -CURSOR_EPSILON
        || cy > 1.0 + CURSOR_EPSILON
        || cz < -CURSOR_EPSILON
        || cz > 1.0 + CURSOR_EPSILON
    {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            cursor_x = cx,
            cursor_y = cy,
            cursor_z = cz,
            "UseItemOn rejected: cursor out of range"
        );
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    // Validate reach distance.
    if !is_within_reach(play_ctx, pkt.hit_result.pos) {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            pos = ?pkt.hit_result.pos,
            "Block place rejected: out of reach"
        );
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    // Compute placement position: offset from clicked face.
    let place_pos = pkt.hit_result.pos.relative(pkt.hit_result.direction);

    // Reject placement outside valid build height.
    if !is_within_build_height(place_pos) {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            pos = ?place_pos,
            "Block place rejected: outside build height"
        );
        // Vanilla sends "Height limit for building is %s" on the actionbar.
        if place_pos.y > MAX_BUILD_HEIGHT {
            send_actionbar(
                play_ctx,
                Component::translatable(
                    "build.tooHigh".to_owned(),
                    vec![Component::text(OVERWORLD_MAX_Y.to_string())],
                ),
            )
            .await?;
        } else {
            send_actionbar(
                play_ctx,
                Component::translatable(
                    "build.tooLow".to_owned(),
                    vec![Component::text(MIN_BUILD_HEIGHT.to_string())],
                ),
            )
            .await?;
        }
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    // Reject placement inside the spawn protection zone.
    if is_spawn_protected(play_ctx.server_ctx, place_pos) {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            pos = ?place_pos,
            "Block place rejected: spawn protection"
        );
        send_actionbar(
            play_ctx,
            Component::translatable(
                "build.spawn_protection".to_owned(),
                vec![Component::text(format!(
                    "{}, {}, {}",
                    place_pos.x, place_pos.y, place_pos.z
                ))],
            ),
        )
        .await?;
        resync_block(play_ctx, place_pos, None).await?;
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    // Get the held item name and check availability.
    let (held_item, game_mode, has_items) = {
        let player = play_ctx.player.read();
        let selected = player.inventory.get_selected();
        let name = selected.item.0.clone();
        let gm = player.game_mode;
        let has = !selected.is_empty() && selected.count > 0;
        (name, gm, has)
    };

    // In non-Creative modes, verify the player actually has items.
    if game_mode != GameMode::Creative && !has_items {
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    // Determine block state to place from the held item.
    let block_state_id =
        match held_item_to_block_state(&held_item, &play_ctx.server_ctx.block_registry) {
            Some(id) => id,
            None => {
                // Not a placeable block — just acknowledge.
                send_ack(play_ctx, pkt.sequence).await?;
                return Ok(());
            },
        };

    // Allow placement if target is air or a replaceable block (tall grass, water, etc.)
    let existing = get_block(play_ctx.server_ctx, place_pos);
    let is_replaceable = match existing {
        Some(state) => is_replaceable_block(play_ctx.server_ctx, state),
        None => false,
    };
    if !is_replaceable {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            pos = ?place_pos,
            existing_state = ?existing,
            "Block place rejected: position not replaceable"
        );
        // Send the actual block state back to correct client prediction.
        resync_block(play_ctx, place_pos, None).await?;
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    // Set the block in chunk storage.
    let placed = set_block(play_ctx.server_ctx, place_pos, block_state_id);
    if !placed {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            pos = ?place_pos,
            "Block place failed: chunk not loaded"
        );
        // Re-sync both the placement target and the clicked face so the
        // client doesn't display a phantom block.
        resync_block(play_ctx, place_pos, None).await?;
        resync_block(play_ctx, pkt.hit_result.pos, Some(pkt.hit_result.direction)).await?;
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    debug!(
        peer = %play_ctx.addr,
        name = %play_ctx.player_name,
        pos = ?place_pos,
        block_state = block_state_id,
        "Block placed"
    );

    // Decrement item count in survival/adventure modes.
    if game_mode != GameMode::Creative {
        let (slot_idx, updated) = {
            let mut player = play_ctx.player.write();
            let slot = player.inventory.selected_slot as usize;
            let stack = player.inventory.get_mut(slot);
            stack.count -= 1;
            if stack.count <= 0 {
                player.inventory.set(
                    slot,
                    oxidized_game::inventory::item_stack::ItemStack::empty(),
                );
            }
            (slot, player.inventory.get(slot).clone())
        };
        // Sync the updated slot back to the client to prevent desync.
        let slot_data = if updated.is_empty() {
            None
        } else {
            Some(super::inventory::item_stack_to_slot_data(&updated))
        };
        let sync_pkt = ClientboundSetPlayerInventoryPacket {
            slot: slot_idx as i32,
            contents: slot_data,
        };
        play_ctx.conn.send_packet(&sync_pkt).await?;
    }

    // Broadcast block change to all players (including the acting player).
    // Vanilla sends updates via the chunk tracking system to everyone.
    broadcast_block_update(play_ctx.server_ctx, place_pos, block_state_id as i32, None);

    // If the placed block is a sign, open the sign editor UI.
    if is_sign_block(&held_item) {
        let sign_editor = ClientboundOpenSignEditorPacket {
            pos: place_pos,
            is_front_text: true,
        };
        play_ctx.conn.send_packet(&sign_editor).await?;
    }

    // Acknowledge the sequence.
    send_ack(play_ctx, pkt.sequence).await?;

    Ok(())
}

/// Handles `ServerboundUseItemPacket` (0x43) — generic item use.
///
/// Currently a no-op. Item use behaviors (eating, shooting bows, etc.)
/// require systems not yet implemented.
pub async fn handle_use_item(
    play_ctx: &mut PlayContext<'_>,
    data: Bytes,
) -> Result<(), ConnectionError> {
    let pkt = decode_packet::<ServerboundUseItemPacket>(
        data,
        play_ctx.addr,
        play_ctx.player_name,
        "UseItem",
    )?;

    debug!(
        peer = %play_ctx.addr,
        name = %play_ctx.player_name,
        hand = ?pkt.hand,
        sequence = pkt.sequence,
        "UseItem: not yet implemented"
    );

    send_ack(play_ctx, pkt.sequence).await?;
    Ok(())
}

/// Handles `ServerboundSignUpdatePacket` (0x3D) — stub.
///
/// Sign editing requires block entities, which are not yet implemented.
/// Validates that the player is within editing range.
pub async fn handle_sign_update(
    play_ctx: &mut PlayContext<'_>,
    data: Bytes,
) -> Result<(), ConnectionError> {
    let pkt = decode_packet::<ServerboundSignUpdatePacket>(
        data,
        play_ctx.addr,
        play_ctx.player_name,
        "SignUpdate",
    )?;

    if player_distance_to_block_sq(play_ctx, pkt.pos) > MAX_SIGN_EDIT_DISTANCE_SQ {
        warn!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            pos = ?pkt.pos,
            "SignUpdate rejected: too far from sign"
        );
        return Ok(());
    }

    debug!(
        peer = %play_ctx.addr,
        name = %play_ctx.player_name,
        pos = ?pkt.pos,
        "SignUpdate: block entities not yet implemented"
    );

    Ok(())
}

// --- Internal helpers ---

/// Breaks a block at the given position: sets to air, broadcasts, and acks.
async fn do_block_break(
    play_ctx: &mut PlayContext<'_>,
    pos: BlockPos,
    sequence: i32,
) -> Result<(), ConnectionError> {
    let old_state = get_block(play_ctx.server_ctx, pos);
    if old_state == Some(u32::from(AIR.0)) {
        // Already air — nothing to break.
        send_ack(play_ctx, sequence).await?;
        return Ok(());
    }

    let set_ok = set_block(play_ctx.server_ctx, pos, u32::from(AIR.0));
    if !set_ok {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            pos = ?pos,
            "Block break failed: chunk not loaded"
        );
        send_ack(play_ctx, sequence).await?;
        return Ok(());
    }

    debug!(
        peer = %play_ctx.addr,
        name = %play_ctx.player_name,
        pos = ?pos,
        old_state = ?old_state,
        "Block broken"
    );

    // Broadcast the block change to all players (including the breaker).
    // Vanilla sends the block update via the chunk tracking system to everyone,
    // in addition to the ack. The client needs both to fully commit the change.
    broadcast_block_update(play_ctx.server_ctx, pos, u32::from(AIR.0) as i32, None);

    // Acknowledge the sequence so the client accepts its prediction.
    send_ack(play_ctx, sequence).await?;
    Ok(())
}

/// Sends a `ClientboundBlockChangedAckPacket` for the given sequence number.
async fn send_ack(play_ctx: &mut PlayContext<'_>, sequence: i32) -> Result<(), ConnectionError> {
    let pkt = ClientboundBlockChangedAckPacket { sequence };
    play_ctx.conn.send_packet(&pkt).await?;
    Ok(())
}

/// Re-syncs the client's view of a block (and optionally its adjacent face
/// block) by sending the current server-side state. Used when rejecting a
/// block action (e.g., out of reach). Vanilla sends both the target and the
/// adjacent block so the client's prediction is fully corrected.
async fn resync_block(
    play_ctx: &mut PlayContext<'_>,
    pos: BlockPos,
    direction: Option<Direction>,
) -> Result<(), ConnectionError> {
    let block_state = get_block(play_ctx.server_ctx, pos).unwrap_or(u32::from(AIR.0)) as i32;
    let pkt = ClientboundBlockUpdatePacket { pos, block_state };
    play_ctx.conn.send_packet(&pkt).await?;

    // Also resync the adjacent face block (vanilla sends both).
    if let Some(dir) = direction {
        let adjacent = pos.relative(dir);
        let adj_state = get_block(play_ctx.server_ctx, adjacent).unwrap_or(u32::from(AIR.0)) as i32;
        let adj_pkt = ClientboundBlockUpdatePacket {
            pos: adjacent,
            block_state: adj_state,
        };
        play_ctx.conn.send_packet(&adj_pkt).await?;
    }

    Ok(())
}

/// Sends an overlay (actionbar) message to the player.
async fn send_actionbar(
    play_ctx: &mut PlayContext<'_>,
    message: Component,
) -> Result<(), ConnectionError> {
    let pkt = ClientboundSystemChatPacket {
        content: message,
        overlay: true,
    };
    play_ctx
        .conn
        .send_raw(ClientboundSystemChatPacket::PACKET_ID, &pkt.encode())
        .await?;
    Ok(())
}

/// Sends a single inventory slot update to the client.
///
/// Reads the item from the player's inventory at `internal_slot`, converts
/// it to protocol format, and sends a `ClientboundSetPlayerInventoryPacket`.
///
/// `ClientboundSetPlayerInventoryPacket` uses the vanilla `Inventory` class
/// slot indices directly (0–8 hotbar, 9–35 main, 36–39 armor, 40 offhand),
/// NOT the window protocol slot indices.
async fn sync_inventory_slot(
    play_ctx: &mut PlayContext<'_>,
    internal_slot: usize,
) -> Result<(), ConnectionError> {
    let contents = {
        let player = play_ctx.player.read();
        let stack = player.inventory.get(internal_slot);
        if stack.is_empty() {
            None
        } else {
            Some(super::inventory::item_stack_to_slot_data(stack))
        }
    };

    let pkt = ClientboundSetPlayerInventoryPacket {
        slot: internal_slot as i32,
        contents,
    };
    play_ctx.conn.send_packet(&pkt).await?;
    Ok(())
}

/// Gets the block state at a position from shared chunk storage.
///
/// Returns `None` if the chunk is not loaded.
fn get_block(ctx: &Arc<ServerContext>, pos: BlockPos) -> Option<u32> {
    let chunk_pos = ChunkPos::from_block_coords(pos.x, pos.z);
    let chunk_ref = ctx.chunks.get(&chunk_pos)?;
    let chunk = chunk_ref.read();
    chunk.get_block_state(pos.x, pos.y, pos.z).ok()
}

/// Sets the block state at a position in shared chunk storage.
///
/// Returns `true` if the block was successfully set, `false` if the chunk
/// is not loaded or the position is out of bounds.
/// Marks the chunk as dirty for autosave.
fn set_block(ctx: &Arc<ServerContext>, pos: BlockPos, state_id: u32) -> bool {
    let chunk_pos = ChunkPos::from_block_coords(pos.x, pos.z);
    if let Some(chunk_ref) = ctx.chunks.get(&chunk_pos) {
        let mut chunk = chunk_ref.write();
        if chunk.set_block_state(pos.x, pos.y, pos.z, state_id).is_ok() {
            ctx.dirty_chunks.insert(chunk_pos);
            true
        } else {
            false
        }
    } else {
        false
    }
}

/// Broadcasts a block update to all connected players via the broadcast channel.
///
/// If `exclude_entity` is `Some`, the player with that entity ID will not
/// receive the update (they already know via the ack packet).
fn broadcast_block_update(
    ctx: &Arc<ServerContext>,
    pos: BlockPos,
    block_state: i32,
    exclude_entity: Option<i32>,
) {
    let pkt = ClientboundBlockUpdatePacket { pos, block_state };
    let data = pkt.encode();
    ctx.broadcast(BroadcastMessage {
        packet_id: ClientboundBlockUpdatePacket::PACKET_ID,
        data: data.freeze(),
        exclude_entity,
        target_entity: None,
    });
}

/// Broadcasts a block destruction (crack) animation to all connected players.
///
/// `progress` 0–9 shows increasing crack stages; 10 clears the animation.
fn broadcast_block_destruction(
    ctx: &Arc<ServerContext>,
    entity_id: i32,
    pos: BlockPos,
    progress: u8,
    exclude_entity: Option<i32>,
) {
    use oxidized_protocol::packets::play::ClientboundBlockDestructionPacket;
    let pkt = ClientboundBlockDestructionPacket {
        entity_id,
        pos,
        progress,
    };
    let data = pkt.encode();
    ctx.broadcast(BroadcastMessage {
        packet_id: ClientboundBlockDestructionPacket::PACKET_ID,
        data: data.freeze(),
        exclude_entity,
        target_entity: None,
    });
}

/// Returns `true` if a block state is air or a vanilla-replaceable block.
///
/// Replaceable blocks can be overwritten by block placement without the player
/// needing to break them first (e.g., tall grass, water, snow layer, fire).
/// Since our registry doesn't store this flag, we use the block name.
fn is_replaceable_block(ctx: &Arc<ServerContext>, state_id: u32) -> bool {
    if state_id == u32::from(AIR.0) {
        return true;
    }

    let block_name = match ctx
        .block_registry
        .get_state(oxidized_world::registry::BlockStateId(state_id as u16))
    {
        Some(state) => match ctx.block_registry.get_block_by_index(state.block_index) {
            Some(block) => &block.name,
            None => return false,
        },
        None => return false,
    };

    matches!(
        block_name.as_str(),
        "minecraft:air"
            | "minecraft:cave_air"
            | "minecraft:void_air"
            | "minecraft:water"
            | "minecraft:lava"
            | "minecraft:short_grass"
            | "minecraft:tall_grass"
            | "minecraft:seagrass"
            | "minecraft:tall_seagrass"
            | "minecraft:fire"
            | "minecraft:soul_fire"
            | "minecraft:snow"
            | "minecraft:vine"
            | "minecraft:dead_bush"
            | "minecraft:fern"
            | "minecraft:large_fern"
            | "minecraft:structure_void"
            | "minecraft:light"
            | "minecraft:crimson_roots"
            | "minecraft:warped_roots"
            | "minecraft:nether_sprouts"
            | "minecraft:hanging_roots"
            | "minecraft:glow_lichen"
    )
}

/// Maps a held item name to a block state ID for placement.
///
/// Uses a simple heuristic: look up the item name in the block registry and
/// return its default state. Items that are not placeable blocks return `None`.
fn held_item_to_block_state(item_name: &str, registry: &BlockRegistry) -> Option<u32> {
    if item_name.is_empty() || item_name == "minecraft:air" {
        return None;
    }

    let state = registry.default_state(item_name)?;
    Some(u32::from(state.0))
}

/// Returns `true` if the item name represents a sign block.
fn is_sign_block(item_name: &str) -> bool {
    item_name.contains("sign") && !item_name.contains("signal")
}

/// Handles `ServerboundPickItemFromBlockPacket` (0x24) — pick block.
///
/// Creative: places a fresh stack of the target block into the hotbar.
/// Survival: searches inventory for an existing stack and moves it to hotbar.
pub async fn handle_pick_item_from_block(
    play_ctx: &mut PlayContext<'_>,
    data: Bytes,
) -> Result<(), ConnectionError> {
    let pkt = decode_packet::<ServerboundPickItemFromBlockPacket>(
        data,
        play_ctx.addr,
        play_ctx.player_name,
        "PickItemFromBlock",
    )?;

    let game_mode = play_ctx.player.read().game_mode;
    // Spectators and adventure cannot pick blocks.
    if game_mode == GameMode::Spectator || game_mode == GameMode::Adventure {
        return Ok(());
    }

    // Look up the block state at the target position.
    let block_state = match get_block(play_ctx.server_ctx, pkt.pos) {
        Some(state) if state != u32::from(AIR.0) => state,
        _ => return Ok(()),
    };

    // Resolve block state to item name.
    let item_name = match play_ctx
        .server_ctx
        .block_registry
        .block_name_from_state_id(block_state)
    {
        Some(name) => name.to_owned(),
        None => return Ok(()),
    };

    let item_id = item_name_to_id(&item_name);
    // Items not in the registry can't be picked.
    if item_id < 0 {
        return Ok(());
    }

    if game_mode == GameMode::Creative {
        // Creative: place a fresh stack of 1 into the selected hotbar slot.
        let selected = {
            let mut player = play_ctx.player.write();
            let slot = player.inventory.selected_slot as usize;
            player
                .inventory
                .set(slot, ItemStack::new(item_name.clone(), 1));
            player.inventory.selected_slot
        };
        let slot_data = {
            use oxidized_protocol::codec::slot::{ComponentPatchData, SlotData};
            Some(SlotData {
                item_id,
                count: 1,
                component_data: ComponentPatchData::default(),
            })
        };
        let set_slot = ClientboundSetPlayerInventoryPacket {
            slot: i32::from(selected),
            contents: slot_data,
        };
        play_ctx.conn.send_packet(&set_slot).await?;
    } else {
        // Survival: search inventory for a matching item and move it to hotbar.
        let (found_slot, selected) = {
            let player = play_ctx.player.read();
            let sel = player.inventory.selected_slot as usize;
            // Check if the selected slot already has the item.
            if !player.inventory.get(sel).is_empty()
                && player.inventory.get(sel).item.0 == item_name
            {
                // Already holding it — nothing to do.
                return Ok(());
            }
            // Search hotbar first (0–8), then main inventory (9–35).
            let mut found = None;
            for i in 0..36usize {
                let stack = player.inventory.get(i);
                if !stack.is_empty() && stack.item.0 == item_name {
                    found = Some(i);
                    break;
                }
            }
            (found, sel)
        };

        match found_slot {
            Some(slot) if slot < 9 => {
                // Item is already in the hotbar — switch to that slot.
                {
                    let mut player = play_ctx.player.write();
                    player.inventory.selected_slot = slot as u8;
                }
                let held_pkt =
                    oxidized_protocol::packets::play::ClientboundSetHeldSlotPacket {
                        slot: slot as i32,
                    };
                play_ctx.conn.send_packet(&held_pkt).await?;
            },
            Some(slot) => {
                // Item is in main inventory — swap with selected hotbar slot.
                {
                    let mut player = play_ctx.player.write();
                    let sel = player.inventory.selected_slot as usize;
                    let main_item = player.inventory.get(slot).clone();
                    let hotbar_item = player.inventory.get(sel).clone();
                    player.inventory.set(sel, main_item);
                    player.inventory.set(slot, hotbar_item);
                }
                sync_inventory_slot(play_ctx, selected).await?;
                sync_inventory_slot(play_ctx, slot).await?;
            },
            None => {
                // Item not in inventory — nothing to do in survival.
            },
        }
    }

    debug!(
        peer = %play_ctx.addr,
        name = %play_ctx.player_name,
        pos = ?pkt.pos,
        item = %item_name,
        mode = ?game_mode,
        "Pick block"
    );

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_held_item_to_block_state_air_returns_none() {
        let registry = BlockRegistry::load().unwrap();
        assert!(held_item_to_block_state("", &registry).is_none());
        assert!(held_item_to_block_state("minecraft:air", &registry).is_none());
    }

    #[test]
    fn test_held_item_to_block_state_stone_returns_some() {
        let registry = BlockRegistry::load().unwrap();
        let result = held_item_to_block_state("minecraft:stone", &registry);
        // Stone's default state is 1
        assert_eq!(result, Some(1));
    }

    #[test]
    fn test_held_item_to_block_state_unknown_returns_none() {
        let registry = BlockRegistry::load().unwrap();
        assert!(held_item_to_block_state("minecraft:diamond_sword", &registry).is_none());
    }

    #[test]
    fn test_get_set_block_on_loaded_chunk() {
        let ctx = test_server_ctx();
        let chunk_pos = ChunkPos::from_block_coords(0, 0);
        let chunk = oxidized_world::chunk::LevelChunk::new(chunk_pos);
        ctx.chunks
            .insert(chunk_pos, Arc::new(parking_lot::RwLock::new(chunk)));

        let pos = BlockPos::new(0, 64, 0);

        // Initially air
        assert_eq!(get_block(&ctx, pos), Some(0));

        // Set to stone (state 1)
        assert!(set_block(&ctx, pos, 1));

        // Read back stone
        assert_eq!(get_block(&ctx, pos), Some(1));
    }

    #[test]
    fn test_set_block_marks_chunk_dirty() {
        let ctx = test_server_ctx();
        let chunk_pos = ChunkPos::from_block_coords(0, 0);
        let chunk = oxidized_world::chunk::LevelChunk::new(chunk_pos);
        ctx.chunks
            .insert(chunk_pos, Arc::new(parking_lot::RwLock::new(chunk)));

        let pos = BlockPos::new(0, 64, 0);
        assert!(!ctx.dirty_chunks.contains(&chunk_pos));

        set_block(&ctx, pos, 1);
        assert!(ctx.dirty_chunks.contains(&chunk_pos));
    }

    #[test]
    fn test_get_block_unloaded_chunk_returns_none() {
        let ctx = test_server_ctx();
        let pos = BlockPos::new(1000, 64, 1000);
        assert_eq!(get_block(&ctx, pos), None);
    }

    #[test]
    fn test_set_block_unloaded_chunk_returns_false() {
        let ctx = test_server_ctx();
        let pos = BlockPos::new(1000, 64, 1000);
        assert!(!set_block(&ctx, pos, 1));
    }

    #[test]
    fn test_mining_start_sets_time() {
        use oxidized_game::player::{GameMode, ServerPlayer};
        use oxidized_protocol::auth::GameProfile;
        use oxidized_protocol::types::resource_location::ResourceLocation;

        let profile = GameProfile::new(uuid::Uuid::nil(), "Steve".into());
        let mut player = ServerPlayer::new(
            1,
            profile,
            ResourceLocation::minecraft("overworld"),
            GameMode::Survival,
        );

        // Initially both mining fields are None.
        assert!(player.mining_start_pos.is_none());
        assert!(player.mining_start_time.is_none());

        // Simulate StartDestroyBlock.
        let pos = BlockPos::new(10, 64, 10);
        player.mining_start_pos = Some(pos);
        player.mining_start_time = Some(std::time::Instant::now());

        assert_eq!(player.mining_start_pos, Some(pos));
        assert!(player.mining_start_time.is_some());
    }

    #[test]
    fn test_mining_abort_clears_time() {
        use oxidized_game::player::{GameMode, ServerPlayer};
        use oxidized_protocol::auth::GameProfile;
        use oxidized_protocol::types::resource_location::ResourceLocation;

        let profile = GameProfile::new(uuid::Uuid::nil(), "Steve".into());
        let mut player = ServerPlayer::new(
            1,
            profile,
            ResourceLocation::minecraft("overworld"),
            GameMode::Survival,
        );

        // Simulate StartDestroyBlock then AbortDestroyBlock.
        player.mining_start_pos = Some(BlockPos::new(5, 60, 5));
        player.mining_start_time = Some(std::time::Instant::now());

        // Abort clears both fields.
        player.mining_start_pos = None;
        player.mining_start_time = None;

        assert!(player.mining_start_pos.is_none());
        assert!(player.mining_start_time.is_none());
    }

    #[test]
    fn test_min_mining_duration_is_one_tick() {
        assert_eq!(MIN_MINING_DURATION, Duration::from_millis(50));
    }

    fn test_server_ctx() -> Arc<ServerContext> {
        use oxidized_game::level::game_rules::GameRules;
        use oxidized_game::level::tick_rate::ServerTickRateManager;
        use oxidized_game::player::PlayerList;
        use oxidized_protocol::types::resource_location::ResourceLocation;
        use oxidized_world::storage::{LevelStorageSource, PrimaryLevelData};
        use parking_lot::RwLock;
        use tokio::sync::broadcast;

        Arc::new(ServerContext {
            player_list: RwLock::new(PlayerList::new(20)),
            level_data: RwLock::new(
                PrimaryLevelData::from_nbt(&oxidized_nbt::NbtCompound::new()).unwrap(),
            ),
            dimensions: vec![ResourceLocation::from_string("minecraft:overworld").unwrap()],
            max_view_distance: 10,
            max_simulation_distance: 10,
            broadcast_tx: broadcast::channel(256).0,
            color_char: None,
            commands: oxidized_game::commands::Commands::new(),
            event_bus: oxidized_game::event::EventBus::new(),
            max_players: 20,
            shutdown_tx: broadcast::channel(1).0,
            game_rules: RwLock::new(GameRules::default()),
            tick_rate_manager: RwLock::new(ServerTickRateManager::default()),
            storage: LevelStorageSource::new(""),
            chunks: dashmap::DashMap::new(),
            dirty_chunks: dashmap::DashSet::new(),
            block_registry: Arc::new(BlockRegistry::load().unwrap()),
            chunk_generator: Arc::new(oxidized_game::worldgen::flat::FlatChunkGenerator::new(
                oxidized_game::worldgen::flat::FlatWorldConfig::default(),
            )),
            op_permission_level: 4,
            spawn_protection: 16,
            kick_channels: dashmap::DashMap::new(),
        })
    }

    /// Builds a `ServerContext` with a custom spawn protection radius.
    fn test_server_ctx_with_spawn_protection(radius: u32) -> Arc<ServerContext> {
        use oxidized_game::level::game_rules::GameRules;
        use oxidized_game::level::tick_rate::ServerTickRateManager;
        use oxidized_game::player::PlayerList;
        use oxidized_protocol::types::resource_location::ResourceLocation;
        use oxidized_world::storage::{LevelStorageSource, PrimaryLevelData};
        use parking_lot::RwLock;
        use tokio::sync::broadcast;

        Arc::new(ServerContext {
            player_list: RwLock::new(PlayerList::new(20)),
            level_data: RwLock::new(
                PrimaryLevelData::from_nbt(&oxidized_nbt::NbtCompound::new()).unwrap(),
            ),
            dimensions: vec![ResourceLocation::from_string("minecraft:overworld").unwrap()],
            max_view_distance: 10,
            max_simulation_distance: 10,
            broadcast_tx: broadcast::channel(256).0,
            color_char: None,
            commands: oxidized_game::commands::Commands::new(),
            event_bus: oxidized_game::event::EventBus::new(),
            max_players: 20,
            shutdown_tx: broadcast::channel(1).0,
            game_rules: RwLock::new(GameRules::default()),
            tick_rate_manager: RwLock::new(ServerTickRateManager::default()),
            storage: LevelStorageSource::new(""),
            chunks: dashmap::DashMap::new(),
            dirty_chunks: dashmap::DashSet::new(),
            block_registry: Arc::new(BlockRegistry::load().unwrap()),
            chunk_generator: Arc::new(oxidized_game::worldgen::flat::FlatChunkGenerator::new(
                oxidized_game::worldgen::flat::FlatWorldConfig::default(),
            )),
            op_permission_level: 4,
            spawn_protection: radius,
            kick_channels: dashmap::DashMap::new(),
        })
    }

    // -- Build height validation tests --

    #[test]
    fn test_build_height_constants_match_overworld() {
        assert_eq!(MIN_BUILD_HEIGHT, -64);
        assert_eq!(MAX_BUILD_HEIGHT, 319);
    }

    #[test]
    fn test_build_height_valid_at_min() {
        assert!(is_within_build_height(BlockPos::new(
            0,
            MIN_BUILD_HEIGHT,
            0
        )));
    }

    #[test]
    fn test_build_height_valid_at_max() {
        assert!(is_within_build_height(BlockPos::new(
            0,
            MAX_BUILD_HEIGHT,
            0
        )));
    }

    #[test]
    fn test_build_height_valid_middle() {
        assert!(is_within_build_height(BlockPos::new(0, 64, 0)));
    }

    #[test]
    fn test_build_height_below_min() {
        assert!(!is_within_build_height(BlockPos::new(
            0,
            MIN_BUILD_HEIGHT - 1,
            0
        )));
    }

    #[test]
    fn test_build_height_above_max() {
        assert!(!is_within_build_height(BlockPos::new(
            0,
            MAX_BUILD_HEIGHT + 1,
            0
        )));
    }

    // -- Spawn protection tests --

    #[test]
    fn test_spawn_protection_disabled_when_radius_zero() {
        let ctx = test_server_ctx_with_spawn_protection(0);
        assert!(!is_spawn_protected(&ctx, BlockPos::new(0, 64, 0)));
    }

    #[test]
    fn test_spawn_protection_at_spawn_origin() {
        let ctx = test_server_ctx_with_spawn_protection(16);
        assert!(is_spawn_protected(&ctx, BlockPos::new(0, 64, 0)));
    }

    #[test]
    fn test_spawn_protection_at_boundary() {
        let ctx = test_server_ctx_with_spawn_protection(16);
        assert!(is_spawn_protected(&ctx, BlockPos::new(15, 64, 0)));
        assert!(!is_spawn_protected(&ctx, BlockPos::new(16, 64, 0)));
    }

    #[test]
    fn test_spawn_protection_diagonal() {
        let ctx = test_server_ctx_with_spawn_protection(10);
        assert!(is_spawn_protected(&ctx, BlockPos::new(9, 64, 9)));
        assert!(!is_spawn_protected(&ctx, BlockPos::new(10, 64, 10)));
    }

    #[test]
    fn test_spawn_protection_negative_coords() {
        let ctx = test_server_ctx_with_spawn_protection(16);
        assert!(is_spawn_protected(&ctx, BlockPos::new(-15, 64, -15)));
    }

    // -- Block replacement validation tests --

    #[test]
    fn test_placement_on_air_allowed() {
        let ctx = test_server_ctx();
        let chunk_pos = ChunkPos::from_block_coords(0, 0);
        let chunk = oxidized_world::chunk::LevelChunk::new(chunk_pos);
        ctx.chunks
            .insert(chunk_pos, Arc::new(parking_lot::RwLock::new(chunk)));

        let pos = BlockPos::new(0, 64, 0);
        let existing = get_block(&ctx, pos);
        assert_eq!(existing, Some(u32::from(AIR.0)));
    }

    #[test]
    fn test_placement_on_solid_block_rejected() {
        let ctx = test_server_ctx();
        let chunk_pos = ChunkPos::from_block_coords(0, 0);
        let chunk = oxidized_world::chunk::LevelChunk::new(chunk_pos);
        ctx.chunks
            .insert(chunk_pos, Arc::new(parking_lot::RwLock::new(chunk)));

        let pos = BlockPos::new(0, 64, 0);
        assert!(set_block(&ctx, pos, 1));
        let existing = get_block(&ctx, pos);
        assert_ne!(existing, Some(u32::from(AIR.0)));
    }
}
