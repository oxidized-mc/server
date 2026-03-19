//! Serverbound status request packet — the client asks for server status.
//!
//! This is an empty packet (no fields). It corresponds to
//! `net.minecraft.network.protocol.status.ServerboundStatusRequestPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;

/// Serverbound packet `0x00` in the STATUS state — requests the server status JSON.
///
/// This packet has no fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundStatusRequestPacket;

impl ServerboundStatusRequestPacket {
    /// Packet ID in the STATUS state.
    pub const PACKET_ID: i32 = 0x00;

    /// Decodes from raw packet body (expected to be empty).
    pub fn decode(_data: Bytes) -> Self {
        Self
    }
}

impl Packet for ServerboundStatusRequestPacket {
    const PACKET_ID: i32 = 0x00;

    fn decode(_data: Bytes) -> Result<Self, PacketDecodeError> {
        Ok(Self)
    }

    fn encode(&self) -> BytesMut {
        BytesMut::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_empty_body() {
        let pkt = ServerboundStatusRequestPacket::decode(Bytes::new());
        assert_eq!(pkt, ServerboundStatusRequestPacket);
    }

    #[test]
    fn test_packet_trait_decode() {
        let pkt = <ServerboundStatusRequestPacket as Packet>::decode(Bytes::new()).unwrap();
        assert_eq!(pkt, ServerboundStatusRequestPacket);
    }

    #[test]
    fn test_packet_trait_encode_empty() {
        let pkt = ServerboundStatusRequestPacket;
        let encoded = Packet::encode(&pkt);
        assert!(encoded.is_empty());
    }

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = ServerboundStatusRequestPacket;
        let encoded = Packet::encode(&pkt);
        let decoded = <ServerboundStatusRequestPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(<ServerboundStatusRequestPacket as Packet>::PACKET_ID, 0x00);
    }
}
