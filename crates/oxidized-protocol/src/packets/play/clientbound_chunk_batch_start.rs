//! Clientbound chunk batch start packet.
//!
//! Signals the beginning of a chunk batch. The client expects one or more
//! `ClientboundLevelChunkWithLightPacket`s followed by a
//! `ClientboundChunkBatchFinishedPacket`.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundChunkBatchStartPacket`.

impl_empty_packet!(
    ClientboundChunkBatchStartPacket,
    0x0C,
    "Signals the start of a chunk batch. Has no payload."
);

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::codec::Packet;

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
        let decoded = ClientboundChunkBatchStartPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }
}
