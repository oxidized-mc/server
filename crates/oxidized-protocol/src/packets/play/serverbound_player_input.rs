//! Serverbound player input packet.
//!
//! Sent every tick with the player's current movement input state.
//! In 26.1-pre-3, this is how sneak (shift) is communicated — not via
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
/// - Bit 0 (0x01): forward (W)
/// - Bit 1 (0x02): backward (S)
/// - Bit 2 (0x04): left (A)
/// - Bit 3 (0x08): right (D)
/// - Bit 4 (0x10): jump (Space)
/// - Bit 5 (0x20): shift/sneak (Shift)
/// - Bit 6 (0x40): sprint (Ctrl)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayerInput {
    /// W key held.
    pub forward: bool,
    /// S key held.
    pub backward: bool,
    /// A key held.
    pub left: bool,
    /// D key held.
    pub right: bool,
    /// Space bar held.
    pub jump: bool,
    /// Shift key held (sneak).
    pub shift: bool,
    /// Sprint key held (Ctrl).
    pub sprint: bool,
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
            forward: flags & Self::FLAG_FORWARD != 0,
            backward: flags & Self::FLAG_BACKWARD != 0,
            left: flags & Self::FLAG_LEFT != 0,
            right: flags & Self::FLAG_RIGHT != 0,
            jump: flags & Self::FLAG_JUMP != 0,
            shift: flags & Self::FLAG_SHIFT != 0,
            sprint: flags & Self::FLAG_SPRINT != 0,
        }
    }

    /// Encodes to a single byte of packed flags.
    pub fn to_byte(self) -> u8 {
        let mut flags: u8 = 0;
        if self.forward {
            flags |= Self::FLAG_FORWARD;
        }
        if self.backward {
            flags |= Self::FLAG_BACKWARD;
        }
        if self.left {
            flags |= Self::FLAG_LEFT;
        }
        if self.right {
            flags |= Self::FLAG_RIGHT;
        }
        if self.jump {
            flags |= Self::FLAG_JUMP;
        }
        if self.shift {
            flags |= Self::FLAG_SHIFT;
        }
        if self.sprint {
            flags |= Self::FLAG_SPRINT;
        }
        flags
    }
}

/// Serverbound packet carrying per-tick player input.
#[derive(Debug, Clone)]
pub struct ServerboundPlayerInputPacket {
    /// The current input state.
    pub input: PlayerInput,
}

impl ServerboundPlayerInputPacket {
    /// Packet ID in the PLAY state serverbound registry.
    pub const PACKET_ID: i32 = 0x2B;

    /// Decodes the packet from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer is empty.
    pub fn decode(mut data: Bytes) -> Result<Self, std::io::Error> {
        if !data.has_remaining() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "not enough data for PlayerInput flags byte",
            ));
        }
        let flags = data.get_u8();
        Ok(Self {
            input: PlayerInput::from_byte(flags),
        })
    }

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(1);
        buf.put_u8(self.input.to_byte());
        buf
    }
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
        self.encode()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        let input = PlayerInput::from_byte(0x00);
        assert!(!input.forward);
        assert!(!input.shift);
        assert!(!input.sprint);
        assert_eq!(input.to_byte(), 0x00);
    }

    #[test]
    fn test_all_flags() {
        let input = PlayerInput::from_byte(0x7F); // all 7 bits set
        assert!(input.forward);
        assert!(input.backward);
        assert!(input.left);
        assert!(input.right);
        assert!(input.jump);
        assert!(input.shift);
        assert!(input.sprint);
        assert_eq!(input.to_byte(), 0x7F);
    }

    #[test]
    fn test_sneak_only() {
        let input = PlayerInput::from_byte(0x20);
        assert!(!input.forward);
        assert!(input.shift);
        assert!(!input.sprint);
    }

    #[test]
    fn test_sprint_only() {
        let input = PlayerInput::from_byte(0x40);
        assert!(!input.shift);
        assert!(input.sprint);
    }

    #[test]
    fn test_forward_and_sneak() {
        let input = PlayerInput {
            forward: true,
            shift: true,
            ..Default::default()
        };
        assert_eq!(input.to_byte(), 0x21); // 0x01 | 0x20
    }

    #[test]
    fn test_roundtrip() {
        let original = ServerboundPlayerInputPacket {
            input: PlayerInput {
                forward: true,
                backward: false,
                left: true,
                right: false,
                jump: true,
                shift: true,
                sprint: false,
            },
        };
        let encoded = original.encode();
        let decoded = ServerboundPlayerInputPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.input, original.input);
    }

    #[test]
    fn test_packet_id() {
        assert_eq!(ServerboundPlayerInputPacket::PACKET_ID, 0x2B);
    }

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = ServerboundPlayerInputPacket {
            input: PlayerInput {
                forward: true,
                shift: true,
                ..Default::default()
            },
        };
        let encoded = Packet::encode(&pkt);
        let decoded = <ServerboundPlayerInputPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.input.forward, true);
        assert_eq!(decoded.input.shift, true);
        assert_eq!(decoded.input.backward, false);
        assert_eq!(decoded.input.sprint, false);
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(<ServerboundPlayerInputPacket as Packet>::PACKET_ID, 0x2B);
    }
}
