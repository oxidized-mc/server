//! Clientbound block destruction packet.
//!
//! Shows a block-breaking crack animation on the client.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundBlockDestructionPacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::varint;
use crate::types::BlockPos;

/// Shows a block-breaking crack animation.
///
/// `progress` 0–9 shows increasing crack stages; 10 clears the animation.
///
/// Wire format: `entity_id: VarInt | pos: Position | progress: Byte`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundBlockDestructionPacket {
    /// Entity ID of the breaker (usually the player's entity ID).
    pub entity_id: i32,
    /// Block position.
    pub pos: BlockPos,
    /// Crack stage: 0–9 for visible damage, or 10 to clear.
    pub progress: u8,
}

impl Packet for ClientboundBlockDestructionPacket {
    const PACKET_ID: i32 = 0x05;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let entity_id = varint::read_varint_buf(&mut data)?;
        let pos = BlockPos::read(&mut data)?;
        if data.remaining() < 1 {
            return Err(PacketDecodeError::InvalidData(
                "Missing progress byte".into(),
            ));
        }
        let progress = data.get_u8();
        Ok(Self {
            entity_id,
            pos,
            progress,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(14);
        varint::write_varint_buf(self.entity_id, &mut buf);
        self.pos.write(&mut buf);
        buf.put_u8(self.progress);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundBlockDestructionPacket {
            entity_id: 42,
            pos: BlockPos::new(100, 64, -200),
            progress: 5,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundBlockDestructionPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.entity_id, 42);
        assert_eq!(decoded.pos, pkt.pos);
        assert_eq!(decoded.progress, 5);
    }

    #[test]
    fn test_clear_animation() {
        let pkt = ClientboundBlockDestructionPacket {
            entity_id: 1,
            pos: BlockPos::new(0, 0, 0),
            progress: 10,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundBlockDestructionPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.progress, 10);
    }
}
