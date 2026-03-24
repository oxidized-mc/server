//! ServerboundUseItemOnPacket (0x42) — player right-clicks a block face.
//!
//! Sent when the player interacts with a block (place block, activate
//! furnace, open chest, etc.).

use bytes::{BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;
use crate::codec::varint;
use crate::types::{BlockPos, Direction};

/// Which hand the player used for the interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum InteractionHand {
    /// Main (right) hand.
    MainHand = 0,
    /// Off (left) hand.
    OffHand = 1,
}

impl InteractionHand {
    /// Reads from a VarInt on the wire.
    ///
    /// # Errors
    ///
    /// Returns [`PacketDecodeError::InvalidData`] if the value is not 0 or 1.
    pub fn read(buf: &mut Bytes) -> Result<Self, PacketDecodeError> {
        match varint::read_varint_buf(buf)? {
            0 => Ok(Self::MainHand),
            1 => Ok(Self::OffHand),
            other => Err(PacketDecodeError::InvalidData(format!(
                "unknown InteractionHand: {other}"
            ))),
        }
    }

    /// Writes as a VarInt.
    pub fn write(&self, buf: &mut BytesMut) {
        varint::write_varint_buf(*self as i32, buf);
    }
}

/// Result of a block hit — the exact position and face the player clicked.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockHitResult {
    /// The block that was clicked.
    pub pos: BlockPos,
    /// The face of the block that was clicked.
    pub direction: Direction,
    /// Cursor X within the block face (0.0–1.0).
    pub cursor_x: f32,
    /// Cursor Y within the block face (0.0–1.0).
    pub cursor_y: f32,
    /// Cursor Z within the block face (0.0–1.0).
    pub cursor_z: f32,
    /// Whether the player's head is inside the target block.
    pub is_inside: bool,
}

/// Serverbound packet for right-clicking a block face.
///
/// # Wire Format
///
/// | Field | Type | Notes |
/// |-------|------|-------|
/// | hand | VarInt | [`InteractionHand`] (0=main, 1=off) |
/// | pos | Position | Packed i64 block position |
/// | direction | VarInt | [`Direction`] face clicked |
/// | cursor_x | Float | X within face (0.0–1.0) |
/// | cursor_y | Float | Y within face (0.0–1.0) |
/// | cursor_z | Float | Z within face (0.0–1.0) |
/// | inside | Boolean | Head inside target block |
/// | sequence | VarInt | Sequence for acknowledgement |
#[derive(Debug, Clone, PartialEq)]
pub struct ServerboundUseItemOnPacket {
    /// Which hand the player used.
    pub hand: InteractionHand,
    /// Block hit result (position, face, cursor offset, inside flag).
    pub hit_result: BlockHitResult,
    /// Sequence number used for block-change acknowledgement.
    pub sequence: i32,
}

impl Packet for ServerboundUseItemOnPacket {
    const PACKET_ID: i32 = 0x42;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let hand = InteractionHand::read(&mut data)?;
        let pos = BlockPos::read(&mut data)?;
        let direction = Direction::read(&mut data)?;
        let cursor_x = types::read_f32(&mut data)?;
        let cursor_y = types::read_f32(&mut data)?;
        let cursor_z = types::read_f32(&mut data)?;
        let is_inside = types::read_bool(&mut data)?;
        let sequence = varint::read_varint_buf(&mut data)?;
        Ok(Self {
            hand,
            hit_result: BlockHitResult {
                pos,
                direction,
                cursor_x,
                cursor_y,
                cursor_z,
                is_inside,
            },
            sequence,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(30);
        self.hand.write(&mut buf);
        self.hit_result.pos.write(&mut buf);
        self.hit_result.direction.write(&mut buf);
        buf.put_f32(self.hit_result.cursor_x);
        buf.put_f32(self.hit_result.cursor_y);
        buf.put_f32(self.hit_result.cursor_z);
        types::write_bool(&mut buf, self.hit_result.is_inside);
        varint::write_varint_buf(self.sequence, &mut buf);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_hand_from_id_valid() {
        let mut buf = BytesMut::new();
        varint::write_varint_buf(0, &mut buf);
        assert_eq!(
            InteractionHand::read(&mut buf.freeze()).unwrap(),
            InteractionHand::MainHand
        );

        let mut buf = BytesMut::new();
        varint::write_varint_buf(1, &mut buf);
        assert_eq!(
            InteractionHand::read(&mut buf.freeze()).unwrap(),
            InteractionHand::OffHand
        );
    }

    #[test]
    fn test_hand_from_id_invalid() {
        let mut buf = BytesMut::new();
        varint::write_varint_buf(2, &mut buf);
        assert!(InteractionHand::read(&mut buf.freeze()).is_err());
    }

    #[test]
    fn test_roundtrip() {
        let original = ServerboundUseItemOnPacket {
            hand: InteractionHand::MainHand,
            hit_result: BlockHitResult {
                pos: BlockPos::new(10, 64, -30),
                direction: Direction::Up,
                cursor_x: 0.5,
                cursor_y: 1.0,
                cursor_z: 0.5,
                is_inside: false,
            },
            sequence: 7,
        };
        let encoded = original.encode();
        let decoded = ServerboundUseItemOnPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.hand, InteractionHand::MainHand);
        assert_eq!(decoded.hit_result.pos, original.hit_result.pos);
        assert_eq!(decoded.hit_result.direction, Direction::Up);
        assert!((decoded.hit_result.cursor_x - 0.5).abs() < 1e-5);
        assert!((decoded.hit_result.cursor_y - 1.0).abs() < 1e-5);
        assert!((decoded.hit_result.cursor_z - 0.5).abs() < 1e-5);
        assert!(!decoded.hit_result.is_inside);
        assert_eq!(decoded.sequence, 7);
    }

    #[test]
    fn test_offhand_inside() {
        let original = ServerboundUseItemOnPacket {
            hand: InteractionHand::OffHand,
            hit_result: BlockHitResult {
                pos: BlockPos::new(0, 0, 0),
                direction: Direction::North,
                cursor_x: 0.0,
                cursor_y: 0.0,
                cursor_z: 0.0,
                is_inside: true,
            },
            sequence: 0,
        };
        let encoded = original.encode();
        let decoded = ServerboundUseItemOnPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.hand, InteractionHand::OffHand);
        assert!(decoded.hit_result.is_inside);
    }
}
