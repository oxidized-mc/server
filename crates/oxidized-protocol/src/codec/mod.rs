//! Codec primitives for the Minecraft wire protocol.
//!
//! Provides VarInt/VarLong encoding, packet framing, and related utilities.

pub mod frame;
pub mod lp_vec3;
pub mod packet;
pub mod types;
pub mod varint;

pub use packet::{Packet, PacketDecodeError};
