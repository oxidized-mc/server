//! Game rule system — typed boolean and integer rules with vanilla defaults.
//!
//! Each rule has a snake_case string name (new in 26.1) for NBT serialisation
//! and the `/gamerule` command. The [`GameRules`] struct stores current values
//! in a flat array indexed by enum discriminant (zero allocation, O(1) access).
//!
//! Corresponds to `net.minecraft.world.level.gamerules.GameRules`.

/// Declares all game rules in a single source of truth.
///
/// Generates:
/// - `GameRuleKey` enum with a variant per rule
/// - `GameRuleKey::COUNT` (total number of rules)
/// - `GameRuleKey::name()` → `&'static str` (26.1 snake_case name)
/// - `GameRuleKey::legacy_name()` → `Option<&'static str>` (pre-26.1 camelCase)
/// - `GameRuleKey::from_name()` → `Option<GameRuleKey>` (accepts both names)
/// - `GameRuleKey::all_sorted()` → sorted slice of all keys
/// - `GameRuleKey::default_value()` → `GameRuleValue`
macro_rules! define_game_rules {
    (
        $(
            $variant:ident => {
                name: $name:literal,
                $(legacy: $legacy:literal,)?
                default: $default:expr $(,)?
            }
        ),+ $(,)?
    ) => {
        /// Identifies a specific game rule.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(u16)]
        #[allow(missing_docs)]
        pub enum GameRuleKey {
            $($variant),+
        }

        impl GameRuleKey {
            /// Total number of game rules.
            pub const COUNT: usize = {
                let mut n = 0u16;
                $(let _ = stringify!($variant); n += 1;)+
                n as usize
            };

            /// Returns the 26.1 snake_case wire name for this rule.
            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name),+
                }
            }

            /// Returns the pre-26.1 camelCase name, if one existed.
            pub fn legacy_name(self) -> Option<&'static str> {
                match self {
                    $(Self::$variant => define_game_rules!(@legacy $($legacy)?)),+
                }
            }

            /// Resolves a name (26.1 or legacy) to a [`GameRuleKey`].
            pub fn from_name(name: &str) -> Option<Self> {
                // Try 26.1 names first, then legacy.
                $(if name == $name { return Some(Self::$variant); })+
                $($(if name == $legacy { return Some(Self::$variant); })?)+
                None
            }

            /// Returns all rule keys sorted alphabetically by their 26.1 name.
            pub fn all_sorted() -> &'static [GameRuleKey] {
                static SORTED: std::sync::LazyLock<Vec<GameRuleKey>> = std::sync::LazyLock::new(|| {
                    let mut keys = vec![$(GameRuleKey::$variant),+];
                    keys.sort_by_key(|k| k.name());
                    keys
                });
                &SORTED
            }

            /// Returns all 26.1 rule names sorted alphabetically.
            pub fn all_names() -> Vec<&'static str> {
                Self::all_sorted().iter().map(|k| k.name()).collect()
            }

            /// Returns the vanilla default value for this rule.
            pub fn default_value(self) -> GameRuleValue {
                match self {
                    $(Self::$variant => $default),+
                }
            }
        }
    };

    // Helper: emit None when no legacy name is provided.
    (@legacy) => { None };
    (@legacy $legacy:literal) => { Some($legacy) };
}

use GameRuleValue::{Bool, Int};

