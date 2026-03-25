//! ClientboundPlayerChatPacket (0x41) — signed player chat message.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::chat::Component;
use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::{types, varint};
use crate::packets::play::clientbound_system_chat::{read_component_nbt, write_component_nbt};

/// Filter mask type for chat messages.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterMask {
    /// No filtering — message passes through completely.
    PassThrough,
    /// Entire message is filtered/hidden.
    FullyFiltered,
    /// Specific characters are filtered (indicated by BitSet).
    PartiallyFiltered(Vec<i64>),
}

/// 0x41 — Signed player chat message.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundPlayerChatPacket {
    /// Per-connection global message index (increments for each message sent).
    pub global_index: i32,
    /// Sender UUID.
    pub sender: uuid::Uuid,
    /// Sender's message index (VarInt).
    pub index: i32,
    /// Optional 256-byte message signature.
    pub message_signature: Option<[u8; 256]>,
    /// Plain text content of the message (max 256 chars).
    pub message_content: String,
    /// Timestamp in milliseconds since epoch.
    pub timestamp: i64,
    /// Salt for signature verification.
    pub salt: i64,
    /// Optional unsigned (decorated) content as a [`Component`].
    pub unsigned_content: Option<Component>,
    /// Filter mask for profanity/moderation.
    pub filter_mask: FilterMask,
    /// Chat type registry ID.
    pub chat_type_id: i32,
    /// Sender display name.
    pub sender_name: Component,
    /// Target name (for DMs).
    pub target_name: Option<Component>,
}

impl Packet for ClientboundPlayerChatPacket {
    const PACKET_ID: i32 = 0x41;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let global_index = varint::read_varint_buf(&mut data)?;
        let sender = types::read_uuid(&mut data)?;
        let index = varint::read_varint_buf(&mut data)?;
        let has_sig = types::read_bool(&mut data)?;
        let message_signature = if has_sig {
            types::ensure_remaining(&data, 256, "PlayerChatPacket message signature")?;
            let mut sig = [0u8; 256];
            data.copy_to_slice(&mut sig);
            Some(sig)
        } else {
            None
        };

        let message_content = types::read_string(&mut data, 256)?;
        let timestamp = types::read_i64(&mut data)?;
        let salt = types::read_i64(&mut data)?;
        let last_seen_count = varint::read_varint_buf(&mut data)?;
        if !(0..=128).contains(&last_seen_count) {
            return Err(PacketDecodeError::InvalidData(format!(
                "invalid last_seen_count: {last_seen_count}"
            )));
        }
        for _ in 0..last_seen_count {
            let packed_id = varint::read_varint_buf(&mut data)?;
            if packed_id == 0 {
                types::ensure_remaining(&data, 256, "PlayerChatPacket last_seen signature")?;
                data.advance(256);
            }
        }

        let has_unsigned = types::read_bool(&mut data)?;
        let unsigned_content = if has_unsigned {
            Some(read_component_nbt(&mut data)?)
        } else {
            None
        };

