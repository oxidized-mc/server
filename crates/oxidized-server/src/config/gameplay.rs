//! Gameplay-related configuration.

use serde::{Deserialize, Serialize};

/// Per-category entity tracking ranges in blocks.
///
/// Controls how far away entities are visible to players. Higher values
/// increase bandwidth and CPU usage but improve player experience.
/// Values are in blocks (1 chunk = 16 blocks).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct EntityTrackingConfig {
    /// Tracking range for players (default `512` = 32 chunks).
    pub player: i32,
    /// Tracking range for animals — cows, pigs, chickens, etc. (default `160` = 10 chunks).
    pub animal: i32,
    /// Tracking range for hostile mobs — zombies, skeletons, etc. (default `128` = 8 chunks).
    pub monster: i32,
    /// Tracking range for miscellaneous entities — items, XP orbs (default `96` = 6 chunks).
    pub misc: i32,
    /// Tracking range for projectiles — arrows, fireballs (default `64` = 4 chunks).
    pub projectile: i32,
    /// Default tracking range when entity type is unspecified (default `80` = 5 chunks).
    pub default: i32,
}

impl Default for EntityTrackingConfig {
    fn default() -> Self {
        Self {
            player: 512,
            animal: 160,
            monster: 128,
            misc: 96,
            projectile: 64,
            default: 80,
        }
    }
}

/// Weather cycle timing in ticks (20 ticks = 1 second).
///
/// Controls how long the server waits before starting/stopping rain
/// and thunder. Validate that min ≤ max for each pair at config load time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WeatherConfig {
    /// Minimum delay before rain starts (default `12000` ticks ≈ 10 min).
    pub rain_delay_min: i32,
    /// Maximum delay before rain starts (default `180000` ticks ≈ 2.5 hours).
    pub rain_delay_max: i32,
    /// Minimum rain duration (default `12000` ticks ≈ 10 min).
    pub rain_duration_min: i32,
    /// Maximum rain duration (default `24000` ticks ≈ 20 min).
    pub rain_duration_max: i32,
    /// Minimum delay before thunder starts (default `12000` ticks ≈ 10 min).
    pub thunder_delay_min: i32,
    /// Maximum delay before thunder starts (default `180000` ticks ≈ 2.5 hours).
    pub thunder_delay_max: i32,
    /// Minimum thunder duration (default `3600` ticks ≈ 3 min).
    pub thunder_duration_min: i32,
    /// Maximum thunder duration (default `15600` ticks ≈ 13 min).
    pub thunder_duration_max: i32,
}

impl Default for WeatherConfig {
    fn default() -> Self {
        Self {
            rain_delay_min: 12_000,
            rain_delay_max: 180_000,
            rain_duration_min: 12_000,
            rain_duration_max: 24_000,
            thunder_delay_min: 12_000,
            thunder_delay_max: 180_000,
            thunder_duration_min: 3_600,
            thunder_duration_max: 15_600,
        }
    }
}

/// Gameplay-related settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GameplayConfig {
    /// Default game mode (default `"survival"`).
    pub gamemode: String,
    /// World difficulty (default `"easy"`).
    pub difficulty: String,
    /// Hardcore mode — one life (default `false`).
    pub is_hardcore: bool,
    /// Force players into the default game mode on join (default `false`).
    pub is_force_gamemode: bool,
    /// Maximum concurrent players (default `20`).
    pub max_players: u32,
    /// Radius around spawn where non-ops cannot build (default `16`).
    pub spawn_protection: u32,
    /// Maximum world border radius in blocks (default `29999984`).
    pub max_world_size: i32,
    /// Allow players to fly in survival (default `false`).
    pub is_flight_allowed: bool,
    /// Spawn NPC villagers (default `true`).
    pub is_spawning_npcs: bool,
    /// Spawn passive animals (default `true`).
    pub is_spawning_animals: bool,
    /// Spawn hostile mobs (default `true`).
    pub is_spawning_monsters: bool,
    /// Enable the Nether dimension (default `true`).
    pub is_nether_allowed: bool,
    /// Limit on chained neighbour block updates (default `1000000`).
    pub max_chained_neighbor_updates: i32,
    /// Whether player-vs-player combat is enabled (default `true`).
    pub is_pvp_enabled: bool,
    /// Per-category entity tracking ranges.
    pub entity_tracking: EntityTrackingConfig,
    /// Weather cycle timing.
    pub weather: WeatherConfig,
}

impl Default for GameplayConfig {
    fn default() -> Self {
        Self {
            gamemode: "survival".to_string(),
            difficulty: "easy".to_string(),
            is_hardcore: false,
            is_force_gamemode: false,
            max_players: 20,
            spawn_protection: 16,
            max_world_size: 29_999_984,
            is_flight_allowed: false,
            is_spawning_npcs: true,
            is_spawning_animals: true,
            is_spawning_monsters: true,
            is_nether_allowed: true,
            max_chained_neighbor_updates: 1_000_000,
            is_pvp_enabled: true,
            entity_tracking: EntityTrackingConfig::default(),
            weather: WeatherConfig::default(),
        }
    }
}
