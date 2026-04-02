//! Serverbound player command packet.
//!
//! Sent when the player starts/stops sprinting, stops sleeping, starts
//! elytra flight, etc. In 26.1, sneak is handled by
//! [`ServerboundPlayerInputPacket`](super::serverbound_player_input::ServerboundPlayerInputPacket)
//! instead.

use bytes::{Bytes, BytesMut};

use oxidized_codec::Packet;
use oxidized_codec::packet::PacketDecodeError;
use oxidized_codec::varint;

/// Actions the client can trigger via player command.
///
/// Matches the `Action` enum in `ServerboundPlayerCommandPacket.java`
/// for protocol version 26.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum PlayerCommandAction {
    /// Player stopped sleeping (clicked "Leave Bed").
    StopSleeping = 0,
    /// Player started sprinting (double-tap W or Ctrl+W).
    StartSprinting = 1,
    /// Player stopped sprinting.
    StopSprinting = 2,
    /// Player started a riding jump (horse).
    StartRidingJump = 3,
    /// Player stopped a riding jump.
    StopRidingJump = 4,
    /// Player opened vehicle inventory (horse, llama, etc.).
    OpenInventory = 5,
    /// Player activated elytra flight (jump while falling).
    StartFallFlying = 6,
}

impl PlayerCommandAction {
    /// Converts a VarInt ordinal to an action variant.
    ///
    /// # Errors
    ///
    /// Returns [`PacketDecodeError::InvalidData`] if `id` is not a known action.
    pub fn from_id(id: i32) -> Result<Self, PacketDecodeError> {
        match id {
            0 => Ok(Self::StopSleeping),
            1 => Ok(Self::StartSprinting),
            2 => Ok(Self::StopSprinting),
            3 => Ok(Self::StartRidingJump),
            4 => Ok(Self::StopRidingJump),
            5 => Ok(Self::OpenInventory),
            6 => Ok(Self::StartFallFlying),
            _ => Err(PacketDecodeError::InvalidData(format!(
                "unknown PlayerCommandAction: {id}"
            ))),
        }
    }
}

/// Serverbound packet for player commands (sprint, sleep, elytra, etc.).
///
/// # Wire Format
///
/// | Field | Type | Notes |
/// |-------|------|-------|
/// | entity_id | VarInt | Player's entity ID |
/// | action | VarInt | [`PlayerCommandAction`] ordinal |
/// | data | VarInt | Action-specific (jump boost for riding) |
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundPlayerCommandPacket {
    /// The player's entity ID.
    pub entity_id: i32,
    /// The action being performed.
    pub action: PlayerCommandAction,
    /// Action-specific data (jump boost power for `StartRidingJump`).
    pub data: i32,
}

impl Packet for ServerboundPlayerCommandPacket {
    const PACKET_ID: i32 = 0x2A;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let entity_id = varint::read_varint_buf(&mut data)?;
        let action_id = varint::read_varint_buf(&mut data)?;
        let action = PlayerCommandAction::from_id(action_id)?;
        let extra = varint::read_varint_buf(&mut data)?;
        Ok(Self {
            entity_id,
            action,
            data: extra,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(15);
        varint::write_varint_buf(self.entity_id, &mut buf);
        varint::write_varint_buf(self.action as i32, &mut buf);
        varint::write_varint_buf(self.data, &mut buf);
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
            PlayerCommandAction::from_id(0).unwrap(),
            PlayerCommandAction::StopSleeping
        );
        assert_eq!(
            PlayerCommandAction::from_id(1).unwrap(),
            PlayerCommandAction::StartSprinting
        );
        assert_eq!(
            PlayerCommandAction::from_id(2).unwrap(),
            PlayerCommandAction::StopSprinting
        );
        assert_eq!(
            PlayerCommandAction::from_id(6).unwrap(),
            PlayerCommandAction::StartFallFlying
        );
    }

    #[test]
    fn test_action_from_id_invalid() {
        assert!(PlayerCommandAction::from_id(7).is_err());
        assert!(PlayerCommandAction::from_id(-1).is_err());
    }

    #[test]
    fn test_roundtrip() {
        let original = ServerboundPlayerCommandPacket {
            entity_id: 42,
            action: PlayerCommandAction::StartSprinting,
            data: 0,
        };
        let encoded = original.encode();
        let decoded = ServerboundPlayerCommandPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.entity_id, 42);
        assert_eq!(decoded.action, PlayerCommandAction::StartSprinting);
        assert_eq!(decoded.data, 0);
    }

    #[test]
    fn test_riding_jump_with_data() {
        let original = ServerboundPlayerCommandPacket {
            entity_id: 1,
            action: PlayerCommandAction::StartRidingJump,
            data: 80,
        };
        let encoded = original.encode();
        let decoded = ServerboundPlayerCommandPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.action, PlayerCommandAction::StartRidingJump);
        assert_eq!(decoded.data, 80);
    }
}
