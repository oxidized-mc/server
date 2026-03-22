//! Clientbound open-sign-editor packet.
//!
//! Sent after placing a sign to open the sign editing UI on the client.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundOpenSignEditorPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;
use crate::types::BlockPos;

/// Opens the sign editor on the client (0x3C).
///
/// # Wire Format
///
/// | Field | Type | Description |
/// |-------|------|-------------|
/// | pos | i64 | Packed block position |
/// | is_front_text | bool | `true` for front, `false` for back |
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundOpenSignEditorPacket {
    /// Block position of the sign.
    pub pos: BlockPos,
    /// Whether to edit the front text (`true`) or back text (`false`).
    pub is_front_text: bool,
}

impl Packet for ClientboundOpenSignEditorPacket {
    const PACKET_ID: i32 = 0x3C;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let raw = types::read_i64(&mut data)?;
        let pos = BlockPos::from_long(raw);
        let is_front_text = types::read_bool(&mut data)?;
        Ok(Self { pos, is_front_text })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(9);
        types::write_i64(&mut buf, self.pos.as_long());
        types::write_bool(&mut buf, self.is_front_text);
        buf
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_packet_id() {
        assert_eq!(<ClientboundOpenSignEditorPacket as Packet>::PACKET_ID, 0x3C);
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let pkt = ClientboundOpenSignEditorPacket {
            pos: BlockPos::new(100, 64, -200),
            is_front_text: true,
        };
        let buf = pkt.encode();
        let decoded = ClientboundOpenSignEditorPacket::decode(buf.freeze()).unwrap();
        assert_eq!(decoded.pos.x, 100);
        assert_eq!(decoded.pos.y, 64);
        assert_eq!(decoded.pos.z, -200);
        assert!(decoded.is_front_text);
    }

    #[test]
    fn test_back_text() {
        let pkt = ClientboundOpenSignEditorPacket {
            pos: BlockPos::new(0, 0, 0),
            is_front_text: false,
        };
        let buf = pkt.encode();
        let decoded = ClientboundOpenSignEditorPacket::decode(buf.freeze()).unwrap();
        assert!(!decoded.is_front_text);
    }
}
