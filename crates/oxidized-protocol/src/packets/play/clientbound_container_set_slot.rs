//! ClientboundContainerSetSlotPacket (0x14) — single slot update.
//!
//! Updates a single slot in a container. Used after server-side changes
//! (e.g., creative mode set, pick-block) to synchronize a specific slot.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundContainerSetSlotPacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::slot::{self, SlotData};
use crate::codec::varint;

/// 0x14 — Single slot update.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundContainerSetSlotPacket {
    /// Container ID. `-1` = cursor, `0` = player inventory.
    pub container_id: i8,
    /// Optimistic lock counter.
    pub state_id: i32,
    /// Protocol slot index within the container.
    pub slot: i16,
    /// The item in the slot, or `None` for empty.
    pub item: Option<SlotData>,
}

impl Packet for ClientboundContainerSetSlotPacket {
    const PACKET_ID: i32 = 0x14;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let container_id = data.get_i8();
        let state_id = varint::read_varint_buf(&mut data)?;
        let slot = data.get_i16();
        let item = slot::read_slot(&mut data)?;
        Ok(Self {
            container_id,
            state_id,
            slot,
            item,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        buf.put_i8(self.container_id);
        varint::write_varint_buf(self.state_id, &mut buf);
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
    fn test_roundtrip_empty_slot() {
        let pkt = ClientboundContainerSetSlotPacket {
            container_id: 0,
            state_id: 3,
            slot: 36,
            item: None,
        };
        let encoded = pkt.encode();
        let decoded =
            ClientboundContainerSetSlotPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.container_id, 0);
        assert_eq!(decoded.state_id, 3);
        assert_eq!(decoded.slot, 36);
        assert!(decoded.item.is_none());
    }

    #[test]
    fn test_roundtrip_with_item() {
        let pkt = ClientboundContainerSetSlotPacket {
            container_id: 0,
            state_id: 7,
            slot: 36,
            item: Some(SlotData {
                count: 1,
                item_id: 42,
                component_data: ComponentPatchData::default(),
            }),
        };
        let encoded = pkt.encode();
        let decoded =
            ClientboundContainerSetSlotPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.slot, 36);
        let item = decoded.item.unwrap();
        assert_eq!(item.count, 1);
        assert_eq!(item.item_id, 42);
    }

    #[test]
    fn test_cursor_container_id() {
        let pkt = ClientboundContainerSetSlotPacket {
            container_id: -1,
            state_id: 0,
            slot: -1,
            item: None,
        };
        let encoded = pkt.encode();
        let decoded =
            ClientboundContainerSetSlotPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.container_id, -1);
        assert_eq!(decoded.slot, -1);
    }
}
