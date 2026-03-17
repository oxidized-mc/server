//! Server configuration loaded from `server.properties`.
//!
//! Implements a hand-rolled Java Properties parser and a strongly typed
//! [`ServerConfig`] struct whose defaults match vanilla Minecraft 26.1-pre-3.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

use anyhow::Context;

// ---------------------------------------------------------------------------
// Java Properties parser
// ---------------------------------------------------------------------------

/// Parses a Java Properties format string into key-value pairs.
///
/// Supports:
/// - `key=value` and `key: value` and `key value` separators
/// - `#` and `!` comment lines
/// - Leading/trailing whitespace trimming on keys and values
/// - Empty lines are skipped
/// - `\` at end of line for line continuation
pub fn parse_properties(input: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut lines = input.lines().peekable();

    while let Some(raw_line) = lines.next() {
        let trimmed = raw_line.trim();

        // Skip blanks and comments.
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }

        // Handle line continuation: if the logical line ends with `\`, keep
        // appending the next physical line.
        let mut logical = String::from(trimmed);
        while logical.ends_with('\\') {
            logical.pop(); // remove trailing backslash
            if let Some(next) = lines.next() {
                logical.push_str(next.trim());
            } else {
                break;
            }
        }

        // Find the separator – the first unescaped `=`, `:`, or whitespace.
        let sep_pos = logical
            .find(['=', ':'])
            .or_else(|| logical.find(char::is_whitespace));

        let (key, value) = match sep_pos {
            Some(pos) => {
                let key = logical[..pos].trim().to_string();
                let rest = &logical[pos..];
                // Skip the separator character itself (and any leading whitespace
                // on the value).
                let value = if rest.starts_with('=') || rest.starts_with(':') {
                    rest[1..].trim_start().to_string()
                } else {
                    rest.trim_start().to_string()
                };
                (key, value)
            },
            None => {
                // Key with no value.
                (logical.trim().to_string(), String::new())
            },
        };

        if !key.is_empty() {
            map.insert(key, value);
        }
    }

    map
}

// ---------------------------------------------------------------------------
// Typed property helpers
// ---------------------------------------------------------------------------

/// Returns a `String` from the map, falling back to `default`.
fn get_string(props: &HashMap<String, String>, key: &str, default: &str) -> String {
    props
        .get(key)
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

/// Returns a `bool` from the map, falling back to `default`.
fn get_bool(props: &HashMap<String, String>, key: &str, default: bool) -> bool {
    props
        .get(key)
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(default)
}

/// Returns a `u16` from the map, falling back to `default`.
fn get_u16(props: &HashMap<String, String>, key: &str, default: u16) -> u16 {
    props
        .get(key)
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(default)
}

/// Returns a `u32` from the map, falling back to `default`.
fn get_u32(props: &HashMap<String, String>, key: &str, default: u32) -> u32 {
    props
        .get(key)
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(default)
}

/// Returns an `i32` from the map, falling back to `default`.
fn get_i32(props: &HashMap<String, String>, key: &str, default: i32) -> i32 {
    props
        .get(key)
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(default)
}

/// Returns an `i64` from the map, falling back to `default`.
fn get_i64(props: &HashMap<String, String>, key: &str, default: i64) -> i64 {
    props
        .get(key)
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(default)
}

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
// ServerConfig
// ---------------------------------------------------------------------------

/// Server configuration loaded from `server.properties`.
///
/// All default values match the vanilla Minecraft 26.1-pre-3 server.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct ServerConfig {
    // -- Network --------------------------------------------------------
    /// Port the server listens on (default `25565`).
    pub server_port: u16,
    /// IP address to bind to (default `""` — all interfaces).
    pub server_ip: String,
    /// Whether the server authenticates players with Mojang (default `true`).
    pub online_mode: bool,
    /// Block proxy / VPN connections (default `false`).
    pub prevent_proxy_connections: bool,
    /// Byte threshold for packet compression; `-1` disables (default `256`).
    pub network_compression_threshold: i32,
    /// Use epoll/kqueue native transport (default `true`).
    pub use_native_transport: bool,
    /// Maximum packets per second before kicking; `0` disables (default `0`).
    pub rate_limit: i32,

    // -- Gameplay -------------------------------------------------------
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
    /// Chunk view distance (default `10`).
    pub view_distance: u32,
    /// Simulation distance in chunks (default `10`).
    pub simulation_distance: u32,
    /// Radius around spawn where non-ops cannot build (default `16`).
    pub spawn_protection: u32,
    /// Maximum world border radius in blocks (default `29999984`).
    pub max_world_size: i32,
    /// Allow players to fly in survival (default `false`).
    pub allow_flight: bool,

    // -- World ----------------------------------------------------------
    /// Name of the world folder (default `"world"`).
    pub level_name: String,
    /// World seed; empty means random (default `""`).
    pub level_seed: String,
    /// Generate structures such as villages (default `true`).
    pub generate_structures: bool,
    /// Limit on chained neighbour block updates (default `1000000`).
    pub max_chained_neighbor_updates: i32,
    /// Spawn NPC villagers (default `true`).
    pub spawn_npcs: bool,
    /// Spawn passive animals (default `true`).
    pub spawn_animals: bool,
    /// Spawn hostile mobs (default `true`).
    pub spawn_monsters: bool,
    /// Enable the Nether dimension (default `true`).
    pub allow_nether: bool,

    // -- Display / MOTD -------------------------------------------------
    /// Message of the day shown in the server list (default `"A Minecraft Server"`).
    pub motd: String,
    /// Respond to server-list pings (default `true`).
    pub enable_status: bool,
    /// Hide player names from the server list (default `false`).
    pub hide_online_players: bool,
    /// Entity tracking range as a percentage (default `100`).
    pub entity_broadcast_range_percentage: i32,

    // -- Admin / Security -----------------------------------------------
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

    // -- RCON -----------------------------------------------------------
    /// Enable the RCON remote console (default `false`).
    pub enable_rcon: bool,
    /// RCON listening port (default `25575`).
    pub rcon_port: u16,
    /// RCON password (default `""`).
    pub rcon_password: String,

    // -- Query ----------------------------------------------------------
    /// Enable the GameSpy4 query protocol (default `false`).
    pub enable_query: bool,
    /// Query protocol port (default `25565`).
    pub query_port: u16,

    // -- Resource pack --------------------------------------------------
    /// URL of the server resource pack (default `""`).
    pub resource_pack: String,
    /// SHA-1 hash of the resource pack (default `""`).
    pub resource_pack_sha1: String,
    /// Prompt shown to players for the resource pack (default `""`).
    pub resource_pack_prompt: String,
    /// Whether the resource pack is mandatory (default `false`).
    pub require_resource_pack: bool,

    // -- Management server (26.1 feature) --------------------------------
    /// Enable the management server (default `false`).
    pub management_server_enabled: bool,
    /// Management server host (default `"localhost"`).
    pub management_server_host: String,
    /// Management server port; `0` means auto-assign (default `0`).
    pub management_server_port: u16,
    /// Shared secret for the management server (default `""`).
    pub management_server_secret: String,
    /// Require TLS on the management server (default `true`).
    pub management_server_tls_enabled: bool,

    // -- Code of conduct (26.1 feature) ----------------------------------
    /// Show a code-of-conduct prompt on join (default `false`).
    pub enable_code_of_conduct: bool,
    /// Link to the server's bug report page (default `""`).
    pub bug_report_link: String,

    // -- Misc -----------------------------------------------------------
    /// Synchronous chunk writes for data safety (default `true`).
    pub sync_chunk_writes: bool,
    /// Region file compression algorithm (default `"deflate"`).
    pub region_file_compression: String,
    /// Enable JMX monitoring beans (default `false`).
    pub enable_jmx_monitoring: bool,
    /// External text-filtering service config path (default `""`).
    pub text_filtering_config: String,
    /// Text-filtering protocol version (default `0`).
    pub text_filtering_version: i32,
    /// Accept transfer packets from other servers (default `false`).
    pub accepts_transfers: bool,
    /// Seconds to pause the game loop when the server is empty (default `60`).
    pub pause_when_empty_seconds: i32,
    /// Comma-separated data packs enabled at startup (default `"vanilla"`).
    pub initial_enabled_packs: String,
    /// Comma-separated data packs disabled at startup (default `""`).
    pub initial_disabled_packs: String,
    /// Heartbeat interval in seconds for status polling (default `0`).
    pub status_heartbeat_interval: i32,

    /// Unknown keys from the properties file, preserved for forward-compatibility.
    /// These are written back verbatim on save so vanilla-generated files aren't truncated.
    unknown_keys: BTreeMap<String, String>,
}

