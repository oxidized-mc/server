//! Network-related configuration.

use serde::{Deserialize, Serialize};

/// Network timeout settings.
///
/// All values are in seconds. These control how long the server waits
/// before disconnecting unresponsive clients in each connection phase.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NetworkTimeoutsConfig {
    /// Seconds between keepalive pings (default `15`).
    pub keepalive_interval_secs: u64,
    /// Seconds before a missing keepalive response disconnects the client (default `30`).
    pub keepalive_timeout_secs: u64,
    /// Seconds to complete the login phase (default `30`).
    pub login_timeout_secs: u64,
    /// Seconds to complete the configuration phase (default `30`).
    pub configuration_timeout_secs: u64,
    /// Seconds before a slow-write client is disconnected (default `30`).
    pub write_timeout_secs: u64,
}

impl Default for NetworkTimeoutsConfig {
    fn default() -> Self {
        Self {
            keepalive_interval_secs: 15,
            keepalive_timeout_secs: 30,
            login_timeout_secs: 30,
            configuration_timeout_secs: 30,
            write_timeout_secs: 30,
        }
    }
}

/// Connection rate-limiting settings.
///
/// Limits the number of new TCP connections a single IP address can open
/// within a sliding time window. Helps mitigate connection-flood attacks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RateLimitConfig {
    /// Max new connections per IP within the window (default `10`).
    pub max_connections_per_window: u32,
    /// Duration of the rate-limit window in seconds (default `10`).
    pub window_secs: u64,
    /// Seconds between stale-entry cleanup passes (default `60`).
    pub cleanup_interval_secs: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_connections_per_window: 10,
            window_secs: 10,
            cleanup_interval_secs: 60,
        }
    }
}

/// Network-related settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NetworkConfig {
    /// Port the server listens on (default `25565`).
    pub port: u16,
    /// IP address to bind to (default `""` — all interfaces).
    pub ip: String,
    /// Whether the server authenticates players with Mojang (default `true`).
    pub is_online_mode: bool,
    /// Block proxy / VPN connections (default `false`).
    pub is_preventing_proxy_connections: bool,
    /// Byte threshold for packet compression; `-1` disables (default `256`).
    pub compression_threshold: i32,
    /// Maximum packets per second before kicking; `0` disables (default `0`).
    pub rate_limit: i32,
    /// Accept transfer packets from other servers (default `false`).
    pub is_accepting_transfers: bool,
    /// Network timeout settings.
    pub timeouts: NetworkTimeoutsConfig,
    /// Connection rate-limiting settings.
    pub connection_rate_limit: RateLimitConfig,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            port: 25565,
            ip: String::new(),
            is_online_mode: true,
            is_preventing_proxy_connections: false,
            compression_threshold: 256,
            rate_limit: 0,
            is_accepting_transfers: false,
            timeouts: NetworkTimeoutsConfig::default(),
            connection_rate_limit: RateLimitConfig::default(),
        }
    }
}
