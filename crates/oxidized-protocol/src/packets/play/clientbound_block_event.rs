//! Clientbound block event packet.
//!
//! Block action event for pistons, note blocks, chests, etc.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundBlockEventPacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::varint;
use crate::types::BlockPos;

/// Block event (piston movement, note block sound, chest open/close).
///
/// Wire format: `pos: Position | action_type: Byte | action_param: Byte
/// | block_type_id: VarInt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundBlockEventPacket {
    /// Block position.
    pub pos: BlockPos,
    /// Action type (block-specific meaning).
    pub action_type: u8,
    /// Action parameter (block-specific meaning).
    pub action_param: u8,
    /// Block type ID — identifies which block type this event belongs to.
    pub block_type_id: i32,
}

impl Packet for ClientboundBlockEventPacket {
    const PACKET_ID: i32 = 0x07;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let pos = BlockPos::read(&mut data)?;
        if data.remaining() < 2 {
            return Err(PacketDecodeError::InvalidData(
                "Missing action bytes".into(),
            ));
        }
        let action_type = data.get_u8();
        let action_param = data.get_u8();
        let block_type_id = varint::read_varint_buf(&mut data)?;
        Ok(Self {
            pos,
            action_type,
            action_param,
            block_type_id,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(16);
        self.pos.write(&mut buf);
        buf.put_u8(self.action_type);
        buf.put_u8(self.action_param);
        varint::write_varint_buf(self.block_type_id, &mut buf);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundBlockEventPacket {
            pos: BlockPos::new(10, 64, 20),
            action_type: 1,
            action_param: 2,
            block_type_id: 100,
        };
        let encoded = pkt.encode();
        let decoded =
            ClientboundBlockEventPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.pos, pkt.pos);
        assert_eq!(decoded.action_type, 1);
        assert_eq!(decoded.action_param, 2);
        assert_eq!(decoded.block_type_id, 100);
    }
}
