//! ClientboundSetHeldSlotPacket (0x69) — tell client which hotbar slot is selected.
//!
//! Sent during login and when the server needs to change the selected hotbar slot.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundSetHeldSlotPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::varint;

/// 0x69 — Server sets the client's active hotbar slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundSetHeldSlotPacket {
    /// Hotbar slot index (0–8).
    pub slot: i32,
}

impl Packet for ClientboundSetHeldSlotPacket {
    const PACKET_ID: i32 = 0x69;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let slot = varint::read_varint_buf(&mut data)?;
        Ok(Self { slot })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        varint::write_varint_buf(self.slot, &mut buf);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundSetHeldSlotPacket { slot: 3 };
        let encoded = pkt.encode();
        let decoded = ClientboundSetHeldSlotPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.slot, 3);
    }

    #[test]
    fn test_slot_zero() {
        let pkt = ClientboundSetHeldSlotPacket { slot: 0 };
        let encoded = pkt.encode();
        let decoded = ClientboundSetHeldSlotPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.slot, 0);
    }

    #[test]
    fn test_slot_eight() {
        let pkt = ClientboundSetHeldSlotPacket { slot: 8 };
        let encoded = pkt.encode();
        let decoded = ClientboundSetHeldSlotPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.slot, 8);
    }
}
