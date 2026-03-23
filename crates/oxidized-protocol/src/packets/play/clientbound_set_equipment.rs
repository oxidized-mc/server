//! Clientbound set-equipment packet.
//!
//! Tells the client what items an entity is wearing/holding. Used to display
//! armor, main-hand, and off-hand items on other players (and mobs).
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundSetEquipmentPacket`.

use bytes::{BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::slot::{self, SlotData};
use crate::codec::varint;

/// Equipment slot indices as sent on the wire.
///
/// These match the vanilla `EquipmentSlot` ordinal values.
pub mod equipment_slot {
    /// Main hand (selected hotbar slot).
    pub const MAIN_HAND: u8 = 0;
    /// Off-hand.
    pub const OFF_HAND: u8 = 1;
    /// Feet armor (boots).
    pub const FEET: u8 = 2;
    /// Legs armor (leggings).
    pub const LEGS: u8 = 3;
    /// Chest armor (chestplate).
    pub const CHEST: u8 = 4;
    /// Head armor (helmet).
    pub const HEAD: u8 = 5;
}

/// Bit mask for the "more entries follow" continuation flag.
const CONTINUE_MASK: u8 = 0x80;

/// Set equipment packet (0x66).
///
/// # Wire Format
///
/// ```text
/// VarInt    entity_id
/// repeat:
///   u8      slot_id | (has_more ? 0x80 : 0x00)
///   Slot    item_stack (optional-item format)
/// ```
///
/// The top bit of the slot byte indicates whether more entries follow.
#[derive(Debug, Clone)]
pub struct ClientboundSetEquipmentPacket {
    /// Entity network ID.
    pub entity_id: i32,
    /// Equipment entries: `(slot_index, item)`.
    pub equipments: Vec<(u8, Option<SlotData>)>,
}

impl Packet for ClientboundSetEquipmentPacket {
    const PACKET_ID: i32 = 0x66;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let entity_id = varint::read_varint_buf(&mut data)?;
        let mut equipments = Vec::new();

        loop {
            if data.is_empty() {
                return Err(PacketDecodeError::InvalidData(
                    "unexpected end of equipment data".into(),
                ));
            }
            let raw_slot = data[0];
            data = data.slice(1..);

            let slot_id = raw_slot & !CONTINUE_MASK;
            let has_more = (raw_slot & CONTINUE_MASK) != 0;

            let item = slot::read_slot(&mut data)?;
            equipments.push((slot_id, item));

            if !has_more {
                break;
            }
        }

        Ok(Self {
            entity_id,
            equipments,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        varint::write_varint_buf(self.entity_id, &mut buf);

        let len = self.equipments.len();
        for (i, (slot, item)) in self.equipments.iter().enumerate() {
            let has_more = i + 1 < len;
            let slot_byte = slot & !CONTINUE_MASK | if has_more { CONTINUE_MASK } else { 0 };
            buf.put_u8(slot_byte);
            slot::write_slot(&mut buf, item.as_ref());
        }

        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::codec::slot::ComponentPatchData;

    #[test]
    fn test_packet_id() {
        assert_eq!(<ClientboundSetEquipmentPacket as Packet>::PACKET_ID, 0x66);
    }

    #[test]
    fn test_encode_single_empty_slot() {
        let pkt = ClientboundSetEquipmentPacket {
            entity_id: 42,
            equipments: vec![(equipment_slot::MAIN_HAND, None)],
        };
        let encoded = pkt.encode();
        let decoded = ClientboundSetEquipmentPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.entity_id, 42);
        assert_eq!(decoded.equipments.len(), 1);
        assert_eq!(decoded.equipments[0].0, equipment_slot::MAIN_HAND);
        assert!(decoded.equipments[0].1.is_none());
    }

    #[test]
    fn test_encode_decode_roundtrip_multiple() {
        let stone = SlotData {
            count: 1,
            item_id: 1,
            component_data: ComponentPatchData::default(),
        };
        let pkt = ClientboundSetEquipmentPacket {
            entity_id: 7,
            equipments: vec![
                (equipment_slot::MAIN_HAND, Some(stone.clone())),
                (equipment_slot::OFF_HAND, None),
                (equipment_slot::HEAD, Some(stone.clone())),
            ],
        };
        let encoded = pkt.encode();
        let decoded = ClientboundSetEquipmentPacket::decode(encoded.freeze()).unwrap();

        assert_eq!(decoded.entity_id, 7);
        assert_eq!(decoded.equipments.len(), 3);
        assert_eq!(decoded.equipments[0].0, equipment_slot::MAIN_HAND);
        assert!(decoded.equipments[0].1.is_some());
        assert_eq!(decoded.equipments[1].0, equipment_slot::OFF_HAND);
        assert!(decoded.equipments[1].1.is_none());
        assert_eq!(decoded.equipments[2].0, equipment_slot::HEAD);
        assert!(decoded.equipments[2].1.is_some());
    }

    #[test]
    fn test_continue_bit_set_correctly() {
        let pkt = ClientboundSetEquipmentPacket {
            entity_id: 1,
            equipments: vec![
                (equipment_slot::MAIN_HAND, None),
                (equipment_slot::CHEST, None),
            ],
        };
        let buf = pkt.encode();
        let data = buf.to_vec();
        // After VarInt(1) = 1 byte, first slot byte should have continue bit set
        assert_ne!(
            data[1] & CONTINUE_MASK,
            0,
            "first entry should have continue bit"
        );
        // VarInt(0) for empty slot = 1 byte, so second slot byte is at [3]
        assert_eq!(
            data[3] & CONTINUE_MASK,
            0,
            "last entry should NOT have continue bit"
        );
    }
}
