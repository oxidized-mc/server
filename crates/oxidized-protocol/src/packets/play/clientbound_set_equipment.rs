//! Clientbound set-equipment packet.
//!
//! Tells the client what items an entity is wearing/holding. Used to display
//! armor, main-hand, and off-hand items on other players (and mobs).
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundSetEquipmentPacket`.

use bytes::{BufMut, Bytes, BytesMut};

use oxidized_codec::Packet;
use oxidized_codec::packet::PacketDecodeError;
use oxidized_codec::slot::{self, SlotData};
use oxidized_codec::varint;
use oxidized_mc_types::EquipmentSlot;

/// Bit mask for the "more entries follow" continuation flag.
const CONTINUE_MASK: u8 = 0x80;

/// Converts an `EquipmentSlot` to the Java enum ordinal used on the wire.
///
/// The SetEquipment packet uses ordinals (matching the Java enum declaration
/// order), not the non-sequential `EquipmentSlot::id()` values.
const fn slot_to_ordinal(slot: EquipmentSlot) -> u8 {
    match slot {
        EquipmentSlot::MainHand => 0,
        EquipmentSlot::OffHand => 1,
        EquipmentSlot::Feet => 2,
        EquipmentSlot::Legs => 3,
        EquipmentSlot::Chest => 4,
        EquipmentSlot::Head => 5,
        EquipmentSlot::Body => 6,
        EquipmentSlot::Saddle => 7,
    }
}

/// Converts a wire ordinal back to an `EquipmentSlot`.
const fn ordinal_to_slot(ordinal: u8) -> Option<EquipmentSlot> {
    match ordinal {
        0 => Some(EquipmentSlot::MainHand),
        1 => Some(EquipmentSlot::OffHand),
        2 => Some(EquipmentSlot::Feet),
        3 => Some(EquipmentSlot::Legs),
        4 => Some(EquipmentSlot::Chest),
        5 => Some(EquipmentSlot::Head),
        6 => Some(EquipmentSlot::Body),
        7 => Some(EquipmentSlot::Saddle),
        _ => None,
    }
}

/// Set equipment packet (0x66).
///
/// # Wire Format
///
/// ```text
/// VarInt    entity_id
/// repeat:
///   u8      slot_ordinal | (has_more ? 0x80 : 0x00)
///   Slot    item_stack (optional-item format)
/// ```
///
/// The top bit of the slot byte indicates whether more entries follow.
/// The lower 7 bits are the Java `EquipmentSlot` ordinal (not the slot's
/// `getIndex()` value).
#[derive(Debug, Clone)]
pub struct ClientboundSetEquipmentPacket {
    /// Entity network ID.
    pub entity_id: i32,
    /// Equipment entries: `(slot, item)`.
    pub equipments: Vec<(EquipmentSlot, Option<SlotData>)>,
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

            let ordinal = raw_slot & !CONTINUE_MASK;
            let has_more = (raw_slot & CONTINUE_MASK) != 0;

            let slot = ordinal_to_slot(ordinal).ok_or_else(|| {
                PacketDecodeError::InvalidData(format!(
                    "unknown equipment slot ordinal: {ordinal}"
                ))
            })?;
            let item = slot::read_slot(&mut data)?;
            equipments.push((slot, item));

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
            let ordinal = slot_to_ordinal(*slot);
            let slot_byte =
                ordinal & !CONTINUE_MASK | if has_more { CONTINUE_MASK } else { 0 };
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
    use oxidized_codec::slot::ComponentPatchData;

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ClientboundSetEquipmentPacket, 0x66);
    }

    #[test]
    fn test_encode_single_empty_slot() {
        let pkt = ClientboundSetEquipmentPacket {
            entity_id: 42,
            equipments: vec![(EquipmentSlot::MainHand, None)],
        };
        let encoded = pkt.encode();
        let decoded = ClientboundSetEquipmentPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.entity_id, 42);
        assert_eq!(decoded.equipments.len(), 1);
        assert_eq!(decoded.equipments[0].0, EquipmentSlot::MainHand);
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
                (EquipmentSlot::MainHand, Some(stone.clone())),
                (EquipmentSlot::OffHand, None),
                (EquipmentSlot::Head, Some(stone)),
            ],
        };
        let encoded = pkt.encode();
        let decoded = ClientboundSetEquipmentPacket::decode(encoded.freeze()).unwrap();

        assert_eq!(decoded.entity_id, 7);
        assert_eq!(decoded.equipments.len(), 3);
        assert_eq!(decoded.equipments[0].0, EquipmentSlot::MainHand);
        assert!(decoded.equipments[0].1.is_some());
        assert_eq!(decoded.equipments[1].0, EquipmentSlot::OffHand);
        assert!(decoded.equipments[1].1.is_none());
        assert_eq!(decoded.equipments[2].0, EquipmentSlot::Head);
        assert!(decoded.equipments[2].1.is_some());
    }

    #[test]
    fn test_continue_bit_set_correctly() {
        let pkt = ClientboundSetEquipmentPacket {
            entity_id: 1,
            equipments: vec![
                (EquipmentSlot::MainHand, None),
                (EquipmentSlot::Chest, None),
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

    #[test]
    fn test_ordinal_roundtrip_all_slots() {
        let all_slots = [
            EquipmentSlot::MainHand,
            EquipmentSlot::OffHand,
            EquipmentSlot::Feet,
            EquipmentSlot::Legs,
            EquipmentSlot::Chest,
            EquipmentSlot::Head,
            EquipmentSlot::Body,
            EquipmentSlot::Saddle,
        ];
        for (i, &slot) in all_slots.iter().enumerate() {
            let ordinal = slot_to_ordinal(slot);
            assert_eq!(ordinal, i as u8, "ordinal mismatch for {slot:?}");
            let back = ordinal_to_slot(ordinal).unwrap();
            assert_eq!(back, slot, "roundtrip failed for {slot:?}");
        }
    }
}
