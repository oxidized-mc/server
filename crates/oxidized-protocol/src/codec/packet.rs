//! Unified [`Packet`] trait and [`PacketDecodeError`] for all protocol packets.
//!
//! Every Minecraft protocol packet implements the [`Packet`] trait, providing
//! a compile-time packet ID and encode/decode methods with a single unified
//! error type. This replaces per-packet error enums and enables generic
//! `send_packet`/`decode_packet` operations.
//!
//! See [ADR-038](../../../../docs/adr/adr-038-packet-trait.md) for design rationale.

use bytes::{Bytes, BytesMut};
use thiserror::Error;

use super::types::TypeError;
use super::varint::VarIntError;
use crate::types::resource_location::ResourceLocationError;

/// Errors that can occur when decoding any packet from wire bytes.
///
/// This unified error type replaces the per-packet error enums (e.g.
/// `HelloError`, `PingError`, `IntentionError`) that previously existed
/// on each packet struct.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PacketDecodeError {
    /// A wire type could not be read from the buffer.
    #[error(transparent)]
    Type(#[from] TypeError),

    /// A VarInt/VarLong exceeded its maximum size.
    #[error(transparent)]
    VarInt(#[from] VarIntError),

    /// An I/O error occurred during decode.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// A resource location could not be parsed.
    #[error(transparent)]
    ResourceLocation(#[from] ResourceLocationError),

    /// An NBT value could not be decoded.
    #[error(transparent)]
    Nbt(#[from] oxidized_nbt::NbtError),

    /// Packet-specific decode failure with a descriptive message.
    #[error("{0}")]
    InvalidData(String),
}

/// A Minecraft protocol packet that can be encoded and decoded.
///
/// All packets in the protocol implement this trait, providing their wire
/// packet ID and encode/decode methods with a unified error type.
///
/// # Associated Constants
///
/// - [`PACKET_ID`](Packet::PACKET_ID) — the packet ID on the wire
///   (state-dependent, assigned by Mojang).
///
/// # Examples
///
/// ```rust,ignore
/// use oxidized_protocol::codec::Packet;
///
/// fn roundtrip<P: Packet + PartialEq>(pkt: &P) {
///     let encoded = pkt.encode();
///     let decoded = P::decode(encoded.freeze()).unwrap();
///     assert_eq!(pkt, &decoded);
/// }
/// ```
pub trait Packet: Sized + std::fmt::Debug {
    /// The packet ID on the wire (state-dependent).
    const PACKET_ID: i32;

    /// Decodes the packet from raw body bytes (after the packet ID has been
    /// stripped by the framing layer).
    ///
    /// # Errors
    ///
    /// Returns [`PacketDecodeError`] if the bytes cannot be decoded into
    /// a valid packet of this type.
    fn decode(data: Bytes) -> Result<Self, PacketDecodeError>;

    /// Encodes the packet body to bytes (without the packet ID or framing).
    fn encode(&self) -> BytesMut;
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    // --- From conversion tests ---

    #[test]
    fn test_from_type_error() {
        let err = TypeError::UnexpectedEof { need: 4, have: 2 };
        let pde: PacketDecodeError = err.into();
        assert!(matches!(
            pde,
            PacketDecodeError::Type(TypeError::UnexpectedEof { .. })
        ));
    }

    #[test]
    fn test_from_varint_error() {
        let err = VarIntError::TooLarge { max_bytes: 5 };
        let pde: PacketDecodeError = err.into();
        assert!(matches!(
            pde,
            PacketDecodeError::VarInt(VarIntError::TooLarge { .. })
        ));
    }

    #[test]
    fn test_from_io_error() {
        let err = std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "eof");
        let pde: PacketDecodeError = err.into();
        assert!(matches!(pde, PacketDecodeError::Io(_)));
    }

    #[test]
    fn test_from_resource_location_error() {
        let err = ResourceLocationError::EmptyNamespace;
        let pde: PacketDecodeError = err.into();
        assert!(matches!(pde, PacketDecodeError::ResourceLocation(_)));
    }

    #[test]
    fn test_from_nbt_error() {
        let err = oxidized_nbt::NbtError::InvalidTagType(99);
        let pde: PacketDecodeError = err.into();
        assert!(matches!(pde, PacketDecodeError::Nbt(_)));
    }

    #[test]
    fn test_invalid_data_display() {
        let pde = PacketDecodeError::InvalidData("bad intent: 42".into());
        assert_eq!(pde.to_string(), "bad intent: 42");
    }

    #[test]
    fn test_transparent_display_delegates() {
        let err = VarIntError::TooLarge { max_bytes: 5 };
        let pde: PacketDecodeError = err.into();
        assert!(pde.to_string().contains("5 bytes"));
    }

    // --- Question-mark operator ergonomics ---

    fn _decode_with_question_mark(mut data: Bytes) -> Result<u32, PacketDecodeError> {
        let val = super::super::types::read_i32(&mut data)?;
        Ok(val as u32)
    }

    #[test]
    fn test_question_mark_propagation() {
        let result = _decode_with_question_mark(Bytes::new());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PacketDecodeError::Type(TypeError::UnexpectedEof { .. })
        ));
    }

    // --- Packet trait usage with a mock ---

    #[derive(Debug, Clone, PartialEq)]
    struct MockPacket {
        value: i32,
    }

    impl Packet for MockPacket {
        const PACKET_ID: i32 = 0xFF;

        fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
            let value = super::super::types::read_i32(&mut data)?;
            Ok(Self { value })
        }

        fn encode(&self) -> BytesMut {
            let mut buf = BytesMut::with_capacity(4);
            super::super::types::write_i32(&mut buf, self.value);
            buf
        }
    }

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = MockPacket { value: 42 };
        let encoded = pkt.encode();
        let decoded = MockPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(MockPacket::PACKET_ID, 0xFF);
    }

    #[test]
    fn test_packet_trait_decode_error() {
        let result = MockPacket::decode(Bytes::new());
        assert!(result.is_err());
    }

    fn _generic_roundtrip<P: Packet + PartialEq>(pkt: &P) {
        let encoded = pkt.encode();
        let decoded = P::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, &decoded);
    }

    #[test]
    fn test_generic_roundtrip_helper() {
        let pkt = MockPacket { value: -1_000_000 };
        _generic_roundtrip(&pkt);
    }
}