impl ServerConfig {
    /// Creates a `ServerConfig` with all vanilla defaults.
    pub fn default_config() -> Self {
        Self {
            // Network
            server_port: 25565,
            server_ip: String::new(),
            online_mode: true,
            prevent_proxy_connections: false,
            network_compression_threshold: 256,
            use_native_transport: true,
            rate_limit: 0,

            // Gameplay
            gamemode: "survival".to_string(),
            difficulty: "easy".to_string(),
            hardcore: false,
            force_gamemode: false,
            max_players: 20,
            view_distance: 10,
            simulation_distance: 10,
            spawn_protection: 16,
            max_world_size: 29_999_984,
            allow_flight: false,

            // World
            level_name: "world".to_string(),
            level_seed: String::new(),
            generate_structures: true,
            max_chained_neighbor_updates: 1_000_000,
            spawn_npcs: true,
            spawn_animals: true,
            spawn_monsters: true,
            allow_nether: true,

            // Display / MOTD
            motd: "A Minecraft Server".to_string(),
            enable_status: true,
            hide_online_players: false,
            entity_broadcast_range_percentage: 100,

            // Admin / Security
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

            // RCON
            enable_rcon: false,
            rcon_port: 25575,
            rcon_password: String::new(),

            // Query
            enable_query: false,
            query_port: 25565,

            // Resource pack
            resource_pack: String::new(),
            resource_pack_sha1: String::new(),
            resource_pack_prompt: String::new(),
            require_resource_pack: false,

            // Management server
            management_server_enabled: false,
            management_server_host: "localhost".to_string(),
            management_server_port: 0,
            management_server_secret: String::new(),
            management_server_tls_enabled: true,

            // Code of conduct
            enable_code_of_conduct: false,
            bug_report_link: String::new(),

            // Misc
            sync_chunk_writes: true,
            region_file_compression: "deflate".to_string(),
            enable_jmx_monitoring: false,
            text_filtering_config: String::new(),
            text_filtering_version: 0,
            accepts_transfers: false,
            pause_when_empty_seconds: 60,
            initial_enabled_packs: "vanilla".to_string(),
            initial_disabled_packs: String::new(),
            status_heartbeat_interval: 0,
            unknown_keys: BTreeMap::new(),
        }
    }

    /// Loads configuration from a properties file.
    ///
    /// If the file doesn't exist, creates a default `server.properties` file
    /// and returns default configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be read or parsed.
    pub fn load_or_create(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            let config = Self::default_config();
            config
                .save(path)
                .context("failed to write default server.properties")?;
            return Ok(config);
        }