define_game_rules! {
    // ── Boolean rules ────────────────────────────────────────────────
    AdvanceTime => {
        name: "advance_time",
        legacy: "doDaylightCycle",
        default: Bool(true),
    },
    AdvanceWeather => {
        name: "advance_weather",
        legacy: "doWeatherCycle",
        default: Bool(true),
    },
    AllowEnteringNetherUsingPortals => {
        name: "allow_entering_nether_using_portals",
        default: Bool(true),
    },
    BlockDrops => {
        name: "block_drops",
        legacy: "doTileDrops",
        default: Bool(true),
    },
    BlockExplosionDropDecay => {
        name: "block_explosion_drop_decay",
        legacy: "blockExplosionDropDecay",
        default: Bool(true),
    },
    CommandBlockOutput => {
        name: "command_block_output",
        legacy: "commandBlockOutput",
        default: Bool(true),
    },
    CommandBlocksWork => {
        name: "command_blocks_work",
        default: Bool(true),
    },
    DrowningDamage => {
        name: "drowning_damage",
        legacy: "doDrowningDamage",
        default: Bool(true),
    },
    ElytraMovementCheck => {
        name: "elytra_movement_check",
        default: Bool(true),
    },
    EnderPearlsVanishOnDeath => {
        name: "ender_pearls_vanish_on_death",
        default: Bool(true),
    },
    EntityDrops => {
        name: "entity_drops",
        legacy: "doEntityDrops",
        default: Bool(true),
    },
    FallDamage => {
        name: "fall_damage",
        legacy: "fallDamage",
        default: Bool(true),
    },
    FireDamage => {
        name: "fire_damage",
        legacy: "doFireDamage",
        default: Bool(true),
    },
    ForgiveDeadPlayers => {
        name: "forgive_dead_players",
        legacy: "forgiveDeadPlayers",
        default: Bool(true),
    },
    FreezeDamage => {
        name: "freeze_damage",
        legacy: "doFreezeDamage",
        default: Bool(true),
    },
    GlobalSoundEvents => {
        name: "global_sound_events",
        default: Bool(true),
    },
    ImmediateRespawn => {
        name: "immediate_respawn",
        legacy: "doImmediateRespawn",
        default: Bool(false),
    },
    KeepInventory => {
        name: "keep_inventory",
        legacy: "keepInventory",
        default: Bool(false),
    },
    LavaSourceConversion => {
        name: "lava_source_conversion",
        default: Bool(false),
    },
    LimitedCrafting => {
        name: "limited_crafting",
        legacy: "doLimitedCrafting",
        default: Bool(false),
    },
    LocatorBar => {
        name: "locator_bar",
        default: Bool(true),
    },
    LogAdminCommands => {
        name: "log_admin_commands",
        legacy: "logAdminCommands",
        default: Bool(true),
    },
    MobDrops => {
        name: "mob_drops",
        legacy: "doMobLoot",
        default: Bool(true),
    },
    MobExplosionDropDecay => {
        name: "mob_explosion_drop_decay",
        legacy: "mobExplosionDropDecay",
        default: Bool(true),
    },
    MobGriefing => {
        name: "mob_griefing",
        legacy: "mobGriefing",
        default: Bool(true),
    },
    NaturalHealthRegeneration => {
        name: "natural_health_regeneration",
        legacy: "naturalRegeneration",
        default: Bool(true),
    },
    PlayerMovementCheck => {
        name: "player_movement_check",
        default: Bool(true),
    },
    ProjectilesCanBreakBlocks => {
        name: "projectiles_can_break_blocks",
        default: Bool(true),
    },
    Pvp => {
        name: "pvp",
        legacy: "pvp",
        default: Bool(true),
    },
    Raids => {
        name: "raids",
        default: Bool(true),
    },
    ReducedDebugInfo => {
        name: "reduced_debug_info",
        legacy: "reducedDebugInfo",
        default: Bool(false),
    },
    SendCommandFeedback => {
        name: "send_command_feedback",
        legacy: "sendCommandFeedback",
        default: Bool(true),
    },
    ShowAdvancementMessages => {
        name: "show_advancement_messages",
        legacy: "announceAdvancements",
        default: Bool(true),
    },
    ShowDeathMessages => {
        name: "show_death_messages",
        legacy: "showDeathMessages",
        default: Bool(true),
    },
    SpawnerBlocksWork => {
        name: "spawner_blocks_work",
        default: Bool(true),
    },
    SpawnMobs => {
        name: "spawn_mobs",
        legacy: "doMobSpawning",
        default: Bool(true),
    },
    SpawnMonsters => {
        name: "spawn_monsters",
        default: Bool(true),
    },
    SpawnPatrols => {
        name: "spawn_patrols",
        legacy: "doPatrolSpawning",
        default: Bool(true),
    },
    SpawnPhantoms => {
        name: "spawn_phantoms",
        legacy: "doInsomnia",
        default: Bool(true),
    },
    SpawnWanderingTraders => {
        name: "spawn_wandering_traders",
        legacy: "doTraderSpawning",
        default: Bool(true),
    },
    SpawnWardens => {
        name: "spawn_wardens",
        legacy: "doWardenSpawning",
        default: Bool(true),
    },
    SpectatorsGenerateChunks => {
        name: "spectators_generate_chunks",
        default: Bool(true),
    },
    SpreadVines => {
        name: "spread_vines",
        default: Bool(true),
    },
    TntExplodes => {
        name: "tnt_explodes",
        legacy: "tntExplodes",
        default: Bool(true),
    },
    TntExplosionDropDecay => {
        name: "tnt_explosion_drop_decay",
        legacy: "tntExplosionDropDecay",
        default: Bool(false),
    },
    UniversalAnger => {
        name: "universal_anger",
        legacy: "universalAnger",
        default: Bool(false),
    },
    WaterSourceConversion => {
        name: "water_source_conversion",
        default: Bool(true),
    },

    // ── Integer rules ────────────────────────────────────────────────
    FireSpreadRadiusAroundPlayer => {
        name: "fire_spread_radius_around_player",
        default: Int(128),
    },
    MaxBlockModifications => {
        name: "max_block_modifications",
        default: Int(32768),
    },
    MaxCommandForks => {
        name: "max_command_forks",
        legacy: "maxCommandForkCount",
        default: Int(65536),
    },
    MaxCommandSequenceLength => {
        name: "max_command_sequence_length",
        legacy: "maxCommandChainLength",
        default: Int(65536),
    },
    MaxEntityCramming => {
        name: "max_entity_cramming",
        legacy: "maxEntityCramming",
        default: Int(24),
    },
    MaxMinecartSpeed => {
        name: "max_minecart_speed",
        default: Int(8),
    },
    MaxSnowAccumulationHeight => {
        name: "max_snow_accumulation_height",
        legacy: "snowAccumulationHeight",
        default: Int(1),
    },
    PlayersNetherPortalCreativeDelay => {
        name: "players_nether_portal_creative_delay",
        legacy: "playersNetherPortalCreativeDelay",
        default: Int(0),
    },
    PlayersNetherPortalDefaultDelay => {
        name: "players_nether_portal_default_delay",
        legacy: "playersNetherPortalDefaultDelay",
        default: Int(80),
    },
    PlayersSleepingPercentage => {
        name: "players_sleeping_percentage",
        legacy: "playersSleepingPercentage",
        default: Int(100),
    },
    RandomTickSpeed => {
        name: "random_tick_speed",
        legacy: "randomTickSpeed",
        default: Int(3),
    },
    RespawnRadius => {
        name: "respawn_radius",
        legacy: "spawnRadius",
        default: Int(10),
    },
}

