//! Server configuration loaded from `oxidized.toml`.
//!
//! Uses TOML format with serde derives for type-safe deserialization.
//! See [ADR-033](../../../docs/adr/adr-033-configuration-format.md) for rationale.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ConfigError
// ---------------------------------------------------------------------------

/// Errors that can occur when validating server configuration.
#[derive(Debug, thiserror::Error)]
#[allow(clippy::enum_variant_names)]
pub enum ConfigError {
    /// Port number is out of valid range (1-65535).
    #[error("invalid port: {0} (must be 1-65535)")]
    InvalidPort(u16),

    /// View distance is out of valid range (2-32).
    #[error("invalid view distance: {0} (must be 2-32)")]
    InvalidViewDistance(u32),

    /// Simulation distance is out of valid range (2-32).
    #[error("invalid simulation distance: {0} (must be 2-32)")]
    InvalidSimulationDistance(u32),

    /// Max players must be positive.
    #[error("invalid max players: {0} (must be 1+)")]
    InvalidMaxPlayers(u32),
}

// ---------------------------------------------------------------------------
// Sub-config structs
// ---------------------------------------------------------------------------

/// Network-related settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NetworkConfig {
    /// Port the server listens on (default `25565`).
    pub port: u16,
    /// IP address to bind to (default `""` — all interfaces).
    pub ip: String,
    /// Whether the server authenticates players with Mojang (default `true`).
    pub online_mode: bool,
    /// Block proxy / VPN connections (default `false`).
    pub prevent_proxy_connections: bool,
    /// Byte threshold for packet compression; `-1` disables (default `256`).
    pub compression_threshold: i32,
    /// Use epoll/kqueue native transport (default `true`).
    pub use_native_transport: bool,
    /// Maximum packets per second before kicking; `0` disables (default `0`).
    pub rate_limit: i32,
    /// Accept transfer packets from other servers (default `false`).
    pub accepts_transfers: bool,
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

/// World generation and storage settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WorldConfig {
    /// Name of the world folder (default `"world"`).
    pub name: String,
    /// World seed; empty means random (default `""`).
    pub seed: String,
    /// Generate structures such as villages (default `true`).
    pub generate_structures: bool,
    /// Chunk view distance (default `10`).
    pub view_distance: u32,
    /// Simulation distance in chunks (default `10`).
    pub simulation_distance: u32,
    /// Synchronous chunk writes for data safety (default `true`).
    pub sync_chunk_writes: bool,
    /// Region file compression algorithm (default `"deflate"`).
    pub region_file_compression: String,
}

/// Display and MOTD settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DisplayConfig {
    /// Message of the day shown in the server list.
    pub motd: String,
    /// Respond to server-list pings (default `true`).
    pub enable_status: bool,
    /// Hide player names from the server list (default `false`).
    pub hide_online_players: bool,
    /// Entity tracking range as a percentage (default `100`).
    pub entity_broadcast_range_percentage: i32,
    /// Heartbeat interval in seconds for status polling (default `5`).
    pub status_heartbeat_interval: i32,
}

/// Admin and security settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AdminConfig {
    /// Enable the server whitelist (default `false`).
    pub white_list: bool,
    /// Kick non-whitelisted players immediately on reload (default `false`).
    pub enforce_whitelist: bool,
    /// Default permission level for ops (default `4`).
    pub op_permission_level: i32,
    /// Permission level for function commands (default `2`).
    pub function_permission_level: i32,
    /// Require signed chat profiles (default `true`).
    pub enforce_secure_profile: bool,
    /// Log player IP addresses (default `true`).
    pub log_ips: bool,
    /// Max time a single tick may take in ms before watchdog kills the server (default `60000`).
    pub max_tick_time: i64,
    /// Minutes before idle players are kicked; `0` disables (default `0`).
    pub player_idle_timeout: i32,
    /// Broadcast console commands to online ops (default `true`).
    pub broadcast_console_to_ops: bool,
    /// Broadcast RCON output to online ops (default `true`).
    pub broadcast_rcon_to_ops: bool,
    /// Seconds to pause the game loop when the server is empty (default `60`).
    pub pause_when_empty_seconds: i32,
}

/// RCON remote console settings.
///
/// Implements a custom [`Debug`] that redacts `password`.
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RconConfig {
    /// Enable the RCON remote console (default `false`).
    pub enabled: bool,
    /// RCON listening port (default `25575`).
    pub port: u16,
    /// RCON password (default `""`).
    pub password: String,
}