        let contents = fs::read_to_string(path).context("failed to read server.properties")?;
        let props = parse_properties(&contents);
        Ok(Self::from_properties(&props))
    }

    /// Parses a `ServerConfig` from a properties `HashMap`, applying defaults
    /// for any missing keys.
    fn from_properties(props: &HashMap<String, String>) -> Self {
        let d = Self::default_config();

        // All keys that map to a typed field — anything else is "unknown".
        const KNOWN_KEYS: &[&str] = &[
            "server-port",
            "server-ip",
            "online-mode",
            "prevent-proxy-connections",
            "network-compression-threshold",
            "use-native-transport",
            "rate-limit",
            "gamemode",
            "difficulty",
            "hardcore",
            "force-gamemode",
            "max-players",
            "view-distance",
            "simulation-distance",
            "spawn-protection",
            "max-world-size",
            "allow-flight",
            "level-name",
            "level-seed",
            "generate-structures",
            "max-chained-neighbor-updates",
            "spawn-npcs",
            "spawn-animals",
            "spawn-monsters",
            "allow-nether",
            "motd",
            "enable-status",
            "hide-online-players",
            "entity-broadcast-range-percentage",
            "white-list",
            "enforce-whitelist",
            "op-permission-level",
            "function-permission-level",
            "enforce-secure-profile",
            "log-ips",
            "max-tick-time",
            "player-idle-timeout",
            "broadcast-console-to-ops",
            "broadcast-rcon-to-ops",
            "enable-rcon",
            "rcon.port",
            "rcon.password",
            "enable-query",
            "query.port",
            "resource-pack",
            "resource-pack-sha1",
            "resource-pack-prompt",
            "require-resource-pack",
            "management-server-enabled",
            "management-server-host",
            "management-server-port",
            "management-server-secret",
            "management-server-tls-enabled",
            "enable-code-of-conduct",
            "bug-report-link",
            "sync-chunk-writes",
            "region-file-compression",
            "enable-jmx-monitoring",
            "text-filtering-config",
            "text-filtering-version",
            "accepts-transfers",
            "pause-when-empty-seconds",
            "initial-enabled-packs",
            "initial-disabled-packs",
            "status-heartbeat-interval",
        ];

        let unknown_keys: BTreeMap<String, String> = props
            .iter()
            .filter(|(k, _)| !KNOWN_KEYS.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        Self {
            // Network
            server_port: get_u16(props, "server-port", d.server_port),
            server_ip: get_string(props, "server-ip", &d.server_ip),
            online_mode: get_bool(props, "online-mode", d.online_mode),
            prevent_proxy_connections: get_bool(
                props,
                "prevent-proxy-connections",
                d.prevent_proxy_connections,
            ),
            network_compression_threshold: get_i32(
                props,
                "network-compression-threshold",
                d.network_compression_threshold,
            ),
            use_native_transport: get_bool(props, "use-native-transport", d.use_native_transport),
            rate_limit: get_i32(props, "rate-limit", d.rate_limit),

            // Gameplay
            gamemode: get_string(props, "gamemode", &d.gamemode),
            difficulty: get_string(props, "difficulty", &d.difficulty),
            hardcore: get_bool(props, "hardcore", d.hardcore),
            force_gamemode: get_bool(props, "force-gamemode", d.force_gamemode),
            max_players: get_u32(props, "max-players", d.max_players),
            view_distance: get_u32(props, "view-distance", d.view_distance),
            simulation_distance: get_u32(props, "simulation-distance", d.simulation_distance),
            spawn_protection: get_u32(props, "spawn-protection", d.spawn_protection),
            max_world_size: get_i32(props, "max-world-size", d.max_world_size),
            allow_flight: get_bool(props, "allow-flight", d.allow_flight),

            // World
            level_name: get_string(props, "level-name", &d.level_name),
            level_seed: get_string(props, "level-seed", &d.level_seed),
            generate_structures: get_bool(props, "generate-structures", d.generate_structures),
            max_chained_neighbor_updates: get_i32(
                props,
                "max-chained-neighbor-updates",
                d.max_chained_neighbor_updates,
            ),
            spawn_npcs: get_bool(props, "spawn-npcs", d.spawn_npcs),
            spawn_animals: get_bool(props, "spawn-animals", d.spawn_animals),
            spawn_monsters: get_bool(props, "spawn-monsters", d.spawn_monsters),
            allow_nether: get_bool(props, "allow-nether", d.allow_nether),

            // Display / MOTD
            motd: get_string(props, "motd", &d.motd),
            enable_status: get_bool(props, "enable-status", d.enable_status),
            hide_online_players: get_bool(props, "hide-online-players", d.hide_online_players),
            entity_broadcast_range_percentage: get_i32(
                props,
                "entity-broadcast-range-percentage",
                d.entity_broadcast_range_percentage,
            ),

            // Admin / Security
            white_list: get_bool(props, "white-list", d.white_list),
            enforce_whitelist: get_bool(props, "enforce-whitelist", d.enforce_whitelist),
            op_permission_level: get_i32(props, "op-permission-level", d.op_permission_level),
            function_permission_level: get_i32(
                props,
                "function-permission-level",
                d.function_permission_level,
            ),
            enforce_secure_profile: get_bool(
                props,
                "enforce-secure-profile",
                d.enforce_secure_profile,
            ),
            log_ips: get_bool(props, "log-ips", d.log_ips),
            max_tick_time: get_i64(props, "max-tick-time", d.max_tick_time),
            player_idle_timeout: get_i32(props, "player-idle-timeout", d.player_idle_timeout),
            broadcast_console_to_ops: get_bool(
                props,
                "broadcast-console-to-ops",
                d.broadcast_console_to_ops,
            ),
            broadcast_rcon_to_ops: get_bool(
                props,
                "broadcast-rcon-to-ops",
                d.broadcast_rcon_to_ops,
            ),

            // RCON
            enable_rcon: get_bool(props, "enable-rcon", d.enable_rcon),
            rcon_port: get_u16(props, "rcon.port", d.rcon_port),
            rcon_password: get_string(props, "rcon.password", &d.rcon_password),

            // Query
            enable_query: get_bool(props, "enable-query", d.enable_query),
            query_port: get_u16(props, "query.port", d.query_port),

            // Resource pack
            resource_pack: get_string(props, "resource-pack", &d.resource_pack),
            resource_pack_sha1: get_string(props, "resource-pack-sha1", &d.resource_pack_sha1),
            resource_pack_prompt: get_string(
                props,
                "resource-pack-prompt",
                &d.resource_pack_prompt,
            ),
            require_resource_pack: get_bool(
                props,
                "require-resource-pack",
                d.require_resource_pack,
            ),

            // Management server
            management_server_enabled: get_bool(
                props,
                "management-server-enabled",
                d.management_server_enabled,
            ),
            management_server_host: get_string(
                props,
                "management-server-host",
                &d.management_server_host,
            ),
            management_server_port: get_u16(
                props,
                "management-server-port",
                d.management_server_port,
            ),
            management_server_secret: get_string(
                props,
                "management-server-secret",
                &d.management_server_secret,
            ),
            management_server_tls_enabled: get_bool(
                props,
                "management-server-tls-enabled",
                d.management_server_tls_enabled,
            ),

            // Code of conduct
            enable_code_of_conduct: get_bool(
                props,
                "enable-code-of-conduct",
                d.enable_code_of_conduct,
            ),
            bug_report_link: get_string(props, "bug-report-link", &d.bug_report_link),

            // Misc
            sync_chunk_writes: get_bool(props, "sync-chunk-writes", d.sync_chunk_writes),
            region_file_compression: get_string(
                props,
                "region-file-compression",
                &d.region_file_compression,
            ),
            enable_jmx_monitoring: get_bool(
                props,
                "enable-jmx-monitoring",
                d.enable_jmx_monitoring,
            ),
            text_filtering_config: get_string(
                props,
                "text-filtering-config",
                &d.text_filtering_config,
            ),
            text_filtering_version: get_i32(
                props,
                "text-filtering-version",
                d.text_filtering_version,
            ),
            accepts_transfers: get_bool(props, "accepts-transfers", d.accepts_transfers),
            pause_when_empty_seconds: get_i32(
                props,
                "pause-when-empty-seconds",
                d.pause_when_empty_seconds,
            ),
            initial_enabled_packs: get_string(
                props,
                "initial-enabled-packs",
                &d.initial_enabled_packs,
            ),
            initial_disabled_packs: get_string(
                props,
                "initial-disabled-packs",
                &d.initial_disabled_packs,
            ),
            status_heartbeat_interval: get_i32(
                props,
                "status-heartbeat-interval",
                d.status_heartbeat_interval,
            ),
            unknown_keys,
        }
    }

    /// Saves the current configuration to a properties file.
    ///
    /// The output format mirrors vanilla Minecraft's `server.properties`.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        use std::fmt::Write as _;

        let mut buf = String::with_capacity(2048);
        writeln!(buf, "#Minecraft server properties")?;
        writeln!(buf, "#{}", chrono_lite_timestamp())?;

        // Helper: append a property line.
        macro_rules! prop {
            ($key:expr, $val:expr) => {
                writeln!(buf, "{}={}", $key, $val)?;
            };
        }

        // Network
        prop!("server-port", self.server_port);
        prop!("server-ip", self.server_ip);
        prop!("online-mode", self.online_mode);
        prop!("prevent-proxy-connections", self.prevent_proxy_connections);
        prop!(
            "network-compression-threshold",
            self.network_compression_threshold
        );
        prop!("use-native-transport", self.use_native_transport);
        prop!("rate-limit", self.rate_limit);

        // Gameplay
        prop!("gamemode", self.gamemode);
        prop!("difficulty", self.difficulty);
        prop!("hardcore", self.hardcore);
        prop!("force-gamemode", self.force_gamemode);
        prop!("max-players", self.max_players);
        prop!("view-distance", self.view_distance);
        prop!("simulation-distance", self.simulation_distance);
        prop!("spawn-protection", self.spawn_protection);
        prop!("max-world-size", self.max_world_size);
        prop!("allow-flight", self.allow_flight);

        // World
        prop!("level-name", self.level_name);
        prop!("level-seed", self.level_seed);
        prop!("generate-structures", self.generate_structures);
        prop!(
            "max-chained-neighbor-updates",
            self.max_chained_neighbor_updates
        );
        prop!("spawn-npcs", self.spawn_npcs);
        prop!("spawn-animals", self.spawn_animals);
        prop!("spawn-monsters", self.spawn_monsters);
        prop!("allow-nether", self.allow_nether);

        // Display / MOTD
        prop!("motd", self.motd);
        prop!("enable-status", self.enable_status);
        prop!("hide-online-players", self.hide_online_players);
        prop!(
            "entity-broadcast-range-percentage",
            self.entity_broadcast_range_percentage
        );

        // Admin / Security
        prop!("white-list", self.white_list);
        prop!("enforce-whitelist", self.enforce_whitelist);
        prop!("op-permission-level", self.op_permission_level);
        prop!("function-permission-level", self.function_permission_level);
        prop!("enforce-secure-profile", self.enforce_secure_profile);
        prop!("log-ips", self.log_ips);
        prop!("max-tick-time", self.max_tick_time);
        prop!("player-idle-timeout", self.player_idle_timeout);
        prop!("broadcast-console-to-ops", self.broadcast_console_to_ops);
        prop!("broadcast-rcon-to-ops", self.broadcast_rcon_to_ops);

        // RCON
        prop!("enable-rcon", self.enable_rcon);
        prop!("rcon.port", self.rcon_port);
        prop!("rcon.password", self.rcon_password);

        // Query
        prop!("enable-query", self.enable_query);
        prop!("query.port", self.query_port);

        // Resource pack
        prop!("resource-pack", self.resource_pack);
        prop!("resource-pack-sha1", self.resource_pack_sha1);
        prop!("resource-pack-prompt", self.resource_pack_prompt);
        prop!("require-resource-pack", self.require_resource_pack);

        // Management server
        prop!("management-server-enabled", self.management_server_enabled);
        prop!("management-server-host", self.management_server_host);
        prop!("management-server-port", self.management_server_port);
        prop!("management-server-secret", self.management_server_secret);
        prop!(
            "management-server-tls-enabled",
            self.management_server_tls_enabled
        );

        // Code of conduct
        prop!("enable-code-of-conduct", self.enable_code_of_conduct);
        prop!("bug-report-link", self.bug_report_link);

        // Misc
        prop!("sync-chunk-writes", self.sync_chunk_writes);
        prop!("region-file-compression", self.region_file_compression);
        prop!("enable-jmx-monitoring", self.enable_jmx_monitoring);
        prop!("text-filtering-config", self.text_filtering_config);
        prop!("text-filtering-version", self.text_filtering_version);
        prop!("accepts-transfers", self.accepts_transfers);
        prop!("pause-when-empty-seconds", self.pause_when_empty_seconds);
        prop!("initial-enabled-packs", self.initial_enabled_packs);
        prop!("initial-disabled-packs", self.initial_disabled_packs);
        prop!("status-heartbeat-interval", self.status_heartbeat_interval);

        // Unknown keys (preserved verbatim for forward-compatibility).
        if !self.unknown_keys.is_empty() {
            writeln!(buf)?;
            for (key, value) in &self.unknown_keys {
                prop!(key, value);
            }
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .context("failed to create parent directories for server.properties")?;
        }
        fs::write(path, buf).context("failed to write server.properties")?;
        Ok(())
    }

    /// Validates all config values are within acceptable ranges.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigError`] describing the first invalid value found.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.server_port == 0 {
            return Err(ConfigError::InvalidPort(self.server_port));
        }
        if self.rcon_port == 0 && self.enable_rcon {
            return Err(ConfigError::InvalidPort(self.rcon_port));
        }
        if self.query_port == 0 && self.enable_query {
            return Err(ConfigError::InvalidPort(self.query_port));
        }
        if !(2..=32).contains(&self.view_distance) {
            return Err(ConfigError::InvalidViewDistance(self.view_distance));
        }
        if !(2..=32).contains(&self.simulation_distance) {
            return Err(ConfigError::InvalidSimulationDistance(
                self.simulation_distance,
            ));
        }
        if self.max_players == 0 {
            return Err(ConfigError::InvalidMaxPlayers(self.max_players));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Lightweight timestamp (avoids pulling in `chrono`)
// ---------------------------------------------------------------------------

/// Returns a simple UTC timestamp string suitable for the properties header.
fn chrono_lite_timestamp() -> String {
    // Use `SystemTime` to avoid an external dependency.
    use std::time::SystemTime;

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();

    // Very small date formatter — enough for the comment line.
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Days since epoch → year/month/day (simplified leap-year aware).
    let (year, month, day) = epoch_days_to_ymd(days);

    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}:{seconds:02} UTC")
}

