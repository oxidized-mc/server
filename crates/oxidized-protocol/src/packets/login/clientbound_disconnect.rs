//! Clientbound disconnect — the server kicks the client during login with a
//! JSON text component reason.
//!
//! Corresponds to `net.minecraft.network.protocol.login.ClientboundLoginDisconnectPacket`.

use bytes::{Bytes, BytesMut};
use thiserror::Error;

use crate::codec::types::{self, TypeError};

/// Errors from decoding a [`ClientboundDisconnectPacket`].
#[derive(Debug, Error)]
pub enum DisconnectError {
    /// Type decode failure.
    #[error("type error: {0}")]
    Type(#[from] TypeError),
}

/// Clientbound packet `0x00` in the LOGIN state — disconnect.
///
/// Sent by the server to terminate the login sequence. The `reason` field is a
/// JSON text component displayed to the player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundDisconnectPacket {
    /// The disconnect reason as a JSON text component (max 262144 chars).
    pub reason: String,
}

impl ClientboundDisconnectPacket {
    /// Packet ID in the LOGIN state.
    pub const PACKET_ID: i32 = 0x00;

    /// Maximum character length for the reason JSON.
    const MAX_REASON_CHARS: usize = 262_144;

    /// Decodes from the raw packet body.
    ///
    /// # Errors
    ///
    /// Returns [`DisconnectError`] if the buffer is truncated or the string
    /// is invalid.
    pub fn decode(mut data: Bytes) -> Result<Self, DisconnectError> {
        let reason = types::read_string(&mut data, Self::MAX_REASON_CHARS)?;
        Ok(Self { reason })
    }

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        types::write_string(&mut buf, &self.reason);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundDisconnectPacket {
            reason: r#"{"text":"You are banned!"}"#.to_string(),
        };
        let encoded = pkt.encode();
        let decoded = ClientboundDisconnectPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }
}
