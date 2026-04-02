//! ServerboundSetCarriedItemPacket (0x35) — player selects a hotbar slot.
//!
//! Sent when the player changes their selected hotbar slot (scroll wheel
//! or number keys). The slot value is 0–8.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ServerboundSetCarriedItemPacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use oxidized_codec::Packet;
use oxidized_codec::packet::PacketDecodeError;

/// 0x35 — Player selects a hotbar slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundSetCarriedItemPacket {
    /// Hotbar slot index (0–8).
    pub slot: i16,
}

impl Packet for ServerboundSetCarriedItemPacket {
    const PACKET_ID: i32 = 0x35;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let slot = data.get_i16();
        Ok(Self { slot })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        buf.put_i16(self.slot);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ServerboundSetCarriedItemPacket { slot: 5 };
        let encoded = pkt.encode();
        let decoded = ServerboundSetCarriedItemPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.slot, 5);
    }

    #[test]
    fn test_slot_zero() {
        let pkt = ServerboundSetCarriedItemPacket { slot: 0 };
        let encoded = pkt.encode();
        let decoded = ServerboundSetCarriedItemPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.slot, 0);
    }

    #[test]
    fn test_slot_eight() {
        let pkt = ServerboundSetCarriedItemPacket { slot: 8 };
        let encoded = pkt.encode();
        let decoded = ServerboundSetCarriedItemPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.slot, 8);
    }
}
