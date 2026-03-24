//! Display, MOTD, and chat formatting configuration.

use serde::{Deserialize, Serialize};

/// Display and MOTD settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DisplayConfig {
    /// Message of the day shown in the server list.
    pub motd: String,
    /// Respond to server-list pings (default `true`).
    pub is_status_enabled: bool,
    /// Hide player names from the server list (default `false`).
    pub is_hiding_online_players: bool,
    /// Entity tracking range as a percentage (default `100`).
    pub entity_broadcast_range_percentage: i32,
    /// Heartbeat interval in seconds for status polling (default `5`).
    pub status_heartbeat_interval: i32,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            motd: "An Oxidized Minecraft Server".to_string(),
            is_status_enabled: true,
            is_hiding_online_players: false,
            entity_broadcast_range_percentage: 100,
            status_heartbeat_interval: 5,
        }
    }
}

/// Chat formatting settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ChatConfig {
    /// Alternate color code prefix character (default `'&'`).
    ///
    /// Players and config strings (MOTD, etc.) can use this character
    /// instead of the standard `§` to apply color and formatting codes.
    /// Set to `""` to disable alternate color codes.
    pub color_char: String,
}

impl ChatConfig {
    /// Returns the configured color char, or `None` if disabled (empty string).
    pub fn color_char(&self) -> Option<char> {
        self.color_char.chars().next()
    }
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            color_char: "&".to_string(),
        }
    }
}
