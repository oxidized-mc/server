//! Block and item registry system.
//!
//! Provides O(1) lookup of block states by numeric ID and blocks/items by name.
//! Data is loaded from embedded gzipped JSON files extracted from vanilla
//! 26.1-pre-3.

mod block;
mod block_registry;
mod constants;
mod error;
mod item;
mod item_registry;

pub use block::{Block, BlockProperty, BlockState, BlockStateId};
pub use block_registry::BlockRegistry;
pub use constants::*;
pub use error::RegistryError;
pub use item::{Item, ItemStack};
pub use item_registry::ItemRegistry;
