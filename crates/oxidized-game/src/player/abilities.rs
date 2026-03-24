//! Player abilities: flying, invulnerability, build speed.
//!
//! Maps game mode to the correct ability flags and provides
//! serialization to the wire `flags` byte.

use super::game_mode::GameMode;

/// A player's current ability state.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerAbilities {
    /// Whether the player takes no damage.
    pub is_invulnerable: bool,
    /// Whether the player is currently flying.
    pub is_flying: bool,
    /// Whether the player is allowed to fly.
    pub can_fly: bool,
    /// Whether the player can break blocks instantly (creative).
    pub is_instabuild: bool,
    /// Flying speed in blocks per tick (default 0.05).
    pub fly_speed: f32,
    /// Walking speed modifier (default 0.1).
    pub walk_speed: f32,
}

impl Default for PlayerAbilities {
    fn default() -> Self {
        Self {
            is_invulnerable: false,
            is_flying: false,
            can_fly: false,
            is_instabuild: false,
            fly_speed: 0.05,
            walk_speed: 0.1,
        }
    }
}

impl PlayerAbilities {
    /// Returns the default abilities for a given game mode.
    pub fn for_game_mode(mode: GameMode) -> Self {
        match mode {
            GameMode::Survival => Self::default(),
            GameMode::Creative => Self {
                is_invulnerable: true,
                is_flying: false,
                can_fly: true,
                is_instabuild: true,
                ..Self::default()
            },
            GameMode::Adventure => Self::default(),
            GameMode::Spectator => Self {
                is_invulnerable: true,
                is_flying: true,
                can_fly: true,
                is_instabuild: false,
                ..Self::default()
            },
        }
    }

    /// Packs the abilities into a wire-format flags byte.
    ///
    /// Bit layout: `is_invulnerable(0x01) | is_flying(0x02) | can_fly(0x04) | is_instabuild(0x08)`.
    pub fn flags_byte(&self) -> u8 {
        let mut flags = 0u8;
        if self.is_invulnerable {
            flags |= 0x01;
        }
        if self.is_flying {
            flags |= 0x02;
        }
        if self.can_fly {
            flags |= 0x04;
        }
        if self.is_instabuild {
            flags |= 0x08;
        }
        flags
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_default_abilities() {
        let a = PlayerAbilities::default();
        assert!(!a.is_invulnerable);
        assert!(!a.is_flying);
        assert!(!a.can_fly);
        assert!(!a.is_instabuild);
        assert!((a.fly_speed - 0.05).abs() < f32::EPSILON);
        assert!((a.walk_speed - 0.1).abs() < f32::EPSILON);
    }

    #[test]
    fn test_survival_abilities() {
        let a = PlayerAbilities::for_game_mode(GameMode::Survival);
        assert_eq!(a.flags_byte(), 0x00);
    }

    #[test]
    fn test_creative_abilities() {
        let a = PlayerAbilities::for_game_mode(GameMode::Creative);
        assert!(a.is_invulnerable);
        assert!(a.can_fly);
        assert!(a.is_instabuild);
        assert!(!a.is_flying);
        assert_eq!(a.flags_byte(), 0x01 | 0x04 | 0x08);
    }

    #[test]
    fn test_spectator_abilities() {
        let a = PlayerAbilities::for_game_mode(GameMode::Spectator);
        assert!(a.is_invulnerable);
        assert!(a.is_flying);
        assert!(a.can_fly);
        assert!(!a.is_instabuild);
        assert_eq!(a.flags_byte(), 0x01 | 0x02 | 0x04);
    }

    #[test]
    fn test_adventure_abilities() {
        let a = PlayerAbilities::for_game_mode(GameMode::Adventure);
        assert_eq!(a.flags_byte(), 0x00);
    }

    #[test]
    fn test_flags_byte_all_set() {
        let a = PlayerAbilities {
            is_invulnerable: true,
            is_flying: true,
            can_fly: true,
            is_instabuild: true,
            ..Default::default()
        };
        assert_eq!(a.flags_byte(), 0x0F);
    }
}
