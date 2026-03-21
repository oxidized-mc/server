//! ClientboundDisguisedChatPacket (0x21) — disguised chat message.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::chat::Component;
use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::varint;
use crate::packets::play::clientbound_system_chat::{read_component_nbt, write_component_nbt};

/// 0x21 — Disguised chat (chat type + sender name, no UUID/signature).
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundDisguisedChatPacket {
    /// The message content.
    pub message: Component,
    /// Registry ID of the chat type (VarInt, encoded as id+1 for Holder).
    pub chat_type_id: i32,
    /// Sender display name.
    pub sender_name: Component,
    /// Target name (for whisper messages).
    pub target_name: Option<Component>,
}

impl Packet for ClientboundDisguisedChatPacket {
    const PACKET_ID: i32 = 0x21;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let message = read_component_nbt(&mut data)?;
        let holder_id = varint::read_varint_buf(&mut data)?;
        let chat_type_id = holder_id - 1;
        let sender_name = read_component_nbt(&mut data)?;
        let has_target = if data.has_remaining() {
            data.get_u8() != 0
        } else {
            false
        };
        let target_name = if has_target {
            Some(read_component_nbt(&mut data)?)
        } else {
            None
        };
        Ok(Self {
            message,
            chat_type_id,
            sender_name,
            target_name,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(256);
        write_component_nbt(&mut buf, &self.message);
        // Chat type as Holder reference (id + 1)
        varint::write_varint_buf(self.chat_type_id + 1, &mut buf);
        write_component_nbt(&mut buf, &self.sender_name);
        // Optional target name
        if let Some(ref target) = self.target_name {
            buf.put_u8(1);
            write_component_nbt(&mut buf, target);
        } else {
            buf.put_u8(0);
        }
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_id() {
        assert_eq!(<ClientboundDisguisedChatPacket as Packet>::PACKET_ID, 0x21);
    }

    #[test]
    fn test_roundtrip_no_target() {
        let pkt = ClientboundDisguisedChatPacket {
            message: Component::text("Hello"),
            chat_type_id: 0,
            sender_name: Component::text("Server"),
            target_name: None,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundDisguisedChatPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.message, Component::text("Hello"));
        assert_eq!(decoded.chat_type_id, 0);
        assert_eq!(decoded.sender_name, Component::text("Server"));
        assert!(decoded.target_name.is_none());
    }

    #[test]
    fn test_roundtrip_with_target() {
        let pkt = ClientboundDisguisedChatPacket {
            message: Component::text("whisper"),
            chat_type_id: 2,
            sender_name: Component::text("Alice"),
            target_name: Some(Component::text("Bob")),
        };
        let encoded = pkt.encode();
        let decoded = ClientboundDisguisedChatPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.target_name, Some(Component::text("Bob")));
    }
}
