//! ClientboundTickingStepPacket (0x80) — frozen tick stepping.
//!
//! Tells the client how many ticks remain in a step-forward sequence
//! while the server is frozen. Sent after `/tick step N`.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundTickingStepPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::varint;

/// 0x80 — Remaining frozen tick steps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundTickingStepPacket {
    /// Number of tick steps remaining (0 = done stepping).
    pub tick_steps: i32,
}

impl Packet for ClientboundTickingStepPacket {
    const PACKET_ID: i32 = 0x80;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let tick_steps = varint::read_varint_buf(&mut data)?;
        Ok(Self { tick_steps })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(5);
        varint::write_varint_buf(self.tick_steps, &mut buf);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_zero() {
        let pkt = ClientboundTickingStepPacket { tick_steps: 0 };
        let encoded = pkt.encode();
        let decoded = ClientboundTickingStepPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_nonzero() {
        let pkt = ClientboundTickingStepPacket { tick_steps: 42 };
        let encoded = pkt.encode();
        let decoded = ClientboundTickingStepPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ClientboundTickingStepPacket, 0x80);
    }
}
