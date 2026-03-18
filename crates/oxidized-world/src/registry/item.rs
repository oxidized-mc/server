//! Item type definitions.

/// An item definition from the item registry.
#[derive(Debug, Clone)]
pub struct Item {
    /// Registry name (e.g., `"minecraft:diamond_sword"`).
    pub name: String,
    /// Maximum number of this item that can stack in one slot.
    pub max_stack_size: u8,
    /// Maximum durability. `0` means the item has no durability.
    pub max_damage: u16,
}

/// A stack of items in an inventory slot.
#[derive(Debug, Clone)]
pub struct ItemStack {
    /// Item registry name.
    pub item: String,
    /// Number of items in the stack.
    pub count: u8,
}

impl ItemStack {
    /// Create a new item stack.
    pub fn new(item: impl Into<String>, count: u8) -> Self {
        Self {
            item: item.into(),
            count,
        }
    }

    /// Returns `true` if this stack is empty (count is zero).
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}