impl fmt::Debug for RconConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RconConfig")
            .field("enabled", &self.enabled)
            .field("port", &self.port)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

/// GameSpy4 query protocol settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct QueryConfig {
    /// Enable the GameSpy4 query protocol (default `false`).
    pub enabled: bool,
    /// Query protocol port (default `25565`).
    pub port: u16,
}

/// Server resource pack settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct ResourcePackConfig {
    /// URL of the server resource pack (default `""`).
    pub url: String,
    /// SHA-1 hash of the resource pack (default `""`).
    pub sha1: String,
    /// Prompt shown to players for the resource pack (default `""`).
    pub prompt: String,
    /// Whether the resource pack is mandatory (default `false`).
    pub required: bool,
}

/// Management server settings (26.1 feature).
///
/// Implements a custom [`Debug`] that redacts `secret`.
#[derive(Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct ManagementConfig {
    /// Enable the management server (default `false`).
    pub enabled: bool,
    /// Management server host (default `""`).
    pub host: String,
    /// Management server port; `0` means auto-assign (default `0`).
    pub port: u16,
    /// Shared secret for the management server (default `""`).
    pub secret: String,
    /// Require TLS on the management server (default `false`).
    pub tls_enabled: bool,
}

impl fmt::Debug for ManagementConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ManagementConfig")
            .field("enabled", &self.enabled)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("secret", &"[REDACTED]")
            .field("tls_enabled", &self.tls_enabled)
            .finish()
    }
}

/// Data pack settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PacksConfig {
    /// Data packs enabled at startup (default `"vanilla"`).
    pub initial_enabled: String,
    /// Data packs disabled at startup (default `""`).
    pub initial_disabled: String,
}

impl Default for PacksConfig {
    fn default() -> Self {
        Self {
            initial_enabled: "vanilla".to_string(),
            initial_disabled: String::new(),
        }
    }
}

/// Advanced / diagnostic settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct AdvancedConfig {
    /// Enable JMX monitoring beans (default `false`).
    pub enable_jmx_monitoring: bool,
    /// External text-filtering service config path (default `""`).
    pub text_filtering_config: String,
    /// Text-filtering protocol version (default `0`).
    pub text_filtering_version: i32,
    /// Show a code-of-conduct prompt on join (default `false`).
    pub enable_code_of_conduct: bool,
    /// Link to the server's bug report page (default `""`).
    pub bug_report_link: String,
}

// ---------------------------------------------------------------------------
// Default implementations
// ---------------------------------------------------------------------------

impl std::default::Default for NetworkConfig {
    fn default() -> Self {
        Self {
            port: 25565,
            ip: String::new(),
            online_mode: true,
            prevent_proxy_connections: false,
            compression_threshold: 256,
            use_native_transport: true,
            rate_limit: 0,
            accepts_transfers: false,
        }
    }
}

impl std::default::Default for GameplayConfig {
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

impl std::default::Default for WorldConfig {
    fn default() -> Self {
        Self {
            name: "world".to_string(),
            seed: String::new(),
            generate_structures: true,
            view_distance: 10,
            simulation_distance: 10,
            sync_chunk_writes: true,
            region_file_compression: "deflate".to_string(),
        }
    }
}

impl std::default::Default for DisplayConfig {
    fn default() -> Self {
        Self {
            motd: "An Oxidized Minecraft Server".to_string(),
            enable_status: true,
            hide_online_players: false,
            entity_broadcast_range_percentage: 100,
            status_heartbeat_interval: 5,
        }
    }
}

impl std::default::Default for AdminConfig {
    fn default() -> Self {
        Self {
            white_list: false,
            enforce_whitelist: false,
            op_permission_level: 4,
            function_permission_level: 2,
            enforce_secure_profile: true,
            log_ips: true,
            max_tick_time: 60_000,
            player_idle_timeout: 0,
            broadcast_console_to_ops: true,
            broadcast_rcon_to_ops: true,
            pause_when_empty_seconds: 60,
        }
    }
}

impl std::default::Default for RconConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 25575,
            password: String::new(),
        }
    }
}

impl std::default::Default for QueryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 25565,
        }
    }
}

// ---------------------------------------------------------------------------
// ServerConfig
// ---------------------------------------------------------------------------

