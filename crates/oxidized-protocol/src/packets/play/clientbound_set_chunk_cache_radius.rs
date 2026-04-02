//! Clientbound set chunk cache radius packet.
//!
//! Tells the client the server's view distance (render distance in chunks).
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundSetChunkCacheRadiusPacket`.

use bytes::{Bytes, BytesMut};

use oxidized_codec::Packet;
use oxidized_codec::packet::PacketDecodeError;
use oxidized_codec::varint;

/// Sets the client's chunk cache radius.
///
/// Wire format: `radius: VarInt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundSetChunkCacheRadiusPacket {
    /// View distance in chunks.
    pub radius: i32,
}

impl Packet for ClientboundSetChunkCacheRadiusPacket {
    const PACKET_ID: i32 = 0x5F;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let radius = varint::read_varint_buf(&mut data)?;
        Ok(Self { radius })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(5);
        varint::write_varint_buf(self.radius, &mut buf);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundSetChunkCacheRadiusPacket { radius: 10 };
        let encoded = pkt.encode();
        let decoded = ClientboundSetChunkCacheRadiusPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.radius, 10);
    }

    #[test]
    fn test_min_radius() {
        let pkt = ClientboundSetChunkCacheRadiusPacket { radius: 2 };
        let encoded = pkt.encode();
        let decoded = ClientboundSetChunkCacheRadiusPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.radius, 2);
    }

    #[test]
    fn test_max_radius() {
        let pkt = ClientboundSetChunkCacheRadiusPacket { radius: 32 };
        let encoded = pkt.encode();
        let decoded = ClientboundSetChunkCacheRadiusPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.radius, 32);
    }
}
