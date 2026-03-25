//! Clientbound remove-entities packet.
//!
//! Sent when one or more entities leave a player's tracking range or
//! are removed from the world. The client should destroy all listed
//! entities.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundRemoveEntitiesPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::{types, varint};

/// Remove one or more entities (0x4D).
///
/// # Wire Format
///
/// | Field | Type | Description |
/// |-------|------|-------------|
/// | count | VarInt | Number of entity IDs |
/// | entity_ids | VarInt[] | Entity network IDs to remove |
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundRemoveEntitiesPacket {
    /// Entity IDs to remove.
    pub entity_ids: Vec<i32>,
}

impl Packet for ClientboundRemoveEntitiesPacket {
    const PACKET_ID: i32 = 0x4D;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let count = varint::read_varint_buf(&mut data)?;
        if count < 0 {
            return Err(PacketDecodeError::InvalidData(format!(
                "negative entity count: {count}"
            )));
        }
        let count = count as usize;
        types::ensure_remaining(
            &data,
            count,
            "RemoveEntitiesPacket entity data",
        )?;
        let mut entity_ids = Vec::with_capacity(count);
        for _ in 0..count {
            entity_ids.push(varint::read_varint_buf(&mut data)?);
        }
        Ok(Self { entity_ids })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        types::write_list(&mut buf, &self.entity_ids, |b, &id| {
            varint::write_varint_buf(id, b);
        });
        buf
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ClientboundRemoveEntitiesPacket, 0x4D);
    }

    #[test]
    fn test_encode_decode_single() {
        let pkt = ClientboundRemoveEntitiesPacket {
            entity_ids: vec![42],
        };
        let buf = pkt.encode();
        let decoded = ClientboundRemoveEntitiesPacket::decode(buf.freeze()).unwrap();
        assert_eq!(decoded.entity_ids, vec![42]);
    }

    #[test]
    fn test_encode_decode_multiple() {
        let pkt = ClientboundRemoveEntitiesPacket {
            entity_ids: vec![1, 2, 3, 100, 999],
        };
        let buf = pkt.encode();
        let decoded = ClientboundRemoveEntitiesPacket::decode(buf.freeze()).unwrap();
        assert_eq!(decoded.entity_ids, vec![1, 2, 3, 100, 999]);
    }

    #[test]
    fn test_encode_decode_empty() {
        let pkt = ClientboundRemoveEntitiesPacket { entity_ids: vec![] };
        let buf = pkt.encode();
        let decoded = ClientboundRemoveEntitiesPacket::decode(buf.freeze()).unwrap();
        assert!(decoded.entity_ids.is_empty());
    }

    #[test]
    fn test_decode_rejects_bogus_count() {
        // VarInt count = 1_000_000 but only 0 bytes of entity data follow.
        let mut buf = BytesMut::new();
        varint::write_varint_buf(1_000_000, &mut buf);
        let err = ClientboundRemoveEntitiesPacket::decode(buf.freeze()).unwrap_err();
        assert!(
            matches!(err, PacketDecodeError::InvalidData(ref msg) if msg.contains("need")),
            "expected bounds error, got: {err:?}"
        );
    }
}
