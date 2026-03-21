//! Player inventory — 41 physical slots with protocol slot mapping.
//!
//! The player inventory consists of 41 physical storage slots:
//! - Hotbar: indices 0–8 (9 slots)
//! - Main inventory: indices 9–35 (27 slots)
//! - Armor: indices 36–39 (4 slots: head/chest/legs/feet)
//! - Offhand: index 40 (1 slot)
//!
//! Protocol window 0 uses a different numbering scheme. Use
//! [`PlayerInventory::to_protocol_slot`] and [`PlayerInventory::from_protocol_slot`]
//! to convert between internal and protocol indices.

use crate::inventory::item_stack::{max_stack_size, ItemStack};

/// The number of protocol slots for window 0 (includes crafting grid).
pub const PROTOCOL_SLOT_COUNT: usize = 46;

/// A player's inventory.
///
/// Stores 41 physical item slots in internal order:
/// `[hotbar 0..9][main 9..36][armor 36..40][offhand 40]`
///
/// The `selected` field tracks which hotbar slot (0–8) is currently active.
///
/// # Examples
///
/// ```
/// use oxidized_game::player::PlayerInventory;
/// use oxidized_game::inventory::ItemStack;
///
/// let mut inv = PlayerInventory::new();
/// inv.set(0, ItemStack::new("minecraft:stone", 64));
/// assert_eq!(inv.get(0).count, 64);
/// assert_eq!(inv.get_selected().count, 64);
/// ```
#[derive(Debug, Clone)]
pub struct PlayerInventory {
    /// 41 physical slots: [hotbar 0..9][main 9..36][armor 36..40][offhand 40]
    slots: [ItemStack; Self::TOTAL_SLOTS],
    /// Selected hotbar slot (0–8).
    pub selected_slot: u8,
}

impl PlayerInventory {
    /// Start of hotbar slots (internal index).
    pub const HOTBAR_START: usize = 0;
    /// End of hotbar slots (exclusive).
    pub const HOTBAR_END: usize = 9;
    /// Start of main inventory slots (internal index).
    pub const MAIN_START: usize = 9;
    /// End of main inventory slots (exclusive).
    pub const MAIN_END: usize = 36;
    /// Start of armor slots (internal index).
    pub const ARMOR_START: usize = 36;
    /// End of armor slots (exclusive).
    pub const ARMOR_END: usize = 40;
    /// Offhand slot (internal index).
    pub const OFFHAND_SLOT: usize = 40;
    /// Total number of physical slots.
    pub const TOTAL_SLOTS: usize = 41;

