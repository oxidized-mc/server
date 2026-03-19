//! ServerboundCommandSuggestionPacket (0x0F) — client requests tab-completions.

use bytes::{Bytes, BytesMut};

use crate::codec::packet::PacketDecodeError;
use crate::codec::{types, varint, Packet};
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

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(self.command.len() + 10);
        varint::write_varint_buf(self.id, &mut buf);
        types::write_string(&mut buf, &self.command);
        buf
    }
}

impl Packet for ServerboundCommandSuggestionPacket {
    const PACKET_ID: i32 = 0x0F;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let id = varint::read_varint_buf(&mut data)?;
        let command = types::read_string(&mut data, 32500)?;
        Ok(Self { id, command })
    }

    fn encode(&self) -> BytesMut {
        self.encode()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = ServerboundCommandSuggestionPacket {
            id: 42,
            command: "/tp Steve".into(),
        };
        let encoded = Packet::encode(&pkt);
        let decoded = <ServerboundCommandSuggestionPacket as Packet>::decode(
            encoded.freeze(),
        )
        .unwrap();
        assert_eq!(decoded.id, 42);
        assert_eq!(decoded.command, "/tp Steve");
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(
            <ServerboundCommandSuggestionPacket as Packet>::PACKET_ID,
            0x0F
        );
    }
}
