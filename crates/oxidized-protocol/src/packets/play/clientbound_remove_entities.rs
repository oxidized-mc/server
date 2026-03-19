//! Clientbound remove-entities packet.
//!
//! Sent when one or more entities leave a player's tracking range or
//! are removed from the world. The client should destroy all listed
//! entities.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundRemoveEntitiesPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::varint;

use super::clientbound_login::PlayPacketError;

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

impl ClientboundRemoveEntitiesPacket {
    /// Packet ID in the PLAY state clientbound registry.
    pub const PACKET_ID: i32 = 0x4D;

    /// Decodes from the raw packet body.
    pub fn decode(mut data: Bytes) -> Result<Self, PlayPacketError> {
        let count = varint::read_varint_buf(&mut data)?;
        if count < 0 {
            return Err(PlayPacketError::InvalidData(format!(
                "negative entity count: {count}"
            )));
        }
        let mut entity_ids = Vec::with_capacity(count as usize);
        for _ in 0..count {
            entity_ids.push(varint::read_varint_buf(&mut data)?);
        }
        Ok(Self { entity_ids })
    }

    /// Encodes the packet body into `buf`.
    pub fn encode(&self, buf: &mut BytesMut) {
        varint::write_varint_buf(self.entity_ids.len() as i32, buf);
        for &id in &self.entity_ids {
            varint::write_varint_buf(id, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_packet_id() {
        assert_eq!(ClientboundRemoveEntitiesPacket::PACKET_ID, 0x4D);
    }

    #[test]
    fn test_encode_decode_single() {
        let pkt = ClientboundRemoveEntitiesPacket {
            entity_ids: vec![42],
        };
        let mut buf = BytesMut::new();
        pkt.encode(&mut buf);
        let decoded = ClientboundRemoveEntitiesPacket::decode(buf.freeze()).unwrap();
        assert_eq!(decoded.entity_ids, vec![42]);
    }

    #[test]
    fn test_encode_decode_multiple() {
        let pkt = ClientboundRemoveEntitiesPacket {
            entity_ids: vec![1, 2, 3, 100, 999],
        };
        let mut buf = BytesMut::new();
        pkt.encode(&mut buf);
        let decoded = ClientboundRemoveEntitiesPacket::decode(buf.freeze()).unwrap();
        assert_eq!(decoded.entity_ids, vec![1, 2, 3, 100, 999]);
    }

    #[test]
    fn test_encode_decode_empty() {
        let pkt = ClientboundRemoveEntitiesPacket { entity_ids: vec![] };
        let mut buf = BytesMut::new();
        pkt.encode(&mut buf);
        let decoded = ClientboundRemoveEntitiesPacket::decode(buf.freeze()).unwrap();
        assert!(decoded.entity_ids.is_empty());
    }
}
