//! Block and item registry system.
//!
//! Block data is generated at compile time from embedded vanilla JSON
//! (see `generated`).  Provides O(1) lookup of block states by numeric ID
//! and blocks/items by name.

mod block;
mod block_registry;
mod constants;
mod error;
pub(crate) mod generated;
mod item;
mod item_registry;

pub use block::{BlockDef, BlockStateEntry, BlockStateFlags, BlockStateId, PropertyDef};
pub use block_registry::BlockRegistry;
pub use constants::*;
pub use error::RegistryError;
pub use item::{Item, ItemStack};
pub use item_registry::ItemRegistry;
