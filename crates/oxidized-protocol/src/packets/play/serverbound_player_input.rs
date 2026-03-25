//! Serverbound player input packet.
//!
//! Sent every tick with the player's current movement input state.
//! In 26.1-pre-3, this is how sneak (is_shifting) is communicated — not via
//! [`ServerboundPlayerCommandPacket`](super::serverbound_player_command::ServerboundPlayerCommandPacket).

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;

/// Player movement input state, sent every tick.
///
/// Each field represents a currently-held input key. All 7 fields are
/// packed into a single byte on the wire.
///
/// # Wire Format
///
/// Single byte with bit flags:
/// - Bit 0 (0x01): is_forward (W)
/// - Bit 1 (0x02): is_backward (S)
/// - Bit 2 (0x04): is_left (A)
/// - Bit 3 (0x08): is_right (D)
/// - Bit 4 (0x10): is_jumping (Space)
/// - Bit 5 (0x20): is_shifting/sneak (Shift)
/// - Bit 6 (0x40): sprinting (Ctrl)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayerInput {
    /// W key held.
    pub is_forward: bool,
    /// S key held.
    pub is_backward: bool,
    /// A key held.
    pub is_left: bool,
    /// D key held.
    pub is_right: bool,
    /// Space bar held.
    pub is_jumping: bool,
    /// Shift key held (sneak).
    pub is_shifting: bool,
    /// Sprint key held (Ctrl).
    pub is_sprinting: bool,
}

impl PlayerInput {
    const FLAG_FORWARD: u8 = 0x01;
    const FLAG_BACKWARD: u8 = 0x02;
    const FLAG_LEFT: u8 = 0x04;
    const FLAG_RIGHT: u8 = 0x08;
    const FLAG_JUMP: u8 = 0x10;
    const FLAG_SHIFT: u8 = 0x20;
    const FLAG_SPRINT: u8 = 0x40;

    /// Decodes from a single byte of packed flags.
    pub fn from_byte(flags: u8) -> Self {
        Self {
            is_forward: flags & Self::FLAG_FORWARD != 0,
            is_backward: flags & Self::FLAG_BACKWARD != 0,
            is_left: flags & Self::FLAG_LEFT != 0,
            is_right: flags & Self::FLAG_RIGHT != 0,
            is_jumping: flags & Self::FLAG_JUMP != 0,
            is_shifting: flags & Self::FLAG_SHIFT != 0,
            is_sprinting: flags & Self::FLAG_SPRINT != 0,
        }
    }

    /// Encodes to a single byte of packed flags.
    pub fn to_byte(self) -> u8 {
        let mut flags: u8 = 0;
        if self.is_forward {
            flags |= Self::FLAG_FORWARD;
        }
        if self.is_backward {
            flags |= Self::FLAG_BACKWARD;
        }
        if self.is_left {
            flags |= Self::FLAG_LEFT;
        }
        if self.is_right {
            flags |= Self::FLAG_RIGHT;
        }
        if self.is_jumping {
            flags |= Self::FLAG_JUMP;
        }
        if self.is_shifting {
            flags |= Self::FLAG_SHIFT;
        }
        if self.is_sprinting {
            flags |= Self::FLAG_SPRINT;
        }
        flags
    }
}

/// Serverbound packet carrying per-tick player input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundPlayerInputPacket {
    /// The current input state.
    pub input: PlayerInput,
}

impl Packet for ServerboundPlayerInputPacket {
    const PACKET_ID: i32 = 0x2B;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        if !data.has_remaining() {
            return Err(PacketDecodeError::InvalidData(
                "not enough data for PlayerInput flags byte".into(),
            ));
        }
        let flags = data.get_u8();
        Ok(Self {
            input: PlayerInput::from_byte(flags),
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(1);
        buf.put_u8(self.input.to_byte());
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        let input = PlayerInput::from_byte(0x00);
        assert!(!input.is_forward);
        assert!(!input.is_shifting);
        assert!(!input.is_sprinting);
        assert_eq!(input.to_byte(), 0x00);
    }

    #[test]
    fn test_all_flags() {
        let input = PlayerInput::from_byte(0x7F); // all 7 bits set
        assert!(input.is_forward);
        assert!(input.is_backward);
        assert!(input.is_left);
        assert!(input.is_right);
        assert!(input.is_jumping);
        assert!(input.is_shifting);
        assert!(input.is_sprinting);
        assert_eq!(input.to_byte(), 0x7F);
    }

    #[test]
    fn test_sneak_only() {
        let input = PlayerInput::from_byte(0x20);
        assert!(!input.is_forward);
        assert!(input.is_shifting);
        assert!(!input.is_sprinting);
    }

    #[test]
    fn test_sprint_only() {
        let input = PlayerInput::from_byte(0x40);
        assert!(!input.is_shifting);
        assert!(input.is_sprinting);
    }

    #[test]
    fn test_forward_and_sneak() {
        let input = PlayerInput {
            is_forward: true,
            is_shifting: true,
            ..Default::default()
        };
        assert_eq!(input.to_byte(), 0x21); // 0x01 | 0x20
    }

    #[test]
    fn test_roundtrip() {
        let original = ServerboundPlayerInputPacket {
            input: PlayerInput {
                is_forward: true,
                is_backward: false,
                is_left: true,
                is_right: false,
                is_jumping: true,
                is_shifting: true,
                is_sprinting: false,
            },
        };
        let encoded = original.encode();
        let decoded = ServerboundPlayerInputPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.input, original.input);
    }

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ServerboundPlayerInputPacket, 0x2B);
    }
}
