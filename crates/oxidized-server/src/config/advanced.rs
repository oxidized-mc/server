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
