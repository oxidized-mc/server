//! ServerboundSignUpdatePacket (0x3D) — player finishes editing a sign.
//!
//! Sent when the player closes the sign editor after placing or
//! interacting with a sign block.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;
use crate::types::BlockPos;

/// Maximum characters per sign line (protocol limit).
const MAX_LINE_LENGTH: usize = 384;

/// Serverbound packet for updating sign text.
///
/// # Wire Format
///
/// | Field | Type | Notes |
/// |-------|------|-------|
/// | pos | Position | Packed i64 block position |
/// | is_front_text | Boolean | `true` = front, `false` = back |
/// | line_1 | String(384) | First line of text |
/// | line_2 | String(384) | Second line of text |
/// | line_3 | String(384) | Third line of text |
/// | line_4 | String(384) | Fourth line of text |
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundSignUpdatePacket {
    /// Position of the sign block.
    pub pos: BlockPos,
    /// Whether the player is editing the front (`true`) or back (`false`).
    pub is_front_text: bool,
    /// The four lines of sign text.
    pub lines: [String; 4],
}

impl Packet for ServerboundSignUpdatePacket {
    const PACKET_ID: i32 = 0x3D;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let pos = BlockPos::read(&mut data)?;
        let is_front_text = types::read_bool(&mut data)?;
        let line0 = types::read_string(&mut data, MAX_LINE_LENGTH)?;
        let line1 = types::read_string(&mut data, MAX_LINE_LENGTH)?;
        let line2 = types::read_string(&mut data, MAX_LINE_LENGTH)?;
        let line3 = types::read_string(&mut data, MAX_LINE_LENGTH)?;
        Ok(Self {
            pos,
            is_front_text,
            lines: [line0, line1, line2, line3],
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(64);
        self.pos.write(&mut buf);
        types::write_bool(&mut buf, self.is_front_text);
        for line in &self.lines {
            types::write_string(&mut buf, line);
        }
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_front() {
        let original = ServerboundSignUpdatePacket {
            pos: BlockPos::new(10, 64, -5),
            is_front_text: true,
            lines: ["Hello".into(), "World".into(), String::new(), "!".into()],
        };
        let encoded = original.encode();
        let decoded = ServerboundSignUpdatePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.pos, original.pos);
        assert!(decoded.is_front_text);
        assert_eq!(decoded.lines[0], "Hello");
        assert_eq!(decoded.lines[1], "World");
        assert_eq!(decoded.lines[2], "");
        assert_eq!(decoded.lines[3], "!");
    }

    #[test]
    fn test_roundtrip_back() {
        let original = ServerboundSignUpdatePacket {
            pos: BlockPos::new(0, 0, 0),
            is_front_text: false,
            lines: [String::new(), String::new(), String::new(), String::new()],
        };
        let encoded = original.encode();
        let decoded = ServerboundSignUpdatePacket::decode(encoded.freeze()).unwrap();
        assert!(!decoded.is_front_text);
        assert!(decoded.lines.iter().all(|l| l.is_empty()));
    }
}
