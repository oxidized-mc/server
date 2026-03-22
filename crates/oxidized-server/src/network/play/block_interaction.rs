//! Block interaction handlers for play state.
//!
//! Handles block breaking (creative instant + survival mining), block placing,
//! and the sequence acknowledgement protocol. Sign updates are stubbed.

use std::sync::Arc;

use bytes::Bytes;
use tracing::debug;

use oxidized_game::player::GameMode;
use oxidized_protocol::codec::Packet;
use oxidized_protocol::packets::play::serverbound_player_action::PlayerAction;
use oxidized_protocol::packets::play::{
    ClientboundBlockChangedAckPacket, ClientboundBlockUpdatePacket, ServerboundPlayerActionPacket,
    ServerboundSignUpdatePacket, ServerboundUseItemOnPacket, ServerboundUseItemPacket,
};
use oxidized_protocol::types::BlockPos;
use oxidized_world::chunk::ChunkPos;
use oxidized_world::registry::AIR;

use super::PlayContext;
use crate::network::helpers::decode_packet;
use crate::network::{ChatBroadcastMessage, ConnectionError, ServerContext};

/// Handles `ServerboundPlayerActionPacket` (0x29) — block digging actions.
///
/// In creative mode, `StartDestroyBlock` instantly breaks the block.
/// In survival mode, `StopDestroyBlock` finishes the break (tick-based progress
/// tracking is not yet implemented — we accept all break-finish packets).
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

    match pkt.action {
        PlayerAction::StartDestroyBlock => {
            let game_mode = play_ctx.player.read().game_mode;
            if game_mode == GameMode::Creative {
                // Creative mode: instant break.
                do_block_break(play_ctx, pkt.pos, pkt.sequence).await?;
            } else {
                // Survival: client will send StopDestroyBlock when done mining.
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
                // Survival: finish the break.
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

    // Compute placement position: offset from clicked face.
    let place_pos = pkt.hit_result.pos.relative(pkt.hit_result.direction);

    // Get the held item to determine what block to place.
    let held_item = {
        let player = play_ctx.player.read();
        let selected = player.inventory.get_selected();
        selected.item.0.clone()
    };

    // Determine block state to place from the held item.
    // Simple mapping: if item name matches a block, use its default state.
    let block_state_id = match held_item_to_block_state(&held_item, play_ctx.server_ctx) {
        Some(id) => id,
        None => {
            // Not a placeable block — just acknowledge.
            send_ack(play_ctx, pkt.sequence).await?;
            return Ok(());
        },
    };

    // Set the block in chunk storage.
    let placed = set_block(play_ctx.server_ctx, place_pos, block_state_id as u32);
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

    // Broadcast block change to all players.
    broadcast_block_update(play_ctx.server_ctx, place_pos, block_state_id);

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
    if old_state == Some(AIR.0 as i32) {
        // Already air — nothing to break.
        send_ack(play_ctx, sequence).await?;
        return Ok(());
    }

    let set_ok = set_block(play_ctx.server_ctx, pos, AIR.0 as u32);
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

    // Broadcast the block change to all players.
    broadcast_block_update(play_ctx.server_ctx, pos, AIR.0 as i32);

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

/// Gets the block state at a position from shared chunk storage.
///
/// Returns `None` if the chunk is not loaded.
fn get_block(ctx: &Arc<ServerContext>, pos: BlockPos) -> Option<i32> {
    let chunk_pos = ChunkPos::from_block_coords(pos.x, pos.z);
    let chunk_ref = ctx.chunks.get(&chunk_pos)?;
    let chunk = chunk_ref.read();
    chunk
        .get_block_state(pos.x, pos.y, pos.z)
        .ok()
        .map(|s| s as i32)
}

/// Sets the block state at a position in shared chunk storage.
///
/// Returns `true` if the block was successfully set, `false` if the chunk
/// is not loaded or the position is out of bounds.
fn set_block(ctx: &Arc<ServerContext>, pos: BlockPos, state_id: u32) -> bool {
    let chunk_pos = ChunkPos::from_block_coords(pos.x, pos.z);
    if let Some(chunk_ref) = ctx.chunks.get(&chunk_pos) {
        let mut chunk = chunk_ref.write();
        chunk.set_block_state(pos.x, pos.y, pos.z, state_id).is_ok()
    } else {
        false
    }
}

/// Broadcasts a block update to all connected players via the chat broadcast channel.
fn broadcast_block_update(ctx: &Arc<ServerContext>, pos: BlockPos, block_state: i32) {
    let pkt = ClientboundBlockUpdatePacket { pos, block_state };
    let data = pkt.encode();
    let _ = ctx.chat_tx.send(ChatBroadcastMessage {
        packet_id: ClientboundBlockUpdatePacket::PACKET_ID,
        data: data.freeze(),
    });
}

/// Maps a held item name to a block state ID for placement.
///
/// Uses a simple heuristic: strip `minecraft:` prefix, look up the block name
/// in the block registry, and return its default state. Items that are not
/// placeable blocks return `None`.
fn held_item_to_block_state(item_name: &str, _ctx: &Arc<ServerContext>) -> Option<i32> {
    if item_name.is_empty() || item_name == "minecraft:air" {
        return None;
    }

    // Most block items have the same name as the block.
    // Load the block registry (lazy static in oxidized-world).
    let registry = oxidized_world::registry::BlockRegistry::load().ok()?;
    let state = registry.default_state(item_name)?;
    Some(state.0 as i32)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_held_item_to_block_state_air_returns_none() {
        assert!(held_item_to_block_state("", &test_server_ctx()).is_none());
        assert!(held_item_to_block_state("minecraft:air", &test_server_ctx()).is_none());
    }

    #[test]
    fn test_held_item_to_block_state_stone_returns_some() {
        let ctx = test_server_ctx();
        let result = held_item_to_block_state("minecraft:stone", &ctx);
        // Stone's default state is 1
        assert_eq!(result, Some(1));
    }

    #[test]
    fn test_held_item_to_block_state_unknown_returns_none() {
        let ctx = test_server_ctx();
        assert!(held_item_to_block_state("minecraft:diamond_sword", &ctx).is_none());
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
            chat_tx: broadcast::channel(256).0,
            color_char: None,
            commands: oxidized_game::commands::Commands::new(),
            event_bus: oxidized_game::event::EventBus::new(),
            max_players: 20,
            shutdown_tx: broadcast::channel(1).0,
            game_rules: RwLock::new(GameRules::default()),
            tick_rate_manager: RwLock::new(ServerTickRateManager::default()),
            storage: LevelStorageSource::new(""),
            chunks: dashmap::DashMap::new(),
        })
    }
}
