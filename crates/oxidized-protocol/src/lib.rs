//! Minecraft 26.1 protocol implementation.
//!
//! Packet codec, connection state machine, encryption, compression,
//! and all packet definitions.

pub mod auth;
pub mod chat;
#[macro_use]
pub mod codec;
pub mod constants;
pub mod packets;
pub mod registry;
pub mod status;
pub mod transport;
pub mod types;

// Re-export transport sub-modules at the crate root for backwards compatibility.
pub use transport::channel;
pub use transport::compression;
pub use transport::connection;
pub use transport::crypto;
pub use transport::handle;
