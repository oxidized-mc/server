//! ClientboundTickingStatePacket (0x7F) — tick rate and freeze state.
//!
//! Informs the client of the server's current tick rate and whether
//! ticking is frozen. Sent when the state changes (e.g., `/tick freeze`).
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundTickingStatePacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;

/// 0x7F — Server tick rate and freeze state.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundTickingStatePacket {
    /// Current server tick rate (TPS). Default: 20.0.
    pub tick_rate: f32,
    /// Whether the server is frozen (no game ticks advance).
    pub is_frozen: bool,
}

impl Packet for ClientboundTickingStatePacket {
    const PACKET_ID: i32 = 0x7F;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        if data.remaining() < 5 {
            return Err(PacketDecodeError::Io(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "not enough data for TickingStatePacket",
            )));
        }
        let tick_rate = data.get_f32();
        let is_frozen = data.get_u8() != 0;
        Ok(Self {
            tick_rate,
            is_frozen,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(5);
        buf.put_f32(self.tick_rate);
        buf.put_u8(u8::from(self.is_frozen));
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_normal() {
        let pkt = ClientboundTickingStatePacket {
            tick_rate: 20.0,
            is_frozen: false,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundTickingStatePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_frozen() {
        let pkt = ClientboundTickingStatePacket {
            tick_rate: 20.0,
            is_frozen: true,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundTickingStatePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_custom_rate() {
        let pkt = ClientboundTickingStatePacket {
            tick_rate: 40.0,
            is_frozen: false,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundTickingStatePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_id() {
        assert_eq!(<ClientboundTickingStatePacket as Packet>::PACKET_ID, 0x7F);
    }
}
