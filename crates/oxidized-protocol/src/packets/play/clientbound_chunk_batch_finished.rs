//! Clientbound chunk batch finished packet.
//!
//! Signals the end of a chunk batch. Contains the number of chunks sent in
//! the batch. The client responds with `ServerboundChunkBatchReceivedPacket`.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundChunkBatchFinishedPacket`.

use bytes::{Bytes, BytesMut};

use oxidized_codec::Packet;
use oxidized_codec::packet::PacketDecodeError;
use oxidized_codec::varint;

/// Signals the end of a chunk batch.
///
/// Wire format: `batch_size: VarInt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundChunkBatchFinishedPacket {
    /// Number of chunks in this batch.
    pub batch_size: i32,
}

impl Packet for ClientboundChunkBatchFinishedPacket {
    const PACKET_ID: i32 = 0x0B;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let batch_size = varint::read_varint_buf(&mut data)?;
        Ok(Self { batch_size })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(5);
        varint::write_varint_buf(self.batch_size, &mut buf);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundChunkBatchFinishedPacket { batch_size: 49 };
        let encoded = pkt.encode();
        let decoded = ClientboundChunkBatchFinishedPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.batch_size, 49);
    }

    #[test]
    fn test_zero_batch() {
        let pkt = ClientboundChunkBatchFinishedPacket { batch_size: 0 };
        let encoded = pkt.encode();
        let decoded = ClientboundChunkBatchFinishedPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.batch_size, 0);
    }
}
