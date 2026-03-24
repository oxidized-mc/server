//! Pick block handler (middle-click).
//!
//! Creative: places a fresh stack of the target block into the selected hotbar slot.
//! Survival: searches inventory for an existing stack and moves it to hotbar.

use bytes::Bytes;
use tracing::debug;

use oxidized_game::inventory::item_ids::item_name_to_id;
use oxidized_game::inventory::item_stack::ItemStack;
use oxidized_game::player::GameMode;
use oxidized_protocol::packets::play::{
    ClientboundSetPlayerInventoryPacket, ServerboundPickItemFromBlockPacket,
};
use oxidized_world::registry::AIR;

use super::PlayContext;
use super::block_interaction::{get_block, sync_inventory_slot};
use crate::network::ConnectionError;
use crate::network::helpers::decode_packet;

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
        play_ctx.conn_handle.send_packet(&set_slot).await?;
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
                let held_pkt = oxidized_protocol::packets::play::ClientboundSetHeldSlotPacket {
                    slot: slot as i32,
                };
                play_ctx.conn_handle.send_packet(&held_pkt).await?;
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
