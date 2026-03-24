//! Block placement and item use handlers.
//!
//! Handles `UseItemOn` (block placement with face/cursor validation) and
//! `UseItem` (generic item use — currently stubbed).

use bytes::Bytes;
use tracing::debug;

use oxidized_game::player::GameMode;
use oxidized_protocol::chat::Component;
use oxidized_protocol::packets::play::{
    ClientboundOpenSignEditorPacket, ClientboundSetPlayerInventoryPacket,
    ServerboundUseItemOnPacket, ServerboundUseItemPacket,
};
use oxidized_world::chunk::level_chunk::OVERWORLD_MAX_Y;
use oxidized_world::registry::{AIR, BlockRegistry};

use super::PlayContext;
use super::block_interaction::{
    MIN_BUILD_HEIGHT, broadcast_block_update, get_block, is_spawn_protected,
    is_within_build_height, is_within_reach, resync_block, send_ack, send_actionbar, set_block,
};
use crate::network::helpers::decode_packet;
use crate::network::{ConnectionError, ServerContext};
use std::sync::Arc;

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
    let valid_range = -CURSOR_EPSILON..=1.0 + CURSOR_EPSILON;
    if !valid_range.contains(&cx) || !valid_range.contains(&cy) || !valid_range.contains(&cz) {
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
        if place_pos.y > super::block_interaction::MAX_BUILD_HEIGHT {
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

/// Returns `true` if a block state is air or a vanilla-replaceable block.
///
/// Replaceable blocks can be overwritten by block placement without the player
/// needing to break them first (e.g., tall grass, water, snow layer, fire).
/// Since our registry doesn't store this flag, we use the block name.
fn is_replaceable_block(ctx: &Arc<ServerContext>, state_id: u32) -> bool {
    if state_id == u32::from(AIR.0) {
        return true;
    }

    let bsid = oxidized_world::registry::BlockStateId(state_id as u16);
    let block_name = if (bsid.0 as usize) < ctx.block_registry.state_count() {
        bsid.block_name()
    } else {
        return false;
    };

    matches!(
        block_name,
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
