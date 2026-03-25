//! ServerboundChatCommandPacket (0x07) — client dispatches an unsigned command.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;

/// 0x07 — Client dispatches an unsigned command (leading `/` already stripped).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundChatCommandPacket {
    /// The command text without the leading `/`.
    pub command: String,
}

impl Packet for ServerboundChatCommandPacket {
    const PACKET_ID: i32 = 0x07;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let command = types::read_string(&mut data, 32767)?;
        Ok(Self { command })
    }

    fn encode(&self) -> BytesMut {
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
        assert_packet_id!(ServerboundChatCommandPacket, 0x07);
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

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = ServerboundChatCommandPacket {
            command: "say hello".to_string(),
        };
        let encoded = Packet::encode(&pkt);
        let decoded = <ServerboundChatCommandPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.command, "say hello");
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(<ServerboundChatCommandPacket as Packet>::PACKET_ID, 0x07);
    }
}
