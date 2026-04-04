//! Game logic: entities, AI, combat, commands, crafting, and advancements.
//!
//! Uses an ECS architecture for entity management with data-oriented
//! design for cache-friendly tick processing.

pub mod chat;
pub mod chunk;
pub mod commands;
pub mod entity;
pub mod event;
pub mod level;
pub mod net;
pub mod player;

// Re-export extracted crates for backward compatibility.
pub use oxidized_lighting as lighting;
pub use oxidized_physics as physics;
pub use oxidized_worldgen as worldgen;
