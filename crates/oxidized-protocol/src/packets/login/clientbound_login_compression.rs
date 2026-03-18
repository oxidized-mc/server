//! Clientbound set compression — the server tells the client to enable
//! compression above a given threshold.
//!
//! Corresponds to `net.minecraft.network.protocol.login.ClientboundLoginCompressionPacket`.

use bytes::{Bytes, BytesMut};
use thiserror::Error;

use crate::codec::varint::{self, VarIntError};

/// Errors from decoding a [`ClientboundLoginCompressionPacket`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LoginCompressionError {
    /// VarInt decode failure.
    #[error("varint error: {0}")]
    VarInt(#[from] VarIntError),
}

/// Clientbound packet `0x03` in the LOGIN state — set compression.
///
/// Sent by the server to enable protocol compression. Packets larger than
/// `threshold` bytes (uncompressed) will be zlib-compressed. A negative
/// threshold disables compression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundLoginCompressionPacket {
    /// The compression threshold in bytes. Negative values disable compression.
    pub threshold: i32,
}

impl ClientboundLoginCompressionPacket {
    /// Packet ID in the LOGIN state.
    pub const PACKET_ID: i32 = 0x03;

    /// Decodes from the raw packet body.
    ///
    /// # Errors
    ///
    /// Returns [`LoginCompressionError`] if the VarInt is malformed.
    pub fn decode(mut data: Bytes) -> Result<Self, LoginCompressionError> {
        let threshold = varint::read_varint_buf(&mut data)?;
        Ok(Self { threshold })
    }

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        varint::write_varint_buf(self.threshold, &mut buf);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundLoginCompressionPacket { threshold: 256 };
        let encoded = pkt.encode();
        let decoded = ClientboundLoginCompressionPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_negative_threshold() {
        let pkt = ClientboundLoginCompressionPacket { threshold: -1 };
        let encoded = pkt.encode();
        let decoded = ClientboundLoginCompressionPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.threshold, -1);
    }
}
