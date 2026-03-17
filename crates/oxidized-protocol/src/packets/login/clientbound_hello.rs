//! Clientbound encryption request — the server sends its public key and a
//! challenge token for the client to encrypt.
//!
//! Corresponds to `net.minecraft.network.protocol.login.ClientboundHelloPacket`.

use bytes::{Bytes, BytesMut};
use thiserror::Error;

use crate::codec::types::{self, TypeError};

/// Errors from decoding a [`ClientboundHelloPacket`].
#[derive(Debug, Error)]
pub enum HelloError {
    /// Type decode failure.
    #[error("type error: {0}")]
    Type(#[from] TypeError),
}

/// Clientbound packet `0x01` in the LOGIN state — encryption request.
///
/// Sent by the server when online-mode authentication is enabled. The client
/// must encrypt a shared secret and the challenge token using the provided
/// public key and respond with a
/// [`ServerboundKeyPacket`](super::ServerboundKeyPacket).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundHelloPacket {
    /// The server ID string (max 20 characters). Empty for vanilla servers.
    pub server_id: String,
    /// The server's RSA public key in DER (ASN.1) format.
    pub public_key: Vec<u8>,
    /// A random challenge token for the client to encrypt.
    pub challenge: Vec<u8>,
    /// Whether the client should proceed with Mojang authentication.
    pub should_authenticate: bool,
}

impl ClientboundHelloPacket {
    /// Packet ID in the LOGIN state.
    pub const PACKET_ID: i32 = 0x01;

    /// Decodes from the raw packet body.
    ///
    /// # Errors
    ///
    /// Returns [`HelloError`] if the buffer is truncated or a field is
    /// malformed.
    pub fn decode(mut data: Bytes) -> Result<Self, HelloError> {
        let server_id = types::read_string(&mut data, 20)?;
        let public_key = types::read_byte_array(&mut data, 256)?;
        let challenge = types::read_byte_array(&mut data, 256)?;
        let should_authenticate = types::read_bool(&mut data)?;
        Ok(Self {
            server_id,
            public_key,
            challenge,
            should_authenticate,
        })
    }

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        types::write_string(&mut buf, &self.server_id);
        types::write_byte_array(&mut buf, &self.public_key);
        types::write_byte_array(&mut buf, &self.challenge);
        types::write_bool(&mut buf, self.should_authenticate);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundHelloPacket {
            server_id: "".to_string(),
            public_key: vec![0x30, 0x82, 0x01, 0x22],
            challenge: vec![0x01, 0x02, 0x03, 0x04],
            should_authenticate: true,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundHelloPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }
}