/// A game rule value — either boolean or integer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameRuleValue {
    /// A boolean flag.
    Bool(bool),
    /// An integer parameter.
    Int(i32),
}

impl GameRuleValue {
    /// Returns the value formatted as a string (for commands and NBT).
    pub fn as_string(&self) -> String {
        match self {
            Self::Bool(v) => v.to_string(),
            Self::Int(v) => v.to_string(),
        }
    }
}

/// Storage for all game rules with typed getters/setters.
///
/// Values are stored in a flat array indexed by [`GameRuleKey`] discriminant,
/// giving O(1) access with no hashing overhead.
#[derive(Debug, Clone)]
pub struct GameRules {
    values: Box<[GameRuleValue]>,
}

impl Default for GameRules {
    fn default() -> Self {
        let mut vals = vec![GameRuleValue::Bool(false); GameRuleKey::COUNT];
        for &key in GameRuleKey::all_sorted() {
            vals[key as u16 as usize] = key.default_value();
        }
        Self {
            values: vals.into_boxed_slice(),
        }
    }
}

impl GameRules {
    /// Creates a new [`GameRules`] with vanilla defaults.
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    fn idx(key: GameRuleKey) -> usize {
        key as u16 as usize
    }

    /// Returns a boolean rule's value, or `false` if the key is not boolean.
    pub fn get_bool(&self, key: GameRuleKey) -> bool {
        match &self.values[Self::idx(key)] {
            GameRuleValue::Bool(v) => *v,
            _ => false,
        }
    }

    /// Returns an integer rule's value, or `0` if the key is not integer.
    pub fn get_int(&self, key: GameRuleKey) -> i32 {
        match &self.values[Self::idx(key)] {
            GameRuleValue::Int(v) => *v,
            _ => 0,
        }
    }

    /// Sets a boolean rule.
    pub fn set_bool(&mut self, key: GameRuleKey, value: bool) {
        self.values[Self::idx(key)] = GameRuleValue::Bool(value);
    }

    /// Sets an integer rule.
    pub fn set_int(&mut self, key: GameRuleKey, value: i32) {
        self.values[Self::idx(key)] = GameRuleValue::Int(value);
    }

    /// Returns the raw value for a rule.
    pub fn get(&self, key: GameRuleKey) -> &GameRuleValue {
        &self.values[Self::idx(key)]
    }

    /// Returns the value of a rule as a displayable string.
    pub fn get_as_string(&self, key: GameRuleKey) -> String {
        self.values[Self::idx(key)].as_string()
    }

    /// Sets a rule from a string value. Returns `Err` if the value is invalid
    /// for the rule's type.
    pub fn set_from_string(&mut self, key: GameRuleKey, value: &str) -> Result<(), String> {
        match &self.values[Self::idx(key)] {
            GameRuleValue::Bool(_) => {
                let b = value
                    .parse::<bool>()
                    .map_err(|_| format!("expected 'true' or 'false', got '{value}'"))?;
                self.set_bool(key, b);
                Ok(())
            },
            GameRuleValue::Int(_) => {
                let n = value
                    .parse::<i32>()
                    .map_err(|_| format!("expected integer, got '{value}'"))?;
                self.set_int(key, n);
                Ok(())
            },
        }
    }

    // ── Convenience aliases for the old API (delegates to GameRuleKey) ──

