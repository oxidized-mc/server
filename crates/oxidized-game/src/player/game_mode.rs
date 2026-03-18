//! Minecraft game modes.
//!
//! Maps the four game modes (Survival, Creative, Adventure, Spectator)
//! to their protocol IDs and provides conversion helpers.
//!
//! Mirrors `net.minecraft.world.level.GameType`.

/// The four Minecraft game modes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum GameMode {
    /// Survival mode — resource gathering and health.
    #[default]
    Survival = 0,
    /// Creative mode — unlimited resources and flying.
    Creative = 1,
    /// Adventure mode — cannot break/place blocks without tools.
    Adventure = 2,
    /// Spectator mode — invisible, can fly through blocks.
    Spectator = 3,
}

impl GameMode {
    /// Converts a protocol ID to a `GameMode`.
    ///
    /// Unknown IDs default to [`GameMode::Survival`], matching vanilla
    /// behavior for corrupt player data.
    pub fn from_id(id: i32) -> Self {
        match id {
            0 => Self::Survival,
            1 => Self::Creative,
            2 => Self::Adventure,
            3 => Self::Spectator,
            _ => Self::Survival,
        }
    }

    /// Returns the protocol integer ID.
    pub fn id(self) -> i32 {
        self as i32
    }

    /// Returns the "nullable" game mode byte used in login/respawn packets.
    ///
    /// A value of `-1` means "no previous game mode".
    pub fn nullable_id(gm: Option<Self>) -> i8 {
        match gm {
            Some(mode) => mode.id() as i8,
            None => -1,
        }
    }

    /// Returns `true` if the player can fly and has creative-like powers.
    pub fn is_creative_or_spectator(self) -> bool {
        matches!(self, Self::Creative | Self::Spectator)
    }

    /// Returns `true` if the game mode allows flight.
    pub fn allow_flight(self) -> bool {
        matches!(self, Self::Creative | Self::Spectator)
    }

    /// Returns `true` if this game mode is survival.
    pub fn is_survival(self) -> bool {
        self == Self::Survival
    }

    /// Returns `true` if this game mode is creative.
    pub fn is_creative(self) -> bool {
        self == Self::Creative
    }

    /// Returns `true` if this game mode is spectator.
    pub fn is_spectator(self) -> bool {
        self == Self::Spectator
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_from_id_valid() {
        assert_eq!(GameMode::from_id(0), GameMode::Survival);
        assert_eq!(GameMode::from_id(1), GameMode::Creative);
        assert_eq!(GameMode::from_id(2), GameMode::Adventure);
        assert_eq!(GameMode::from_id(3), GameMode::Spectator);
    }

    #[test]
    fn test_from_id_unknown_defaults_to_survival() {
        assert_eq!(GameMode::from_id(4), GameMode::Survival);
        assert_eq!(GameMode::from_id(-1), GameMode::Survival);
        assert_eq!(GameMode::from_id(255), GameMode::Survival);
    }

    #[test]
    fn test_default_is_survival() {
        assert_eq!(GameMode::default(), GameMode::Survival);
    }

    #[test]
    fn test_id_roundtrip() {
        for id in 0..=3i32 {
            let gm = GameMode::from_id(id);
            assert_eq!(gm.id(), id);
        }
    }

    #[test]
    fn test_nullable_id() {
        assert_eq!(GameMode::nullable_id(None), -1);
        assert_eq!(GameMode::nullable_id(Some(GameMode::Survival)), 0);
        assert_eq!(GameMode::nullable_id(Some(GameMode::Creative)), 1);
    }

    #[test]
    fn test_creative_or_spectator() {
        assert!(!GameMode::Survival.is_creative_or_spectator());
        assert!(GameMode::Creative.is_creative_or_spectator());
        assert!(!GameMode::Adventure.is_creative_or_spectator());
        assert!(GameMode::Spectator.is_creative_or_spectator());
    }

    #[test]
    fn test_allow_flight() {
        assert!(!GameMode::Survival.allow_flight());
        assert!(GameMode::Creative.allow_flight());
        assert!(!GameMode::Adventure.allow_flight());
        assert!(GameMode::Spectator.allow_flight());
    }

    #[test]
    fn test_mode_checks() {
        assert!(GameMode::Survival.is_survival());
        assert!(!GameMode::Survival.is_creative());
        assert!(GameMode::Creative.is_creative());
        assert!(GameMode::Spectator.is_spectator());
    }
}
