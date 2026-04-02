//! ClientboundSetPlayerInventoryPacket (0x6C) — update a single player inventory slot.
//!
//! Unlike `ContainerSetSlot`, this packet doesn't use a container ID or state ID.
//! It directly addresses a slot in the player's inventory.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundSetPlayerInventoryPacket`.

use bytes::{Bytes, BytesMut};

use oxidized_codec::Packet;
use oxidized_codec::packet::PacketDecodeError;
use oxidized_codec::slot::{self, SlotData};
use oxidized_codec::varint;

/// 0x6C — Direct player inventory slot update.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundSetPlayerInventoryPacket {
    /// Inventory slot index.
    pub slot: i32,
    /// The item contents of the slot.
    pub contents: Option<SlotData>,
}

impl Packet for ClientboundSetPlayerInventoryPacket {
    const PACKET_ID: i32 = 0x6C;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let slot = varint::read_varint_buf(&mut data)?;
        let contents = slot::read_slot(&mut data)?;
        Ok(Self { slot, contents })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        varint::write_varint_buf(self.slot, &mut buf);
        slot::write_slot(&mut buf, self.contents.as_ref());
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use oxidized_codec::slot::ComponentPatchData;

    #[test]
    fn test_roundtrip_empty() {
        let pkt = ClientboundSetPlayerInventoryPacket {
            slot: 36,
            contents: None,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundSetPlayerInventoryPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.slot, 36);
        assert!(decoded.contents.is_none());
    }

    #[test]
    fn test_roundtrip_with_item() {
        let pkt = ClientboundSetPlayerInventoryPacket {
            slot: 0,
            contents: Some(SlotData {
                count: 32,
                item_id: 5,
                component_data: ComponentPatchData::default(),
            }),
        };
        let encoded = pkt.encode();
        let decoded = ClientboundSetPlayerInventoryPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.slot, 0);
        assert_eq!(decoded.contents.as_ref().unwrap().count, 32);
    }
}