    /// Returns the 26.1 snake_case name for a rule.
    pub fn name_of(key: GameRuleKey) -> &'static str {
        key.name()
    }

    /// Resolves a name (26.1 or legacy) to a [`GameRuleKey`].
    pub fn from_name(name: &str) -> Option<GameRuleKey> {
        GameRuleKey::from_name(name)
    }

    /// Returns all rule names sorted alphabetically.
    pub fn all_names() -> Vec<&'static str> {
        GameRuleKey::all_names()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_match_vanilla() {
        let rules = GameRules::default();
        assert!(rules.get_bool(GameRuleKey::AdvanceTime));
        assert!(rules.get_bool(GameRuleKey::AdvanceWeather));
        assert!(!rules.get_bool(GameRuleKey::KeepInventory));
        assert!(!rules.get_bool(GameRuleKey::ImmediateRespawn));
        assert_eq!(rules.get_int(GameRuleKey::RandomTickSpeed), 3);
        assert_eq!(rules.get_int(GameRuleKey::RespawnRadius), 10);
        assert_eq!(rules.get_int(GameRuleKey::MaxEntityCramming), 24);
        assert_eq!(rules.get_int(GameRuleKey::PlayersSleepingPercentage), 100);
    }

    #[test]
    fn test_set_bool_updates_value() {
        let mut rules = GameRules::default();
        rules.set_bool(GameRuleKey::KeepInventory, true);
        assert!(rules.get_bool(GameRuleKey::KeepInventory));
    }

    #[test]
    fn test_set_int_updates_value() {
        let mut rules = GameRules::default();
        rules.set_int(GameRuleKey::RandomTickSpeed, 10);
        assert_eq!(rules.get_int(GameRuleKey::RandomTickSpeed), 10);
    }

    #[test]
    fn test_name_roundtrips_all_keys() {
        let all_names = GameRules::all_names();
        for name in &all_names {
            let key = GameRules::from_name(name)
                .unwrap_or_else(|| panic!("from_name failed for '{name}'"));
            assert_eq!(
                GameRules::name_of(key),
                *name,
                "name_of roundtrip failed for '{name}'"
            );
        }
    }

    #[test]
    fn test_legacy_names_resolve() {
        assert_eq!(
            GameRuleKey::from_name("doDaylightCycle"),
            Some(GameRuleKey::AdvanceTime)
        );
        assert_eq!(
            GameRuleKey::from_name("doWeatherCycle"),
            Some(GameRuleKey::AdvanceWeather)
        );
        assert_eq!(
            GameRuleKey::from_name("doMobSpawning"),
            Some(GameRuleKey::SpawnMobs)
        );
        assert_eq!(
            GameRuleKey::from_name("keepInventory"),
            Some(GameRuleKey::KeepInventory)
        );
        assert_eq!(
            GameRuleKey::from_name("randomTickSpeed"),
            Some(GameRuleKey::RandomTickSpeed)
        );
    }

    #[test]
    fn test_from_name_unknown_returns_none() {
        assert!(GameRules::from_name("notARealRule").is_none());
    }

    #[test]
    fn test_set_from_string_bool() {
        let mut rules = GameRules::default();
        rules
            .set_from_string(GameRuleKey::KeepInventory, "true")
            .unwrap();
        assert!(rules.get_bool(GameRuleKey::KeepInventory));
    }

    #[test]
    fn test_set_from_string_int() {
        let mut rules = GameRules::default();
        rules
            .set_from_string(GameRuleKey::RandomTickSpeed, "10")
            .unwrap();
        assert_eq!(rules.get_int(GameRuleKey::RandomTickSpeed), 10);
    }

    #[test]
    fn test_set_from_string_invalid() {
        let mut rules = GameRules::default();
        assert!(
            rules
                .set_from_string(GameRuleKey::KeepInventory, "notbool")
                .is_err()
        );
        assert!(
            rules
                .set_from_string(GameRuleKey::RandomTickSpeed, "notint")
                .is_err()
        );
    }

    #[test]
    fn test_get_as_string() {
        let rules = GameRules::default();
        assert_eq!(rules.get_as_string(GameRuleKey::AdvanceTime), "true");
        assert_eq!(rules.get_as_string(GameRuleKey::RandomTickSpeed), "3");
    }

    #[test]
    fn test_all_names_sorted() {
        let names = GameRules::all_names();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "all_names() should return sorted names");
    }

    #[test]
    fn test_rule_count() {
        // 46 boolean + 12 integer = 58 total rules in vanilla 26.1
        let count = GameRuleKey::COUNT;
        assert!(count >= 50, "expected at least 50 rules, got {count}");
    }

    #[test]
    fn test_array_storage_correct() {
        // Every key can be accessed without panic.
        let rules = GameRules::default();
        for key in GameRuleKey::all_sorted() {
            let _ = rules.get(*key);
        }
    }
}
