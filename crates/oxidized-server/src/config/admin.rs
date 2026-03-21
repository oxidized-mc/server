//! Admin, security, RCON, and query configuration.

use std::fmt;

use serde::{Deserialize, Serialize};

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

impl Default for AdminConfig {
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

impl Default for RconConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 25575,
            password: String::new(),
        }
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

impl Default for QueryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 25565,
        }
    }
}
