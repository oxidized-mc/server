//! Inventory packet handlers for play state.
//!
//! Handles hotbar selection and creative mode slot placement.

use tracing::{debug, warn};

use oxidized_game::inventory::ItemStack;
use oxidized_game::inventory::item_ids::{item_id_to_name, item_name_to_id};
use oxidized_game::player::inventory::PROTOCOL_SLOT_COUNT;
use oxidized_game::player::{GameMode, PlayerInventory};
use oxidized_protocol::codec::Packet;
use oxidized_protocol::codec::slot::{ComponentPatchData, SlotData};
use oxidized_protocol::packets::play::{
    ClientboundContainerSetContentPacket, ClientboundSetEquipmentPacket,
    ClientboundSetHeldSlotPacket, ServerboundSetCarriedItemPacket,
    ServerboundSetCreativeModeSlotPacket, equipment_slot,
};

use super::PlayContext;
use crate::network::{BroadcastMessage, ConnectionError};
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

    // Broadcast main-hand equipment change to other players.
    let main_hand_item = player.inventory.get_selected();
    let slot_data = if main_hand_item.is_empty() {
        None
    } else {
        Some(item_stack_to_slot_data(main_hand_item))
    };
    let equip_pkt = ClientboundSetEquipmentPacket {
        entity_id: player.entity_id,
        equipments: vec![(equipment_slot::MAIN_HAND, slot_data)],
    };
    let encoded = equip_pkt.encode();
    play_ctx.server_ctx.broadcast(BroadcastMessage {
        packet_id: ClientboundSetEquipmentPacket::PACKET_ID,
        data: encoded.freeze(),
        exclude_entity: Some(player.entity_id),
        target_entity: None,
    });

    debug!(
        peer = %play_ctx.addr,
        name = %play_ctx.player_name,
        slot,
        "Hotbar selection changed"
    );
    Ok(())
}

