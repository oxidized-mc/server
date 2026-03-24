//! ServerboundKeepAlivePacket (0x1C) — client keepalive response.
//!
//! Sent by the client in response to a [`ClientboundKeepAlivePacket`](crate::packets::play::ClientboundKeepAlivePacket).
//! The `id` field must match the challenge sent by the server.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
/// 0x1C — Keepalive response from client to server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundKeepAlivePacket {
    /// Challenge ID, must match the server's most recent keepalive.
    pub id: i64,
}

impl Packet for ServerboundKeepAlivePacket {
    const PACKET_ID: i32 = 0x1C;

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
        assert_eq!(<ServerboundKeepAlivePacket as Packet>::PACKET_ID, 0x1C);
    }

    #[test]
    fn test_roundtrip() {
        let pkt = ServerboundKeepAlivePacket { id: 987_654_321 };
        let encoded = pkt.encode();
        let decoded = ServerboundKeepAlivePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_zero() {
        let pkt = ServerboundKeepAlivePacket { id: 0 };
        let encoded = pkt.encode();
        let decoded = ServerboundKeepAlivePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_decode_too_short() {
        let data = Bytes::from_static(&[0]);
        assert!(ServerboundKeepAlivePacket::decode(data).is_err());
    }
}
