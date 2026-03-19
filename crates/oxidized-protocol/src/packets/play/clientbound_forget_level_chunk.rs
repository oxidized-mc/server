//! Clientbound forget level chunk packet.
//!
//! Tells the client to unload a previously sent chunk from its cache.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundForgetLevelChunkPacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;

/// Unloads a chunk from the client's cache.
///
/// Wire format: `pos: i64` (packed as `(x & 0xFFFF_FFFF) | ((z & 0xFFFF_FFFF) << 32)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundForgetLevelChunkPacket {
    /// Chunk X coordinate.
    pub chunk_x: i32,
    /// Chunk Z coordinate.
    pub chunk_z: i32,
}

impl ClientboundForgetLevelChunkPacket {
    /// Packet ID in the PLAY state.
    pub const PACKET_ID: i32 = 0x25; // 37

    /// Decodes from the raw packet body.
    ///
    /// # Errors
    ///
    /// Returns an error if the data is too short.
    pub fn decode(mut data: Bytes) -> Result<Self, std::io::Error> {
        if data.remaining() < 8 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "not enough data for i64",
            ));
        }
        let packed = data.get_i64();
        let chunk_x = packed as i32;
        let chunk_z = (packed >> 32) as i32;
        Ok(Self { chunk_x, chunk_z })
    }

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(8);
        let packed =
            (self.chunk_x as i64 & 0xFFFF_FFFF) | ((self.chunk_z as i64 & 0xFFFF_FFFF) << 32);
        buf.put_i64(packed);
        buf
    }
}

impl Packet for ClientboundForgetLevelChunkPacket {
    const PACKET_ID: i32 = 0x25;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        if data.remaining() < 8 {
            return Err(PacketDecodeError::Io(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "not enough data for i64",
            )));
        }
        let packed = data.get_i64();
        let chunk_x = packed as i32;
        let chunk_z = (packed >> 32) as i32;
        Ok(Self { chunk_x, chunk_z })
    }

    fn encode(&self) -> BytesMut {
        self.encode()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundForgetLevelChunkPacket {
            chunk_x: 5,
            chunk_z: -3,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundForgetLevelChunkPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.chunk_x, 5);
        assert_eq!(decoded.chunk_z, -3);
    }

    #[test]
    fn test_origin() {
        let pkt = ClientboundForgetLevelChunkPacket {
            chunk_x: 0,
            chunk_z: 0,
        };
        let encoded = pkt.encode();
        assert_eq!(encoded.len(), 8);
        let decoded = ClientboundForgetLevelChunkPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_large_negative() {
        let pkt = ClientboundForgetLevelChunkPacket {
            chunk_x: -1000000,
            chunk_z: -2000000,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundForgetLevelChunkPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.chunk_x, -1000000);
        assert_eq!(decoded.chunk_z, -2000000);
    }

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = ClientboundForgetLevelChunkPacket {
            chunk_x: 5,
            chunk_z: -3,
        };
        let encoded = Packet::encode(&pkt);
        let decoded =
            <ClientboundForgetLevelChunkPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(
            <ClientboundForgetLevelChunkPacket as Packet>::PACKET_ID,
            0x25
        );
    }
}
