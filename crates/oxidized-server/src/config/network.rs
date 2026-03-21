//! Network-related configuration.

use serde::{Deserialize, Serialize};

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

impl Default for NetworkConfig {
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