/// Converts days since the Unix epoch to (year, month, day).
fn epoch_days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    let leap = is_leap(year);
    let month_days: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];

    let mut month = 0u64;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }

    (year, month + 1, days + 1)
}

/// Returns `true` if `year` is a leap year.
const fn is_leap(year: u64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_properties_basic() {
        let input = "server-port=25565\nonline-mode=true\nmotd=Hello World\n";
        let props = parse_properties(input);

        assert_eq!(props.get("server-port").map(String::as_str), Some("25565"));
        assert_eq!(props.get("online-mode").map(String::as_str), Some("true"));
        assert_eq!(props.get("motd").map(String::as_str), Some("Hello World"));
    }

    #[test]
    fn test_parse_properties_comments() {
        let input = "# This is a comment\n! Also a comment\nkey=value\n";
        let props = parse_properties(input);

        assert_eq!(props.len(), 1);
        assert_eq!(props.get("key").map(String::as_str), Some("value"));
    }

    #[test]
    fn test_parse_properties_whitespace() {
        let input = "  key1  =  value1  \n  key2  =  value2  \n";
        let props = parse_properties(input);

        assert_eq!(props.get("key1").map(String::as_str), Some("value1"));
        assert_eq!(props.get("key2").map(String::as_str), Some("value2"));
    }

    #[test]
    fn test_parse_properties_colon_separator() {
        let input = "key1: value1\nkey2:value2\n";
        let props = parse_properties(input);

        assert_eq!(props.get("key1").map(String::as_str), Some("value1"));
        assert_eq!(props.get("key2").map(String::as_str), Some("value2"));
    }

    #[test]
    fn test_parse_properties_empty_value() {
        let input = "server-ip=\nlevel-seed=\n";
        let props = parse_properties(input);

        assert_eq!(props.get("server-ip").map(String::as_str), Some(""));
        assert_eq!(props.get("level-seed").map(String::as_str), Some(""));
    }

    #[test]
    fn test_default_config_values() {
        let cfg = ServerConfig::default_config();

        // Network
        assert_eq!(cfg.server_port, 25565);
        assert_eq!(cfg.server_ip, "");
        assert!(cfg.online_mode);
        assert!(!cfg.prevent_proxy_connections);
        assert_eq!(cfg.network_compression_threshold, 256);
        assert!(cfg.use_native_transport);
        assert_eq!(cfg.rate_limit, 0);

        // Gameplay
        assert_eq!(cfg.gamemode, "survival");
        assert_eq!(cfg.difficulty, "easy");
        assert!(!cfg.hardcore);
        assert!(!cfg.force_gamemode);
        assert_eq!(cfg.max_players, 20);
        assert_eq!(cfg.view_distance, 10);
        assert_eq!(cfg.simulation_distance, 10);
        assert_eq!(cfg.spawn_protection, 16);
        assert_eq!(cfg.max_world_size, 29_999_984);
        assert!(!cfg.allow_flight);

        // World
        assert_eq!(cfg.level_name, "world");
        assert_eq!(cfg.level_seed, "");
        assert!(cfg.generate_structures);
        assert_eq!(cfg.max_chained_neighbor_updates, 1_000_000);
        assert!(cfg.spawn_npcs);
        assert!(cfg.spawn_animals);
        assert!(cfg.spawn_monsters);
        assert!(cfg.allow_nether);

        // Display / MOTD
        assert_eq!(cfg.motd, "A Minecraft Server");
        assert!(cfg.enable_status);
        assert!(!cfg.hide_online_players);
        assert_eq!(cfg.entity_broadcast_range_percentage, 100);

        // Admin / Security
        assert!(!cfg.white_list);
        assert!(!cfg.enforce_whitelist);
        assert_eq!(cfg.op_permission_level, 4);
        assert_eq!(cfg.function_permission_level, 2);
        assert!(cfg.enforce_secure_profile);
        assert!(cfg.log_ips);
        assert_eq!(cfg.max_tick_time, 60_000);
        assert_eq!(cfg.player_idle_timeout, 0);
        assert!(cfg.broadcast_console_to_ops);
        assert!(cfg.broadcast_rcon_to_ops);

        // RCON
        assert!(!cfg.enable_rcon);
        assert_eq!(cfg.rcon_port, 25575);
        assert_eq!(cfg.rcon_password, "");

        // Query
        assert!(!cfg.enable_query);
        assert_eq!(cfg.query_port, 25565);

        // Resource pack
        assert_eq!(cfg.resource_pack, "");
        assert_eq!(cfg.resource_pack_sha1, "");
        assert_eq!(cfg.resource_pack_prompt, "");
        assert!(!cfg.require_resource_pack);

        // Management server
        assert!(!cfg.management_server_enabled);
        assert_eq!(cfg.management_server_host, "localhost");
        assert_eq!(cfg.management_server_port, 0);
        assert_eq!(cfg.management_server_secret, "");
        assert!(cfg.management_server_tls_enabled);

        // Code of conduct
        assert!(!cfg.enable_code_of_conduct);
        assert_eq!(cfg.bug_report_link, "");

        // Misc
        assert!(cfg.sync_chunk_writes);
        assert_eq!(cfg.region_file_compression, "deflate");
        assert!(!cfg.enable_jmx_monitoring);
        assert_eq!(cfg.text_filtering_config, "");
        assert_eq!(cfg.text_filtering_version, 0);
        assert!(!cfg.accepts_transfers);
        assert_eq!(cfg.pause_when_empty_seconds, 60);
        assert_eq!(cfg.initial_enabled_packs, "vanilla");
        assert_eq!(cfg.initial_disabled_packs, "");
        assert_eq!(cfg.status_heartbeat_interval, 0);
    }

    #[test]
    fn test_from_properties_overrides() {
        let mut props = HashMap::new();
        props.insert("server-port".to_string(), "19132".to_string());
        props.insert("online-mode".to_string(), "false".to_string());
        props.insert("max-players".to_string(), "100".to_string());
        props.insert("motd".to_string(), "Custom Server".to_string());
        props.insert("view-distance".to_string(), "16".to_string());
        props.insert("difficulty".to_string(), "hard".to_string());

        let cfg = ServerConfig::from_properties(&props);

        assert_eq!(cfg.server_port, 19132);
        assert!(!cfg.online_mode);
        assert_eq!(cfg.max_players, 100);
        assert_eq!(cfg.motd, "Custom Server");
        assert_eq!(cfg.view_distance, 16);
        assert_eq!(cfg.difficulty, "hard");

        // Non-overridden fields keep defaults.
        assert_eq!(cfg.simulation_distance, 10);
        assert!(cfg.generate_structures);
        assert_eq!(cfg.gamemode, "survival");
    }

    #[test]
    fn test_validate_valid_config() {
        let cfg = ServerConfig::default_config();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_port() {
        let mut cfg = ServerConfig::default_config();
        cfg.server_port = 0;

        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidPort(0)));
    }

    #[test]
    fn test_validate_invalid_view_distance() {
        // Too low
        let mut cfg = ServerConfig::default_config();
        cfg.view_distance = 0;
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidViewDistance(0)));

        // Too high
        cfg.view_distance = 100;
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidViewDistance(100)));
    }

    #[test]
    fn test_validate_invalid_simulation_distance() {
        let mut cfg = ServerConfig::default_config();
        cfg.simulation_distance = 1;
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidSimulationDistance(1)));

        cfg.simulation_distance = 33;
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidSimulationDistance(33)));
    }

    #[test]
    fn test_validate_invalid_max_players() {
        let mut cfg = ServerConfig::default_config();
        cfg.max_players = 0;
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidMaxPlayers(0)));
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("oxidized_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("server.properties");

        // Ensure clean state.
        let _ = fs::remove_file(&path);

        let mut original = ServerConfig::default_config();
        original.server_port = 19132;
        original.motd = "Roundtrip Test".to_string();
        original.online_mode = false;
        original.max_players = 50;
        original.view_distance = 16;
        original.difficulty = "hard".to_string();

        original.save(&path).expect("save should succeed");

        let loaded = ServerConfig::load_or_create(&path).expect("load should succeed");

        assert_eq!(loaded.server_port, 19132);
        assert_eq!(loaded.motd, "Roundtrip Test");
        assert!(!loaded.online_mode);
        assert_eq!(loaded.max_players, 50);
        assert_eq!(loaded.view_distance, 16);
        assert_eq!(loaded.difficulty, "hard");

        // Fields not overridden should still be defaults.
        assert_eq!(loaded.simulation_distance, 10);
        assert!(loaded.generate_structures);

        // Cleanup.
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn test_load_or_create_creates_file() {
        let dir = std::env::temp_dir().join(format!("oxidized_test_create_{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("server.properties");
        let _ = fs::remove_file(&path);

        assert!(!path.exists());

        let cfg = ServerConfig::load_or_create(&path).expect("should create default");
        assert!(path.exists());
        assert_eq!(cfg.server_port, 25565);

        // Verify the file content is parseable.
        let contents = fs::read_to_string(&path).expect("should read");
        assert!(contents.contains("server-port=25565"));

        // Cleanup.
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn test_parse_properties_line_continuation() {
        let input = "motd=Hello \\\nWorld\n";
        let props = parse_properties(input);

        assert_eq!(props.get("motd").map(String::as_str), Some("Hello World"));
    }

    #[test]
    fn test_parse_properties_empty_lines() {
        let input = "\n\nkey=value\n\n";
        let props = parse_properties(input);

        assert_eq!(props.len(), 1);
        assert_eq!(props.get("key").map(String::as_str), Some("value"));
    }

    #[test]
    fn test_epoch_days_to_ymd_known_dates() {
        // 1970-01-01
        assert_eq!(epoch_days_to_ymd(0), (1970, 1, 1));
        // 2000-01-01 = day 10957
        assert_eq!(epoch_days_to_ymd(10957), (2000, 1, 1));
        // 2025-06-15 = day 20254
        assert_eq!(epoch_days_to_ymd(20254), (2025, 6, 15));
    }

    #[test]
    fn test_unknown_keys_preserved_through_roundtrip() {
        let dir =
            std::env::temp_dir().join(format!("oxidized_test_unknown_keys_{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("server.properties");
        let _ = fs::remove_file(&path);

        // Write a properties file with a known key and an unknown key.
        let input = "server-port=19132\ncustom-setting=hello\n";
        fs::write(&path, input).expect("write should succeed");

        // Load → the unknown key should be captured internally.
        let config = ServerConfig::load_or_create(&path).expect("load should succeed");
        assert_eq!(config.server_port, 19132);

        // Save back to disk.
        config.save(&path).expect("save should succeed");

        // Re-read the file and verify the unknown key is present.
        let contents = fs::read_to_string(&path).expect("read should succeed");
        assert!(
            contents.contains("custom-setting=hello"),
            "unknown key should be preserved in saved output, got:\n{contents}"
        );

        // Also verify a known key is still present.
        assert!(contents.contains("server-port=19132"));

        // Verify the unknown key appears after known keys (separated by blank line).
        let known_pos = contents
            .find("status-heartbeat-interval=")
            .expect("last known key should exist");
        let unknown_pos = contents
            .find("custom-setting=hello")
            .expect("unknown key should exist");
        assert!(
            unknown_pos > known_pos,
            "unknown keys should appear after all known keys"
        );

        // Cleanup.
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }

    // -----------------------------------------------------------------------
    // All-keys parsing test
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_all_known_keys_from_properties() {
        // Every known key set to a non-default value.
        let input = "\
server-port=19132\n\
server-ip=127.0.0.1\n\
online-mode=false\n\
prevent-proxy-connections=true\n\
network-compression-threshold=512\n\
use-native-transport=false\n\
rate-limit=100\n\
gamemode=creative\n\
difficulty=hard\n\
hardcore=true\n\
force-gamemode=true\n\
max-players=50\n\
view-distance=16\n\
simulation-distance=8\n\
spawn-protection=0\n\
max-world-size=1000\n\
allow-flight=true\n\
level-name=custom_world\n\
level-seed=12345\n\
generate-structures=false\n\
max-chained-neighbor-updates=500\n\
spawn-npcs=false\n\
spawn-animals=false\n\
spawn-monsters=false\n\
allow-nether=false\n\
motd=Custom MOTD\n\
enable-status=false\n\
hide-online-players=true\n\
entity-broadcast-range-percentage=75\n\
white-list=true\n\
enforce-whitelist=true\n\
op-permission-level=2\n\
function-permission-level=4\n\
enforce-secure-profile=false\n\
log-ips=false\n\
max-tick-time=30000\n\
player-idle-timeout=15\n\
broadcast-console-to-ops=false\n\
broadcast-rcon-to-ops=false\n\
enable-rcon=true\n\
rcon.port=25576\n\
rcon.password=secret123\n\
enable-query=true\n\
query.port=25567\n\
resource-pack=https://example.com/pack.zip\n\
resource-pack-sha1=abc123\n\
resource-pack-prompt=Please accept\n\
require-resource-pack=true\n\
management-server-enabled=true\n\
management-server-host=0.0.0.0\n\
management-server-port=8080\n\
management-server-secret=topsecret\n\
management-server-tls-enabled=false\n\
enable-code-of-conduct=true\n\
bug-report-link=https://bugs.example.com\n\
sync-chunk-writes=false\n\
region-file-compression=lz4\n\
enable-jmx-monitoring=true\n\
text-filtering-config=/etc/filter.json\n\
text-filtering-version=2\n\
accepts-transfers=true\n\
pause-when-empty-seconds=120\n\
initial-enabled-packs=vanilla,custom\n\
initial-disabled-packs=experimental\n\
status-heartbeat-interval=30\n";

        let props = parse_properties(input);
        let cfg = ServerConfig::from_properties(&props);

        // Network
        assert_eq!(cfg.server_port, 19132, "server_port");
        assert_eq!(cfg.server_ip, "127.0.0.1", "server_ip");
        assert!(!cfg.online_mode, "online_mode should be false");
        assert!(
            cfg.prevent_proxy_connections,
            "prevent_proxy_connections should be true"
        );
        assert_eq!(
            cfg.network_compression_threshold, 512,
            "network_compression_threshold"
        );
        assert!(
            !cfg.use_native_transport,
            "use_native_transport should be false"
        );
        assert_eq!(cfg.rate_limit, 100, "rate_limit");

        // Gameplay
        assert_eq!(cfg.gamemode, "creative", "gamemode");
        assert_eq!(cfg.difficulty, "hard", "difficulty");
        assert!(cfg.hardcore, "hardcore should be true");
        assert!(cfg.force_gamemode, "force_gamemode should be true");
        assert_eq!(cfg.max_players, 50, "max_players");
        assert_eq!(cfg.view_distance, 16, "view_distance");
        assert_eq!(cfg.simulation_distance, 8, "simulation_distance");
        assert_eq!(cfg.spawn_protection, 0, "spawn_protection");
        assert_eq!(cfg.max_world_size, 1000, "max_world_size");
        assert!(cfg.allow_flight, "allow_flight should be true");

        // World
        assert_eq!(cfg.level_name, "custom_world", "level_name");
        assert_eq!(cfg.level_seed, "12345", "level_seed");
        assert!(
            !cfg.generate_structures,
            "generate_structures should be false"
        );
        assert_eq!(
            cfg.max_chained_neighbor_updates, 500,
            "max_chained_neighbor_updates"
        );
        assert!(!cfg.spawn_npcs, "spawn_npcs should be false");
        assert!(!cfg.spawn_animals, "spawn_animals should be false");
        assert!(!cfg.spawn_monsters, "spawn_monsters should be false");
        assert!(!cfg.allow_nether, "allow_nether should be false");

        // Display / MOTD
        assert_eq!(cfg.motd, "Custom MOTD", "motd");
        assert!(!cfg.enable_status, "enable_status should be false");
        assert!(
            cfg.hide_online_players,
            "hide_online_players should be true"
        );
        assert_eq!(
            cfg.entity_broadcast_range_percentage, 75,
            "entity_broadcast_range_percentage"
        );

        // Admin / Security
        assert!(cfg.white_list, "white_list should be true");
        assert!(cfg.enforce_whitelist, "enforce_whitelist should be true");
        assert_eq!(cfg.op_permission_level, 2, "op_permission_level");
        assert_eq!(
            cfg.function_permission_level, 4,
            "function_permission_level"
        );
        assert!(
            !cfg.enforce_secure_profile,
            "enforce_secure_profile should be false"
        );
        assert!(!cfg.log_ips, "log_ips should be false");
        assert_eq!(cfg.max_tick_time, 30_000, "max_tick_time");
        assert_eq!(cfg.player_idle_timeout, 15, "player_idle_timeout");
        assert!(
            !cfg.broadcast_console_to_ops,
            "broadcast_console_to_ops should be false"
        );
        assert!(
            !cfg.broadcast_rcon_to_ops,
            "broadcast_rcon_to_ops should be false"
        );

        // RCON
        assert!(cfg.enable_rcon, "enable_rcon should be true");
        assert_eq!(cfg.rcon_port, 25576, "rcon_port");
        assert_eq!(cfg.rcon_password, "secret123", "rcon_password");

        // Query
        assert!(cfg.enable_query, "enable_query should be true");
        assert_eq!(cfg.query_port, 25567, "query_port");

        // Resource pack
        assert_eq!(
            cfg.resource_pack, "https://example.com/pack.zip",
            "resource_pack"
        );
        assert_eq!(cfg.resource_pack_sha1, "abc123", "resource_pack_sha1");
        assert_eq!(
            cfg.resource_pack_prompt, "Please accept",
            "resource_pack_prompt"
        );
        assert!(
            cfg.require_resource_pack,
            "require_resource_pack should be true"
        );

        // Management server
        assert!(
            cfg.management_server_enabled,
            "management_server_enabled should be true"
        );
        assert_eq!(
            cfg.management_server_host, "0.0.0.0",
            "management_server_host"
        );
        assert_eq!(cfg.management_server_port, 8080, "management_server_port");
        assert_eq!(
            cfg.management_server_secret, "topsecret",
            "management_server_secret"
        );
        assert!(
            !cfg.management_server_tls_enabled,
            "management_server_tls_enabled should be false"
        );

        // Code of conduct
        assert!(
            cfg.enable_code_of_conduct,
            "enable_code_of_conduct should be true"
        );
        assert_eq!(
            cfg.bug_report_link, "https://bugs.example.com",
            "bug_report_link"
        );

        // Misc
        assert!(!cfg.sync_chunk_writes, "sync_chunk_writes should be false");
        assert_eq!(
            cfg.region_file_compression, "lz4",
            "region_file_compression"
        );
        assert!(
            cfg.enable_jmx_monitoring,
            "enable_jmx_monitoring should be true"
        );
        assert_eq!(
            cfg.text_filtering_config, "/etc/filter.json",
            "text_filtering_config"
        );
        assert_eq!(cfg.text_filtering_version, 2, "text_filtering_version");
        assert!(cfg.accepts_transfers, "accepts_transfers should be true");
        assert_eq!(
            cfg.pause_when_empty_seconds, 120,
            "pause_when_empty_seconds"
        );
        assert_eq!(
            cfg.initial_enabled_packs, "vanilla,custom",
            "initial_enabled_packs"
        );
        assert_eq!(
            cfg.initial_disabled_packs, "experimental",
            "initial_disabled_packs"
        );
        assert_eq!(
            cfg.status_heartbeat_interval, 30,
            "status_heartbeat_interval"
        );
    }

    // -----------------------------------------------------------------------
    // Boundary validation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_validate_port_accepts_min_valid() {
        let mut cfg = ServerConfig::default_config();
        cfg.server_port = 1;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_validate_port_accepts_max_valid() {
        let mut cfg = ServerConfig::default_config();
        cfg.server_port = 65535;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_validate_port_rejects_zero() {
        let mut cfg = ServerConfig::default_config();
        cfg.server_port = 0;
        assert!(matches!(cfg.validate(), Err(ConfigError::InvalidPort(0))));
    }

    #[test]
    fn test_validate_view_distance_accepts_min() {
        let mut cfg = ServerConfig::default_config();
        cfg.view_distance = 2;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_validate_view_distance_accepts_max() {
        let mut cfg = ServerConfig::default_config();
        cfg.view_distance = 32;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_validate_view_distance_rejects_below_min() {
        let mut cfg = ServerConfig::default_config();
        cfg.view_distance = 1;
        assert!(matches!(
            cfg.validate(),
            Err(ConfigError::InvalidViewDistance(1))
        ));
    }

    #[test]
    fn test_validate_view_distance_rejects_above_max() {
        let mut cfg = ServerConfig::default_config();
        cfg.view_distance = 33;
        assert!(matches!(
            cfg.validate(),
            Err(ConfigError::InvalidViewDistance(33))
        ));
    }

    #[test]
    fn test_validate_simulation_distance_accepts_min() {
        let mut cfg = ServerConfig::default_config();
        cfg.simulation_distance = 2;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_validate_simulation_distance_accepts_max() {
        let mut cfg = ServerConfig::default_config();
        cfg.simulation_distance = 32;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_validate_simulation_distance_rejects_below_min() {
        let mut cfg = ServerConfig::default_config();
        cfg.simulation_distance = 1;
        assert!(matches!(
            cfg.validate(),
            Err(ConfigError::InvalidSimulationDistance(1))
        ));
    }

    #[test]
    fn test_validate_simulation_distance_rejects_above_max() {
        let mut cfg = ServerConfig::default_config();
        cfg.simulation_distance = 33;
        assert!(matches!(
            cfg.validate(),
            Err(ConfigError::InvalidSimulationDistance(33))
        ));
    }

    #[test]
    fn test_validate_max_players_accepts_one() {
        let mut cfg = ServerConfig::default_config();
        cfg.max_players = 1;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_validate_max_players_rejects_zero() {
        let mut cfg = ServerConfig::default_config();
        cfg.max_players = 0;
        assert!(matches!(
            cfg.validate(),
            Err(ConfigError::InvalidMaxPlayers(0))
        ));
    }

    // -----------------------------------------------------------------------
    // Properties format edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_space_only_separator() {
        let input = "key value\n";
        let props = parse_properties(input);
        assert_eq!(props.get("key").map(String::as_str), Some("value"));
    }

    #[test]
    fn test_parse_colon_separator() {
        let input = "key:value\n";
        let props = parse_properties(input);
        assert_eq!(props.get("key").map(String::as_str), Some("value"));
    }

    #[test]
    fn test_parse_multiple_line_continuations() {
        let input = "motd=Hello \\\nbeautiful \\\nworld\n";
        let props = parse_properties(input);
        assert_eq!(
            props.get("motd").map(String::as_str),
            Some("Hello beautiful world")
        );
    }

    #[test]
    fn test_parse_exclamation_comment() {
        let input = "! this is a comment\nkey=value\n";
        let props = parse_properties(input);
        assert_eq!(props.len(), 1);
        assert_eq!(props.get("key").map(String::as_str), Some("value"));
    }

    #[test]
    fn test_parse_empty_value() {
        let input = "key=\n";
        let props = parse_properties(input);
        assert_eq!(props.get("key").map(String::as_str), Some(""));
    }

    #[test]
    fn test_parse_value_with_equals() {
        let input = "key=a=b\n";
        let props = parse_properties(input);
        assert_eq!(
            props.get("key").map(String::as_str),
            Some("a=b"),
            "only the first = should be treated as separator"
        );
    }

    #[test]
    fn test_full_roundtrip_preserves_all_fields() {
        let dir =
            std::env::temp_dir().join(format!("oxidized_test_full_rt_{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("server.properties");
        let _ = fs::remove_file(&path);

        let mut cfg = ServerConfig::default_config();

        // Set every field to a non-default value.
        cfg.server_port = 19132;
        cfg.server_ip = "192.168.1.1".to_string();
        cfg.online_mode = false;
        cfg.prevent_proxy_connections = true;
        cfg.network_compression_threshold = 512;
        cfg.use_native_transport = false;
        cfg.rate_limit = 10;
        cfg.gamemode = "creative".to_string();
        cfg.difficulty = "hard".to_string();
        cfg.hardcore = true;
        cfg.force_gamemode = true;
        cfg.max_players = 100;
        cfg.view_distance = 16;
        cfg.simulation_distance = 8;
        cfg.spawn_protection = 32;
        cfg.max_world_size = 10000;
        cfg.allow_flight = true;
        cfg.level_name = "custom_world".to_string();
        cfg.level_seed = "12345".to_string();
        cfg.generate_structures = false;
        cfg.max_chained_neighbor_updates = 500000;
        cfg.spawn_npcs = false;
        cfg.spawn_animals = false;
        cfg.spawn_monsters = false;
        cfg.allow_nether = false;
        cfg.motd = "Full Roundtrip Test".to_string();
        cfg.enable_status = false;
        cfg.hide_online_players = true;
        cfg.entity_broadcast_range_percentage = 200;
        cfg.white_list = true;
        cfg.enforce_whitelist = true;
        cfg.op_permission_level = 3;
        cfg.function_permission_level = 3;
        cfg.enforce_secure_profile = false;
        cfg.log_ips = false;
        cfg.max_tick_time = 120000;
        cfg.player_idle_timeout = 30;
        cfg.broadcast_console_to_ops = false;
        cfg.broadcast_rcon_to_ops = false;
        cfg.enable_rcon = true;
        cfg.rcon_port = 25576;
        cfg.rcon_password = "secret123".to_string();
        cfg.enable_query = true;
        cfg.query_port = 25566;
        cfg.resource_pack = "https://example.com/pack.zip".to_string();
        cfg.resource_pack_sha1 = "abc123def456".to_string();
        cfg.resource_pack_prompt = "Please install".to_string();
        cfg.require_resource_pack = true;
        cfg.management_server_enabled = true;
        cfg.management_server_host = "mgmt.example.com".to_string();
        cfg.management_server_port = 8443;
        cfg.management_server_secret = "mgmt-secret".to_string();
        cfg.management_server_tls_enabled = true;
        cfg.enable_code_of_conduct = true;
        cfg.bug_report_link = "https://bugs.example.com".to_string();
        cfg.sync_chunk_writes = false;
        cfg.region_file_compression = "none".to_string();
        cfg.enable_jmx_monitoring = true;
        cfg.text_filtering_config = "filter.json".to_string();
        cfg.text_filtering_version = 2;
        cfg.accepts_transfers = true;
        cfg.pause_when_empty_seconds = 120;
        cfg.initial_enabled_packs = "vanilla,fabric".to_string();
        cfg.initial_disabled_packs = "experimental".to_string();
        cfg.status_heartbeat_interval = 15;

        cfg.save(&path).expect("save should succeed");
        let loaded = ServerConfig::load_or_create(&path).expect("load should succeed");

        // Verify every field survived the roundtrip.
        assert_eq!(loaded.server_port, 19132);
        assert_eq!(loaded.server_ip, "192.168.1.1");
        assert!(!loaded.online_mode);
        assert!(loaded.prevent_proxy_connections);
        assert_eq!(loaded.network_compression_threshold, 512);
        assert!(!loaded.use_native_transport);
        assert_eq!(loaded.rate_limit, 10);
        assert_eq!(loaded.gamemode, "creative");
        assert_eq!(loaded.difficulty, "hard");
        assert!(loaded.hardcore);
        assert!(loaded.force_gamemode);
        assert_eq!(loaded.max_players, 100);
        assert_eq!(loaded.view_distance, 16);
        assert_eq!(loaded.simulation_distance, 8);
        assert_eq!(loaded.spawn_protection, 32);
        assert_eq!(loaded.max_world_size, 10000);
        assert!(loaded.allow_flight);
        assert_eq!(loaded.level_name, "custom_world");
        assert_eq!(loaded.level_seed, "12345");
        assert!(!loaded.generate_structures);
        assert_eq!(loaded.max_chained_neighbor_updates, 500000);
        assert!(!loaded.spawn_npcs);
        assert!(!loaded.spawn_animals);
        assert!(!loaded.spawn_monsters);
        assert!(!loaded.allow_nether);
        assert_eq!(loaded.motd, "Full Roundtrip Test");
        assert!(!loaded.enable_status);
        assert!(loaded.hide_online_players);
        assert_eq!(loaded.entity_broadcast_range_percentage, 200);
        assert!(loaded.white_list);
        assert!(loaded.enforce_whitelist);
        assert_eq!(loaded.op_permission_level, 3);
        assert_eq!(loaded.function_permission_level, 3);
        assert!(!loaded.enforce_secure_profile);
        assert!(!loaded.log_ips);
        assert_eq!(loaded.max_tick_time, 120000);
        assert_eq!(loaded.player_idle_timeout, 30);
        assert!(!loaded.broadcast_console_to_ops);
        assert!(!loaded.broadcast_rcon_to_ops);
        assert!(loaded.enable_rcon);
        assert_eq!(loaded.rcon_port, 25576);
        assert_eq!(loaded.rcon_password, "secret123");
        assert!(loaded.enable_query);
        assert_eq!(loaded.query_port, 25566);
        assert_eq!(loaded.resource_pack, "https://example.com/pack.zip");
        assert_eq!(loaded.resource_pack_sha1, "abc123def456");
        assert_eq!(loaded.resource_pack_prompt, "Please install");
        assert!(loaded.require_resource_pack);
        assert!(loaded.management_server_enabled);
        assert_eq!(loaded.management_server_host, "mgmt.example.com");
        assert_eq!(loaded.management_server_port, 8443);
        assert_eq!(loaded.management_server_secret, "mgmt-secret");
        assert!(loaded.management_server_tls_enabled);
        assert!(loaded.enable_code_of_conduct);
        assert_eq!(loaded.bug_report_link, "https://bugs.example.com");
        assert!(!loaded.sync_chunk_writes);
        assert_eq!(loaded.region_file_compression, "none");
        assert!(loaded.enable_jmx_monitoring);
        assert_eq!(loaded.text_filtering_config, "filter.json");
        assert_eq!(loaded.text_filtering_version, 2);
        assert!(loaded.accepts_transfers);
        assert_eq!(loaded.pause_when_empty_seconds, 120);
        assert_eq!(loaded.initial_enabled_packs, "vanilla,fabric");
        assert_eq!(loaded.initial_disabled_packs, "experimental");
        assert_eq!(loaded.status_heartbeat_interval, 15);

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }
}
