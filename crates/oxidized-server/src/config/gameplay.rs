//! Gameplay-related configuration.

use serde::{Deserialize, Serialize};

/// Gameplay-related settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GameplayConfig {
    /// Default game mode (default `"survival"`).
    pub gamemode: String,
    /// World difficulty (default `"easy"`).
    pub difficulty: String,
    /// Hardcore mode — one life (default `false`).
    pub hardcore: bool,
    /// Force players into the default game mode on join (default `false`).
    pub force_gamemode: bool,
    /// Maximum concurrent players (default `20`).
    pub max_players: u32,
    /// Radius around spawn where non-ops cannot build (default `16`).
    pub spawn_protection: u32,
    /// Maximum world border radius in blocks (default `29999984`).
    pub max_world_size: i32,
    /// Allow players to fly in survival (default `false`).
    pub allow_flight: bool,
    /// Spawn NPC villagers (default `true`).
    pub spawn_npcs: bool,
    /// Spawn passive animals (default `true`).
    pub spawn_animals: bool,
    /// Spawn hostile mobs (default `true`).
    pub spawn_monsters: bool,
    /// Enable the Nether dimension (default `true`).
    pub allow_nether: bool,
    /// Limit on chained neighbour block updates (default `1000000`).
    pub max_chained_neighbor_updates: i32,
    /// Whether player-vs-player combat is enabled (default `true`).
    pub pvp: bool,
}

impl Default for GameplayConfig {
    fn default() -> Self {
        Self {
            gamemode: "survival".to_string(),
            difficulty: "easy".to_string(),
            hardcore: false,
            force_gamemode: false,
            max_players: 20,
            spawn_protection: 16,
            max_world_size: 29_999_984,
            allow_flight: false,
            spawn_npcs: true,
            spawn_animals: true,
            spawn_monsters: true,
            allow_nether: true,
            max_chained_neighbor_updates: 1_000_000,
            pvp: true,
        }
    }
}
