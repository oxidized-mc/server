//! Clientbound set-entity-data packet.
//!
//! Sends changed `SynchedEntityData` values to the client. Each entry
//! is a slot index + serializer type ID + encoded value. The list is
//! terminated by `0xFF`.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundSetEntityDataPacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::varint;

use super::clientbound_login::PlayPacketError;

/// End-of-data marker byte.
pub const DATA_EOF_MARKER: u8 = 0xFF;

/// A single entity data entry for wire encoding.
///
/// The `value_bytes` field contains the pre-encoded value payload.
/// The caller is responsible for encoding the value using the
/// appropriate codec for the serializer type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityDataEntry {
    /// Slot index (0–254).
    pub slot: u8,
    /// Serializer type ID (from `EntityDataSerializers` registration order).
    pub serializer_type: i32,
    /// Pre-encoded value bytes.
    pub value_bytes: Vec<u8>,
}

/// Set entity data packet (0x63).
///
/// # Wire Format
///
/// ```text
/// VarInt    entity_id
/// repeat:
///   u8      slot_id (0–254)
///   VarInt  serializer_type_id
///   ...     serializer-specific value
/// u8        0xFF (end marker)
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundSetEntityDataPacket {
    /// Entity network ID.
    pub entity_id: i32,
    /// Data entries to send.
    pub entries: Vec<EntityDataEntry>,
}

impl ClientboundSetEntityDataPacket {
    /// Packet ID in the PLAY state clientbound registry.
    pub const PACKET_ID: i32 = 0x63;

    /// Decodes from the raw packet body.
    ///
    /// **Note:** Full decoding of individual entry values requires
    /// knowing the codec for each serializer type (which depends on
    /// registry state). This decoder consumes entries correctly by
    /// reading the slot + serializer type, but stores all remaining
    /// bytes as the value of the *last* entry since we cannot determine
    /// per-entry value boundaries without the codec registry.
    ///
    /// For packets with a single entry (the common case for flags,
    /// pose, etc.), this produces an exact result. For multi-entry
    /// packets, prefer [`encode`](Self::encode) on the sending side
    /// and use a registry-aware decoder on the receiving side.
    pub fn decode(mut data: Bytes) -> Result<Self, PlayPacketError> {
        let entity_id = varint::read_varint_buf(&mut data)?;
        let mut entries = Vec::new();

        loop {
            if data.is_empty() {
                return Err(PlayPacketError::InvalidData(
                    "unexpected end of entity data".into(),
                ));
            }

            let slot = data[0];
            data.advance(1);

            if slot == DATA_EOF_MARKER {
                break;
            }

            let serializer_type = varint::read_varint_buf(&mut data)?;
            // Without knowing the codec, we cannot determine where one
            // entry's value ends and the next begins. Collect all remaining
            // bytes, strip the trailing 0xFF terminator, and store as
            // this entry's value. Correct for single-entry packets.
            let mut value_bytes = data.to_vec();
            data.clear();
            // Remove trailing EOF marker if present
            if value_bytes.last() == Some(&DATA_EOF_MARKER) {
                value_bytes.pop();
            }

            entries.push(EntityDataEntry {
                slot,
                serializer_type,
                value_bytes,
            });
            // Cannot continue — remaining data boundary is unknown without
            // a serializer codec registry.
            break;
        }

        Ok(Self {
            entity_id,
            entries,
        })
    }

    /// Encodes the packet body into `buf`.
    pub fn encode(&self, buf: &mut BytesMut) {
        varint::write_varint_buf(self.entity_id, buf);
        for entry in &self.entries {
            buf.put_u8(entry.slot);
            varint::write_varint_buf(entry.serializer_type, buf);
            buf.extend_from_slice(&entry.value_bytes);
        }
        buf.put_u8(DATA_EOF_MARKER);
    }

    /// Creates a packet with a single byte-type entry.
    ///
    /// Convenience for the common case of updating entity flags.
    pub fn single_byte(
        entity_id: i32,
        slot: u8,
        value: u8,
    ) -> Self {
        Self {
            entity_id,
            entries: vec![EntityDataEntry {
                slot,
                serializer_type: 0, // Byte
                value_bytes: vec![value],
            }],
        }
    }

    /// Creates a packet with a single VarInt-type entry.
    pub fn single_varint(
        entity_id: i32,
        slot: u8,
        value: i32,
    ) -> Self {
        let mut value_bytes = BytesMut::new();
        varint::write_varint_buf(value, &mut value_bytes);
        Self {
            entity_id,
            entries: vec![EntityDataEntry {
                slot,
                serializer_type: 1, // Int (VarInt)
                value_bytes: value_bytes.to_vec(),
            }],
        }
    }

    /// Creates a packet with a single boolean-type entry.
    pub fn single_bool(
        entity_id: i32,
        slot: u8,
        value: bool,
    ) -> Self {
        Self {
            entity_id,
            entries: vec![EntityDataEntry {
                slot,
                serializer_type: 8, // Boolean
                value_bytes: vec![u8::from(value)],
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_packet_id() {
        assert_eq!(ClientboundSetEntityDataPacket::PACKET_ID, 0x63);
    }

    #[test]
    fn test_encode_single_byte_entry() {
        let pkt =
            ClientboundSetEntityDataPacket::single_byte(42, 0, 0x05);
        let mut buf = BytesMut::new();
        pkt.encode(&mut buf);

        // entity_id=42 (VarInt=1 byte) + slot=0 + serializer=0 (VarInt=1) + value=0x05 + 0xFF
        let data = buf.to_vec();
        assert_eq!(data[0], 42); // entity_id VarInt
        assert_eq!(data[1], 0); // slot
        assert_eq!(data[2], 0); // serializer type (Byte)
        assert_eq!(data[3], 0x05); // value
        assert_eq!(data[4], 0xFF); // EOF
    }

    #[test]
    fn test_encode_multiple_entries() {
        let pkt = ClientboundSetEntityDataPacket {
            entity_id: 1,
            entries: vec![
                EntityDataEntry {
                    slot: 0,
                    serializer_type: 0,
                    value_bytes: vec![0x03],
                },
                EntityDataEntry {
                    slot: 4,
                    serializer_type: 8,
                    value_bytes: vec![0x01],
                },
            ],
        };
        let mut buf = BytesMut::new();
        pkt.encode(&mut buf);

        let data = buf.to_vec();
        let last = data[data.len() - 1];
        assert_eq!(last, 0xFF, "must end with EOF marker");
    }

    #[test]
    fn test_single_bool_convenience() {
        let pkt = ClientboundSetEntityDataPacket::single_bool(
            10, 4, true,
        );
        assert_eq!(pkt.entity_id, 10);
        assert_eq!(pkt.entries.len(), 1);
        assert_eq!(pkt.entries[0].slot, 4);
        assert_eq!(pkt.entries[0].serializer_type, 8);
        assert_eq!(pkt.entries[0].value_bytes, vec![1]);
    }

    #[test]
    fn test_single_varint_convenience() {
        let pkt = ClientboundSetEntityDataPacket::single_varint(
            10, 1, 300,
        );
        assert_eq!(pkt.entity_id, 10);
        assert_eq!(pkt.entries.len(), 1);
        assert_eq!(pkt.entries[0].slot, 1);
        assert_eq!(pkt.entries[0].serializer_type, 1);
    }

    #[test]
    fn test_eof_marker_constant() {
        assert_eq!(DATA_EOF_MARKER, 255);
    }
}
