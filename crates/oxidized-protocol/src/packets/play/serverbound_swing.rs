//! Serverbound swing (arm animation) packet.
//!
//! Sent when the player swings their arm (left click in air).
//!
//! Corresponds to `net.minecraft.network.protocol.game.ServerboundSwingPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::varint;

/// Serverbound packet sent when the player swings their arm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundSwingPacket {
    /// 0 = main hand, 1 = off hand.
    pub hand: i32,
}

impl Packet for ServerboundSwingPacket {
    const PACKET_ID: i32 = 0x3F;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let hand = varint::read_varint_buf(&mut data)?;
        Ok(Self { hand })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        varint::write_varint_buf(self.hand, &mut buf);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_main_hand() {
        let pkt = ServerboundSwingPacket { hand: 0 };
        let encoded = pkt.encode();
        let decoded = ServerboundSwingPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_off_hand() {
        let pkt = ServerboundSwingPacket { hand: 1 };
        let encoded = pkt.encode();
        let decoded = ServerboundSwingPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_id() {
        assert_eq!(ServerboundSwingPacket::PACKET_ID, 0x3F);
    }

    #[test]
    fn test_decode_empty_buffer() {
        let data = Bytes::new();
        assert!(ServerboundSwingPacket::decode(data).is_err());
    }
}