    /// Creates an empty inventory with all slots set to [`ItemStack::empty()`].
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| ItemStack::empty()),
            selected_slot: 0,
        }
    }

    /// Returns a reference to the item in the given internal slot.
    ///
    /// # Panics
    ///
    /// Panics if `slot >= TOTAL_SLOTS`.
    pub fn get(&self, slot: usize) -> &ItemStack {
        &self.slots[slot]
    }

    /// Returns a mutable reference to the item in the given internal slot.
    ///
    /// # Panics
    ///
    /// Panics if `slot >= TOTAL_SLOTS`.
    pub fn get_mut(&mut self, slot: usize) -> &mut ItemStack {
        &mut self.slots[slot]
    }

    /// Sets the item in the given internal slot.
    ///
    /// # Panics
    ///
    /// Panics if `slot >= TOTAL_SLOTS`.
    pub fn set(&mut self, slot: usize, stack: ItemStack) {
        self.slots[slot] = stack;
    }

    /// Returns a reference to the currently selected hotbar item.
    pub fn get_selected(&self) -> &ItemStack {
        &self.slots[self.selected_slot as usize]
    }

    /// Returns a reference to the armor item at the given armor index (0–3).
    ///
    /// 0 = head, 1 = chest, 2 = legs, 3 = feet.
    ///
    /// # Panics
    ///
    /// Panics if `index > 3`.
    pub fn get_armor(&self, index: u8) -> &ItemStack {
        &self.slots[Self::ARMOR_START + index as usize]
    }

    /// Returns a reference to the offhand item.
    pub fn get_offhand(&self) -> &ItemStack {
        &self.slots[Self::OFFHAND_SLOT]
    }

    /// Iterates over all slots as `(internal_index, &ItemStack)`.
    pub fn all_slots(&self) -> impl Iterator<Item = (usize, &ItemStack)> {
        self.slots.iter().enumerate()
    }

    /// Adds a stack to the inventory, first filling existing stacks then empty slots.
    ///
    /// Only searches hotbar (0–8) and main inventory (9–35), matching vanilla
    /// behavior. Armor and offhand slots are never auto-filled.
    ///
    /// Returns the remaining count that could not be inserted (0 = fully inserted).
    pub fn add_item(&mut self, mut stack: ItemStack) -> i32 {
        // First try the currently selected slot (vanilla priority)
        let sel = self.selected_slot as usize;
        if !self.slots[sel].is_empty() && self.slots[sel].is_stackable_with(&stack) {
            let max = max_stack_size(&stack.item);
            let space = max - self.slots[sel].count;
            if space > 0 {
                let moved = stack.count.min(space);
                self.slots[sel].count += moved;
                stack.count -= moved;
                if stack.count <= 0 {
                    return 0;
                }
            }
        }

        // Fill existing stacks of the same item (hotbar + main only)
        for i in Self::HOTBAR_START..Self::MAIN_END {
            if i == sel {
                continue; // already checked
            }
            if !self.slots[i].is_empty() && self.slots[i].is_stackable_with(&stack) {
                let max = max_stack_size(&stack.item);
                let space = max - self.slots[i].count;
                if space > 0 {
                    let moved = stack.count.min(space);
                    self.slots[i].count += moved;
                    stack.count -= moved;
                    if stack.count <= 0 {
                        return 0;
                    }
                }
            }
        }
        // Then fill empty slots (hotbar + main only)
        for i in Self::HOTBAR_START..Self::MAIN_END {
            if self.slots[i].is_empty() {
                self.slots[i] = stack;
                return 0;
            }
        }
        stack.count // leftovers
    }

    /// Converts an internal slot index to a protocol window-0 slot index.
    ///
    /// Protocol window 0 layout:
    /// - 0: crafting output
    /// - 1–4: crafting grid (2×2)
    /// - 5–8: armor (head/chest/legs/feet)
    /// - 9–35: main inventory
    /// - 36–44: hotbar
    /// - 45: offhand
    pub fn to_protocol_slot(internal: usize) -> i16 {
        match internal {
            0..9 => (internal as i16) + 36,  // hotbar → protocol 36–44
            9..36 => internal as i16,         // main   → protocol 9–35
            36..40 => (internal as i16) - 31, // armor  → protocol 5–8
            40 => 45,                          // offhand → protocol 45
            _ => -1,
        }
    }

    /// Converts a protocol window-0 slot index to an internal index.
    ///
    /// Returns `None` for crafting slots (0–4) or out-of-range values,
    /// since crafting slots have no backing storage in `PlayerInventory`.
    pub fn from_protocol_slot(protocol: i16) -> Option<usize> {
        match protocol {
            5..=8 => Some((protocol as usize) + 31),   // armor
            9..=35 => Some(protocol as usize),           // main
            36..=44 => Some((protocol as usize) - 36),  // hotbar
            45 => Some(40),                               // offhand
            _ => None,
        }
    }
}

