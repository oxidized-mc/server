//! Clientbound status response — sends the server status JSON to the client.
//!
//! Corresponds to `net.minecraft.network.protocol.status.ClientboundStatusResponsePacket`.

use bytes::BytesMut;

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

impl ClientboundStatusResponsePacket {
    /// Packet ID in the STATUS state.
    pub const PACKET_ID: i32 = 0x00;

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        types::write_string(&mut buf, &self.status_json);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[test]
    fn test_encode_status_response() {
        let pkt = ClientboundStatusResponsePacket {
            status_json: r#"{"version":{"name":"26.1-pre-3","protocol":1073742124}}"#.to_string(),
        };
        let encoded = pkt.encode();
        // Verify we can read the string back
        let mut data = Bytes::from(encoded.to_vec());
        let decoded_str = types::read_string(&mut data, 32767).unwrap();
        assert_eq!(decoded_str, pkt.status_json);
    }
}
