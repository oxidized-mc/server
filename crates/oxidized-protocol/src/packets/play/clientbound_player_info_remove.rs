//! ClientboundPlayerInfoRemovePacket (0x45) — removes players from the tab list.

use bytes::{Bytes, BytesMut};
use uuid::Uuid;

use crate::codec::{types, varint};
use crate::packets::play::PlayPacketError;

/// 0x45 — Removes one or more players from the tab list.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundPlayerInfoRemovePacket {
    /// UUIDs of players to remove.
    pub uuids: Vec<Uuid>,
}

impl ClientboundPlayerInfoRemovePacket {
    /// Packet ID in the PLAY state.
    pub const PACKET_ID: i32 = 0x45;

    /// Decodes from the raw packet body.
    pub fn decode(mut data: Bytes) -> Result<Self, PlayPacketError> {
        let count = varint::read_varint_buf(&mut data)? as usize;
        if count > 1000 {
            return Err(PlayPacketError::UnexpectedEof);
        }
        let mut uuids = Vec::with_capacity(count);
        for _ in 0..count {
            uuids.push(types::read_uuid(&mut data)?);
        }
        Ok(Self { uuids })
    }

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(5 + self.uuids.len() * 16);
        varint::write_varint_buf(self.uuids.len() as i32, &mut buf);
        for uuid in &self.uuids {
            types::write_uuid(&mut buf, uuid);
        }
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_id() {
        assert_eq!(ClientboundPlayerInfoRemovePacket::PACKET_ID, 0x45);
    }

    #[test]
    fn test_roundtrip_single() {
        let uuid = Uuid::new_v4();
        let pkt = ClientboundPlayerInfoRemovePacket { uuids: vec![uuid] };
        let encoded = pkt.encode();
        let decoded = ClientboundPlayerInfoRemovePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.uuids, vec![uuid]);
    }

    #[test]
    fn test_roundtrip_multiple() {
        let uuids = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        let pkt = ClientboundPlayerInfoRemovePacket {
            uuids: uuids.clone(),
        };
        let encoded = pkt.encode();
        let decoded = ClientboundPlayerInfoRemovePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.uuids, uuids);
    }

    #[test]
    fn test_roundtrip_empty() {
        let pkt = ClientboundPlayerInfoRemovePacket { uuids: vec![] };
        let encoded = pkt.encode();
        let decoded = ClientboundPlayerInfoRemovePacket::decode(encoded.freeze()).unwrap();
        assert!(decoded.uuids.is_empty());
    }
}
