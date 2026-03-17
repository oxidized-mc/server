//! Clientbound pong response — echoes the client's ping timestamp.
//!
//! Corresponds to `net.minecraft.network.protocol.status.ClientboundPongResponsePacket`.

use bytes::BytesMut;

use crate::codec::types;

/// Clientbound packet `0x01` in the STATUS state — pong response.
///
/// Echoes the `time` value from the client's
/// [`ServerboundPingRequestPacket`](super::ServerboundPingRequestPacket).
/// After sending this, the server closes the connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundPongResponsePacket {
    /// The timestamp echoed from the client's ping request.
    pub time: i64,
}

impl ClientboundPongResponsePacket {
    /// Packet ID in the STATUS state.
    pub const PACKET_ID: i32 = 0x01;

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        types::write_i64(&mut buf, self.time);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[test]
    fn test_encode_pong() {
        let pkt = ClientboundPongResponsePacket {
            time: 1_719_000_000_000,
        };
        let encoded = pkt.encode();
        let mut data = Bytes::from(encoded.to_vec());
        let time = types::read_i64(&mut data).unwrap();
        assert_eq!(time, 1_719_000_000_000);
    }
}
