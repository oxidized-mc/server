//! Serverbound encryption response — the client sends its encrypted shared
//! secret and challenge token.
//!
//! Corresponds to `net.minecraft.network.protocol.login.ServerboundKeyPacket`.

use bytes::{Bytes, BytesMut};

use oxidized_codec::Packet;
use oxidized_codec::packet::PacketDecodeError;
use oxidized_codec::types;

/// Maximum byte length for encrypted payloads (RSA-2048 output).
const MAX_ENCRYPTED_LEN: usize = 256;

/// Serverbound packet `0x01` in the LOGIN state — encryption response.
///
/// Contains the RSA-encrypted shared secret and the RSA-encrypted challenge
/// token. Both are at most 256 bytes (RSA-1024 output is 128 bytes; RSA-2048
/// output is 256 bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundKeyPacket {
    /// The RSA-encrypted shared secret.
    pub key_bytes: Vec<u8>,
    /// The RSA-encrypted challenge token.
    pub encrypted_challenge: Vec<u8>,
}

impl Packet for ServerboundKeyPacket {
    const PACKET_ID: i32 = 0x01;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let key_bytes = types::read_byte_array(&mut data, MAX_ENCRYPTED_LEN)?;
        let encrypted_challenge = types::read_byte_array(&mut data, MAX_ENCRYPTED_LEN)?;
        Ok(Self {
            key_bytes,
            encrypted_challenge,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        types::write_byte_array(&mut buf, &self.key_bytes);
        types::write_byte_array(&mut buf, &self.encrypted_challenge);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ServerboundKeyPacket {
            key_bytes: vec![0xDE, 0xAD, 0xBE, 0xEF],
            encrypted_challenge: vec![0xCA, 0xFE, 0xBA, 0xBE],
        };
        let encoded = pkt.encode();
        let decoded = ServerboundKeyPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ServerboundKeyPacket, 0x01);
    }
}
