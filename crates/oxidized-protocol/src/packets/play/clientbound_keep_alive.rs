//! ClientboundKeepAlivePacket (0x16) — server keepalive ping.
//!
//! Sent every 15 seconds during the PLAY state. The client must respond
//! with [`ServerboundKeepAlivePacket`] echoing the same `id`. If no
//! response arrives within 30 seconds, the server disconnects the client.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::packets::play::PlayPacketError;

/// 0x16 — Keepalive ping from server to client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundKeepAlivePacket {
    /// Challenge ID. The client must echo this back in its response.
    pub id: i64,
}

impl ClientboundKeepAlivePacket {
    /// Packet ID in the PLAY state.
    pub const PACKET_ID: i32 = 0x16;

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(8);
        buf.put_i64(self.id);
        buf
    }

    /// Decodes the packet from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer has fewer than 8 bytes.
    pub fn decode(mut data: Bytes) -> Result<Self, PlayPacketError> {
        if data.remaining() < 8 {
            return Err(PlayPacketError::InvalidData(
                "KeepAlive packet too short".to_string(),
            ));
        }
        Ok(Self { id: data.get_i64() })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_id() {
        assert_eq!(ClientboundKeepAlivePacket::PACKET_ID, 0x16);
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
