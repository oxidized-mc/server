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

/// Asserts that encoding then decoding a packet produces the original value.
///
/// Invoked inside `#[test]` functions to reduce roundtrip-test boilerplate.
/// The packet must implement [`Packet`], [`PartialEq`], and [`Debug`].
///
/// # Usage
///
/// ```rust,ignore
/// #[test]
/// fn test_roundtrip() {
///     assert_packet_roundtrip!(MyPacket { field: 42 });
/// }
/// ```
#[cfg(test)]
macro_rules! assert_packet_roundtrip {
    ($pkt:expr) => {{
        let pkt = $pkt;
        let encoded = $crate::codec::Packet::encode(&pkt);
        let decoded =
            <_ as $crate::codec::Packet>::decode(encoded.freeze()).expect("decode failed");
        assert_eq!(pkt, decoded);
    }};
}

/// Asserts that a packet type's [`Packet::PACKET_ID`] matches the expected value.
///
/// # Usage
///
/// ```rust,ignore
/// #[test]
/// fn test_packet_id() {
///     assert_packet_id!(MyPacket, 0x2C);
/// }
/// ```
#[cfg(test)]
macro_rules! assert_packet_id {
    ($pkt_type:ty, $expected:expr) => {
        assert_eq!(<$pkt_type as $crate::codec::Packet>::PACKET_ID, $expected);
    };
}
