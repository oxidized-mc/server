//! Clientbound set chunk cache center packet.
//!
//! Tells the client which chunk is the center of its loaded chunk grid.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundSetChunkCacheCenterPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::packet::PacketDecodeError;
use crate::codec::varint;
use crate::codec::Packet;

/// Sets the center chunk for the client's chunk cache.
///
/// Wire format: `chunk_x: VarInt | chunk_z: VarInt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundSetChunkCacheCenterPacket {
    /// Chunk X coordinate.
    pub chunk_x: i32,
    /// Chunk Z coordinate.
    pub chunk_z: i32,
}

impl ClientboundSetChunkCacheCenterPacket {
    /// Packet ID in the PLAY state.
    pub const PACKET_ID: i32 = 0x5E; // 94

    /// Decodes from the raw packet body.
    pub fn decode(mut data: Bytes) -> Result<Self, varint::VarIntError> {
        let chunk_x = varint::read_varint_buf(&mut data)?;
        let chunk_z = varint::read_varint_buf(&mut data)?;
        Ok(Self { chunk_x, chunk_z })
    }

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(10);
        varint::write_varint_buf(self.chunk_x, &mut buf);
        varint::write_varint_buf(self.chunk_z, &mut buf);
        buf
    }
}

impl Packet for ClientboundSetChunkCacheCenterPacket {
    const PACKET_ID: i32 = 0x5E;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let chunk_x = varint::read_varint_buf(&mut data)?;
        let chunk_z = varint::read_varint_buf(&mut data)?;
        Ok(Self { chunk_x, chunk_z })
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
        let pkt = ClientboundSetChunkCacheCenterPacket {
            chunk_x: 5,
            chunk_z: -3,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundSetChunkCacheCenterPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.chunk_x, 5);
        assert_eq!(decoded.chunk_z, -3);
    }

    #[test]
    fn test_zero_center() {
        let pkt = ClientboundSetChunkCacheCenterPacket {
            chunk_x: 0,
            chunk_z: 0,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundSetChunkCacheCenterPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = ClientboundSetChunkCacheCenterPacket {
            chunk_x: 5,
            chunk_z: -3,
        };
        let encoded = Packet::encode(&pkt);
        let decoded =
            <ClientboundSetChunkCacheCenterPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(
            <ClientboundSetChunkCacheCenterPacket as Packet>::PACKET_ID,
            0x5E
        );
    }
}
