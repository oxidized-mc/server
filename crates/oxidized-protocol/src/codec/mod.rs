//! Codec primitives for the Minecraft wire protocol.
//!
//! Provides VarInt/VarLong encoding, packet framing, and related utilities.

/// Defines an empty packet (no fields) with a trivial [`Packet`] impl.
///
/// Generates a unit struct with `Debug`, `Clone`, `PartialEq`, `Eq` derives,
/// plus a `Packet` impl whose `decode` ignores the body and `encode` returns
/// an empty buffer.
///
/// # Usage
///
/// ```ignore
/// impl_empty_packet!(ServerboundStatusRequestPacket, 0x00,
///     "Requests the server status JSON (STATUS state).");
/// ```
macro_rules! impl_empty_packet {
    ($name:ident, $id:expr, $doc:literal) => {
        #[doc = $doc]
        ///
        /// This packet has no fields.
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name;

        impl $crate::codec::Packet for $name {
            const PACKET_ID: i32 = $id;

            fn decode(
                _data: ::bytes::Bytes,
            ) -> Result<Self, $crate::codec::packet::PacketDecodeError> {
                Ok(Self)
            }

            fn encode(&self) -> ::bytes::BytesMut {
                ::bytes::BytesMut::new()
            }
        }
    };
}

pub mod frame;
pub mod lp_vec3;
pub mod packet;
pub mod slot;
pub mod types;
pub mod varint;

pub use packet::{Packet, PacketDecodeError};
