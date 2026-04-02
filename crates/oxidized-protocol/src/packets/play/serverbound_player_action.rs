//! ServerboundPlayerActionPacket (0x29) — block digging and item-related actions.
//!
//! Sent when the player starts/stops mining a block, drops items, or
//! swaps items with the offhand.

use bytes::{Bytes, BytesMut};

use oxidized_codec::Packet;
use oxidized_codec::packet::PacketDecodeError;
use oxidized_codec::varint;
use oxidized_mc_types::{BlockPos, Direction};

/// Player block/item action type.
///
/// Matches the `Action` enum in `ServerboundPlayerActionPacket.java`
/// for protocol version 26.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum PlayerAction {
    /// Player started mining a block (left-click hold).
    StartDestroyBlock = 0,
    /// Player cancelled mining.
    AbortDestroyBlock = 1,
    /// Player finished mining (block should break).
    StopDestroyBlock = 2,
    /// Player dropped the entire held stack (Ctrl+Q).
    DropAllItems = 3,
    /// Player dropped a single item (Q).
    DropItem = 4,
    /// Player released the use-item button (e.g. finished eating, stopped drawing bow).
    ReleaseUseItem = 5,
    /// Player pressed the swap-offhand key (F).
    SwapItemWithOffhand = 6,
}

impl PlayerAction {
    /// Converts a VarInt ordinal to an action variant.
    ///
    /// # Errors
    ///
    /// Returns [`PacketDecodeError::InvalidData`] if `id` is not a known action.
    pub fn from_id(id: i32) -> Result<Self, PacketDecodeError> {
        match id {
            0 => Ok(Self::StartDestroyBlock),
            1 => Ok(Self::AbortDestroyBlock),
            2 => Ok(Self::StopDestroyBlock),
            3 => Ok(Self::DropAllItems),
            4 => Ok(Self::DropItem),
            5 => Ok(Self::ReleaseUseItem),
            6 => Ok(Self::SwapItemWithOffhand),
            _ => Err(PacketDecodeError::InvalidData(format!(
                "unknown PlayerAction: {id}"
            ))),
        }
    }
}

/// Serverbound packet for block digging and item-related actions.
///
/// # Wire Format
///
/// | Field | Type | Notes |
/// |-------|------|-------|
/// | action | VarInt | [`PlayerAction`] ordinal |
/// | pos | Position | Block position (packed i64) |
/// | direction | VarInt | [`Direction`] face |
/// | sequence | VarInt | Sequence number for acknowledgement |
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundPlayerActionPacket {
    /// The action being performed.
    pub action: PlayerAction,
    /// Block position (relevant for destroy actions).
    pub pos: BlockPos,
    /// Block face (relevant for destroy actions).
    pub direction: Direction,
    /// Sequence number used for block-change acknowledgement.
    pub sequence: i32,
}

impl Packet for ServerboundPlayerActionPacket {
    const PACKET_ID: i32 = 0x29;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let action_id = varint::read_varint_buf(&mut data)?;
        let action = PlayerAction::from_id(action_id)?;
        let pos = BlockPos::read(&mut data)?;
        let direction = Direction::read(&mut data)?;
        let sequence = varint::read_varint_buf(&mut data)?;
        Ok(Self {
            action,
            pos,
            direction,
            sequence,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(20);
        varint::write_varint_buf(self.action as i32, &mut buf);
        self.pos.write(&mut buf);
        self.direction.write(&mut buf);
        varint::write_varint_buf(self.sequence, &mut buf);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_action_from_id_valid() {
        assert_eq!(
            PlayerAction::from_id(0).unwrap(),
            PlayerAction::StartDestroyBlock
        );
        assert_eq!(
            PlayerAction::from_id(6).unwrap(),
            PlayerAction::SwapItemWithOffhand
        );
    }

    #[test]
    fn test_action_from_id_invalid() {
        assert!(PlayerAction::from_id(7).is_err());
        assert!(PlayerAction::from_id(-1).is_err());
    }

    #[test]
    fn test_roundtrip_start_destroy() {
        let original = ServerboundPlayerActionPacket {
            action: PlayerAction::StartDestroyBlock,
            pos: BlockPos::new(100, 64, -200),
            direction: Direction::Up,
            sequence: 42,
        };
        let encoded = original.encode();
        let decoded = ServerboundPlayerActionPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.action, original.action);
        assert_eq!(decoded.pos, original.pos);
        assert_eq!(decoded.direction, original.direction);
        assert_eq!(decoded.sequence, original.sequence);
    }

    #[test]
    fn test_roundtrip_swap_offhand() {
        let original = ServerboundPlayerActionPacket {
            action: PlayerAction::SwapItemWithOffhand,
            pos: BlockPos::new(0, 0, 0),
            direction: Direction::Down,
            sequence: 0,
        };
        let encoded = original.encode();
        let decoded = ServerboundPlayerActionPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.action, PlayerAction::SwapItemWithOffhand);
        assert_eq!(decoded.sequence, 0);
    }

    #[test]
    fn test_invalid_action_id() {
        let mut buf = BytesMut::new();
        varint::write_varint_buf(99, &mut buf);
        BlockPos::new(0, 0, 0).write(&mut buf);
        Direction::Down.write(&mut buf);
        varint::write_varint_buf(0, &mut buf);
        assert!(ServerboundPlayerActionPacket::decode(buf.freeze()).is_err());
    }
}
