//! Serverbound finish configuration — the client acknowledges it has received
//! all configuration data and is ready for the PLAY state.
//!
//! Corresponds to `net.minecraft.network.protocol.configuration.ServerboundFinishConfigurationPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;

/// Serverbound packet `0x01` in the CONFIGURATION state — finish configuration.
///
/// This is an empty packet with no fields. The client sends it after receiving
/// [`ClientboundFinishConfigurationPacket`](super::ClientboundFinishConfigurationPacket)
/// to confirm it is ready to transition to the PLAY state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundFinishConfigurationPacket;

impl ServerboundFinishConfigurationPacket {
    /// Packet ID in the CONFIGURATION state.
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

impl Packet for ServerboundFinishConfigurationPacket {
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
        let pkt = ServerboundFinishConfigurationPacket;
        let encoded = pkt.encode();
        assert!(encoded.is_empty());
        let decoded = ServerboundFinishConfigurationPacket::decode(encoded.freeze());
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = ServerboundFinishConfigurationPacket;
        let encoded = Packet::encode(&pkt);
        let decoded =
            <ServerboundFinishConfigurationPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(
            <ServerboundFinishConfigurationPacket as Packet>::PACKET_ID,
            0x03
        );
    }
}
