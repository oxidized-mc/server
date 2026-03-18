//! Player inventory — minimal stub for Phase 12.
//!
//! Full inventory implementation (item stacks, slot mapping, crafting grid)
//! is deferred to Phase 22. This stub provides the type so `ServerPlayer`
//! can hold an inventory reference.

/// The number of inventory slots (main inventory + armor + offhand + crafting).
pub const INVENTORY_SIZE: usize = 46;

/// A player's inventory.
///
/// Currently a placeholder — Phase 22 will add item stack tracking,
/// slot indices, and container interaction.
#[derive(Debug, Clone)]
pub struct PlayerInventory {
    /// Selected hotbar slot (0–8).
    pub selected_slot: u8,
}

impl PlayerInventory {
    /// Creates an empty inventory.
    pub fn new() -> Self {
        Self { selected_slot: 0 }
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
    }

    #[test]
    fn test_default_inventory() {
        let inv = PlayerInventory::default();
        assert_eq!(inv.selected_slot, 0);
    }
}
