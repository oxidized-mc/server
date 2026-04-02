//! Clientbound block changed ack packet.
//!
//! Acknowledges a client's block change prediction sequence.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundBlockChangedAckPacket`.

use bytes::{Bytes, BytesMut};

use oxidized_codec::Packet;
use oxidized_codec::packet::PacketDecodeError;
use oxidized_codec::varint;

/// Acknowledges a client's block change prediction.
///
/// Wire format: `sequence: VarInt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundBlockChangedAckPacket {
    /// Sequence number to acknowledge (matches the client's
    /// `ServerboundPlayerAction.sequence`).
    pub sequence: i32,
}

impl Packet for ClientboundBlockChangedAckPacket {
    const PACKET_ID: i32 = 0x04;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let sequence = varint::read_varint_buf(&mut data)?;
        Ok(Self { sequence })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(5);
        varint::write_varint_buf(self.sequence, &mut buf);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundBlockChangedAckPacket { sequence: 42 };
        let encoded = pkt.encode();
        let decoded = ClientboundBlockChangedAckPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.sequence, 42);
    }

    #[test]
    fn test_zero_sequence() {
        let pkt = ClientboundBlockChangedAckPacket { sequence: 0 };
        let encoded = pkt.encode();
        let decoded = ClientboundBlockChangedAckPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }
}
