//! Game rule system — typed boolean and integer rules with vanilla defaults.
//!
//! Each rule has a camelCase string name for NBT serialisation and the
//! `/gamerule` command. The [`GameRules`] struct stores current values and
//! provides typed getters/setters.
//!
//! Corresponds to `net.minecraft.world.level.gamerules.GameRules`.

use std::collections::HashMap;

/// Identifies a specific game rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameRuleKey {
    // Boolean rules
    /// Advance the day/night cycle.
    DoDaylightCycle,
    /// Advance weather transitions.
    DoWeatherCycle,
    /// Allow natural mob spawning.
    DoMobSpawning,
    /// Mobs drop loot on death.
    DoMobLoot,
    /// Blocks drop items when broken.
    DoTileDrops,
    /// Fire spreads and destroys blocks.
    DoFireTick,
    /// Mobs can modify blocks (creeper explosions, enderman griefing, etc.).
    MobGriefing,
    /// Players keep inventory on death.
    KeepInventory,
    /// Players regenerate health naturally.
    NaturalRegeneration,
    /// Players take fall damage.
    FallDamage,
    /// Show death messages in chat.
    ShowDeathMessages,
    /// Log admin commands to server log.
    LogAdminCommands,
    /// Command blocks produce output.
    CommandBlockOutput,
    /// Commands send feedback to the executing player.
    SendCommandFeedback,
    /// Players can only craft recipes they have unlocked.
    DoLimitedCrafting,
    /// Entities other than mobs drop items.
    DoEntityDrops,
    /// Player-vs-player combat is allowed.
    Pvp,
    /// Entities deal fire damage when on fire.
    DoFireDamage,
    /// Drowning damage is applied.
    DoDrowningDamage,
    /// Freeze damage is applied.
    DoFreezeDamage,
    /// Immediate respawn without death screen.
    DoImmediateRespawn,
    /// Forgive angry neutral mobs when the target player dies.
    ForgiveDeadPlayers,
    /// Angry neutral mobs attack any nearby player.
    UniversalAnger,
    /// Players can sleep during thunderstorms.
    DoInsomnia,
    /// Patrol and wandering trader spawning.
    DoPatrolSpawning,
    /// Wandering trader spawning.
    DoTraderSpawning,
    /// Wardens spawn from sculk shriekers.
    DoWardenSpawning,
    /// TNT explodes.
    TntExplodes,
    /// Block explosions drop items.
    BlockExplosionDropDecay,
    /// Mob explosions drop items.
    MobExplosionDropDecay,
    /// TNT explosions drop items.
    TntExplosionDropDecay,
    /// Show coordinates on death screen.
    ShowCoordinates,

    // Integer rules
    /// Number of random ticks per chunk section per game tick.
    RandomTickSpeed,
    /// Radius around the world spawn where players initially spawn.
    SpawnRadius,
    /// Max entities pushed into the same space before suffocation.
    MaxEntityCramming,
    /// Max length of a command chain (command blocks).
    MaxCommandChainLength,
    /// Default nether portal cooldown in ticks (survival).
    PlayersNetherPortalDefaultDelay,
    /// Nether portal cooldown in ticks (creative).
    PlayersNetherPortalCreativeDelay,
    /// Percentage of players that must sleep to skip the night.
    PlayersSleepingPercentage,
    /// Max distance from snow layer to ground for snow to form.
    SnowAccumulationHeight,
    /// Max length of a command block output string.
    MaxCommandForkCount,
    /// How many entity collisions an entity processes per tick.
    SpawnChunkRadius,
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
#[derive(Debug, Clone)]
pub struct GameRules {
    values: HashMap<GameRuleKey, GameRuleValue>,
}

