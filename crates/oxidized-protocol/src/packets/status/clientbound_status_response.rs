//! Clientbound status response — sends the server status JSON to the client.
//!
//! Corresponds to `net.minecraft.network.protocol.status.ClientboundStatusResponsePacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;

/// Clientbound packet `0x00` in the STATUS state — the server status as JSON.
///
/// The client uses this to display the server in the multiplayer list
/// (MOTD, player count, version, favicon).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundStatusResponsePacket {
    /// The full server status as a JSON string.
    pub status_json: String,
}

impl Packet for ClientboundStatusResponsePacket {
    const PACKET_ID: i32 = 0x00;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let status_json = types::read_string(&mut data, 32767)?;
        Ok(Self { status_json })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        types::write_string(&mut buf, &self.status_json);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_status_response() {
        let pkt = ClientboundStatusResponsePacket {
            status_json: r#"{"version":{"name":"26.1","protocol":775}}"#.to_string(),
        };
        let encoded = pkt.encode();
        let mut data = Bytes::from(encoded.to_vec());
        let decoded_str = types::read_string(&mut data, 32767).unwrap();
        assert_eq!(decoded_str, pkt.status_json);
    }

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = ClientboundStatusResponsePacket {
            status_json: r#"{"description":"A Minecraft Server"}"#.to_string(),
        };
        let encoded = Packet::encode(&pkt);
        let decoded =
            <ClientboundStatusResponsePacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_packet_trait_decode_empty_fails() {
        let result = <ClientboundStatusResponsePacket as Packet>::decode(Bytes::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(<ClientboundStatusResponsePacket as Packet>::PACKET_ID, 0x00);
    }
}
