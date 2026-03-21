//! ClientboundKeepAlivePacket (0x2C) — server keepalive ping.
//!
//! Sent every 15 seconds during the PLAY state. The client must respond
//! with [`ServerboundKeepAlivePacket`] echoing the same `id`. If no
//! response arrives within 30 seconds, the server disconnects the client.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
/// 0x2C — Keepalive ping from server to client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundKeepAlivePacket {
    /// Challenge ID. The client must echo this back in its response.
    pub id: i64,
}

impl Packet for ClientboundKeepAlivePacket {
    const PACKET_ID: i32 = 0x2C;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        if data.remaining() < 8 {
            return Err(PacketDecodeError::InvalidData(
                "KeepAlive packet too short".to_string(),
            ));
        }
        Ok(Self { id: data.get_i64() })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(8);
        buf.put_i64(self.id);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_id() {
        assert_eq!(<ClientboundKeepAlivePacket as Packet>::PACKET_ID, 0x2C);
    }

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundKeepAlivePacket { id: 123_456_789 };
        let encoded = pkt.encode();
        let decoded = ClientboundKeepAlivePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_negative() {
        let pkt = ClientboundKeepAlivePacket { id: -42 };
        let encoded = pkt.encode();
        let decoded = ClientboundKeepAlivePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_decode_too_short() {
        let data = Bytes::from_static(&[0, 1, 2]);
        assert!(ClientboundKeepAlivePacket::decode(data).is_err());
    }
}
