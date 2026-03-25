//! Clientbound forget level chunk packet.
//!
//! Tells the client to unload a previously sent chunk from its cache.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundForgetLevelChunkPacket`.

use bytes::{BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;

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

impl Packet for ClientboundForgetLevelChunkPacket {
    const PACKET_ID: i32 = 0x25;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let packed = types::read_i64(&mut data)?;
        let chunk_x = packed as i32;
        let chunk_z = (packed >> 32) as i32;
        Ok(Self { chunk_x, chunk_z })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(8);
        let packed =
            (self.chunk_x as i64 & 0xFFFF_FFFF) | ((self.chunk_z as i64 & 0xFFFF_FFFF) << 32);
        buf.put_i64(packed);
        buf
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
}
