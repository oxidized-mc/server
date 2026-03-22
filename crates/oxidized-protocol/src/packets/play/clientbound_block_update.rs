//! Clientbound block update packet.
//!
//! Notifies the client that a single block has changed.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundBlockUpdatePacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::varint;
use crate::types::BlockPos;

/// Notifies the client that a single block has changed.
///
/// Wire format: `pos: Position | block_state: VarInt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundBlockUpdatePacket {
    /// Block position.
    pub pos: BlockPos,
    /// The new block state ID (from the global palette).
    pub block_state: i32,
}

impl Packet for ClientboundBlockUpdatePacket {
    const PACKET_ID: i32 = 0x08;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let pos = BlockPos::read(&mut data)?;
        let block_state = varint::read_varint_buf(&mut data)?;
        Ok(Self { pos, block_state })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(13);
        self.pos.write(&mut buf);
        varint::write_varint_buf(self.block_state, &mut buf);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundBlockUpdatePacket {
            pos: BlockPos::new(100, 64, -200),
            block_state: 1,
        };
        let encoded = pkt.encode();
        let decoded =
            ClientboundBlockUpdatePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.pos, pkt.pos);
        assert_eq!(decoded.block_state, 1);
    }

    #[test]
    fn test_air_state() {
        let pkt = ClientboundBlockUpdatePacket {
            pos: BlockPos::new(0, 0, 0),
            block_state: 0,
        };
        let encoded = pkt.encode();
        let decoded =
            ClientboundBlockUpdatePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }
}