/// Server configuration loaded from `oxidized.toml`.
///
/// All default values match the vanilla Minecraft 26.1-pre-3 server
/// (with Oxidized-specific branding for the MOTD).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct ServerConfig {
    /// Network-related settings.
    pub network: NetworkConfig,
    /// Gameplay-related settings.
    pub gameplay: GameplayConfig,
    /// World generation and storage settings.
    pub world: WorldConfig,
    /// Display and MOTD settings.
    pub display: DisplayConfig,
    /// Admin and security settings.
    pub admin: AdminConfig,
    /// RCON remote console settings.
    pub rcon: RconConfig,
    /// GameSpy4 query protocol settings.
    pub query: QueryConfig,
    /// Server resource pack settings.
    pub resource_pack: ResourcePackConfig,
    /// Management server settings.
    pub management: ManagementConfig,
    /// Data pack settings.
    pub packs: PacksConfig,
    /// Advanced / diagnostic settings.
    pub advanced: AdvancedConfig,
    /// Unknown/future keys preserved for forward compatibility.
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

impl ServerConfig {
    /// Loads configuration from a TOML file, or creates a default if the file doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be read or parsed.
    pub fn load_or_create(path: &Path) -> anyhow::Result<Self> {
        if path.exists() {
            let content = fs::read_to_string(path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let config: Self = toml::from_str(&content)
                .with_context(|| format!("failed to parse {}", path.display()))?;
            Ok(config)
        } else {
            let config = Self::default();
            config.save(path)?;
            Ok(config)
        }
    }

    /// Saves the configuration to a TOML file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self).context("failed to serialize configuration")?;
        let output = format!(
            "# Oxidized Minecraft Server Configuration\n\
             # Generated by Oxidized v{}\n\
             # See https://github.com/dodoflix/Oxidized for documentation\n\n{}",
            env!("CARGO_PKG_VERSION"),
            content
        );
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create parent directories for {}", path.display())
            })?;
        }
        fs::write(path, output).with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }

    /// Validates all config values are within acceptable ranges.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigError`] describing the first invalid value found.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.network.port == 0 {
            return Err(ConfigError::InvalidPort(self.network.port));
        }
        if self.rcon.port == 0 && self.rcon.enabled {
            return Err(ConfigError::InvalidPort(self.rcon.port));
        }
        if self.query.port == 0 && self.query.enabled {
            return Err(ConfigError::InvalidPort(self.query.port));
        }
        if !(2..=32).contains(&self.world.view_distance) {
            return Err(ConfigError::InvalidViewDistance(self.world.view_distance));
        }
        if !(2..=32).contains(&self.world.simulation_distance) {
            return Err(ConfigError::InvalidSimulationDistance(
                self.world.simulation_distance,
            ));
        }
        if self.gameplay.max_players == 0 {
            return Err(ConfigError::InvalidMaxPlayers(self.gameplay.max_players));
        }
        Ok(())
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::needless_pass_by_value
)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_default_config_serializes_to_valid_toml() {
        let config = ServerConfig::default();
        let toml_str =
            toml::to_string_pretty(&config).expect("default config should serialize to TOML");
        let deserialized: ServerConfig =
            toml::from_str(&toml_str).expect("serialized TOML should deserialize back");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_load_or_create_creates_default_file() {
        let dir = tempfile::tempdir().expect("should create tempdir");
        let path = dir.path().join("oxidized.toml");

        assert!(!path.exists());

        let config = ServerConfig::load_or_create(&path).expect("should create default");
        assert!(path.exists());
        assert_eq!(config, ServerConfig::default());

        // Verify the file contains TOML content.
        let contents = fs::read_to_string(&path).expect("should read");
        assert!(contents.contains("[network]"));
        assert!(contents.contains("[gameplay]"));
    }

    #[test]
    fn test_validate_accepts_valid_defaults() {
        let config = ServerConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_rejects_invalid_port() {
        let mut config = ServerConfig::default();
        config.network.port = 0;
        let err = config.validate().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidPort(0)));
    }

    #[test]
    fn test_validate_rejects_invalid_view_distance() {
        let mut config = ServerConfig::default();

        config.world.view_distance = 1;
        let err = config.validate().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidViewDistance(1)));

        config.world.view_distance = 33;
        let err = config.validate().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidViewDistance(33)));
    }

    #[test]
    fn test_validate_rejects_invalid_simulation_distance() {
        let mut config = ServerConfig::default();

        config.world.simulation_distance = 1;
        let err = config.validate().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidSimulationDistance(1)));

        config.world.simulation_distance = 33;
        let err = config.validate().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidSimulationDistance(33)));
    }

    #[test]
    fn test_validate_rejects_invalid_max_players() {
        let mut config = ServerConfig::default();
        config.gameplay.max_players = 0;
        let err = config.validate().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidMaxPlayers(0)));
    }

    #[test]
    fn test_roundtrip_preserves_all_fields() {
        let dir = tempfile::tempdir().expect("should create tempdir");
        let path = dir.path().join("oxidized.toml");

        let config = ServerConfig {
            network: NetworkConfig {
                port: 19132,
                ip: "192.168.1.1".to_string(),
                online_mode: false,
                prevent_proxy_connections: true,
                compression_threshold: 512,
                use_native_transport: false,
                rate_limit: 10,
                accepts_transfers: true,
            },
            gameplay: GameplayConfig {
                gamemode: "creative".to_string(),
                difficulty: "hard".to_string(),
                hardcore: true,
                force_gamemode: true,
                max_players: 100,
                spawn_protection: 32,
                max_world_size: 10000,
                allow_flight: true,
                spawn_npcs: false,
                spawn_animals: false,
                spawn_monsters: false,
                allow_nether: false,
                max_chained_neighbor_updates: 500_000,
                pvp: false,
            },
            world: WorldConfig {
                name: "custom_world".to_string(),
                seed: "12345".to_string(),
                generate_structures: false,
                view_distance: 16,
                simulation_distance: 8,
                sync_chunk_writes: false,
                region_file_compression: "none".to_string(),
            },
            display: DisplayConfig {
                motd: "Full Roundtrip Test".to_string(),
                enable_status: false,
                hide_online_players: true,
                entity_broadcast_range_percentage: 200,
                status_heartbeat_interval: 15,
            },
            admin: AdminConfig {
                white_list: true,
                enforce_whitelist: true,
                op_permission_level: 3,
                function_permission_level: 3,
                enforce_secure_profile: false,
                log_ips: false,
                max_tick_time: 120_000,
                player_idle_timeout: 30,
                broadcast_console_to_ops: false,
                broadcast_rcon_to_ops: false,
                pause_when_empty_seconds: 120,
            },
            rcon: RconConfig {
                enabled: true,
                port: 25576,
                password: "secret123".to_string(),
            },
            query: QueryConfig {
                enabled: true,
                port: 25566,
            },
            resource_pack: ResourcePackConfig {
                url: "https://example.com/pack.zip".to_string(),
                sha1: "abc123def456".to_string(),
                prompt: "Please install".to_string(),
                required: true,
            },
            management: ManagementConfig {
                enabled: true,
                host: "mgmt.example.com".to_string(),
                port: 8443,
                secret: "mgmt-secret".to_string(),
                tls_enabled: true,
            },
            packs: PacksConfig {
                initial_enabled: "vanilla,fabric".to_string(),
                initial_disabled: "experimental".to_string(),
            },
            advanced: AdvancedConfig {
                enable_jmx_monitoring: true,
                text_filtering_config: "filter.json".to_string(),
                text_filtering_version: 2,
                enable_code_of_conduct: true,
                bug_report_link: "https://bugs.example.com".to_string(),
            },
            extra: BTreeMap::new(),
        };

        config.save(&path).expect("save should succeed");
        let loaded = ServerConfig::load_or_create(&path).expect("load should succeed");
        assert_eq!(config, loaded);
    }

    // -----------------------------------------------------------------------
    // Property-based tests (proptest)
    // -----------------------------------------------------------------------

    proptest! {
        #[test]
        fn test_roundtrip_invariant_for_network_config(
            port in 1u16..=65535u16,
            online_mode in proptest::bool::ANY,
            compression in -1i32..1024i32,
            rate_limit in 0i32..1000i32,
        ) {
            let mut config = ServerConfig::default();
            config.network.port = port;
            config.network.online_mode = online_mode;
            config.network.compression_threshold = compression;
            config.network.rate_limit = rate_limit;

            let toml_str = toml::to_string_pretty(&config).unwrap();
            let loaded: ServerConfig = toml::from_str(&toml_str).unwrap();

            assert_eq!(config.network.port, loaded.network.port);
            assert_eq!(config.network.online_mode, loaded.network.online_mode);
            assert_eq!(
                config.network.compression_threshold,
                loaded.network.compression_threshold
            );
            assert_eq!(config.network.rate_limit, loaded.network.rate_limit);
        }
    }

    proptest! {
        #[test]
        fn test_roundtrip_invariant_for_gameplay_config(
            max_players in 1u32..1000u32,
            view_distance in 2u32..=32u32,
            simulation_distance in 2u32..=32u32,
            hardcore in proptest::bool::ANY,
            spawn_protection in 0u32..100u32,
        ) {
            let mut config = ServerConfig::default();
            config.gameplay.max_players = max_players;
            config.world.view_distance = view_distance;
            config.world.simulation_distance = simulation_distance;
            config.gameplay.hardcore = hardcore;
            config.gameplay.spawn_protection = spawn_protection;

            let toml_str = toml::to_string_pretty(&config).unwrap();
            let loaded: ServerConfig = toml::from_str(&toml_str).unwrap();

            assert_eq!(config.gameplay.max_players, loaded.gameplay.max_players);
            assert_eq!(config.world.view_distance, loaded.world.view_distance);
            assert_eq!(
                config.world.simulation_distance,
                loaded.world.simulation_distance
            );
            assert_eq!(config.gameplay.hardcore, loaded.gameplay.hardcore);
            assert_eq!(
                config.gameplay.spawn_protection,
                loaded.gameplay.spawn_protection
            );
        }
    }

    proptest! {
        #[test]
        fn test_roundtrip_invariant_for_string_fields(
            motd in "[a-zA-Z0-9 ]{0,50}",
            world_name in "[a-zA-Z0-9_]{1,30}",
            seed in "[a-zA-Z0-9]{0,20}",
        ) {
            let mut config = ServerConfig::default();
            config.display.motd = motd.clone();
            config.world.name = world_name.clone();
            config.world.seed = seed.clone();

            let toml_str = toml::to_string_pretty(&config).unwrap();
            let loaded: ServerConfig = toml::from_str(&toml_str).unwrap();

            assert_eq!(motd, loaded.display.motd);
            assert_eq!(world_name, loaded.world.name);
            assert_eq!(seed, loaded.world.seed);
        }
    }

    // -----------------------------------------------------------------------
    // Snapshot test (insta)
    // -----------------------------------------------------------------------

    #[test]
    fn test_default_config_snapshot() {
        let config = ServerConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        insta::assert_snapshot!(toml_str);
    }

    // -----------------------------------------------------------------------
    // Boundary validation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_validate_port_accepts_boundary_values() {
        let mut config = ServerConfig::default();
        config.network.port = 1;
        assert!(config.validate().is_ok(), "port 1 should be valid");

        config.network.port = 65535;
        assert!(config.validate().is_ok(), "port 65535 should be valid");
    }

    #[test]
    fn test_validate_view_distance_accepts_boundary_values() {
        let mut config = ServerConfig::default();
        config.world.view_distance = 2;
        assert!(config.validate().is_ok(), "view_distance 2 should be valid");

        config.world.view_distance = 32;
        assert!(
            config.validate().is_ok(),
            "view_distance 32 should be valid"
        );
    }

    #[test]
    fn test_validate_simulation_distance_accepts_boundary_values() {
        let mut config = ServerConfig::default();
        config.world.simulation_distance = 2;
        assert!(config.validate().is_ok(), "sim_distance 2 should be valid");

        config.world.simulation_distance = 32;
        assert!(config.validate().is_ok(), "sim_distance 32 should be valid");
    }

    #[test]
    fn test_validate_rejects_rcon_port_zero_when_enabled() {
        let mut config = ServerConfig::default();
        config.rcon.enabled = true;
        config.rcon.port = 0;
        assert!(
            matches!(config.validate(), Err(ConfigError::InvalidPort(0))),
            "rcon port 0 with enabled=true should fail"
        );
    }

    #[test]
    fn test_validate_allows_rcon_port_zero_when_disabled() {
        let mut config = ServerConfig::default();
        config.rcon.enabled = false;
        config.rcon.port = 0;
        assert!(
            config.validate().is_ok(),
            "rcon port 0 with enabled=false should be ok"
        );
    }

    #[test]
    fn test_validate_rejects_query_port_zero_when_enabled() {
        let mut config = ServerConfig::default();
        config.query.enabled = true;
        config.query.port = 0;
        assert!(
            matches!(config.validate(), Err(ConfigError::InvalidPort(0))),
            "query port 0 with enabled=true should fail"
        );
    }

    #[test]
    fn test_validate_allows_query_port_zero_when_disabled() {
        let mut config = ServerConfig::default();
        config.query.enabled = false;
        config.query.port = 0;
        assert!(
            config.validate().is_ok(),
            "query port 0 with enabled=false should be ok"
        );
    }

    // -----------------------------------------------------------------------
    // Partial TOML tests (missing sections get defaults)
    // -----------------------------------------------------------------------

    #[test]
    fn test_partial_toml_gets_defaults_for_missing_sections() {
        let partial = r#"
[network]
port = 19132
"#;
        let config: ServerConfig = toml::from_str(partial).unwrap();
        assert_eq!(config.network.port, 19132);
        // All other sections should be defaults
        assert_eq!(config.gameplay.max_players, 20);
        assert_eq!(config.world.name, "world");
        assert_eq!(config.display.motd, "An Oxidized Minecraft Server");
        assert!(!config.rcon.enabled);
    }

    #[test]
    fn test_empty_toml_produces_all_defaults() {
        let config: ServerConfig = toml::from_str("").unwrap();
        let default = ServerConfig::default();
        assert_eq!(config, default);
    }

    #[test]
    fn test_unknown_keys_are_preserved_through_roundtrip() {
        let input = r#"
[network]
port = 25565

[unknown_section]
key = "value"
"#;
        let config: ServerConfig = toml::from_str(input).unwrap();
        assert_eq!(config.network.port, 25565);
        // Unknown top-level section is captured in the `extra` map
        assert!(
            config.extra.contains_key("unknown_section"),
            "unknown sections should be preserved in extra"
        );

        // Roundtrip: serialize back and re-parse
        let serialized = toml::to_string_pretty(&config).unwrap();
        let reloaded: ServerConfig = toml::from_str(&serialized).unwrap();
        assert!(
            reloaded.extra.contains_key("unknown_section"),
            "unknown sections should survive roundtrip"
        );
    }

    // -----------------------------------------------------------------------
    // TOML format edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_toml_with_inline_comments_parses_correctly() {
        let input = r#"
