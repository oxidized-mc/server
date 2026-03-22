//! Block interaction handlers for play state.
//!
//! Handles block breaking (creative instant + survival mining), block placing,
//! and the sequence acknowledgement protocol. Sign updates are stubbed.

use std::sync::Arc;

use bytes::Bytes;
use tracing::{debug, warn};

use oxidized_game::inventory::item_ids::item_name_to_id;
use oxidized_game::inventory::item_stack::ItemStack;
use oxidized_game::player::GameMode;
use oxidized_protocol::codec::Packet;
use oxidized_protocol::packets::play::serverbound_player_action::PlayerAction;
use oxidized_protocol::packets::play::{
    ClientboundBlockChangedAckPacket, ClientboundBlockUpdatePacket,
    ClientboundOpenSignEditorPacket, ClientboundSetPlayerInventoryPacket,
    ServerboundPickItemFromBlockPacket, ServerboundPlayerActionPacket, ServerboundSignUpdatePacket,
    ServerboundUseItemOnPacket, ServerboundUseItemPacket,
};
use oxidized_protocol::types::{BlockPos, Direction, Vec3};
use oxidized_world::chunk::ChunkPos;
use oxidized_world::registry::{AIR, BlockRegistry};

use super::PlayContext;
use crate::network::helpers::decode_packet;
use crate::network::{BroadcastMessage, ConnectionError, ServerContext};

/// Maximum block interaction reach (squared) — 6 blocks + 1 tolerance.
/// Vanilla uses `isWithinBlockInteractionRange(pos, 1.0)` → ~6 blocks.
const MAX_REACH_DISTANCE_SQ: f64 = 7.0 * 7.0;

/// Maximum distance from a sign the player can edit (squared).
const MAX_SIGN_EDIT_DISTANCE_SQ: f64 = 8.0 * 8.0;

/// Returns the squared distance from the player's eye position to the center
/// of the given block.
fn player_distance_to_block_sq(play_ctx: &PlayContext<'_>, pos: BlockPos) -> f64 {
    let player = play_ctx.player.read();
    // Eye position is pos + 1.62 (standing eye height).
    let eye = Vec3::new(player.pos.x, player.pos.y + 1.62, player.pos.z);
    let block_center = Vec3::new(pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5);
    eye.distance_to_sqr(block_center)
}

/// Returns `true` if the player is within block interaction range.
fn is_within_reach(play_ctx: &PlayContext<'_>, pos: BlockPos) -> bool {
    player_distance_to_block_sq(play_ctx, pos) <= MAX_REACH_DISTANCE_SQ
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
                // Record mining start position for StopDestroyBlock validation.
                play_ctx.player.write().spawn_pos = pkt.pos;
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
                let mining_pos = play_ctx.player.read().spawn_pos;
                if mining_pos != pkt.pos {
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
                do_block_break(play_ctx, pkt.pos, pkt.sequence).await?;
            } else {
                send_ack(play_ctx, pkt.sequence).await?;
            }
        },
        PlayerAction::AbortDestroyBlock => {
            debug!(
                peer = %play_ctx.addr,
                name = %play_ctx.player_name,
                pos = ?pkt.pos,
                "Block mining aborted"
            );
            send_ack(play_ctx, pkt.sequence).await?;
        },
        PlayerAction::DropAllItems
        | PlayerAction::DropItem
        | PlayerAction::ReleaseUseItem
        | PlayerAction::SwapItemWithOffhand => {
            debug!(
                peer = %play_ctx.addr,
                name = %play_ctx.player_name,
                action = ?pkt.action,
                "PlayerAction: not yet implemented"
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

    // Set the block in chunk storage.
    let placed = set_block(play_ctx.server_ctx, place_pos, block_state_id);
    if !placed {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            pos = ?place_pos,
            "Block place failed: chunk not loaded"
        );
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
    }

    // Broadcast block change to all players except the acting player.
    let entity_id = play_ctx.player.read().entity_id;
    broadcast_block_update(
        play_ctx.server_ctx,
        place_pos,
        block_state_id as i32,
        Some(entity_id),
    );

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

    // Broadcast the block change to all players except the breaker.
    let entity_id = play_ctx.player.read().entity_id;
    broadcast_block_update(
        play_ctx.server_ctx,
        pos,
        u32::from(AIR.0) as i32,
        Some(entity_id),
    );

    // Acknowledge the sequence.
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
        let adj_state =
            get_block(play_ctx.server_ctx, adjacent).unwrap_or(u32::from(AIR.0)) as i32;
        let adj_pkt = ClientboundBlockUpdatePacket {
            pos: adjacent,
            block_state: adj_state,
        };
        play_ctx.conn.send_packet(&adj_pkt).await?;
    }

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
    let _ = ctx.broadcast_tx.send(BroadcastMessage {
        packet_id: ClientboundBlockUpdatePacket::PACKET_ID,
        data: data.freeze(),
        exclude_entity,
        target_entity: None,
    });
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

/// Handles `ServerboundPickItemFromBlockPacket` (0x24) — creative pick block.
///
/// Looks up the block at the target position and places the corresponding
/// item into the player's hotbar, then notifies the client of the slot change.
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

    // Only creative players can pick blocks.
    let game_mode = play_ctx.player.read().game_mode;
    if game_mode != GameMode::Creative {
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

    // Resolve item name to item ID.
    let item_id = item_name_to_id(&item_name);

    // Place the item into the selected hotbar slot.
    let (selected, item_name_log) = {
        let mut player = play_ctx.player.write();
        let slot = player.inventory.selected_slot as usize;
        let name_log = item_name.clone();
        player.inventory.set(slot, ItemStack::new(item_name, 1));
        (player.inventory.selected_slot, name_log)
    };

    // Notify the client of the slot change.
    let slot_data = {
        use oxidized_protocol::codec::slot::{ComponentPatchData, SlotData};
        Some(SlotData {
            item_id,
            count: 1,
            component_data: ComponentPatchData::default(),
        })
    };

    let set_slot = ClientboundSetPlayerInventoryPacket {
        slot: i32::from(selected) + 36, // hotbar protocol offset
        contents: slot_data,
    };
    play_ctx.conn.send_packet(&set_slot).await?;

    debug!(
        peer = %play_ctx.addr,
        name = %play_ctx.player_name,
        pos = ?pkt.pos,
        item = %item_name_log,
        "Creative pick block"
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
        })
    }
}
