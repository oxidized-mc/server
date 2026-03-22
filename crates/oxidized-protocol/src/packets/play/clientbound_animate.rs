//! Clientbound animate packet.
//!
//! Broadcasts an entity animation to nearby players.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundAnimatePacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::varint;

/// Animation type: swing main hand.
pub const SWING_MAIN_HAND: u8 = 0;
/// Animation type: wake up (leave bed).
pub const WAKE_UP: u8 = 2;
/// Animation type: swing off hand.
pub const SWING_OFF_HAND: u8 = 3;
/// Animation type: critical hit effect.
pub const CRITICAL_HIT: u8 = 4;
/// Animation type: magic critical hit effect.
pub const MAGIC_CRITICAL_HIT: u8 = 5;

/// Clientbound packet that triggers an entity animation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundAnimatePacket {
    /// The entity performing the animation.
    pub entity_id: i32,
    /// Animation type: 0 = swing main hand, 2 = wake up, 3 = swing off hand,
    /// 4 = critical hit, 5 = magic critical hit.
    pub action: u8,
}

impl Packet for ClientboundAnimatePacket {
    const PACKET_ID: i32 = 0x02;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let entity_id = varint::read_varint_buf(&mut data)?;
        if data.remaining() < 1 {
            return Err(PacketDecodeError::Io(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "not enough data for ClientboundAnimatePacket action",
            )));
        }
        let action = data.get_u8();
        Ok(Self { entity_id, action })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        varint::write_varint_buf(self.entity_id, &mut buf);
        buf.put_u8(self.action);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_swing_main_hand() {
        let pkt = ClientboundAnimatePacket {
            entity_id: 42,
            action: SWING_MAIN_HAND,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundAnimatePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_swing_off_hand() {
        let pkt = ClientboundAnimatePacket {
            entity_id: 99,
            action: SWING_OFF_HAND,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundAnimatePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_critical_hit() {
        let pkt = ClientboundAnimatePacket {
            entity_id: 1,
            action: CRITICAL_HIT,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundAnimatePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_id() {
        assert_eq!(ClientboundAnimatePacket::PACKET_ID, 0x02);
    }

    #[test]
    fn test_decode_missing_action() {
        // Only entity_id, no action byte
        let mut buf = BytesMut::new();
        varint::write_varint_buf(42, &mut buf);
        assert!(ClientboundAnimatePacket::decode(buf.freeze()).is_err());
    }

    #[test]
    fn test_decode_empty_buffer() {
        let data = Bytes::new();
        assert!(ClientboundAnimatePacket::decode(data).is_err());
    }
}
