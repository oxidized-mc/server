//! Clientbound finish configuration — signals the client to transition from
//! CONFIGURATION state to PLAY state.
//!
//! Corresponds to `net.minecraft.network.protocol.configuration.ClientboundFinishConfigurationPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;

/// Clientbound packet `0x01` in the CONFIGURATION state — finish configuration.
///
/// This is an empty packet with no fields. The server sends it when all
/// configuration data has been transmitted and the client should transition
/// to the PLAY state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundFinishConfigurationPacket;

impl Packet for ClientboundFinishConfigurationPacket {
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
        let pkt = ClientboundFinishConfigurationPacket;
        let encoded = Packet::encode(&pkt);
        assert!(encoded.is_empty());
        let decoded =
            <ClientboundFinishConfigurationPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_id() {
        assert_eq!(
            <ClientboundFinishConfigurationPacket as Packet>::PACKET_ID,
            0x03
        );
    }
}