# Server configuration
[network]
port = 25565 # Standard MC port
online_mode = true
"#;
        let config: ServerConfig = toml::from_str(input).unwrap();
        assert_eq!(config.network.port, 25565);
        assert!(config.network.online_mode);
    }

    #[test]
    fn test_toml_with_extra_whitespace_parses_correctly() {
        let input = r#"
[network]
port   =   19132
online_mode   =   false
"#;
        let config: ServerConfig = toml::from_str(input).unwrap();
        assert_eq!(config.network.port, 19132);
        assert!(!config.network.online_mode);
    }

    // -----------------------------------------------------------------------
    // File I/O integration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_save_and_reload_produces_identical_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("oxidized.toml");

        let mut config = ServerConfig::default();
        config.network.port = 19132;
        config.gameplay.max_players = 100;
        config.display.motd = "Test Server".to_string();
        config.rcon.enabled = true;
        config.rcon.password = "secret".to_string();

        config.save(&path).unwrap();
        let loaded = ServerConfig::load_or_create(&path).unwrap();

        assert_eq!(config, loaded);
    }

    #[test]
    fn test_saved_file_starts_with_header_comment() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("oxidized.toml");

        let config = ServerConfig::default();
        config.save(&path).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(
            content.starts_with("# Oxidized"),
            "file should start with header comment"
        );
    }

    // -----------------------------------------------------------------------
    // Security — secret redaction in Debug output
    // -----------------------------------------------------------------------

    #[test]
    fn test_debug_redacts_rcon_password() {
        let rcon = RconConfig {
            password: "super_secret_password".to_string(),
            ..RconConfig::default()
        };
        let debug_output = format!("{:?}", rcon);
        assert!(
            !debug_output.contains("super_secret_password"),
            "Debug output must not contain the actual password"
        );
        assert!(
            debug_output.contains("[REDACTED]"),
            "Debug output should show [REDACTED]"
        );
    }

    #[test]
    fn test_debug_redacts_management_secret() {
        let mgmt = ManagementConfig {
            secret: "top_secret_key".to_string(),
            ..ManagementConfig::default()
        };
        let debug_output = format!("{:?}", mgmt);
        assert!(
            !debug_output.contains("top_secret_key"),
            "Debug output must not contain the actual secret"
        );
        assert!(
            debug_output.contains("[REDACTED]"),
            "Debug output should show [REDACTED]"
        );
    }
}
