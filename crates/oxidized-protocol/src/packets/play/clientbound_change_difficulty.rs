//! Clientbound change difficulty packet.
//!
//! Informs the client of the current world difficulty and lock status.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundChangeDifficultyPacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;

/// Tells the client the current difficulty level and whether it is locked.
///
/// Wire format: `difficulty: u8 | is_locked: bool`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundChangeDifficultyPacket {
    /// Difficulty level (0 = Peaceful, 1 = Easy, 2 = Normal, 3 = Hard).
    pub difficulty: u8,
    /// Whether the difficulty is locked.
    pub is_locked: bool,
}

impl Packet for ClientboundChangeDifficultyPacket {
    const PACKET_ID: i32 = 0x0A;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        if data.remaining() < 1 {
            return Err(PacketDecodeError::Io(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "not enough data for ChangeDifficultyPacket",
            )));
        }
        let difficulty = data.get_u8();
        let is_locked = types::read_bool(&mut data)?;
        Ok(Self { difficulty, is_locked })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(2);
        buf.put_u8(self.difficulty);
        types::write_bool(&mut buf, self.is_locked);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_normal() {
        let pkt = ClientboundChangeDifficultyPacket {
            difficulty: 2,
            is_locked: false,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundChangeDifficultyPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.difficulty, 2);
        assert!(!decoded.is_locked);
    }

    #[test]
    fn test_roundtrip_locked() {
        let pkt = ClientboundChangeDifficultyPacket {
            difficulty: 3,
            is_locked: true,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundChangeDifficultyPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.difficulty, 3);
        assert!(decoded.is_locked);
    }

    #[test]
    fn test_roundtrip_peaceful() {
        let pkt = ClientboundChangeDifficultyPacket {
            difficulty: 0,
            is_locked: false,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundChangeDifficultyPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.difficulty, 0);
    }
}
