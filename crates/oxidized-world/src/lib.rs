//! World, chunks, blocks, items, and Anvil region file I/O.
//!
//! Handles chunk storage, lighting engine, and world generation.
//! Block/item registries live in [`oxidized_registry`].
//!
//! Chunk data structures are provided by [`oxidized_chunks`] and re-exported here
//! for backward compatibility.

pub use oxidized_chunks as chunk;

pub use oxidized_anvil::anvil;
pub use oxidized_anvil::storage;
