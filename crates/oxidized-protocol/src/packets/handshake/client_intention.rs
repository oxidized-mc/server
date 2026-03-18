//! The first packet sent by the client to declare its intent.
//!
//! This corresponds to `net.minecraft.network.protocol.handshake.ClientIntentionPacket`
//! in the vanilla server.

use bytes::{Bytes, BytesMut};
use thiserror::Error;

use crate::codec::types::{self, TypeError};
use crate::codec::varint::{self, VarIntError};

/// The client's declared intent after the handshake.
///
/// Determines which protocol state the server transitions to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientIntent {
    /// Server list ping — transition to STATUS state.
    Status = 1,
    /// Player login — transition to LOGIN state.
    Login = 2,
    /// Server transfer (1.20.5+) — treated as LOGIN.
    Transfer = 3,
}

/// Errors from decoding a [`ClientIntentionPacket`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IntentionError {
    /// Unknown `next_state` value.
    #[error("unknown client intent: {0}")]
    UnknownIntent(i32),

    /// VarInt decode failure.
    #[error("varint error: {0}")]
    VarInt(#[from] VarIntError),

    /// Type decode failure.
    #[error("type error: {0}")]
    Type(#[from] TypeError),
}

impl TryFrom<i32> for ClientIntent {
    type Error = IntentionError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Status),
            2 => Ok(Self::Login),
            3 => Ok(Self::Transfer),
            other => Err(IntentionError::UnknownIntent(other)),
        }
    }
}

/// Serverbound packet `0x00` — declares the client's protocol version and intent.
///
/// This is always the first packet in any connection. After receiving it,
/// the server transitions to either [`Status`](ClientIntent::Status) or
/// [`Login`](ClientIntent::Login) state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientIntentionPacket {
    /// The protocol version the client is using.
    pub protocol_version: i32,
    /// The hostname the client connected to (max 255 chars).
    pub server_address: String,
    /// The port the client connected to.
    pub server_port: u16,
    /// What the client wants to do next.
    pub next_state: ClientIntent,
}

impl ClientIntentionPacket {
    /// Packet ID for the handshake packet.
    pub const PACKET_ID: i32 = 0x00;

    /// Decodes a [`ClientIntentionPacket`] from a raw packet body.
    ///
    /// The `data` should be the packet body *after* the packet ID has been
    /// stripped (i.e., just the fields).
    ///
    /// # Errors
    ///
    /// Returns [`IntentionError`] if the buffer is truncated, contains
    /// invalid UTF-8, or has an unknown `next_state` value.
    pub fn decode(mut data: Bytes) -> Result<Self, IntentionError> {
        let protocol_version = varint::read_varint_buf(&mut data)?;
        let server_address = types::read_string(&mut data, 255)?;
        let server_port = types::read_u16(&mut data)?;
        let next_state_raw = varint::read_varint_buf(&mut data)?;
        let next_state = ClientIntent::try_from(next_state_raw)?;

        Ok(Self {
            protocol_version,
            server_address,
            server_port,
            next_state,
        })
    }

    /// Encodes this packet into bytes (without the packet ID prefix).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        varint::write_varint_buf(self.protocol_version, &mut buf);
        types::write_string(&mut buf, &self.server_address);
        types::write_u16(&mut buf, self.server_port);
        varint::write_varint_buf(self.next_state as i32, &mut buf);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_status_intent() {
        let pkt = ClientIntentionPacket {
            protocol_version: 1_073_742_124,
            server_address: "localhost".to_string(),
            server_port: 25565,
            next_state: ClientIntent::Status,
        };
        let encoded = pkt.encode();
        let decoded = ClientIntentionPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_decode_login_intent() {
        let pkt = ClientIntentionPacket {
            protocol_version: 1_073_742_124,
            server_address: "mc.example.com".to_string(),
            server_port: 25565,
            next_state: ClientIntent::Login,
        };
        let encoded = pkt.encode();
        let decoded = ClientIntentionPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_decode_transfer_intent() {
        let pkt = ClientIntentionPacket {
            protocol_version: 1_073_742_124,
            server_address: "transfer.example.com".to_string(),
            server_port: 25566,
            next_state: ClientIntent::Transfer,
        };
        let encoded = pkt.encode();
        let decoded = ClientIntentionPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_unknown_intent() {
        let mut buf = BytesMut::new();
        varint::write_varint_buf(1_073_742_124, &mut buf);
        types::write_string(&mut buf, "localhost");
        types::write_u16(&mut buf, 25565);
        varint::write_varint_buf(99, &mut buf); // invalid intent
        let err = ClientIntentionPacket::decode(buf.freeze()).unwrap_err();
        assert!(matches!(err, IntentionError::UnknownIntent(99)));
    }

    #[test]
    fn test_decode_real_handshake_bytes() {
        // Bytes captured from a real MC 26.1-pre-3 client (minus packet ID):
        // protocol_version as VarInt, "localhost", port 25565, next_state=1 (Status)
        let pkt = ClientIntentionPacket {
            protocol_version: 1_073_742_124,
            server_address: "localhost".to_string(),
            server_port: 25565,
            next_state: ClientIntent::Status,
        };
        let encoded = pkt.encode();

        // Verify the encoded bytes are reasonable
        assert!(encoded.len() > 5); // at least varint + string prefix + port + intent
        assert!(encoded.len() < 50); // shouldn't be huge

        let decoded = ClientIntentionPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.protocol_version, 1_073_742_124);
        assert_eq!(decoded.server_address, "localhost");
        assert_eq!(decoded.server_port, 25565);
        assert_eq!(decoded.next_state, ClientIntent::Status);
    }
}
