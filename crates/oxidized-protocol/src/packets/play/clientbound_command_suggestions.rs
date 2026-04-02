//! ClientboundCommandSuggestionsPacket (0x0F) — server returns tab-completions.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use oxidized_chat::Component;
use oxidized_codec::Packet;
use oxidized_codec::packet::PacketDecodeError;
use oxidized_codec::types::{read_string, write_string};
use oxidized_codec::varint::{read_varint_buf, write_varint_buf};

/// 0x0F — Server returns tab-completion suggestions.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundCommandSuggestionsPacket {
    /// Transaction ID — echoed from the request.
    pub id: i32,
    /// Start index in the input string.
    pub start: i32,
    /// Length of the range to replace.
    pub length: i32,
    /// Suggestion entries.
    pub suggestions: Vec<SuggestionEntry>,
}

/// A single tab-completion suggestion.
#[derive(Debug, Clone, PartialEq)]
pub struct SuggestionEntry {
    /// The suggested text.
    pub text: String,
    /// An optional tooltip.
    pub tooltip: Option<Component>,
}

impl Packet for ClientboundCommandSuggestionsPacket {
    const PACKET_ID: i32 = 0x0F;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let id = read_varint_buf(&mut data)?;
        let start = read_varint_buf(&mut data)?;
        let length = read_varint_buf(&mut data)?;
        let count = read_varint_buf(&mut data)?;
        let mut suggestions = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let text = read_string(&mut data, 32767)?;
            let has_tooltip = if data.has_remaining() {
                data.get_u8() != 0
            } else {
                false
            };
            let tooltip = if has_tooltip {
                let json_str = read_string(&mut data, 262144)?;
                let comp = serde_json::from_str(&json_str).map_err(|e| {
                    PacketDecodeError::InvalidData(format!("invalid tooltip JSON: {e}"))
                })?;
                Some(comp)
            } else {
                None
            };
            suggestions.push(SuggestionEntry { text, tooltip });
        }
        Ok(Self {
            id,
            start,
            length,
            suggestions,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(256);

        write_varint_buf(self.id, &mut buf);
        write_varint_buf(self.start, &mut buf);
        write_varint_buf(self.length, &mut buf);
        write_varint_buf(self.suggestions.len() as i32, &mut buf);

        for entry in &self.suggestions {
            write_string(&mut buf, &entry.text);
            // Has tooltip?
            if let Some(ref tooltip) = entry.tooltip {
                buf.put_u8(1); // true — has tooltip
                // Tooltip is a Chat Component serialized as JSON string
                let tooltip_json = tooltip.to_json().unwrap_or_default();
                write_string(&mut buf, &tooltip_json);
            } else {
                buf.put_u8(0); // false — no tooltip
            }
        }

        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = ClientboundCommandSuggestionsPacket {
            id: 1,
            start: 0,
            length: 5,
            suggestions: vec![SuggestionEntry {
                text: "test".into(),
                tooltip: None,
            }],
        };
        let encoded = Packet::encode(&pkt);
        let decoded =
            <ClientboundCommandSuggestionsPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.id, pkt.id);
        assert_eq!(decoded.start, pkt.start);
        assert_eq!(decoded.length, pkt.length);
        assert_eq!(decoded.suggestions.len(), 1);
        assert_eq!(decoded.suggestions[0].text, "test");
        assert!(decoded.suggestions[0].tooltip.is_none());
    }

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ClientboundCommandSuggestionsPacket, 0x0F);
    }
}
