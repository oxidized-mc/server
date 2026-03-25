//! Resource pack, management server, data pack, and advanced configuration.

use std::fmt;

use serde::{Deserialize, Serialize};

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
    pub is_required: bool,
}

/// Management server settings (26.1 feature).
///
/// Implements a custom [`Debug`] that redacts `secret`.
#[derive(Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct ManagementConfig {
    /// Enable the management server (default `false`).
    pub is_enabled: bool,
    /// Management server host (default `""`).
    pub host: String,
    /// Management server port; `0` means auto-assign (default `0`).
    pub port: u16,
    /// Shared secret for the management server (default `""`).
    pub secret: String,
    /// Require TLS on the management server (default `false`).
    pub is_tls_enabled: bool,
}

impl fmt::Debug for ManagementConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ManagementConfig")
            .field("is_enabled", &self.is_enabled)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("secret", &"[REDACTED]")
            .field("is_tls_enabled", &self.is_tls_enabled)
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AdvancedConfig {
    /// External text-filtering service config path (default `""`).
    pub text_filtering_config: String,
    /// Text-filtering protocol version (default `0`).
    pub text_filtering_version: i32,
    /// Show a code-of-conduct prompt on join (default `false`).
    pub is_code_of_conduct_enabled: bool,
    /// Link to the server's bug report page (default `""`).
    pub bug_report_link: String,
    /// Inbound packet channel capacity per connection (default `128`).
    ///
    /// Backpressure: when game logic is slow to consume packets, the reader
    /// task blocks, triggering TCP flow control on the client.
    pub inbound_channel_capacity: usize,
    /// Outbound packet channel capacity per connection (default `512`).
    ///
    /// Sized for burst traffic (join sequence, chunk loading). If the writer
    /// cannot drain fast enough (slow client), senders see backpressure.
    pub outbound_channel_capacity: usize,
}

impl AdvancedConfig {
    /// Default inbound channel capacity (matches protocol crate default).
    pub const DEFAULT_INBOUND_CHANNEL_CAPACITY: usize = 128;
    /// Default outbound channel capacity (matches protocol crate default).
    pub const DEFAULT_OUTBOUND_CHANNEL_CAPACITY: usize = 512;
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            text_filtering_config: String::new(),
            text_filtering_version: 0,
            is_code_of_conduct_enabled: false,
            bug_report_link: String::new(),
            inbound_channel_capacity: Self::DEFAULT_INBOUND_CHANNEL_CAPACITY,
            outbound_channel_capacity: Self::DEFAULT_OUTBOUND_CHANNEL_CAPACITY,
        }
    }
}
