//! Shared coordinate types for the Oxidized workspace.
//!
//! This crate provides fundamental spatial types (`ChunkPos`) that are needed
//! by multiple crates (protocol, world, game) without introducing circular
//! dependencies.

mod chunk_pos;

pub use chunk_pos::ChunkPos;
