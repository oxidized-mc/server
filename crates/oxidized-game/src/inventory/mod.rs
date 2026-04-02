//! Re-exports from [`oxidized_inventory`].
//!
//! The core inventory types have been extracted to the standalone
//! `oxidized-inventory` crate. This module provides backward-compatible
//! re-exports so existing `crate::inventory::*` paths continue to work.

/// Re-exports from [`oxidized_inventory::item_stack`].
pub mod item_stack {
    pub use oxidized_inventory::item_stack::*;
}

/// Re-exports from [`oxidized_inventory::item_ids`].
pub mod item_ids {
    pub use oxidized_inventory::item_ids::*;
}

/// Re-exports from [`oxidized_inventory::container`].
pub mod container {
    pub use oxidized_inventory::container::*;
}

pub use oxidized_inventory::{
    ContainerStateId, DataComponentPatch, ItemError, ItemId, ItemStack, MenuType,
    item_id_to_name, item_name_to_id, max_stack_size_by_name,
};
