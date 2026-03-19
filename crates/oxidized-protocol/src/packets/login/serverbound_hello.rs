//! Serverbound login start — the client sends its username and profile UUID.
//!
//! Corresponds to `net.minecraft.network.protocol.login.ServerboundHelloPacket`.

use bytes::{Bytes, BytesMut};
use thiserror::Error;

use crate::codec::packet::PacketDecodeError;
use crate::codec::types::{self, TypeError};
use crate::codec::Packet;

/// Errors from decoding a [`ServerboundHelloPacket`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum HelloError {
    /// Type decode failure.
    #[error("type error: {0}")]
    Type(#[from] TypeError),
}

/// Serverbound packet `0x00` in the LOGIN state — login start.
///
/// Sent by the client immediately after the handshake transitions to the LOGIN
/// state. Contains the player's chosen name and their Mojang profile UUID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundHelloPacket {
    /// The player's username (max 16 characters).
    pub name: String,
    /// The player's Mojang profile UUID.
    pub profile_id: uuid::Uuid,
}

impl ServerboundHelloPacket {
    /// Packet ID in the LOGIN state.
    pub const PACKET_ID: i32 = 0x00;

    /// Decodes from the raw packet body.
    ///
    /// # Errors
    ///
    /// Returns [`HelloError`] if the buffer is truncated or the string is
    /// invalid.
    pub fn decode(mut data: Bytes) -> Result<Self, HelloError> {
        let name = types::read_string(&mut data, 16)?;
        let profile_id = types::read_uuid(&mut data)?;
        Ok(Self { name, profile_id })
    }

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        types::write_string(&mut buf, &self.name);
        types::write_uuid(&mut buf, &self.profile_id);
        buf
    }
}

impl Packet for ServerboundHelloPacket {
    const PACKET_ID: i32 = 0x00;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let name = types::read_string(&mut data, 16)?;
        let profile_id = types::read_uuid(&mut data)?;
        Ok(Self { name, profile_id })
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
    fn test_roundtrip() {
        let pkt = ServerboundHelloPacket {
            name: "Notch".to_string(),
            profile_id: uuid::Uuid::from_u128(0x1234_5678_9abc_def0_1234_5678_9abc_def0),
        };
        let encoded = pkt.encode();
        let decoded = ServerboundHelloPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_max_length_name() {
        let pkt = ServerboundHelloPacket {
            name: "A".repeat(16),
            profile_id: uuid::Uuid::nil(),
        };
        let encoded = pkt.encode();
        let decoded = ServerboundHelloPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = ServerboundHelloPacket {
            name: "Steve".to_string(),
            profile_id: uuid::Uuid::from_u128(0xCAFE),
        };
        let encoded = Packet::encode(&pkt);
        let decoded = <ServerboundHelloPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(<ServerboundHelloPacket as Packet>::PACKET_ID, 0x00);
    }
}
