//! ServerboundChatAckPacket (0x06) — client acknowledges received messages.

use bytes::{Bytes, BytesMut};

use crate::codec::packet::PacketDecodeError;
use crate::codec::varint;
use crate::codec::Packet;
use crate::packets::play::PlayPacketError;

/// 0x06 — Client acknowledges message chain offset.
#[derive(Debug, Clone)]
pub struct ServerboundChatAckPacket {
    /// Offset into the message chain being acknowledged.
    pub offset: i32,
}

impl ServerboundChatAckPacket {
    /// Packet ID in the PLAY state serverbound registry.
    pub const PACKET_ID: i32 = 0x06;

    /// Decodes the packet from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer is malformed.
    pub fn decode(mut data: Bytes) -> Result<Self, PlayPacketError> {
        let offset = varint::read_varint_buf(&mut data)?;
        Ok(Self { offset })
    }

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(5);
        varint::write_varint_buf(self.offset, &mut buf);
        buf
    }
}

impl Packet for ServerboundChatAckPacket {
    const PACKET_ID: i32 = 0x06;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let offset = varint::read_varint_buf(&mut data)?;
        Ok(Self { offset })
    }

    fn encode(&self) -> BytesMut {
        self.encode()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_id() {
        assert_eq!(ServerboundChatAckPacket::PACKET_ID, 0x06);
    }

    #[test]
    fn test_roundtrip() {
        let pkt = ServerboundChatAckPacket { offset: 42 };
        let encoded = pkt.encode();
        let decoded = ServerboundChatAckPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.offset, 42);
    }

    #[test]
    fn test_zero_offset() {
        let pkt = ServerboundChatAckPacket { offset: 0 };
        let encoded = pkt.encode();
        let decoded = ServerboundChatAckPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.offset, 0);
    }

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = ServerboundChatAckPacket { offset: 42 };
        let encoded = Packet::encode(&pkt);
        let decoded =
            <ServerboundChatAckPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.offset, pkt.offset);
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(
            <ServerboundChatAckPacket as Packet>::PACKET_ID,
            0x06
        );
    }
}
