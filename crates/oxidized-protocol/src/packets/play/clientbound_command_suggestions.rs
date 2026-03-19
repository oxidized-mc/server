//! ClientboundCommandSuggestionsPacket (0x0F) — server returns tab-completions.

use bytes::{BufMut, BytesMut};

use crate::chat::Component;
use crate::codec::types::write_string;
use crate::codec::varint::write_varint_buf;

/// 0x0F — Server returns tab-completion suggestions.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct SuggestionEntry {
    /// The suggested text.
    pub text: String,
    /// An optional tooltip.
    pub tooltip: Option<Component>,
}

impl ClientboundCommandSuggestionsPacket {
    /// Packet ID in the PLAY state.
    pub const PACKET_ID: i32 = 0x0F;

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
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