impl Default for GameRules {
    fn default() -> Self {
        use GameRuleKey::*;
        use GameRuleValue::{Bool, Int};

        let entries: &[(GameRuleKey, GameRuleValue)] = &[
            // Boolean defaults
            (DoDaylightCycle, Bool(true)),
            (DoWeatherCycle, Bool(true)),
            (DoMobSpawning, Bool(true)),
            (DoMobLoot, Bool(true)),
            (DoTileDrops, Bool(true)),
            (DoFireTick, Bool(true)),
            (MobGriefing, Bool(true)),
            (KeepInventory, Bool(false)),
            (NaturalRegeneration, Bool(true)),
            (FallDamage, Bool(true)),
            (ShowDeathMessages, Bool(true)),
            (LogAdminCommands, Bool(true)),
            (CommandBlockOutput, Bool(true)),
            (SendCommandFeedback, Bool(true)),
            (DoLimitedCrafting, Bool(false)),
            (DoEntityDrops, Bool(true)),
            (Pvp, Bool(true)),
            (DoFireDamage, Bool(true)),
            (DoDrowningDamage, Bool(true)),
            (DoFreezeDamage, Bool(true)),
            (DoImmediateRespawn, Bool(false)),
            (ForgiveDeadPlayers, Bool(true)),
            (UniversalAnger, Bool(false)),
            (DoInsomnia, Bool(true)),
            (DoPatrolSpawning, Bool(true)),
            (DoTraderSpawning, Bool(true)),
            (DoWardenSpawning, Bool(true)),
            (TntExplodes, Bool(true)),
            (BlockExplosionDropDecay, Bool(true)),
            (MobExplosionDropDecay, Bool(true)),
            (TntExplosionDropDecay, Bool(false)),
            (ShowCoordinates, Bool(true)),
            // Integer defaults
            (RandomTickSpeed, Int(3)),
            (SpawnRadius, Int(8)),
            (MaxEntityCramming, Int(24)),
            (MaxCommandChainLength, Int(65536)),
            (PlayersNetherPortalDefaultDelay, Int(80)),
            (PlayersNetherPortalCreativeDelay, Int(1)),
            (PlayersSleepingPercentage, Int(100)),
            (SnowAccumulationHeight, Int(1)),
            (MaxCommandForkCount, Int(65536)),
            (SpawnChunkRadius, Int(2)),
        ];

        let mut values = HashMap::with_capacity(entries.len());
        for (key, value) in entries {
            values.insert(*key, value.clone());
        }
        Self { values }
    }
}

impl GameRules {
    /// Creates a new [`GameRules`] with vanilla defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a boolean rule's value, or `false` if the key is not boolean.
    pub fn get_bool(&self, key: GameRuleKey) -> bool {
        match self.values.get(&key) {
            Some(GameRuleValue::Bool(v)) => *v,
            _ => false,
        }
    }

    /// Returns an integer rule's value, or `0` if the key is not integer.
    pub fn get_int(&self, key: GameRuleKey) -> i32 {
        match self.values.get(&key) {
            Some(GameRuleValue::Int(v)) => *v,
            _ => 0,
        }
    }

    /// Sets a boolean rule.
    pub fn set_bool(&mut self, key: GameRuleKey, value: bool) {
        self.values.insert(key, GameRuleValue::Bool(value));
    }

    /// Sets an integer rule.
    pub fn set_int(&mut self, key: GameRuleKey, value: i32) {
        self.values.insert(key, GameRuleValue::Int(value));
    }

    /// Returns the raw value for a rule, if present.
    pub fn get(&self, key: GameRuleKey) -> Option<&GameRuleValue> {
        self.values.get(&key)
    }

    /// Returns the value of a rule as a displayable string.
    pub fn get_as_string(&self, key: GameRuleKey) -> String {
        self.values
            .get(&key)
            .map_or_else(String::new, GameRuleValue::as_string)
    }

    /// Sets a rule from a string value. Returns `Err` if the value is invalid
    /// for the rule's type.
    pub fn set_from_string(&mut self, key: GameRuleKey, value: &str) -> Result<(), String> {
        match self.values.get(&key) {
            Some(GameRuleValue::Bool(_)) => {
                let b = value
                    .parse::<bool>()
                    .map_err(|_| format!("expected 'true' or 'false', got '{value}'"))?;
                self.set_bool(key, b);
                Ok(())
            },
            Some(GameRuleValue::Int(_)) => {
                let n = value
                    .parse::<i32>()
                    .map_err(|_| format!("expected integer, got '{value}'"))?;
                self.set_int(key, n);
                Ok(())
            },
            None => Err(format!("unknown game rule key: {key:?}")),
        }
    }

