//! Minecraft 26.1 protocol implementation.
//!
//! Packet codec, connection state machine, encryption, compression,
//! and all packet definitions.

#[macro_use]
pub mod codec;
pub mod constants;
pub mod packets;
pub mod registry;
pub mod status;
pub mod transport;
