//! Serverbound login acknowledged — the client confirms it received the login
//! success and is ready to transition to the CONFIGURATION state.
//!
//! Corresponds to `net.minecraft.network.protocol.login.ServerboundLoginAcknowledgedPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::packet::PacketDecodeError;
use crate::codec::Packet;

/// Serverbound packet `0x03` in the LOGIN state — login acknowledged.
///
/// This is an empty packet with no fields. The client sends it after receiving
/// [`ClientboundLoginFinishedPacket`](super::ClientboundLoginFinishedPacket) to
/// signal it is ready to transition to the CONFIGURATION state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundLoginAcknowledgedPacket;

impl ServerboundLoginAcknowledgedPacket {
    /// Packet ID in the LOGIN state.
    pub const PACKET_ID: i32 = 0x03;

    /// Decodes from the raw packet body.
    ///
    /// This packet carries no fields, so `data` is ignored.
    pub fn decode(_data: Bytes) -> Self {
        Self
    }

    /// Encodes the packet body (without packet ID).
    ///
    /// Returns an empty buffer since this packet has no fields.
    pub fn encode(&self) -> BytesMut {
        BytesMut::new()
    }
}

impl Packet for ServerboundLoginAcknowledgedPacket {
    const PACKET_ID: i32 = 0x03;

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
    fn test_roundtrip() {
        let pkt = ServerboundLoginAcknowledgedPacket;
        let encoded = pkt.encode();
        let decoded = ServerboundLoginAcknowledgedPacket::decode(encoded.freeze());
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = ServerboundLoginAcknowledgedPacket;
        let encoded = Packet::encode(&pkt);
        let decoded =
            <ServerboundLoginAcknowledgedPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(
            <ServerboundLoginAcknowledgedPacket as Packet>::PACKET_ID,
            0x03
        );
    }
}
