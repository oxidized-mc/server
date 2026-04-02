//! World, chunks, blocks, items, and Anvil region file I/O.
//!
//! Handles chunk storage, lighting engine, and world generation.
//! Block/item registries live in [`oxidized_registry`].

pub mod anvil;
pub mod chunk;
pub mod storage;