        let filter_type = varint::read_varint_buf(&mut data)?;
        let filter_mask = match filter_type {
            0 => FilterMask::PassThrough,
            1 => FilterMask::FullyFiltered,
            2 => {
                let len = varint::read_varint_buf(&mut data)?;
                if !(0..=256).contains(&len) {
                    return Err(PacketDecodeError::InvalidData(format!(
                        "filter mask bitset length out of \
                             range: {len}"
                    )));
                }
                let mut bits = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    bits.push(types::read_i64(&mut data)?);
                }
                FilterMask::PartiallyFiltered(bits)
            },
            other => {
                return Err(PacketDecodeError::InvalidData(format!(
                    "unknown filter mask type: {other}"
                )));
            },
        };

        let holder_id = varint::read_varint_buf(&mut data)?;
        let chat_type_id = holder_id - 1;
        let sender_name = read_component_nbt(&mut data)?;
        let has_target = types::read_bool(&mut data)?;
        let target_name = if has_target {
            Some(read_component_nbt(&mut data)?)
        } else {
            None
        };

        Ok(Self {
            global_index,
            sender,
            index,
            message_signature,
            message_content,
            timestamp,
            salt,
            unsigned_content,
            filter_mask,
            chat_type_id,
            sender_name,
            target_name,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(512);

        // Global message index
        varint::write_varint_buf(self.global_index, &mut buf);

        // Sender UUID
        types::write_uuid(&mut buf, &self.sender);

        // Index
        varint::write_varint_buf(self.index, &mut buf);

        // Message signature (optional)
        types::write_bool(&mut buf, self.message_signature.is_some());
        if let Some(ref sig) = self.message_signature {
            buf.put_slice(sig);
        }

        // Message body (content, timestamp, salt, empty last_seen)
        types::write_string(&mut buf, &self.message_content);
        types::write_i64(&mut buf, self.timestamp);
        types::write_i64(&mut buf, self.salt);
        // Last seen packed list (empty = count 0)
        varint::write_varint_buf(0, &mut buf);

        // Unsigned content (optional Component as NBT)
        if let Some(ref content) = self.unsigned_content {
            types::write_bool(&mut buf, true);
            write_component_nbt(&mut buf, content);
        } else {
            types::write_bool(&mut buf, false);
        }

        // Filter mask
        match &self.filter_mask {
            FilterMask::PassThrough => varint::write_varint_buf(0, &mut buf),
            FilterMask::FullyFiltered => varint::write_varint_buf(1, &mut buf),
            FilterMask::PartiallyFiltered(bits) => {
                varint::write_varint_buf(2, &mut buf);
                #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                varint::write_varint_buf(bits.len() as i32, &mut buf);
                for &long in bits {
                    buf.put_i64(long);
                }
            },
        }

        // Chat type bound (Holder<ChatType> + name + optional target)
        varint::write_varint_buf(self.chat_type_id + 1, &mut buf);
        write_component_nbt(&mut buf, &self.sender_name);
        if let Some(ref target) = self.target_name {
            types::write_bool(&mut buf, true);
            write_component_nbt(&mut buf, target);
        } else {
            types::write_bool(&mut buf, false);
        }

        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn make_packet() -> ClientboundPlayerChatPacket {
        ClientboundPlayerChatPacket {
            global_index: 0,
            sender: uuid::Uuid::nil(),
            index: 0,
            message_signature: None,
            message_content: "Hello world".to_string(),
            timestamp: 1_700_000_000_000,
            salt: 42,
            unsigned_content: None,
            filter_mask: FilterMask::PassThrough,
            chat_type_id: 0,
            sender_name: Component::text("Steve"),
            target_name: None,
        }
    }

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ClientboundPlayerChatPacket, 0x41);
    }

    #[test]
    fn test_roundtrip_unsigned() {
        let pkt = make_packet();
        let encoded = pkt.encode();
        let decoded = ClientboundPlayerChatPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.sender, uuid::Uuid::nil());
        assert_eq!(decoded.message_content, "Hello world");
        assert_eq!(decoded.chat_type_id, 0);
        assert_eq!(decoded.sender_name, Component::text("Steve"));
        assert!(decoded.target_name.is_none());
    }

    #[test]
    fn test_roundtrip_with_unsigned_content() {
        let mut pkt = make_packet();
        pkt.unsigned_content = Some(Component::text("Decorated Hello"));
        let encoded = pkt.encode();
        let decoded = ClientboundPlayerChatPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(
            decoded.unsigned_content,
            Some(Component::text("Decorated Hello"))
        );
    }

    #[test]
    fn test_roundtrip_fully_filtered() {
        let mut pkt = make_packet();
        pkt.filter_mask = FilterMask::FullyFiltered;
        let encoded = pkt.encode();
        let decoded = ClientboundPlayerChatPacket::decode(encoded.freeze()).unwrap();
        assert!(matches!(decoded.filter_mask, FilterMask::FullyFiltered));
    }

    #[test]
    fn test_roundtrip_with_target() {
        let mut pkt = make_packet();
        pkt.target_name = Some(Component::text("Alex"));
        let encoded = pkt.encode();
        let decoded = ClientboundPlayerChatPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.target_name, Some(Component::text("Alex")));
    }
}
