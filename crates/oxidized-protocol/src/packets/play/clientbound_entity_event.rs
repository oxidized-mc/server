//! Clientbound entity-event packet.
//!
//! Sent to trigger a client-side event on a specific entity. Events range
//! from particle effects to permission-level indicators.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundEntityEventPacket`.

use bytes::{BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;

/// Entity event packet (0x22).
///
/// # Wire Format
///
/// | Field | Type | Description |
/// |-------|------|-------------|
/// | entity_id | i32 | Network entity ID (raw int, NOT VarInt) |
/// | event_id | u8 | Event type byte |
///
/// # Permission Level Events
///
/// Event IDs 24–28 set the client's op permission level:
/// - 24 = level 0 (default)
/// - 25 = level 1 (moderator)
/// - 26 = level 2 (gamemaster)
/// - 27 = level 3 (admin)
/// - 28 = level 4 (owner)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundEntityEventPacket {
    /// Network entity ID.
    pub entity_id: i32,
    /// Event type byte.
    pub event_id: u8,
}

impl ClientboundEntityEventPacket {
    /// Base event ID for permission levels.
    ///
    /// Permission level N is sent as event `PERMISSION_LEVEL_BASE + N`.
    pub const PERMISSION_LEVEL_BASE: u8 = 24;
}

impl Packet for ClientboundEntityEventPacket {
    const PACKET_ID: i32 = 0x22;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        types::ensure_remaining(&data, 5, "EntityEventPacket")?;
        let entity_id = types::read_i32(&mut data)?;
        let event_id = types::read_u8(&mut data)?;
        Ok(Self {
            entity_id,
            event_id,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(5);
        buf.put_i32(self.entity_id);
        buf.put_u8(self.event_id);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundEntityEventPacket {
            entity_id: 42,
            event_id: 26,
        };
        let encoded = pkt.encode();
        assert_eq!(encoded.len(), 5);
        let decoded = ClientboundEntityEventPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ClientboundEntityEventPacket, 0x22);
    }

    #[test]
    fn test_permission_level_events() {
        for level in 0..=4u8 {
            let event_id = ClientboundEntityEventPacket::PERMISSION_LEVEL_BASE + level;
            assert_eq!(event_id, 24 + level);
        }
    }
}
