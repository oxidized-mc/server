//! Player abilities: flying, invulnerability, build speed.
//!
//! Maps game mode to the correct ability flags and provides
//! serialization to the wire `flags` byte.

use super::game_mode::GameMode;

/// A player's current ability state.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerAbilities {
    /// Whether the player takes no damage.
    pub invulnerable: bool,
    /// Whether the player is currently flying.
    pub flying: bool,
    /// Whether the player is allowed to fly.
    pub can_fly: bool,
    /// Whether the player can break blocks instantly (creative).
    pub instabuild: bool,
    /// Flying speed in blocks per tick (default 0.05).
    pub fly_speed: f32,
    /// Walking speed modifier (default 0.1).
    pub walk_speed: f32,
}

impl Default for PlayerAbilities {
    fn default() -> Self {
        Self {
            invulnerable: false,
            flying: false,
            can_fly: false,
            instabuild: false,
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
                invulnerable: true,
                flying: false,
                can_fly: true,
                instabuild: true,
                ..Self::default()
            },
            GameMode::Adventure => Self::default(),
            GameMode::Spectator => Self {
                invulnerable: true,
                flying: true,
                can_fly: true,
                instabuild: false,
                ..Self::default()
            },
        }
    }

    /// Packs the abilities into a wire-format flags byte.
    ///
    /// Bit layout: `invulnerable(0x01) | flying(0x02) | can_fly(0x04) | instabuild(0x08)`.
    pub fn flags_byte(&self) -> u8 {
        let mut flags = 0u8;
        if self.invulnerable {
            flags |= 0x01;
        }
        if self.flying {
            flags |= 0x02;
        }
        if self.can_fly {
            flags |= 0x04;
        }
        if self.instabuild {
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
        assert!(!a.invulnerable);
        assert!(!a.flying);
        assert!(!a.can_fly);
        assert!(!a.instabuild);
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
        assert!(a.invulnerable);
        assert!(a.can_fly);
        assert!(a.instabuild);
        assert!(!a.flying);
        assert_eq!(a.flags_byte(), 0x01 | 0x04 | 0x08);
    }

    #[test]
    fn test_spectator_abilities() {
        let a = PlayerAbilities::for_game_mode(GameMode::Spectator);
        assert!(a.invulnerable);
        assert!(a.flying);
        assert!(a.can_fly);
        assert!(!a.instabuild);
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
            invulnerable: true,
            flying: true,
            can_fly: true,
            instabuild: true,
            ..Default::default()
        };
        assert_eq!(a.flags_byte(), 0x0F);
    }
}
