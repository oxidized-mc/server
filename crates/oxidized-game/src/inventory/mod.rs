//! Inventory system: item stacks, player inventory, and container menus.
//!
//! This module implements the full Minecraft inventory model including:
//! - [`ItemStack`] with `DataComponentPatch` (1.20.5+ format)
//! - [`PlayerInventory`] with 41 slots and protocol slot mapping
//! - [`ContainerMenu`] trait for container interactions (future phases)
//! - [`MenuType`] enum for all container types
//! - [`item_ids`] placeholder item ID mapping (until Phase 22+ registry)

pub mod container;
pub mod item_ids;
pub mod item_stack;

pub use container::{ContainerStateId, MenuType};
pub use item_ids::{item_id_to_name, item_name_to_id};
pub use item_stack::{DataComponentPatch, ItemError, ItemId, ItemStack};
