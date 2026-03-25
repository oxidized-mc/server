//! Block mining (breaking) handlers.
//!
//! Handles `PlayerAction` digging events: creative instant-break, survival
//! mining with duration validation, abort/drop/swap actions, and the
//! block destruction crack animation.

use std::time::{Duration, Instant};

use bytes::Bytes;
use tracing::debug;

use oxidized_game::player::GameMode;
use oxidized_protocol::codec::Packet;
use oxidized_protocol::constants::MILLIS_PER_TICK;
use oxidized_protocol::packets::play::ServerboundPlayerActionPacket;
use oxidized_protocol::packets::play::serverbound_player_action::PlayerAction;
use oxidized_protocol::types::BlockPos;
use oxidized_protocol::types::direction::Direction;

use super::PlayContext;
use super::block_interaction::{
    broadcast_block_update, get_block, is_spawn_protected, is_within_build_height, is_within_reach,
    resync_block, send_ack, send_actionbar, sync_inventory_slot,
};
use crate::network::helpers::decode_packet;
use crate::network::{BroadcastMessage, ConnectionError, ServerContext};
use oxidized_protocol::chat::Component;
use oxidized_world::registry::{AIR, BlockStateId};
use std::sync::Arc;

/// Minimum survival mining duration.
///
/// Even the fastest tool/block combo takes at least 1 game tick (50 ms).
/// Blocks with hardness 0 (tall grass, etc.) are instant-break in creative
/// only; survival always requires at least 1 tick. This is a conservative
/// lower bound — per-block hardness will tighten it later.
const MIN_MINING_DURATION: Duration = Duration::from_millis(MILLIS_PER_TICK);

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
                    player.mining.start_pos = Some(pkt.pos);
                    player.mining.start_time = Some(Instant::now());
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
            handle_stop_destroy(play_ctx, pkt.pos, pkt.direction, pkt.sequence).await?;
        },
        PlayerAction::AbortDestroyBlock => {
            let entity_id = {
                let mut player = play_ctx.player.write();
                player.mining.start_pos = None;
                player.mining.start_time = None;
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

/// Validates and completes a survival block break for `StopDestroyBlock`.
async fn handle_stop_destroy(
    play_ctx: &mut PlayContext<'_>,
    pos: BlockPos,
    direction: Direction,
    sequence: i32,
) -> Result<(), ConnectionError> {
    let game_mode = play_ctx.player.read().game_mode;
    if game_mode == GameMode::Creative {
        send_ack(play_ctx, sequence).await?;
        return Ok(());
    }

    let (mining_pos, mining_time) = {
        let player = play_ctx.player.read();
        (player.mining.start_pos, player.mining.start_time)
    };
    if mining_pos != Some(pos) {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            pos = ?pos,
            mining_pos = ?mining_pos,
            "StopDestroyBlock rejected: position mismatch"
        );
        resync_block(play_ctx, pos, Some(direction)).await?;
        send_ack(play_ctx, sequence).await?;
        return Ok(());
    }
    if let Some(start) = mining_time {
        if start.elapsed() < MIN_MINING_DURATION {
            debug!(
                peer = %play_ctx.addr,
                name = %play_ctx.player_name,
                pos = ?pos,
                elapsed = ?start.elapsed(),
                "StopDestroyBlock rejected: too fast (possible exploit)"
            );
            resync_block(play_ctx, pos, Some(direction)).await?;
            send_ack(play_ctx, sequence).await?;
            return Ok(());
        }
    }
    do_block_break(play_ctx, pos, sequence).await?;
    {
        let mut player = play_ctx.player.write();
        let eid = player.entity_id;
        player.mining.start_pos = None;
        player.mining.start_time = None;
        broadcast_block_destruction(play_ctx.server_ctx, eid, pos, 10, Some(eid));
    }
    Ok(())
}

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

    let set_ok = super::block_interaction::set_block(play_ctx.server_ctx, pos, u32::from(AIR.0));
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

    // Break the companion block of double-block items (doors, beds, tall plants).
    if let Some(old) = old_state {
        let block_name = BlockStateId(old as u16).block_name();
        if let Some(companion_pos) =
            super::placement::double_block_break_companion(block_name, old, pos)
        {
            let companion_state = get_block(play_ctx.server_ctx, companion_pos);
            if companion_state.is_some_and(|s| s != u32::from(AIR.0)) {
                let removed = super::block_interaction::set_block(
                    play_ctx.server_ctx,
                    companion_pos,
                    u32::from(AIR.0),
                );
                if removed {
                    broadcast_block_update(
                        play_ctx.server_ctx,
                        companion_pos,
                        u32::from(AIR.0) as i32,
                        None,
                    );
                }
            }
        }
    }

    // Acknowledge the sequence so the client accepts its prediction.
    send_ack(play_ctx, sequence).await?;
    Ok(())
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
