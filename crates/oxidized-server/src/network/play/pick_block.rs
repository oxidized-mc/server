//! Pick block handler (middle-click).
//!
//! Unified logic for both Creative and Survival modes (matches vanilla
//! `tryPickItem`):
//!
//! 1. Search hotbar + main inventory for the target item.
//! 2. Found in hotbar → select that slot.
//! 3. Found in main inventory → swap to the best hotbar slot.
//! 4. Not found + Creative → place a fresh stack in the best hotbar slot.
//! 5. Always send `ClientboundSetHeldSlotPacket` to confirm selection.

use bytes::Bytes;
use tracing::debug;

use oxidized_game::inventory::item_ids::item_name_to_id;
use oxidized_game::inventory::item_stack::ItemStack;
use oxidized_game::player::{GameMode, PlayerInventory};
use oxidized_protocol::packets::play::{
    ClientboundSetHeldSlotPacket, ServerboundPickItemFromBlockPacket,
};
use oxidized_world::registry::AIR;

use super::PlayContext;
use super::block_interaction::{get_block, sync_inventory_slot};
use crate::network::ConnectionError;
use crate::network::helpers::decode_packet;

/// Handles `ServerboundPickItemFromBlockPacket` (0x24) — pick block.
///
/// Both Creative and Survival first search the inventory for an existing stack.
/// Creative additionally creates a fresh stack when no match exists.
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
        .world
        .block_registry
        .block_name_from_state_id(block_state)
    {
        Some(name) => name.to_owned(),
        None => return Ok(()),
    };

    // Items not in the registry can't be picked.
    if item_name_to_id(&item_name) < 0 {
        return Ok(());
    }

    // Unified pick-item logic (matches vanilla tryPickItem).
    let target = ItemStack::new(item_name.clone(), 1);
    let (selected_after, changed_slots) = {
        let mut player = play_ctx.player.write();
        let found = player.inventory.find_matching_item(&target);

        let changed = match found {
            Some(slot) if PlayerInventory::is_hotbar_slot(slot) => {
                // Item already in hotbar — just select it.
                player.inventory.selected_slot = slot as u8;
                vec![]
            }
            Some(slot) => {
                // Item in main inventory — swap to best hotbar slot.
                let (hotbar, main) = player.inventory.pick_slot(slot);
                vec![hotbar, main]
            }
            None if game_mode == GameMode::Creative => {
                // Creative: add fresh stack to best hotbar slot.
                player.inventory.add_and_pick_item(target)
            }
            None => {
                // Survival: item not in inventory — nothing to do.
                vec![]
            }
        };
        (player.inventory.selected_slot, changed)
    };

    // Always inform the client of the (possibly new) selected slot.
    let held_pkt = ClientboundSetHeldSlotPacket {
        slot: i32::from(selected_after),
    };
    play_ctx.conn_handle.send_packet(&held_pkt).await?;

    // Sync every inventory slot that was modified.
    for slot in changed_slots {
        sync_inventory_slot(play_ctx, slot).await?;
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