/// Handles `ServerboundSetCreativeModeSlotPacket` (0x38) — creative item placement.
///
/// Vanilla behavior:
/// - Only valid for players with infinite materials (creative mode).
/// - Slot < 0 means "drop item" (not yet implemented — needs physics).
/// - Valid slot range: 1–45 (covers inventory + armor + offhand).
/// - Item count must not exceed max stack size.
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

    // Vanilla: slot < 0 means drop the item (throttled).
    // TODO(Phase 22+): Implement item dropping with physics.
    if pkt.slot < 0 {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            slot = pkt.slot,
            "SetCreativeModeSlot: drop action not yet implemented"
        );
        return Ok(());
    }

    // Vanilla: valid slot range is 1–45; slot 0 is crafting output (read-only).
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
        },
    };

    // Convert wire SlotData to game ItemStack
    let stack = match &pkt.item {
        Some(slot_data) => {
            let s = slot_data_to_item_stack(slot_data);
            // Vanilla: reject items with count > max stack size.
            if !s.is_empty() {
                let max = oxidized_game::inventory::item_stack::max_stack_size(&s.item);
                if s.count > max {
                    warn!(
                        peer = %play_ctx.addr,
                        name = %play_ctx.player_name,
                        count = s.count,
                        max,
                        "SetCreativeModeSlot: count exceeds max stack size"
                    );
                    return Ok(());
                }
            }
            s
        },
        None => ItemStack::empty(),
    };

    {
        let mut player = play_ctx.player.write();
        player.inventory.set(internal, stack);

        // Broadcast equipment change if the affected slot is visible to other
        // players (armor, offhand, or the currently selected hotbar slot).
        if let Some(equip) = internal_slot_to_equipment(
            internal,
            player.inventory.selected_slot as usize,
        ) {
            let slot_data = {
                let item = player.inventory.get(internal);
                if item.is_empty() {
                    None
                } else {
                    Some(item_stack_to_slot_data(item))
                }
            };
            let equip_pkt = ClientboundSetEquipmentPacket {
                entity_id: player.entity_id,
                equipments: vec![(equip, slot_data)],
            };
            let encoded = equip_pkt.encode();
            play_ctx.server_ctx.broadcast(BroadcastMessage {
                packet_id: ClientboundSetEquipmentPacket::PACKET_ID,
                data: encoded.freeze(),
                exclude_entity: Some(player.entity_id),
                target_entity: None,
            });
        }
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
#[allow(dead_code)] // Will be used when container transactions are implemented.
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
#[allow(dead_code)] // Will be used when pick-item is implemented.
pub async fn send_held_slot(
    play_ctx: &mut PlayContext<'_>,
    slot: u8,
) -> Result<(), ConnectionError> {
    let pkt = ClientboundSetHeldSlotPacket { slot: slot as i32 };
    play_ctx.conn.send_packet(&pkt).await?;
    Ok(())
}

/// Builds the 46-element slot list for `ContainerSetContentPacket`.
///
/// Maps each protocol slot (0–45) to the corresponding physical slot.
/// Crafting slots (0–4) have no backing storage and are always empty.
fn build_inventory_slot_list(inventory: &PlayerInventory) -> Vec<Option<SlotData>> {
    (0..PROTOCOL_SLOT_COUNT as i16)
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
/// Uses the vanilla item registry to map item names to protocol numeric IDs.
/// Must only be called for non-empty stacks.
pub(crate) fn item_stack_to_slot_data(stack: &ItemStack) -> SlotData {
    SlotData {
        count: stack.count,
        item_id: item_name_to_id(&stack.item.0),
        component_data: ComponentPatchData::default(),
    }
}

/// Converts a wire `SlotData` to a game `ItemStack`.
fn slot_data_to_item_stack(data: &SlotData) -> ItemStack {
    let item_name = item_id_to_name(data.item_id);
    ItemStack::new(item_name, data.count)
}

/// Maps an internal inventory slot index to an equipment slot ID, if the slot
/// is visible to other players.
///
/// Returns `None` for main-inventory slots that are not currently selected.
fn internal_slot_to_equipment(internal: usize, selected: usize) -> Option<u8> {
    match internal {
        i if i == selected && i < PlayerInventory::HOTBAR_END => {
            Some(equipment_slot::MAIN_HAND)
        },
        PlayerInventory::OFFHAND_SLOT => Some(equipment_slot::OFF_HAND),
        36 => Some(equipment_slot::HEAD),
        37 => Some(equipment_slot::CHEST),
        38 => Some(equipment_slot::LEGS),
        39 => Some(equipment_slot::FEET),
        _ => None,
    }
}

/// Broadcasts the full equipment set for a player to all other players.
///
/// Used after offhand swaps or any operation that changes multiple equipment
/// slots at once.
pub(crate) fn broadcast_full_equipment(play_ctx: &PlayContext<'_>) {
    let player = play_ctx.player.read();
    let equip_pkt = super::build_equipment_packet(&player);
    let encoded = equip_pkt.encode();
    play_ctx.server_ctx.broadcast(BroadcastMessage {
        packet_id: ClientboundSetEquipmentPacket::PACKET_ID,
        data: encoded.freeze(),
        exclude_entity: Some(player.entity_id),
        target_entity: None,
    });
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
    fn test_empty_item_maps_correctly() {
        // Empty string is not a valid item name; returns -1.
        assert_eq!(item_name_to_id(""), -1);
        // minecraft:air has a real vanilla registry ID.
        let air_id = item_name_to_id("minecraft:air");
        assert!(air_id >= 0);
        assert_eq!(item_id_to_name(air_id), "minecraft:air");
    }

    #[test]
    fn test_build_inventory_slot_list_empty() {
        let inv = PlayerInventory::new();
        let slots = build_inventory_slot_list(&inv);
        assert_eq!(slots.len(), PROTOCOL_SLOT_COUNT);
        assert!(slots.iter().all(|s| s.is_none()));
    }

    #[test]
    fn test_build_inventory_slot_list_with_item() {
        let mut inv = PlayerInventory::new();
        inv.set(0, ItemStack::new("minecraft:stone", 64)); // hotbar 0 → proto 36

        let slots = build_inventory_slot_list(&inv);
        assert_eq!(slots.len(), PROTOCOL_SLOT_COUNT);
        // Protocol slot 36 should have the stone
        assert!(slots[36].is_some());
        assert_eq!(slots[36].as_ref().unwrap().count, 64);
        let stone_id = item_name_to_id("minecraft:stone");
        assert_eq!(slots[36].as_ref().unwrap().item_id, stone_id);
    }

    #[test]
    fn test_slot_data_to_item_stack() {
        let stone_id = item_name_to_id("minecraft:stone");
        let data = SlotData {
            count: 32,
            item_id: stone_id,
            component_data: ComponentPatchData::default(),
        };
        let stack = slot_data_to_item_stack(&data);
        assert_eq!(stack.count, 32);
        assert_eq!(stack.item.0, "minecraft:stone");
    }
}
