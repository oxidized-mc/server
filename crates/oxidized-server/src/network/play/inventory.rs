//! Inventory packet handlers for play state.
//!
//! Handles hotbar selection and creative mode slot placement.

#![allow(dead_code, unused_imports)] // Functions ready for future phases.

use tracing::{debug, warn};

use oxidized_game::inventory::ItemStack;
use oxidized_game::player::{GameMode, PlayerInventory};
use oxidized_protocol::codec::Packet;
use oxidized_protocol::codec::slot::{ComponentPatchData, SlotData};
use oxidized_protocol::packets::play::{
    ClientboundContainerSetContentPacket, ClientboundSetHeldSlotPacket,
    ServerboundSetCarriedItemPacket, ServerboundSetCreativeModeSlotPacket,
};

use super::PlayContext;
use crate::network::ConnectionError;
use crate::network::helpers::decode_packet;

/// Handles `ServerboundSetCarriedItemPacket` (0x35) — hotbar selection change.
pub async fn handle_set_carried_item(
    play_ctx: &mut PlayContext<'_>,
    data: bytes::Bytes,
) -> Result<(), ConnectionError> {
    let pkt = decode_packet::<ServerboundSetCarriedItemPacket>(
        data,
        play_ctx.addr,
        play_ctx.player_name,
        "SetCarriedItem",
    )?;

    let slot = pkt.slot;
    if !(0..9).contains(&slot) {
        warn!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            slot,
            "SetCarriedItem: invalid hotbar slot"
        );
        return Ok(());
    }

    let mut player = play_ctx.player.write();
    player.inventory.selected_slot = slot as u8;
    debug!(
        peer = %play_ctx.addr,
        name = %play_ctx.player_name,
        slot,
        "Hotbar selection changed"
    );
    Ok(())
}

/// Handles `ServerboundSetCreativeModeSlotPacket` (0x38) — creative item placement.
pub async fn handle_set_creative_mode_slot(
    play_ctx: &mut PlayContext<'_>,
    data: bytes::Bytes,
) -> Result<(), ConnectionError> {
    let pkt = decode_packet::<ServerboundSetCreativeModeSlotPacket>(
        data,
        play_ctx.addr,
        play_ctx.player_name,
        "SetCreativeModeSlot",
    )?;

    let game_mode = play_ctx.player.read().game_mode;
    if game_mode != GameMode::Creative {
        warn!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            game_mode = ?game_mode,
            "SetCreativeModeSlot rejected: not in creative mode"
        );
        return Ok(());
    }

    let internal = match PlayerInventory::from_protocol_slot(pkt.slot) {
        Some(idx) => idx,
        None => {
            debug!(
                peer = %play_ctx.addr,
                name = %play_ctx.player_name,
                slot = pkt.slot,
                "SetCreativeModeSlot: slot has no backing storage (crafting?)"
            );
            return Ok(());
        }
    };

    // Convert wire SlotData to game ItemStack
    let stack = match &pkt.item {
        Some(slot_data) => slot_data_to_item_stack(slot_data),
        None => ItemStack::empty(),
    };

    {
        let mut player = play_ctx.player.write();
        player.inventory.set(internal, stack);
    }

    debug!(
        peer = %play_ctx.addr,
        name = %play_ctx.player_name,
        proto_slot = pkt.slot,
        internal_slot = internal,
        "Creative mode: slot updated"
    );

    Ok(())
}

/// Sends the full player inventory to the client.
///
/// Builds a `ClientboundContainerSetContentPacket` with all 46 protocol
/// slots mapped from the player's 41 physical slots.
pub async fn send_full_inventory(
    play_ctx: &mut PlayContext<'_>,
    state_id: i32,
) -> Result<(), ConnectionError> {
    let items = {
        let player = play_ctx.player.read();
        build_inventory_slot_list(&player.inventory)
    };

    let pkt = ClientboundContainerSetContentPacket {
        container_id: 0,
        state_id,
        items,
        carried_item: None,
    };

    play_ctx.conn.send_packet(&pkt).await?;
    Ok(())
}

/// Sends the currently selected hotbar slot to the client.
pub async fn send_held_slot(
    play_ctx: &mut PlayContext<'_>,
    slot: u8,
) -> Result<(), ConnectionError> {
    let pkt = ClientboundSetHeldSlotPacket {
        slot: slot as i32,
    };
    play_ctx.conn.send_packet(&pkt).await?;
    Ok(())
}

