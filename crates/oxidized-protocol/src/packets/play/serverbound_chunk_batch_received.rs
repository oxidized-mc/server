//! Serverbound chunk batch received packet.
//!
//! Sent by the client in response to `ClientboundChunkBatchFinishedPacket`.
//! Contains the client's desired chunk receive rate for adaptive streaming.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ServerboundChunkBatchReceivedPacket`.

use bytes::{BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;

/// Client response to a chunk batch.
///
/// Wire format: `desired_chunks_per_tick: f32`.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerboundChunkBatchReceivedPacket {
    /// The client's preferred chunks-per-tick receive rate.
    pub desired_chunks_per_tick: f32,
}

impl Packet for ServerboundChunkBatchReceivedPacket {
    const PACKET_ID: i32 = 0x0B;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let desired_chunks_per_tick = types::read_f32(&mut data)?;
        Ok(Self {
            desired_chunks_per_tick,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(4);
        buf.put_f32(self.desired_chunks_per_tick);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ServerboundChunkBatchReceivedPacket {
            desired_chunks_per_tick: 7.5,
        };
        let encoded = pkt.encode();
        let decoded = ServerboundChunkBatchReceivedPacket::decode(encoded.freeze()).unwrap();
        assert!((decoded.desired_chunks_per_tick - 7.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_zero_rate() {
        let pkt = ServerboundChunkBatchReceivedPacket {
            desired_chunks_per_tick: 0.0,
        };
        let encoded = pkt.encode();
        let decoded = ServerboundChunkBatchReceivedPacket::decode(encoded.freeze()).unwrap();
        assert!((decoded.desired_chunks_per_tick).abs() < f32::EPSILON);
    }

    #[test]
    fn test_nan_rate_roundtrip() {
        let pkt = ServerboundChunkBatchReceivedPacket {
            desired_chunks_per_tick: f32::NAN,
        };
        let encoded = pkt.encode();
        let decoded = ServerboundChunkBatchReceivedPacket::decode(encoded.freeze()).unwrap();
        assert!(decoded.desired_chunks_per_tick.is_nan());
    }

    #[test]
    fn test_infinity_rate_roundtrip() {
        let pkt = ServerboundChunkBatchReceivedPacket {
            desired_chunks_per_tick: f32::INFINITY,
        };
        let encoded = pkt.encode();
        let decoded = ServerboundChunkBatchReceivedPacket::decode(encoded.freeze()).unwrap();
        assert!(decoded.desired_chunks_per_tick.is_infinite());
        assert!(decoded.desired_chunks_per_tick.is_sign_positive());
    }

    #[test]
    fn test_negative_infinity_rate_roundtrip() {
        let pkt = ServerboundChunkBatchReceivedPacket {
            desired_chunks_per_tick: f32::NEG_INFINITY,
        };
        let encoded = pkt.encode();
        let decoded = ServerboundChunkBatchReceivedPacket::decode(encoded.freeze()).unwrap();
        assert!(decoded.desired_chunks_per_tick.is_infinite());
        assert!(decoded.desired_chunks_per_tick.is_sign_negative());
    }

    #[test]
    fn test_negative_rate_roundtrip() {
        let pkt = ServerboundChunkBatchReceivedPacket {
            desired_chunks_per_tick: -5.0,
        };
        let encoded = pkt.encode();
        let decoded = ServerboundChunkBatchReceivedPacket::decode(encoded.freeze()).unwrap();
        assert!((decoded.desired_chunks_per_tick - (-5.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_decode_short_buffer() {
        let data = bytes::Bytes::from_static(&[0x00, 0x00]); // only 2 bytes, need 4
        let result = ServerboundChunkBatchReceivedPacket::decode(data);
        assert!(result.is_err());
    }
}
