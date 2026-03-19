//! Game logic: entities, AI, combat, commands, crafting, and advancements.
//!
//! Uses an ECS architecture for entity management with data-oriented
//! design for cache-friendly tick processing.

pub mod chat;
pub mod chunk;
pub mod commands;
pub mod entity;
pub mod level;
pub mod net;
pub mod physics;
pub mod player;
