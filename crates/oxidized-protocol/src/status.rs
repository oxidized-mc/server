//! Server status data sent to clients during the STATUS protocol state.
//!
//! This corresponds to `net.minecraft.network.protocol.status.ServerStatus`
//! in the vanilla server. The status is serialized as JSON and sent in a
//! [`ClientboundStatusResponsePacket`](crate::packets::status::ClientboundStatusResponsePacket).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Re-export Component from the chat module for backward compatibility.
pub use crate::chat::Component;

// ---------------------------------------------------------------------------
// ServerStatus
// ---------------------------------------------------------------------------

/// The full server status payload sent in the STATUS protocol state.
///
/// Serializes to the JSON format expected by vanilla Minecraft clients.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerStatus {
    /// Version information (name + protocol number).
    pub version: StatusVersion,
    /// Player count and optional sample list.
    pub players: StatusPlayers,
    /// The server's MOTD as a chat component.
    pub description: Component,
    /// Base64-encoded PNG favicon, prefixed with `data:image/png;base64,`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favicon: Option<String>,
    /// Whether the server enforces secure chat signing.
    #[serde(rename = "enforcesSecureChat")]
    pub is_secure_chat_enforced: bool,
}

/// Version block within the server status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusVersion {
    /// Human-readable version name (e.g., "26.1-pre-3").
    pub name: String,
    /// Numeric protocol version.
    pub protocol: i32,
}

/// Player count block within the server status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusPlayers {
    /// Maximum player capacity.
    pub max: u32,
    /// Current online player count.
    pub online: u32,
    /// Optional sample of online players (shown on hover in the server list).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub sample: Vec<PlayerSample>,
}

/// A single player entry in the status player sample.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerSample {
    /// The player's display name.
    pub name: String,
    /// The player's UUID.
    pub id: Uuid,
}

impl ServerStatus {
    /// Serializes the status to a JSON string.
    ///
    /// # Errors
    ///
    /// Returns a [`serde_json::Error`] if serialization fails (should not
    /// happen for valid data).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::constants;

    fn sample_status() -> ServerStatus {
        ServerStatus {
            version: StatusVersion {
                name: constants::VERSION_NAME.to_string(),
                protocol: constants::PROTOCOL_VERSION,
            },
            players: StatusPlayers {
                max: 20,
                online: 0,
                sample: Vec::new(),
            },
            description: Component::text("An Oxidized Minecraft Server"),
            favicon: None,
            is_secure_chat_enforced: false,
        }
    }

    #[test]
    fn test_status_json_has_required_fields() {
        let status = sample_status();
        let json_str = status.to_json().expect("serialize");
        let json: serde_json::Value = serde_json::from_str(&json_str).expect("parse");

        assert_eq!(json["version"]["name"], constants::VERSION_NAME);
        assert_eq!(json["version"]["protocol"], constants::PROTOCOL_VERSION);
        assert_eq!(json["players"]["max"], 20);
        assert_eq!(json["players"]["online"], 0);
        assert_eq!(json["description"]["text"], "An Oxidized Minecraft Server");
        assert_eq!(json["enforcesSecureChat"], false);
    }

    #[test]
    fn test_status_json_omits_null_favicon() {
        let status = sample_status();
        let json_str = status.to_json().expect("serialize");
        let json: serde_json::Value = serde_json::from_str(&json_str).expect("parse");
        assert!(json.get("favicon").is_none());
    }

    #[test]
    fn test_status_json_includes_favicon_when_set() {
        let mut status = sample_status();
        status.favicon = Some("data:image/png;base64,AAAA".to_string());
        let json_str = status.to_json().expect("serialize");
        let json: serde_json::Value = serde_json::from_str(&json_str).expect("parse");
        assert_eq!(json["favicon"], "data:image/png;base64,AAAA");
    }

    #[test]
    fn test_status_json_omits_empty_sample() {
        let status = sample_status();
        let json_str = status.to_json().expect("serialize");
        let json: serde_json::Value = serde_json::from_str(&json_str).expect("parse");
        assert!(json["players"].get("sample").is_none());
    }

    #[test]
    fn test_status_json_includes_player_sample() {
        let mut status = sample_status();
        status.players.online = 1;
        status.players.sample.push(PlayerSample {
            name: "TestPlayer".to_string(),
            id: Uuid::nil(),
        });
        let json_str = status.to_json().expect("serialize");
        let json: serde_json::Value = serde_json::from_str(&json_str).expect("parse");
        assert_eq!(json["players"]["sample"][0]["name"], "TestPlayer");
        assert_eq!(
            json["players"]["sample"][0]["id"],
            "00000000-0000-0000-0000-000000000000"
        );
    }

    #[test]
    fn test_status_roundtrip_deserialize() {
        let status = sample_status();
        let json = status.to_json().expect("serialize");
        let deserialized: ServerStatus = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized, status);
    }

    #[test]
    fn test_component_from_str() {
        let comp = Component::from("hello");
        assert_eq!(format!("{comp}"), "hello");
    }

    #[test]
    fn test_component_text_constructor() {
        let comp = Component::text("world".to_string());
        assert_eq!(format!("{comp}"), "world");
    }
}
