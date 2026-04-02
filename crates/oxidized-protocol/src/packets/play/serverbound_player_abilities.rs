//! Serverbound player abilities packet.
//!
//! Sent by the client when it starts or stops flying.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ServerboundPlayerAbilitiesPacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use oxidized_codec::Packet;
use oxidized_codec::packet::PacketDecodeError;

/// Serverbound packet sent when the client toggles flying.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundPlayerAbilitiesPacket {
    /// Ability flags. Only the flying bit (0x02) is used from client.
    pub flags: u8,
}

impl ServerboundPlayerAbilitiesPacket {
    /// Bit flag for "currently flying".
    pub const FLAG_FLYING: u8 = 0x02;

    /// Returns `true` if the flying flag is set.
    pub fn is_flying(&self) -> bool {
        self.flags & Self::FLAG_FLYING != 0
    }
}

impl Packet for ServerboundPlayerAbilitiesPacket {
    const PACKET_ID: i32 = 0x28;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        if !data.has_remaining() {
            return Err(PacketDecodeError::InvalidData(
                "not enough data for ServerboundPlayerAbilitiesPacket".into(),
            ));
        }
        let flags = data.get_u8();
        Ok(Self { flags })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(1);
        buf.put_u8(self.flags);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_not_flying() {
        let pkt = ServerboundPlayerAbilitiesPacket { flags: 0x00 };
        let encoded = pkt.encode();
        let decoded = ServerboundPlayerAbilitiesPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
        assert!(!decoded.is_flying());
    }

    #[test]
    fn test_roundtrip_flying() {
        let pkt = ServerboundPlayerAbilitiesPacket { flags: 0x02 };
        let encoded = pkt.encode();
        let decoded = ServerboundPlayerAbilitiesPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
        assert!(decoded.is_flying());
    }

    #[test]
    fn test_is_flying_flag() {
        assert!(!ServerboundPlayerAbilitiesPacket { flags: 0x00 }.is_flying());
        assert!(ServerboundPlayerAbilitiesPacket { flags: 0x02 }.is_flying());
        assert!(ServerboundPlayerAbilitiesPacket { flags: 0x06 }.is_flying());
        assert!(!ServerboundPlayerAbilitiesPacket { flags: 0x01 }.is_flying());
    }

    #[test]
    fn test_packet_id() {
        assert_eq!(ServerboundPlayerAbilitiesPacket::PACKET_ID, 0x28);
    }

    #[test]
    fn test_decode_empty_buffer() {
        let data = Bytes::new();
        assert!(ServerboundPlayerAbilitiesPacket::decode(data).is_err());
    }
}
