//! ServerboundCommandSuggestionPacket (0x0F) — client requests tab-completions.

use bytes::Bytes;

use crate::codec::{types, varint};
use crate::packets::play::PlayPacketError;

/// 0x0F — Client requests tab-completion suggestions.
#[derive(Debug, Clone)]
pub struct ServerboundCommandSuggestionPacket {
    /// Transaction ID — echoed back in the response.
    pub id: i32,
    /// The partial command text (up to 32500 chars).
    pub command: String,
}

impl ServerboundCommandSuggestionPacket {
    /// Packet ID in the PLAY state.
    pub const PACKET_ID: i32 = 0x0F;

    /// Decodes the packet from raw bytes.
    pub fn decode(mut data: Bytes) -> Result<Self, PlayPacketError> {
        let id = varint::read_varint_buf(&mut data)?;
        let command = types::read_string(&mut data, 32500)?;
        Ok(Self { id, command })
    }
}
