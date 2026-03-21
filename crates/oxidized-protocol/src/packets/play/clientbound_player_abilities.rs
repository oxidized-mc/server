//! Clientbound player abilities packet.
//!
//! Sends the player's current ability flags (invulnerable, flying,
//! can fly, instabuild) and movement speeds.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundPlayerAbilitiesPacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;

/// Clientbound packet that sets the player's abilities.
///
/// Wire format: `flags: u8 | fly_speed: f32 | walk_speed: f32`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundPlayerAbilitiesPacket {
    /// Packed ability flags: invulnerable(0x01), flying(0x02), can_fly(0x04), instabuild(0x08).
    pub flags: u8,
    /// Flying speed in blocks per tick.
    pub fly_speed: f32,
    /// Walking speed in blocks per tick.
    pub walk_speed: f32,
}

impl Packet for ClientboundPlayerAbilitiesPacket {
    const PACKET_ID: i32 = 0x40;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        if data.remaining() < 9 {
            return Err(PacketDecodeError::Io(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "not enough data for PlayerAbilitiesPacket",
            )));
        }
        let flags = data.get_u8();
        let fly_speed = data.get_f32();
        let walk_speed = data.get_f32();
        Ok(Self {
            flags,
            fly_speed,
            walk_speed,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(9);
        buf.put_u8(self.flags);
        buf.put_f32(self.fly_speed);
        buf.put_f32(self.walk_speed);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundPlayerAbilitiesPacket {
            flags: 0x01 | 0x04 | 0x08,
            fly_speed: 0.05,
            walk_speed: 0.1,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundPlayerAbilitiesPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.flags, pkt.flags);
        assert!((decoded.fly_speed - 0.05).abs() < f32::EPSILON);
        assert!((decoded.walk_speed - 0.1).abs() < f32::EPSILON);
    }

    #[test]
    fn test_flags_encoding() {
        let pkt = ClientboundPlayerAbilitiesPacket {
            flags: 0x02 | 0x04,
            fly_speed: 0.05,
            walk_speed: 0.1,
        };
        let encoded = pkt.encode();
        assert_eq!(encoded[0], 0x06);
    }
}
