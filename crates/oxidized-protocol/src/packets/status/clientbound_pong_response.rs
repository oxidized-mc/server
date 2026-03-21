//! Clientbound pong response — echoes the client's ping timestamp.
//!
//! Corresponds to `net.minecraft.network.protocol.status.ClientboundPongResponsePacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
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

impl Packet for ClientboundPongResponsePacket {
    const PACKET_ID: i32 = 0x01;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let time = types::read_i64(&mut data)?;
        Ok(Self { time })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        types::write_i64(&mut buf, self.time);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

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

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = ClientboundPongResponsePacket {
            time: 1_719_000_000_000,
        };
        let encoded = Packet::encode(&pkt);
        let decoded = <ClientboundPongResponsePacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_packet_trait_decode_empty_fails() {
        let result = <ClientboundPongResponsePacket as Packet>::decode(Bytes::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(<ClientboundPongResponsePacket as Packet>::PACKET_ID, 0x01);
    }
}
