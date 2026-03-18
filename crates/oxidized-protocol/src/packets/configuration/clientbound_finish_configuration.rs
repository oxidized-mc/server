//! Clientbound finish configuration — signals the client to transition from
//! CONFIGURATION state to PLAY state.
//!
//! Corresponds to `net.minecraft.network.protocol.configuration.ClientboundFinishConfigurationPacket`.

use bytes::{Bytes, BytesMut};

/// Clientbound packet `0x01` in the CONFIGURATION state — finish configuration.
///
/// This is an empty packet with no fields. The server sends it when all
/// configuration data has been transmitted and the client should transition
/// to the PLAY state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundFinishConfigurationPacket;

impl ClientboundFinishConfigurationPacket {
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundFinishConfigurationPacket;
        let encoded = pkt.encode();
        assert!(encoded.is_empty());
        let decoded = ClientboundFinishConfigurationPacket::decode(encoded.freeze());
        assert_eq!(decoded, pkt);
    }
}
