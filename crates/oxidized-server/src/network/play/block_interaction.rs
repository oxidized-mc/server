//! Block interaction shared helpers and re-exports.
//!
//! Contains shared validation, block access, and broadcast utilities used
//! by the mining, placement, sign editing, and pick block submodules.

use std::sync::Arc;

use oxidized_protocol::chat::Component;
use oxidized_protocol::codec::Packet;
use oxidized_protocol::packets::play::{
    ClientboundBlockChangedAckPacket, ClientboundBlockUpdatePacket,
    ClientboundSetPlayerInventoryPacket,
};
use oxidized_protocol::types::{BlockPos, Direction};
use oxidized_world::chunk::ChunkPos;
use oxidized_world::chunk::level_chunk::{OVERWORLD_MAX_Y, OVERWORLD_MIN_Y};
use oxidized_world::registry::AIR;

use super::PlayContext;
use crate::network::{BroadcastMessage, ConnectionError, ServerContext};

// Re-export public handlers from submodules so call-sites don't need to change.
pub use super::mining::handle_player_action;
pub use super::pick_block::handle_pick_item_from_block;
pub use super::placement::{handle_use_item, handle_use_item_on};
pub use super::sign_editing::handle_sign_update;

use oxidized_game::player::GameMode;
use oxidized_protocol::types::Vec3;

/// Survival block interaction reach (squared).
///
/// Vanilla: `getBlockInteractionRange() + additionalRange + 0.5`
/// = 4.5 + 1.0 + 0.5 = 6.0 blocks → 36.0 sq
pub(super) const SURVIVAL_REACH_DISTANCE_SQ: f64 = 6.0 * 6.0;

/// Creative block interaction reach (squared).
///
/// Vanilla: 5.0 + 1.0 + 0.5 = 6.5 blocks → 42.25 sq
pub(super) const CREATIVE_REACH_DISTANCE_SQ: f64 = 6.5 * 6.5;

/// Minimum valid build height for overworld (inclusive).
pub(super) const MIN_BUILD_HEIGHT: i32 = OVERWORLD_MIN_Y;

/// Maximum valid build height for overworld (inclusive).
/// `OVERWORLD_MAX_Y` is 320 (exclusive), so the last valid Y is 319.
pub(super) const MAX_BUILD_HEIGHT: i32 = OVERWORLD_MAX_Y - 1;

/// Returns the squared distance from the player's eye position to the center
/// of the given block.
pub(super) fn player_distance_to_block_sq(play_ctx: &PlayContext<'_>, pos: BlockPos) -> f64 {
    let player = play_ctx.player.read();
    let eye_height = if player.is_sneaking { 1.27 } else { 1.62 };
    let eye = Vec3::new(player.pos.x, player.pos.y + eye_height, player.pos.z);
    let block_center = Vec3::new(pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5);
    eye.distance_to_sqr(block_center)
}

/// Returns `true` if the player is within block interaction range.
///
/// Creative mode players have a longer reach (6.5 blocks) than survival/adventure (6.0).
pub(super) fn is_within_reach(play_ctx: &PlayContext<'_>, pos: BlockPos) -> bool {
    let limit = if play_ctx.player.read().game_mode == GameMode::Creative {
        CREATIVE_REACH_DISTANCE_SQ
    } else {
        SURVIVAL_REACH_DISTANCE_SQ
    };
    player_distance_to_block_sq(play_ctx, pos) <= limit
}

/// Returns `true` if the position is within valid overworld build limits.
pub(super) fn is_within_build_height(pos: BlockPos) -> bool {
    pos.y >= MIN_BUILD_HEIGHT && pos.y <= MAX_BUILD_HEIGHT
}

/// Returns `true` if the position is inside the spawn protection zone.
///
/// Vanilla uses Chebyshev distance: `max(|bx - sx|, |bz - sz|)`. A radius
/// of 0 disables spawn protection entirely.
///
/// TODO: Accept player info and skip protection for operators once ops.json
/// is implemented. Currently all players are treated as non-ops.
pub(super) fn is_spawn_protected(ctx: &ServerContext, pos: BlockPos) -> bool {
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

/// Gets the block state at a position from shared chunk storage.
///
/// Returns `None` if the chunk is not loaded.
pub(super) fn get_block(ctx: &Arc<ServerContext>, pos: BlockPos) -> Option<u32> {
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
pub(super) fn set_block(ctx: &Arc<ServerContext>, pos: BlockPos, state_id: u32) -> bool {
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
pub(super) fn broadcast_block_update(
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

/// Re-syncs the client's view of a block (and optionally its adjacent face
/// block) by sending the current server-side state. Used when rejecting a
/// block action (e.g., out of reach). Vanilla sends both the target and the
/// adjacent block so the client's prediction is fully corrected.
pub(super) async fn resync_block(
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

/// Sends a `ClientboundBlockChangedAckPacket` for the given sequence number.
pub(super) async fn send_ack(
    play_ctx: &mut PlayContext<'_>,
    sequence: i32,
) -> Result<(), ConnectionError> {
    let pkt = ClientboundBlockChangedAckPacket { sequence };
    play_ctx.conn.send_packet(&pkt).await?;
    Ok(())
}

/// Sends an overlay (actionbar) message to the player.
pub(super) async fn send_actionbar(
    play_ctx: &mut PlayContext<'_>,
    message: Component,
) -> Result<(), ConnectionError> {
    use oxidized_protocol::packets::play::ClientboundSystemChatPacket;
    let pkt = ClientboundSystemChatPacket {
        content: message,
        is_overlay: true,
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
pub(super) async fn sync_inventory_slot(
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use oxidized_world::registry::BlockRegistry;

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

    // -- Block access tests --

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
}