    /// Returns the camelCase vanilla name for a rule (used in NBT and `/gamerule`).
    pub fn name_of(key: GameRuleKey) -> &'static str {
        use GameRuleKey::*;
        match key {
            DoDaylightCycle => "doDaylightCycle",
            DoWeatherCycle => "doWeatherCycle",
            DoMobSpawning => "doMobSpawning",
            DoMobLoot => "doMobLoot",
            DoTileDrops => "doTileDrops",
            DoFireTick => "doFireTick",
            MobGriefing => "mobGriefing",
            KeepInventory => "keepInventory",
            NaturalRegeneration => "naturalRegeneration",
            FallDamage => "fallDamage",
            ShowDeathMessages => "showDeathMessages",
            LogAdminCommands => "logAdminCommands",
            CommandBlockOutput => "commandBlockOutput",
            SendCommandFeedback => "sendCommandFeedback",
            DoLimitedCrafting => "doLimitedCrafting",
            DoEntityDrops => "doEntityDrops",
            Pvp => "pvp",
            DoFireDamage => "doFireDamage",
            DoDrowningDamage => "doDrowningDamage",
            DoFreezeDamage => "doFreezeDamage",
            DoImmediateRespawn => "doImmediateRespawn",
            ForgiveDeadPlayers => "forgiveDeadPlayers",
            UniversalAnger => "universalAnger",
            DoInsomnia => "doInsomnia",
            DoPatrolSpawning => "doPatrolSpawning",
            DoTraderSpawning => "doTraderSpawning",
            DoWardenSpawning => "doWardenSpawning",
            TntExplodes => "tntExplodes",
            BlockExplosionDropDecay => "blockExplosionDropDecay",
            MobExplosionDropDecay => "mobExplosionDropDecay",
            TntExplosionDropDecay => "tntExplosionDropDecay",
            ShowCoordinates => "showCoordinates",
            RandomTickSpeed => "randomTickSpeed",
            SpawnRadius => "spawnRadius",
            MaxEntityCramming => "maxEntityCramming",
            MaxCommandChainLength => "maxCommandChainLength",
            PlayersNetherPortalDefaultDelay => "playersNetherPortalDefaultDelay",
            PlayersNetherPortalCreativeDelay => "playersNetherPortalCreativeDelay",
            PlayersSleepingPercentage => "playersSleepingPercentage",
            SnowAccumulationHeight => "snowAccumulationHeight",
            MaxCommandForkCount => "maxCommandForkCount",
            SpawnChunkRadius => "spawnChunkRadius",
        }
    }

    /// Resolves a camelCase vanilla name to a [`GameRuleKey`].
    pub fn from_name(name: &str) -> Option<GameRuleKey> {
        use GameRuleKey::*;
        Some(match name {
            "doDaylightCycle" => DoDaylightCycle,
            "doWeatherCycle" => DoWeatherCycle,
            "doMobSpawning" => DoMobSpawning,
            "doMobLoot" => DoMobLoot,
            "doTileDrops" => DoTileDrops,
            "doFireTick" => DoFireTick,
            "mobGriefing" => MobGriefing,
            "keepInventory" => KeepInventory,
            "naturalRegeneration" => NaturalRegeneration,
            "fallDamage" => FallDamage,
            "showDeathMessages" => ShowDeathMessages,
            "logAdminCommands" => LogAdminCommands,
            "commandBlockOutput" => CommandBlockOutput,
            "sendCommandFeedback" => SendCommandFeedback,
            "doLimitedCrafting" => DoLimitedCrafting,
            "doEntityDrops" => DoEntityDrops,
            "pvp" => Pvp,
            "doFireDamage" => DoFireDamage,
            "doDrowningDamage" => DoDrowningDamage,
            "doFreezeDamage" => DoFreezeDamage,
            "doImmediateRespawn" => DoImmediateRespawn,
            "forgiveDeadPlayers" => ForgiveDeadPlayers,
            "universalAnger" => UniversalAnger,
            "doInsomnia" => DoInsomnia,
            "doPatrolSpawning" => DoPatrolSpawning,
            "doTraderSpawning" => DoTraderSpawning,
            "doWardenSpawning" => DoWardenSpawning,
            "tntExplodes" => TntExplodes,
            "blockExplosionDropDecay" => BlockExplosionDropDecay,
            "mobExplosionDropDecay" => MobExplosionDropDecay,
            "tntExplosionDropDecay" => TntExplosionDropDecay,
            "showCoordinates" => ShowCoordinates,
            "randomTickSpeed" => RandomTickSpeed,
            "spawnRadius" => SpawnRadius,
            "maxEntityCramming" => MaxEntityCramming,
            "maxCommandChainLength" => MaxCommandChainLength,
            "playersNetherPortalDefaultDelay" => PlayersNetherPortalDefaultDelay,
            "playersNetherPortalCreativeDelay" => PlayersNetherPortalCreativeDelay,
            "playersSleepingPercentage" => PlayersSleepingPercentage,
            "snowAccumulationHeight" => SnowAccumulationHeight,
            "maxCommandForkCount" => MaxCommandForkCount,
            "spawnChunkRadius" => SpawnChunkRadius,
            _ => return None,
        })
    }

    /// Returns all rule names sorted alphabetically.
    pub fn all_names() -> Vec<&'static str> {
        use GameRuleKey::*;
        let keys = [
            BlockExplosionDropDecay,
            CommandBlockOutput,
            DoDaylightCycle,
            DoDrowningDamage,
            DoEntityDrops,
            DoFireDamage,
            DoFireTick,
            DoFreezeDamage,
            DoImmediateRespawn,
            DoInsomnia,
            DoLimitedCrafting,
            DoMobLoot,
            DoMobSpawning,
            DoPatrolSpawning,
            DoTileDrops,
            DoTraderSpawning,
            DoWardenSpawning,
            DoWeatherCycle,
            FallDamage,
            ForgiveDeadPlayers,
            KeepInventory,
            LogAdminCommands,
            MaxCommandChainLength,
            MaxCommandForkCount,
            MaxEntityCramming,
            MobExplosionDropDecay,
            MobGriefing,
            NaturalRegeneration,
            PlayersNetherPortalCreativeDelay,
            PlayersNetherPortalDefaultDelay,
            PlayersSleepingPercentage,
            Pvp,
            RandomTickSpeed,
            SendCommandFeedback,
            ShowCoordinates,
            ShowDeathMessages,
            SnowAccumulationHeight,
            SpawnChunkRadius,
            SpawnRadius,
            TntExplodes,
            TntExplosionDropDecay,
            UniversalAnger,
        ];
        keys.iter().map(|k| Self::name_of(*k)).collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_match_vanilla() {
        let rules = GameRules::default();
        assert!(rules.get_bool(GameRuleKey::DoDaylightCycle));
        assert!(rules.get_bool(GameRuleKey::DoWeatherCycle));
        assert!(!rules.get_bool(GameRuleKey::KeepInventory));
        assert!(!rules.get_bool(GameRuleKey::DoImmediateRespawn));
        assert_eq!(rules.get_int(GameRuleKey::RandomTickSpeed), 3);
        assert_eq!(rules.get_int(GameRuleKey::SpawnRadius), 8);
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
        assert_eq!(rules.get_as_string(GameRuleKey::DoDaylightCycle), "true");
        assert_eq!(rules.get_as_string(GameRuleKey::RandomTickSpeed), "3");
    }

    #[test]
    fn test_all_names_sorted() {
        let names = GameRules::all_names();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "all_names() should return sorted names");
    }
}
