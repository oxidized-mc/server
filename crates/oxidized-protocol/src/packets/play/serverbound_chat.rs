//! ServerboundChatPacket (0x09) — player sends a plain chat message.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use oxidized_codec::Packet;
use oxidized_codec::packet::PacketDecodeError;
use oxidized_codec::{types, varint};

/// Acknowledgement state for recent chat messages.
///
/// Wire format: `VarInt offset` + `FixedBitSet(20) acknowledged` (3 bytes) + `byte checksum`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LastSeenMessagesUpdate {
    /// Offset into the message chain.
    pub offset: i32,
    /// Fixed 20-bit bitset of acknowledged messages (3 bytes, packed).
    pub acknowledged: [u8; 3],
    /// Integrity checksum (0 = ignore).
    pub checksum: u8,
}

impl LastSeenMessagesUpdate {
    /// Decodes from a buffer.
    ///
    /// # Errors
    ///
    /// Returns [`PacketDecodeError`] if the buffer is truncated or contains
    /// invalid data.
    pub fn decode(data: &mut Bytes) -> Result<Self, PacketDecodeError> {
        let offset = varint::read_varint_buf(data)?;
        types::ensure_remaining(data, 4, "LastSeenMessagesUpdate ack+checksum")?;
        let mut acknowledged = [0u8; 3];
        data.copy_to_slice(&mut acknowledged);
        let checksum = data.get_u8();
        Ok(Self {
            offset,
            acknowledged,
            checksum,
        })
    }

    /// Encodes into a buffer.
    pub fn encode(&self, buf: &mut BytesMut) {
        varint::write_varint_buf(self.offset, buf);
        buf.put_slice(&self.acknowledged);
        buf.put_u8(self.checksum);
    }
}

/// 0x09 — Client sends a plain chat message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundChatPacket {
    /// The chat message text (max 256 characters).
    pub message: String,
    /// Timestamp in milliseconds since epoch.
    pub timestamp: i64,
    /// Random salt for signature verification.
    pub salt: i64,
    /// Optional 256-byte RSA signature.
    pub signature: Option<[u8; 256]>,
    /// Acknowledgement of recently seen messages.
    pub last_seen: LastSeenMessagesUpdate,
}

impl Packet for ServerboundChatPacket {
    const PACKET_ID: i32 = 0x09;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let message = types::read_string(&mut data, 256)?;
        if message.starts_with('/') {
            return Err(PacketDecodeError::InvalidData(
                "chat message must not start with '/'".to_string(),
            ));
        }
        let timestamp = types::read_i64(&mut data)?;
        let salt = types::read_i64(&mut data)?;
        let has_sig = types::read_bool(&mut data)?;
        let signature = if has_sig {
            types::ensure_remaining(&data, 256, "ServerboundChatPacket signature")?;
            let mut sig = [0u8; 256];
            data.copy_to_slice(&mut sig);
            Some(sig)
        } else {
            None
        };
        let last_seen = LastSeenMessagesUpdate::decode(&mut data)?;
        Ok(Self {
            message,
            timestamp,
            salt,
            signature,
            last_seen,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(512);
        types::write_string(&mut buf, &self.message);
        types::write_i64(&mut buf, self.timestamp);
        types::write_i64(&mut buf, self.salt);
        types::write_bool(&mut buf, self.signature.is_some());
        if let Some(ref sig) = self.signature {
            buf.put_slice(sig);
        }
        self.last_seen.encode(&mut buf);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn make_packet(message: &str) -> ServerboundChatPacket {
        ServerboundChatPacket {
            message: message.to_string(),
            timestamp: 1_700_000_000_000,
            salt: 42,
            signature: None,
            last_seen: LastSeenMessagesUpdate::default(),
        }
    }

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ServerboundChatPacket, 0x09);
    }

    #[test]
    fn test_roundtrip() {
        let pkt = make_packet("Hello world");
        let encoded = pkt.encode();
        let decoded = ServerboundChatPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.message, "Hello world");
        assert_eq!(decoded.timestamp, 1_700_000_000_000);
        assert_eq!(decoded.salt, 42);
        assert!(decoded.signature.is_none());
    }

    #[test]
    fn test_roundtrip_with_signature() {
        let mut pkt = make_packet("signed msg");
        let mut sig = [0u8; 256];
        sig[0] = 0xAB;
        sig[255] = 0xCD;
        pkt.signature = Some(sig);
        let encoded = pkt.encode();
        let decoded = ServerboundChatPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.signature.unwrap()[0], 0xAB);
        assert_eq!(decoded.signature.unwrap()[255], 0xCD);
    }

    #[test]
    fn test_rejects_slash_prefix() {
        let pkt = make_packet("/gamemode creative");
        let encoded = pkt.encode();
        let result = ServerboundChatPacket::decode(encoded.freeze());
        assert!(result.is_err(), "slash-prefixed message must be rejected");
    }

    #[test]
    fn test_rejects_over_256_chars() {
        let long_msg = "a".repeat(257);
        let pkt = make_packet(&long_msg);
        let encoded = pkt.encode();
        let result = ServerboundChatPacket::decode(encoded.freeze());
        assert!(result.is_err(), "message over 256 chars must be rejected");
    }

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = ServerboundChatPacket {
            message: "hello".to_string(),
            timestamp: 1234,
            salt: 0,
            signature: None,
            last_seen: LastSeenMessagesUpdate::default(),
        };
        let encoded = Packet::encode(&pkt);
        let decoded = <ServerboundChatPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.message, "hello");
        assert_eq!(decoded.timestamp, 1234);
        assert_eq!(decoded.salt, 0);
        assert!(decoded.signature.is_none());
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(<ServerboundChatPacket as Packet>::PACKET_ID, 0x09);
    }
}
