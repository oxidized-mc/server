//! Clientbound disconnect — the server kicks the client during login with a
//! JSON text component reason.
//!
//! Corresponds to `net.minecraft.network.protocol.login.ClientboundLoginDisconnectPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;

/// Maximum character length for the reason JSON.
const MAX_REASON_CHARS: usize = 262_144;

/// Clientbound packet `0x00` in the LOGIN state — disconnect.
///
/// Sent by the server to terminate the login sequence. The `reason` field is a
/// JSON text component displayed to the player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundDisconnectPacket {
    /// The disconnect reason as a JSON text component (max 262144 chars).
    pub reason: String,
}

impl Packet for ClientboundDisconnectPacket {
    const PACKET_ID: i32 = 0x00;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let reason = types::read_string(&mut data, MAX_REASON_CHARS)?;
        Ok(Self { reason })
    }

    fn encode(&self) -> BytesMut {
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

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ClientboundDisconnectPacket, 0x00);
    }
}
