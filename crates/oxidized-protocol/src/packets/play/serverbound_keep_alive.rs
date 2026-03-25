//! ServerboundKeepAlivePacket (0x1C) — client keepalive response.
//!
//! Sent by the client in response to a [`ClientboundKeepAlivePacket`](crate::packets::play::ClientboundKeepAlivePacket).
//! The `id` field must match the challenge sent by the server.

use bytes::{BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;
/// 0x1C — Keepalive response from client to server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundKeepAlivePacket {
    /// Challenge ID, must match the server's most recent keepalive.
    pub id: i64,
}

impl Packet for ServerboundKeepAlivePacket {
    const PACKET_ID: i32 = 0x1C;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let id = types::read_i64(&mut data)?;
        Ok(Self { id })
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
        assert_packet_id!(ServerboundKeepAlivePacket, 0x1C);
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