impl Default for PlayerInventory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_new_inventory() {
        let inv = PlayerInventory::new();
        assert_eq!(inv.selected_slot, 0);
        for i in 0..PlayerInventory::TOTAL_SLOTS {
            assert!(inv.get(i).is_empty());
        }
    }

    #[test]
    fn test_default_inventory() {
        let inv = PlayerInventory::default();
        assert_eq!(inv.selected_slot, 0);
    }

    #[test]
    fn test_set_and_get() {
        let mut inv = PlayerInventory::new();
        inv.set(0, ItemStack::new("minecraft:stone", 64));
        assert_eq!(inv.get(0).item.0, "minecraft:stone");
        assert_eq!(inv.get(0).count, 64);
    }

    #[test]
    fn test_get_selected() {
        let mut inv = PlayerInventory::new();
        inv.set(0, ItemStack::new("minecraft:stone", 64));
        assert_eq!(inv.get_selected().item.0, "minecraft:stone");

        inv.selected_slot = 3;
        assert!(inv.get_selected().is_empty());
    }

    #[test]
    fn test_get_armor() {
        let mut inv = PlayerInventory::new();
        inv.set(36, ItemStack::new("minecraft:diamond_helmet", 1));
        assert_eq!(inv.get_armor(0).item.0, "minecraft:diamond_helmet");
    }

    #[test]
    fn test_get_offhand() {
        let mut inv = PlayerInventory::new();
        inv.set(40, ItemStack::new("minecraft:shield", 1));
        assert_eq!(inv.get_offhand().item.0, "minecraft:shield");
    }

    // --- Protocol slot mapping roundtrips ---

    #[test]
    fn test_protocol_slot_hotbar_roundtrip() {
        for i in 0u8..9 {
            let proto = PlayerInventory::to_protocol_slot(i as usize);
            assert!(proto >= 36 && proto <= 44, "hotbar {i} -> proto {proto}");
            let back = PlayerInventory::from_protocol_slot(proto).unwrap();
            assert_eq!(back, i as usize, "hotbar slot {i} roundtrip failed");
        }
    }

    #[test]
    fn test_protocol_slot_main_inventory_roundtrip() {
        for i in 9usize..36 {
            let proto = PlayerInventory::to_protocol_slot(i);
            assert_eq!(proto, i as i16, "main slot {i}");
            let back = PlayerInventory::from_protocol_slot(proto).unwrap();
            assert_eq!(back, i, "main inventory slot {i} roundtrip failed");
        }
    }

    #[test]
    fn test_protocol_slot_armor_roundtrip() {
        for i in 36usize..40 {
            let proto = PlayerInventory::to_protocol_slot(i);
            assert!(proto >= 5 && proto <= 8, "armor {i} -> proto {proto}");
            let back = PlayerInventory::from_protocol_slot(proto).unwrap();
            assert_eq!(back, i, "armor slot {i} roundtrip failed");
        }
    }

    #[test]
    fn test_protocol_slot_offhand_roundtrip() {
        let proto = PlayerInventory::to_protocol_slot(40);
        assert_eq!(proto, 45);
        let back = PlayerInventory::from_protocol_slot(45).unwrap();
        assert_eq!(back, 40);
    }

    #[test]
    fn test_invalid_protocol_slot_returns_none() {
        assert!(PlayerInventory::from_protocol_slot(-1).is_none());
        assert!(PlayerInventory::from_protocol_slot(0).is_none()); // crafting output
        assert!(PlayerInventory::from_protocol_slot(1).is_none()); // crafting grid
        assert!(PlayerInventory::from_protocol_slot(4).is_none()); // crafting grid
        assert!(PlayerInventory::from_protocol_slot(46).is_none());
    }

    // --- add_item ---

    #[test]
    fn test_add_item_fills_empty_slot() {
        let mut inv = PlayerInventory::new();
        let leftovers = inv.add_item(ItemStack::new("minecraft:stone", 1));
        assert_eq!(leftovers, 0);
        assert_eq!(inv.get(0).item.0, "minecraft:stone");
    }

    #[test]
    fn test_add_item_stacks_with_existing() {
        let mut inv = PlayerInventory::new();
        inv.set(0, ItemStack::new("minecraft:stone", 32));
        let leftovers = inv.add_item(ItemStack::new("minecraft:stone", 16));
        assert_eq!(leftovers, 0);
        assert_eq!(inv.get(0).count, 48);
    }

    #[test]
    fn test_add_item_returns_overflow_when_full() {
        let mut inv = PlayerInventory::new();
        // Fill hotbar + main (slots 0–35) — add_item only searches these
        for i in 0..PlayerInventory::MAIN_END {
            inv.set(i, ItemStack::new("minecraft:stone", 64));
        }
        let leftovers = inv.add_item(ItemStack::new("minecraft:stone", 32));
        assert_eq!(leftovers, 32, "should return unfitted amount");
    }

    #[test]
    fn test_add_item_to_new_slot_when_existing_full() {
        let mut inv = PlayerInventory::new();
        inv.set(0, ItemStack::new("minecraft:stone", 64)); // full
        let leftovers = inv.add_item(ItemStack::new("minecraft:stone", 10));
        assert_eq!(leftovers, 0);
        assert_eq!(inv.get(1).item.0, "minecraft:stone");
        assert_eq!(inv.get(1).count, 10);
    }

    #[test]
    fn test_add_item_never_fills_armor_or_offhand() {
        let mut inv = PlayerInventory::new();
        // Fill hotbar + main with different items so no stacking occurs
        for i in 0..PlayerInventory::MAIN_END {
            inv.set(i, ItemStack::new(format!("minecraft:item_{i}"), 64));
        }
        // Armor and offhand should remain empty after add_item fails
        let leftovers = inv.add_item(ItemStack::new("minecraft:stone", 10));
        assert_eq!(leftovers, 10);
        for i in PlayerInventory::ARMOR_START..=PlayerInventory::OFFHAND_SLOT {
            assert!(inv.get(i).is_empty(), "slot {i} should remain empty");
        }
    }

    #[test]
    fn test_add_item_prefers_selected_slot() {
        let mut inv = PlayerInventory::new();
        inv.selected_slot = 3;
        // Put partial stacks of stone in slots 0 and 3
        inv.set(0, ItemStack::new("minecraft:stone", 32));
        inv.set(3, ItemStack::new("minecraft:stone", 32));
        // Adding more stone should fill selected slot (3) first
        let leftovers = inv.add_item(ItemStack::new("minecraft:stone", 16));
        assert_eq!(leftovers, 0);
        assert_eq!(inv.get(3).count, 48, "selected slot should be filled first");
        assert_eq!(inv.get(0).count, 32, "non-selected slot should be unchanged");
    }

    // --- all_slots ---

    #[test]
    fn test_all_slots_count() {
        let inv = PlayerInventory::new();
        assert_eq!(inv.all_slots().count(), PlayerInventory::TOTAL_SLOTS);
    }
}

