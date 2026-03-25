//! Serverbound ping request — the client sends a timestamp to measure latency.
//!
//! Corresponds to `net.minecraft.network.protocol.status.ServerboundPingRequestPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;

/// Serverbound packet `0x01` in the STATUS state — ping with a timestamp.
///
/// The server must echo the same `time` value back in a
/// [`ClientboundPongResponsePacket`](super::ClientboundPongResponsePacket).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundPingRequestPacket {
    /// Timestamp sent by the client (typically `System.currentTimeMillis()`).
    pub time: i64,
}

impl Packet for ServerboundPingRequestPacket {
    const PACKET_ID: i32 = 0x01;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let time = types::read_i64(&mut data)?;
        Ok(Self { time })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        types::write_i64(&mut buf, self.time);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        assert_packet_roundtrip!(ServerboundPingRequestPacket {
            time: 1_719_000_000_000,
        });
    }

    #[test]
    fn test_negative_time() {
        let pkt = ServerboundPingRequestPacket { time: -1 };
        let encoded = Packet::encode(&pkt);
        let decoded = <ServerboundPingRequestPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.time, -1);
    }

    #[test]
    fn test_decode_empty_fails() {
        let result = <ServerboundPingRequestPacket as Packet>::decode(Bytes::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ServerboundPingRequestPacket, 0x01);
    }
}
