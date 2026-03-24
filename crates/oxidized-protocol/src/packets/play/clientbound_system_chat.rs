//! ClientboundSystemChatPacket (0x79) — server-originated system message.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::chat::Component;
use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;

/// 0x79 — System chat message (no signature, no player sender).
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundSystemChatPacket {
    /// The message content.
    pub content: Component,
    /// If `true`, display on the action bar; if `false`, in the chat window.
    pub is_overlay: bool,
}

impl Packet for ClientboundSystemChatPacket {
    const PACKET_ID: i32 = 0x79;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let content = read_component_nbt(&mut data)?;
        let is_overlay = if data.has_remaining() {
            data.get_u8() != 0
        } else {
            false
        };
        Ok(Self {
            content,
            is_overlay,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(256);
        write_component_nbt(&mut buf, &self.content);
        buf.put_u8(u8::from(self.is_overlay));
        buf
    }
}

/// Writes a [`Component`] as network NBT (no root name) to the buffer.
pub fn write_component_nbt(buf: &mut BytesMut, component: &Component) {
    use oxidized_nbt::NbtTag;

    let tag = component.to_nbt();
    match tag {
        NbtTag::Compound(compound) => {
            let mut nbt_buf = Vec::new();
            #[allow(clippy::expect_used)]
            oxidized_nbt::write_network_nbt(&mut nbt_buf, &compound)
                .expect("NBT write to Vec should not fail");
            buf.extend_from_slice(&nbt_buf);
        },
        _ => {
            // Component::to_nbt should always return a Compound; fallback to empty.
            let compound = oxidized_nbt::NbtCompound::new();
            let mut nbt_buf = Vec::new();
            #[allow(clippy::expect_used)]
            oxidized_nbt::write_network_nbt(&mut nbt_buf, &compound)
                .expect("NBT write to Vec should not fail");
            buf.extend_from_slice(&nbt_buf);
        },
    }
}

/// Reads a [`Component`] from network NBT in the buffer.
pub fn read_component_nbt(data: &mut Bytes) -> Result<Component, PacketDecodeError> {
    let mut cursor = std::io::Cursor::new(data.as_ref());
    let mut acc = oxidized_nbt::NbtAccounter::unlimited();
    let compound = oxidized_nbt::read_network_nbt(&mut cursor, &mut acc)
        .map_err(|e| PacketDecodeError::InvalidData(format!("NBT decode error: {e}")))?;
    let consumed = cursor.position() as usize;
    data.advance(consumed);
    Component::from_nbt(&oxidized_nbt::NbtTag::Compound(compound))
        .map_err(|e| PacketDecodeError::InvalidData(format!("component decode error: {e}")))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_id() {
        assert_eq!(<ClientboundSystemChatPacket as Packet>::PACKET_ID, 0x79);
    }

    #[test]
    fn test_roundtrip_chat_message() {
        let pkt = ClientboundSystemChatPacket {
            content: Component::text("Hello world"),
            is_overlay: false,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundSystemChatPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.content, Component::text("Hello world"));
        assert!(!decoded.is_overlay);
    }

    #[test]
    fn test_roundtrip_action_bar() {
        let pkt = ClientboundSystemChatPacket {
            content: Component::text("Action bar!"),
            is_overlay: true,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundSystemChatPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.content, Component::text("Action bar!"));
        assert!(decoded.is_overlay);
    }

    #[test]
    fn test_styled_component_roundtrip() {
        use crate::chat::{ChatFormatting, TextColor};

        let pkt = ClientboundSystemChatPacket {
            content: Component::text("Warning!")
                .color(TextColor::Named(ChatFormatting::Red))
                .bold(),
            is_overlay: false,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundSystemChatPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.content, pkt.content);
    }
}
