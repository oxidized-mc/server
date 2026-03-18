//! [`Difficulty`] — the difficulty level of the game.
//!
//! Maps to the vanilla `Difficulty` enum used in server-properties,
//! login/join-game packets, and difficulty-change packets.

use std::fmt;

use bytes::{Bytes, BytesMut};

use crate::codec::types::TypeError;
use crate::codec::varint;

/// The difficulty level of the game.
///
/// # Wire format
///
/// Encoded as a VarInt (0–3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum Difficulty {
    /// Peaceful — no hostile mobs, health regenerates.
    Peaceful = 0,
    /// Easy — hostile mobs deal less damage.
    Easy = 1,
    /// Normal — default difficulty.
    Normal = 2,
    /// Hard — hostile mobs deal more damage, hunger can kill.
    Hard = 3,
}

impl Difficulty {
    /// Returns the numeric ID of this difficulty.
    pub const fn id(self) -> i32 {
        self as i32
    }

    /// Returns the lowercase name of this difficulty.
    pub const fn name(self) -> &'static str {
        match self {
            Difficulty::Peaceful => "peaceful",
            Difficulty::Easy => "easy",
            Difficulty::Normal => "normal",
            Difficulty::Hard => "hard",
        }
    }

    /// Looks up a difficulty by numeric ID.
    ///
    /// Returns `None` if `id` is not in 0–3.
    pub const fn by_id(id: i32) -> Option<Difficulty> {
        match id {
            0 => Some(Difficulty::Peaceful),
            1 => Some(Difficulty::Easy),
            2 => Some(Difficulty::Normal),
            3 => Some(Difficulty::Hard),
            _ => None,
        }
    }

    /// Looks up a difficulty by lowercase name.
    ///
    /// Returns `None` if the name is not recognized.
    pub fn by_name(name: &str) -> Option<Difficulty> {
        match name {
            "peaceful" => Some(Difficulty::Peaceful),
            "easy" => Some(Difficulty::Easy),
            "normal" => Some(Difficulty::Normal),
            "hard" => Some(Difficulty::Hard),
            _ => None,
        }
    }

    /// Reads a [`Difficulty`] from a wire buffer as a VarInt.
    ///
    /// # Errors
    ///
    /// Returns [`TypeError`] if the buffer is truncated or the value is
    /// out of range.
    pub fn read(buf: &mut Bytes) -> Result<Self, TypeError> {
        let id = varint::read_varint_buf(buf)?;
        Difficulty::by_id(id).ok_or(TypeError::UnexpectedEof { need: 1, have: 0 })
    }

    /// Writes this [`Difficulty`] to a wire buffer as a VarInt.
    pub fn write(&self, buf: &mut BytesMut) {
        varint::write_varint_buf(self.id(), buf);
    }
}

impl fmt::Display for Difficulty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ── by_id ───────────────────────────────────────────────────────

    #[test]
    fn test_difficulty_by_id_valid() {
        assert_eq!(Difficulty::by_id(0), Some(Difficulty::Peaceful));
        assert_eq!(Difficulty::by_id(1), Some(Difficulty::Easy));
        assert_eq!(Difficulty::by_id(2), Some(Difficulty::Normal));
        assert_eq!(Difficulty::by_id(3), Some(Difficulty::Hard));
    }

    #[test]
    fn test_difficulty_by_id_invalid() {
        assert_eq!(Difficulty::by_id(-1), None);
        assert_eq!(Difficulty::by_id(4), None);
        assert_eq!(Difficulty::by_id(100), None);
    }

    // ── by_name ─────────────────────────────────────────────────────

    #[test]
    fn test_difficulty_by_name_valid() {
        assert_eq!(Difficulty::by_name("peaceful"), Some(Difficulty::Peaceful));
        assert_eq!(Difficulty::by_name("easy"), Some(Difficulty::Easy));
        assert_eq!(Difficulty::by_name("normal"), Some(Difficulty::Normal));
        assert_eq!(Difficulty::by_name("hard"), Some(Difficulty::Hard));
    }

    #[test]
    fn test_difficulty_by_name_invalid() {
        assert_eq!(Difficulty::by_name("Peaceful"), None);
        assert_eq!(Difficulty::by_name("unknown"), None);
        assert_eq!(Difficulty::by_name(""), None);
    }

    // ── Display ─────────────────────────────────────────────────────

    #[test]
    fn test_difficulty_display() {
        assert_eq!(format!("{}", Difficulty::Peaceful), "peaceful");
        assert_eq!(format!("{}", Difficulty::Easy), "easy");
        assert_eq!(format!("{}", Difficulty::Normal), "normal");
        assert_eq!(format!("{}", Difficulty::Hard), "hard");
    }

    // ── Roundtrip ───────────────────────────────────────────────────

    #[test]
    fn test_difficulty_id_roundtrip() {
        for id in 0..=3 {
            let d = Difficulty::by_id(id).unwrap();
            assert_eq!(d.id(), id);
        }
    }

    // ── Wire roundtrip ──────────────────────────────────────────────

    #[test]
    fn test_difficulty_wire_roundtrip() {
        for id in 0..=3 {
            let d = Difficulty::by_id(id).unwrap();
            let mut buf = BytesMut::new();
            d.write(&mut buf);
            let mut data = buf.freeze();
            let decoded = Difficulty::read(&mut data).unwrap();
            assert_eq!(decoded, d);
        }
    }
}
