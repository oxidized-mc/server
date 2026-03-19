//! ServerboundChatCommandPacket (0x07) — client dispatches an unsigned command.

use bytes::{Bytes, BytesMut};

use crate::codec::types;
use crate::packets::play::PlayPacketError;

/// 0x07 — Client dispatches an unsigned command (leading `/` already stripped).
#[derive(Debug, Clone)]
pub struct ServerboundChatCommandPacket {
    /// The command text without the leading `/`.
    pub command: String,
}

impl ServerboundChatCommandPacket {
    /// Packet ID in the PLAY state serverbound registry.
    pub const PACKET_ID: i32 = 0x07;

    /// Decodes the packet from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer is malformed or the command string
    /// exceeds 32 767 characters.
    pub fn decode(mut data: Bytes) -> Result<Self, PlayPacketError> {
        let command = types::read_string(&mut data, 32767)?;
        Ok(Self { command })
    }

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(self.command.len() + 5);
        types::write_string(&mut buf, &self.command);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_id() {
        assert_eq!(ServerboundChatCommandPacket::PACKET_ID, 0x07);
    }

    #[test]
    fn test_roundtrip() {
        let pkt = ServerboundChatCommandPacket {
            command: "say Hello everyone".to_string(),
        };
        let encoded = pkt.encode();
        let decoded = ServerboundChatCommandPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.command, "say Hello everyone");
    }

    #[test]
    fn test_empty_command() {
        let pkt = ServerboundChatCommandPacket {
            command: String::new(),
        };
        let encoded = pkt.encode();
        let decoded = ServerboundChatCommandPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.command, "");
    }
}
