//! [`GameType`] — the game mode for a player.
//!
//! Maps to the vanilla `GameType` enum used in login packets,
//! player info updates, and game event packets.

use std::fmt;

use bytes::{Bytes, BytesMut};

use crate::codec::types::TypeError;
use crate::codec::varint;

/// The game mode for a player.
///
/// # Wire format
///
/// Encoded as a VarInt (0–3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum GameType {
    /// Survival mode — the player can take damage and must gather resources.
    Survival = 0,
    /// Creative mode — the player has unlimited resources and cannot take damage.
    Creative = 1,
    /// Adventure mode — the player can take damage but cannot break/place blocks
    /// freely.
    Adventure = 2,
    /// Spectator mode — the player is invisible and can fly through blocks.
    Spectator = 3,
}

impl GameType {
    /// Returns the numeric ID of this game type.
    pub const fn id(self) -> i32 {
        self as i32
    }

    /// Returns the lowercase name of this game type.
    pub const fn name(self) -> &'static str {
        match self {
            GameType::Survival => "survival",
            GameType::Creative => "creative",
            GameType::Adventure => "adventure",
            GameType::Spectator => "spectator",
        }
    }

    /// Returns the vanilla translation key for this game type (e.g.,
    /// `"gameMode.survival"`).
    pub const fn translation_key(self) -> &'static str {
        match self {
            GameType::Survival => "gameMode.survival",
            GameType::Creative => "gameMode.creative",
            GameType::Adventure => "gameMode.adventure",
            GameType::Spectator => "gameMode.spectator",
        }
    }

    /// Looks up a game type by numeric ID.
    ///
    /// Returns `None` if `id` is not in 0–3.
    pub const fn by_id(id: i32) -> Option<GameType> {
        match id {
            0 => Some(GameType::Survival),
            1 => Some(GameType::Creative),
            2 => Some(GameType::Adventure),
            3 => Some(GameType::Spectator),
            _ => None,
        }
    }

    /// Looks up a game type by lowercase name.
    ///
    /// Returns `None` if the name is not recognized.
    pub fn by_name(name: &str) -> Option<GameType> {
        match name {
            "survival" => Some(GameType::Survival),
            "creative" => Some(GameType::Creative),
            "adventure" => Some(GameType::Adventure),
            "spectator" => Some(GameType::Spectator),
            _ => None,
        }
    }

    /// Returns `true` if this is [`GameType::Creative`].
    pub const fn is_creative(self) -> bool {
        matches!(self, GameType::Creative)
    }

    /// Returns `true` if this is a "survival-like" mode where the player
    /// can take damage and needs food ([`GameType::Survival`] or
    /// [`GameType::Adventure`]).
    pub const fn is_survival(self) -> bool {
        matches!(self, GameType::Survival | GameType::Adventure)
    }

    /// Returns `true` if block placing is restricted in this game mode
    /// ([`GameType::Adventure`] or [`GameType::Spectator`]).
    pub const fn is_block_placing_restricted(self) -> bool {
        matches!(self, GameType::Adventure | GameType::Spectator)
    }

    /// Reads a [`GameType`] from a wire buffer as a VarInt.
    ///
    /// # Errors
    ///
    /// Returns [`TypeError`] if the buffer is truncated or the value is
    /// out of range.
    pub fn read(buf: &mut Bytes) -> Result<Self, TypeError> {
        let id = varint::read_varint_buf(buf)?;
        GameType::by_id(id).ok_or(TypeError::UnexpectedEof { need: 1, have: 0 })
    }

    /// Writes this [`GameType`] to a wire buffer as a VarInt.
    pub fn write(&self, buf: &mut BytesMut) {
        varint::write_varint_buf(self.id(), buf);
    }
}

impl fmt::Display for GameType {
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
    fn test_game_type_by_id_valid() {
        assert_eq!(GameType::by_id(0), Some(GameType::Survival));
        assert_eq!(GameType::by_id(1), Some(GameType::Creative));
        assert_eq!(GameType::by_id(2), Some(GameType::Adventure));
        assert_eq!(GameType::by_id(3), Some(GameType::Spectator));
    }

    #[test]
    fn test_game_type_by_id_invalid() {
        assert_eq!(GameType::by_id(-1), None);
        assert_eq!(GameType::by_id(4), None);
        assert_eq!(GameType::by_id(100), None);
    }

    // ── by_name ─────────────────────────────────────────────────────

    #[test]
    fn test_game_type_by_name_valid() {
        assert_eq!(GameType::by_name("survival"), Some(GameType::Survival));
        assert_eq!(GameType::by_name("creative"), Some(GameType::Creative));
        assert_eq!(GameType::by_name("adventure"), Some(GameType::Adventure));
        assert_eq!(GameType::by_name("spectator"), Some(GameType::Spectator));
    }

    #[test]
    fn test_game_type_by_name_invalid() {
        assert_eq!(GameType::by_name("Survival"), None);
        assert_eq!(GameType::by_name("unknown"), None);
        assert_eq!(GameType::by_name(""), None);
    }

    // ── Roundtrip id ↔ enum ─────────────────────────────────────────

    #[test]
    fn test_game_type_id_roundtrip() {
        for id in 0..=3 {
            let gt = GameType::by_id(id).unwrap();
            assert_eq!(gt.id(), id);
        }
    }

    // ── Boolean predicates ──────────────────────────────────────────

    #[test]
    fn test_game_type_is_creative() {
        assert!(!GameType::Survival.is_creative());
        assert!(GameType::Creative.is_creative());
        assert!(!GameType::Adventure.is_creative());
        assert!(!GameType::Spectator.is_creative());
    }

    #[test]
    fn test_game_type_is_survival() {
        assert!(GameType::Survival.is_survival());
        assert!(!GameType::Creative.is_survival());
        assert!(GameType::Adventure.is_survival());
        assert!(!GameType::Spectator.is_survival());
    }

    #[test]
    fn test_game_type_is_block_placing_restricted() {
        assert!(!GameType::Survival.is_block_placing_restricted());
        assert!(!GameType::Creative.is_block_placing_restricted());
        assert!(GameType::Adventure.is_block_placing_restricted());
        assert!(GameType::Spectator.is_block_placing_restricted());
    }

    // ── Display ─────────────────────────────────────────────────────

    #[test]
    fn test_game_type_display() {
        assert_eq!(format!("{}", GameType::Survival), "survival");
        assert_eq!(format!("{}", GameType::Creative), "creative");
        assert_eq!(format!("{}", GameType::Adventure), "adventure");
        assert_eq!(format!("{}", GameType::Spectator), "spectator");
    }

    // ── Wire roundtrip ──────────────────────────────────────────────

    #[test]
    fn test_game_type_wire_roundtrip() {
        for id in 0..=3 {
            let gt = GameType::by_id(id).unwrap();
            let mut buf = BytesMut::new();
            gt.write(&mut buf);
            let mut data = buf.freeze();
            let decoded = GameType::read(&mut data).unwrap();
            assert_eq!(decoded, gt);
        }
    }
}
