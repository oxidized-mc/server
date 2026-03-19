//! Clientbound chunk batch start packet.
//!
//! Signals the beginning of a chunk batch. The client expects one or more
//! `ClientboundLevelChunkWithLightPacket`s followed by a
//! `ClientboundChunkBatchFinishedPacket`.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundChunkBatchStartPacket`.

use bytes::{Bytes, BytesMut};

/// Signals the start of a chunk batch. Has no payload.
///
/// Wire format: (empty — zero bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientboundChunkBatchStartPacket;

impl ClientboundChunkBatchStartPacket {
    /// Packet ID in the PLAY state.
    pub const PACKET_ID: i32 = 0x0C; // 12

    /// Decodes from the raw packet body.
    pub fn decode(_data: Bytes) -> Self {
        Self
    }

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        BytesMut::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_empty() {
        let pkt = ClientboundChunkBatchStartPacket;
        let encoded = pkt.encode();
        assert!(encoded.is_empty());
    }

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundChunkBatchStartPacket;
        let encoded = pkt.encode();
        let decoded = ClientboundChunkBatchStartPacket::decode(encoded.freeze());
        assert_eq!(decoded, pkt);
    }
}
