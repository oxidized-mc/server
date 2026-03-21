//! Minecraft 26.1 protocol implementation.
//!
//! Packet codec, connection state machine, encryption, compression,
//! and all packet definitions.

pub mod auth;
pub mod chat;
#[macro_use]
pub mod codec;
pub mod compression;
pub mod connection;
pub mod constants;
pub mod crypto;
pub mod packets;
pub mod registry;
pub mod status;
pub mod types;