/// Builds the 46-element slot list for `ContainerSetContentPacket`.
///
/// Maps each protocol slot (0–45) to the corresponding physical slot.
/// Crafting slots (0–4) have no backing storage and are always empty.
fn build_inventory_slot_list(inventory: &PlayerInventory) -> Vec<Option<SlotData>> {
    (0..46i16)
        .map(|proto_slot| {
            PlayerInventory::from_protocol_slot(proto_slot).and_then(|internal| {
                let stack = inventory.get(internal);
                if stack.is_empty() {
                    None
                } else {
                    Some(item_stack_to_slot_data(stack))
                }
            })
        })
        .collect()
}

/// Converts a game `ItemStack` to a wire `SlotData`.
///
/// Uses item ID 1 as a placeholder — a proper item registry mapping will
/// be implemented in a later phase.
fn item_stack_to_slot_data(stack: &ItemStack) -> SlotData {
    // TODO(Phase 22+): Look up actual item registry ID from stack.item.0
    let item_id = item_name_to_id(&stack.item.0);
    SlotData {
        count: stack.count,
        item_id,
        component_data: ComponentPatchData::default(),
    }
}

/// Converts a wire `SlotData` to a game `ItemStack`.
fn slot_data_to_item_stack(data: &SlotData) -> ItemStack {
    // TODO(Phase 22+): Look up resource name from item registry ID
    let item_name = item_id_to_name(data.item_id);
    ItemStack::new(item_name, data.count)
}

/// Placeholder item name → ID mapping.
///
/// A proper registry will be built in a later phase. For now, use a small
/// hardcoded mapping for common items plus a hash-based fallback.
fn item_name_to_id(name: &str) -> i32 {
    match name {
        "minecraft:air" | "" => 0,
        "minecraft:stone" => 1,
        "minecraft:dirt" => 10,
        "minecraft:grass_block" => 8,
        "minecraft:cobblestone" => 14,
        "minecraft:oak_planks" => 15,
        "minecraft:diamond" => 802,
        "minecraft:diamond_sword" => 824,
        "minecraft:iron_pickaxe" => 813,
        _ => {
            // Fallback: use a simple hash to generate a deterministic ID.
            // This won't match vanilla, but is stable across calls.
            let mut hash: i32 = 0;
            for b in name.bytes() {
                hash = hash.wrapping_mul(31).wrapping_add(b as i32);
            }
            hash.abs() % 2000 + 100
        }
    }
}

/// Placeholder item ID → name mapping.
fn item_id_to_name(id: i32) -> String {
    match id {
        0 => "minecraft:air".to_string(),
        1 => "minecraft:stone".to_string(),
        10 => "minecraft:dirt".to_string(),
        8 => "minecraft:grass_block".to_string(),
        14 => "minecraft:cobblestone".to_string(),
        15 => "minecraft:oak_planks".to_string(),
        802 => "minecraft:diamond".to_string(),
        824 => "minecraft:diamond_sword".to_string(),
        813 => "minecraft:iron_pickaxe".to_string(),
        _ => format!("minecraft:unknown_{id}"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_item_name_to_id_roundtrip_known() {
        let known = [
            "minecraft:stone",
            "minecraft:dirt",
            "minecraft:diamond",
            "minecraft:diamond_sword",
        ];
        for name in known {
            let id = item_name_to_id(name);
            let back = item_id_to_name(id);
            assert_eq!(back, name, "roundtrip failed for {name}");
        }
    }

    #[test]
    fn test_empty_item_maps_to_air() {
        assert_eq!(item_name_to_id(""), 0);
        assert_eq!(item_name_to_id("minecraft:air"), 0);
        assert_eq!(item_id_to_name(0), "minecraft:air");
    }

    #[test]
    fn test_build_inventory_slot_list_empty() {
        let inv = PlayerInventory::new();
        let slots = build_inventory_slot_list(&inv);
        assert_eq!(slots.len(), 46);
        assert!(slots.iter().all(|s| s.is_none()));
    }

    #[test]
    fn test_build_inventory_slot_list_with_item() {
        let mut inv = PlayerInventory::new();
        inv.set(0, ItemStack::new("minecraft:stone", 64)); // hotbar 0 → proto 36

        let slots = build_inventory_slot_list(&inv);
        assert_eq!(slots.len(), 46);
        // Protocol slot 36 should have the stone
        assert!(slots[36].is_some());
        assert_eq!(slots[36].as_ref().unwrap().count, 64);
        assert_eq!(slots[36].as_ref().unwrap().item_id, 1); // stone = 1
    }

    #[test]
    fn test_slot_data_to_item_stack() {
        let data = SlotData {
            count: 32,
            item_id: 1,
            component_data: ComponentPatchData::default(),
        };
        let stack = slot_data_to_item_stack(&data);
        assert_eq!(stack.count, 32);
        assert_eq!(stack.item.0, "minecraft:stone");
    }
}
