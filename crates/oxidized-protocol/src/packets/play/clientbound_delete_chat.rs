//! ClientboundDeleteChatPacket (0x1F) — delete a chat message by index.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::varint;
use crate::packets::play::PlayPacketError;

/// 0x1F — Server requests client to delete a chat message.
///
/// If `packed_message_id` is 0, the message is identified by its full 256-byte
/// signature. Otherwise the value is a cache index (wire value − 1).
#[derive(Debug, Clone)]
pub struct ClientboundDeleteChatPacket {
    /// The packed message ID (VarInt).
    /// If the value is 0, the next 256 bytes are the full signature.
    /// If > 0, it is a cache index (value − 1).
    pub packed_message_id: i32,
    /// Full 256-byte signature, present only when `packed_message_id == 0`.
    pub full_signature: Option<[u8; 256]>,
}

impl ClientboundDeleteChatPacket {
    /// Packet ID in the PLAY state.
    pub const PACKET_ID: i32 = 0x1F;

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(261);
        if let Some(ref sig) = self.full_signature {
            varint::write_varint_buf(0, &mut buf);
            buf.put_slice(sig);
        } else {
            varint::write_varint_buf(self.packed_message_id + 1, &mut buf);
        }
        buf
    }

    /// Decodes the packet from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer is malformed.
    pub fn decode(mut data: Bytes) -> Result<Self, PlayPacketError> {
        let id = varint::read_varint_buf(&mut data)?;
        if id == 0 {
            if data.remaining() < 256 {
                return Err(PlayPacketError::UnexpectedEof);
            }
            let mut sig = [0u8; 256];
            data.copy_to_slice(&mut sig);
            Ok(Self {
                packed_message_id: 0,
                full_signature: Some(sig),
            })
        } else {
            Ok(Self {
                packed_message_id: id - 1,
                full_signature: None,
            })
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_id() {
        assert_eq!(ClientboundDeleteChatPacket::PACKET_ID, 0x1F);
    }

    #[test]
    fn test_roundtrip_cached_id() {
        let pkt = ClientboundDeleteChatPacket {
            packed_message_id: 5,
            full_signature: None,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundDeleteChatPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.packed_message_id, 5);
        assert!(decoded.full_signature.is_none());
    }

    #[test]
    fn test_roundtrip_full_signature() {
        let mut sig = [0u8; 256];
        sig[0] = 0xDE;
        sig[255] = 0xAD;
        let pkt = ClientboundDeleteChatPacket {
            packed_message_id: 0,
            full_signature: Some(sig),
        };
        let encoded = pkt.encode();
        let decoded = ClientboundDeleteChatPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.full_signature.unwrap()[0], 0xDE);
        assert_eq!(decoded.full_signature.unwrap()[255], 0xAD);
    }
}
