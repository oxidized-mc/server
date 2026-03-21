//! ServerboundSetCreativeModeSlotPacket (0x38) — creative mode item placement.
//!
//! Sent when a creative-mode player places an item from the creative
//! inventory into a slot, or modifies a slot directly.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ServerboundSetCreativeModeSlotPacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::slot::{self, SlotData};

/// 0x38 — Creative mode sets item in a slot.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerboundSetCreativeModeSlotPacket {
    /// Protocol slot number.
    pub slot: i16,
    /// Item to place, or `None` to clear the slot.
    pub item: Option<SlotData>,
}

impl Packet for ServerboundSetCreativeModeSlotPacket {
    const PACKET_ID: i32 = 0x38;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let slot = data.get_i16();
        let item = slot::read_slot(&mut data)?;
        Ok(Self { slot, item })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        buf.put_i16(self.slot);
        slot::write_slot(&mut buf, self.item.as_ref());
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::codec::slot::ComponentPatchData;

    #[test]
    fn test_roundtrip_with_item() {
        let pkt = ServerboundSetCreativeModeSlotPacket {
            slot: 36,
            item: Some(SlotData {
                count: 1,
                item_id: 50,
                component_data: ComponentPatchData::default(),
            }),
        };
        let encoded = pkt.encode();
        let decoded = ServerboundSetCreativeModeSlotPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.slot, 36);
        assert_eq!(decoded.item.as_ref().unwrap().count, 1);
        assert_eq!(decoded.item.as_ref().unwrap().item_id, 50);
    }

    #[test]
    fn test_roundtrip_clear_slot() {
        let pkt = ServerboundSetCreativeModeSlotPacket {
            slot: 36,
            item: None,
        };
        let encoded = pkt.encode();
        let decoded = ServerboundSetCreativeModeSlotPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.slot, 36);
        assert!(decoded.item.is_none());
    }
}
