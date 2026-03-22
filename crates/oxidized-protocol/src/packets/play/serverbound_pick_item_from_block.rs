//! Serverbound pick-item-from-block packet.
//!
//! Sent when the player middle-clicks a block in creative mode to pick it.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ServerboundPickItemFromBlockPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;
use crate::types::BlockPos;

/// Pick block item packet (0x24).
///
/// # Wire Format
///
/// | Field | Type | Description |
/// |-------|------|-------------|
/// | pos | i64 | Packed block position |
/// | include_data | bool | Whether to copy block entity data (NBT) |
#[derive(Debug, Clone, PartialEq)]
pub struct ServerboundPickItemFromBlockPacket {
    /// Block position to pick from.
    pub pos: BlockPos,
    /// Whether to include block entity NBT data.
    pub include_data: bool,
}

impl Packet for ServerboundPickItemFromBlockPacket {
    const PACKET_ID: i32 = 0x24;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let raw = types::read_i64(&mut data)?;
        let pos = BlockPos::from_long(raw);
        let include_data = types::read_bool(&mut data)?;
        Ok(Self { pos, include_data })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(9);
        types::write_i64(&mut buf, self.pos.as_long());
        types::write_bool(&mut buf, self.include_data);
        buf
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_packet_id() {
        assert_eq!(
            <ServerboundPickItemFromBlockPacket as Packet>::PACKET_ID,
            0x24
        );
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let pkt = ServerboundPickItemFromBlockPacket {
            pos: BlockPos::new(100, 64, -200),
            include_data: true,
        };
        let buf = pkt.encode();
        let decoded = ServerboundPickItemFromBlockPacket::decode(buf.freeze()).unwrap();
        assert_eq!(decoded.pos.x, 100);
        assert_eq!(decoded.pos.y, 64);
        assert_eq!(decoded.pos.z, -200);
        assert!(decoded.include_data);
    }

    #[test]
    fn test_without_data() {
        let pkt = ServerboundPickItemFromBlockPacket {
            pos: BlockPos::new(0, 0, 0),
            include_data: false,
        };
        let buf = pkt.encode();
        let decoded = ServerboundPickItemFromBlockPacket::decode(buf.freeze()).unwrap();
        assert!(!decoded.include_data);
    }
}
