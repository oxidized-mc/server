//! ServerboundUseItemPacket (0x43) — player right-clicks without targeting a block.
//!
//! Sent when the player uses an item in the air (eat food, throw ender
//! pearl, shoot bow, etc.).

use bytes::{BufMut, Bytes, BytesMut};

use oxidized_codec::Packet;
use oxidized_codec::packet::PacketDecodeError;
use oxidized_codec::types;
use oxidized_codec::varint;

use super::serverbound_use_item_on::InteractionHand;

/// Serverbound packet for using an item (not targeting a block).
///
/// # Wire Format
///
/// | Field | Type | Notes |
/// |-------|------|-------|
/// | hand | VarInt | [`InteractionHand`] (0=main, 1=off) |
/// | sequence | VarInt | Sequence for acknowledgement |
/// | y_rot | Float | Player yaw at time of use |
/// | x_rot | Float | Player pitch at time of use |
#[derive(Debug, Clone, PartialEq)]
pub struct ServerboundUseItemPacket {
    /// Which hand the player used.
    pub hand: InteractionHand,
    /// Sequence number used for block-change acknowledgement.
    pub sequence: i32,
    /// Player yaw at time of use.
    pub y_rot: f32,
    /// Player pitch at time of use.
    pub x_rot: f32,
}

impl Packet for ServerboundUseItemPacket {
    const PACKET_ID: i32 = 0x43;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let hand = InteractionHand::read(&mut data)?;
        let sequence = varint::read_varint_buf(&mut data)?;
        let y_rot = types::read_f32(&mut data)?;
        let x_rot = types::read_f32(&mut data)?;
        Ok(Self {
            hand,
            sequence,
            y_rot,
            x_rot,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(14);
        self.hand.write(&mut buf);
        varint::write_varint_buf(self.sequence, &mut buf);
        buf.put_f32(self.y_rot);
        buf.put_f32(self.x_rot);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_main_hand() {
        let original = ServerboundUseItemPacket {
            hand: InteractionHand::MainHand,
            sequence: 5,
            y_rot: 90.0,
            x_rot: -45.0,
        };
        let encoded = original.encode();
        let decoded = ServerboundUseItemPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.hand, InteractionHand::MainHand);
        assert_eq!(decoded.sequence, 5);
        assert!((decoded.y_rot - 90.0).abs() < 1e-5);
        assert!((decoded.x_rot - (-45.0)).abs() < 1e-5);
    }

    #[test]
    fn test_roundtrip_off_hand() {
        let original = ServerboundUseItemPacket {
            hand: InteractionHand::OffHand,
            sequence: 0,
            y_rot: 0.0,
            x_rot: 0.0,
        };
        let encoded = original.encode();
        let decoded = ServerboundUseItemPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.hand, InteractionHand::OffHand);
        assert_eq!(decoded.sequence, 0);
    }
}
