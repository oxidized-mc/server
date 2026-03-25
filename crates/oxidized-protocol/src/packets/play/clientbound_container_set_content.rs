//! ClientboundContainerSetContentPacket (0x12) — full inventory sync.
//!
//! Sends the complete contents of a container to the client. Used on login
//! to initialize the player inventory, and for full resync when state_id
//! diverges.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundContainerSetContentPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::slot::{self, SlotData};
use crate::codec::varint;

/// 0x12 — Full container content synchronization.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundContainerSetContentPacket {
    /// Container ID (0 = player inventory).
    pub container_id: u8,
    /// Optimistic lock counter.
    pub state_id: i32,
    /// All slots in the container. `None` entries represent empty slots.
    pub items: Vec<Option<SlotData>>,
    /// Item currently held on the cursor.
    pub carried_item: Option<SlotData>,
}

impl Packet for ClientboundContainerSetContentPacket {
    const PACKET_ID: i32 = 0x12;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let container_id = crate::codec::types::read_u8(&mut data)?;
        let state_id = varint::read_varint_buf(&mut data)?;
        let items = crate::codec::types::read_list(&mut data, |d| slot::read_slot(d))?;
        let carried_item = slot::read_slot(&mut data)?;
        Ok(Self {
            container_id,
            state_id,
            items,
            carried_item,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        crate::codec::types::write_u8(&mut buf, self.container_id);
        varint::write_varint_buf(self.state_id, &mut buf);
        crate::codec::types::write_list(&mut buf, &self.items, |b, item| {
            slot::write_slot(b, item.as_ref());
        });
        slot::write_slot(&mut buf, self.carried_item.as_ref());
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::codec::slot::ComponentPatchData;

    #[test]
    fn test_roundtrip_empty_inventory() {
        let pkt = ClientboundContainerSetContentPacket {
            container_id: 0,
            state_id: 1,
            items: vec![None; 46],
            carried_item: None,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundContainerSetContentPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.container_id, 0);
        assert_eq!(decoded.state_id, 1);
        assert_eq!(decoded.items.len(), 46);
        assert!(decoded.items.iter().all(|i| i.is_none()));
        assert!(decoded.carried_item.is_none());
    }

    #[test]
    fn test_roundtrip_with_items() {
        let pkt = ClientboundContainerSetContentPacket {
            container_id: 0,
            state_id: 5,
            items: vec![
                Some(SlotData {
                    count: 64,
                    item_id: 1,
                    component_data: ComponentPatchData::default(),
                }),
                None,
                Some(SlotData {
                    count: 1,
                    item_id: 100,
                    component_data: ComponentPatchData::default(),
                }),
            ],
            carried_item: None,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundContainerSetContentPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.items.len(), 3);
        assert_eq!(decoded.items[0].as_ref().unwrap().count, 64);
        assert!(decoded.items[1].is_none());
        assert_eq!(decoded.items[2].as_ref().unwrap().item_id, 100);
    }
}
