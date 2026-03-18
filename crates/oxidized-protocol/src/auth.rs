//! Mojang session server authentication.
//!
//! Handles online-mode player authentication by verifying with Mojang's
//! session server that a player has legitimately authenticated.
//!
//! The flow:
//! 1. Server sends RSA public key + challenge to client
//! 2. Client encrypts shared secret + challenge with server's public key
//! 3. Server decrypts, computes auth hash (`minecraft_digest`)
//! 4. Server calls `hasJoined` on Mojang's session server
//! 5. Mojang returns the player's profile (UUID, name, skin)

use serde::Deserialize;
use thiserror::Error;
use uuid::Uuid;

/// Mojang session server `hasJoined` endpoint.
const SESSION_SERVER_URL: &str = "https://sessionserver.mojang.com/session/minecraft/hasJoined";

/// Errors from Mojang authentication.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AuthError {
    /// The Mojang session server did not verify the player.
    #[error("authentication failed for player '{0}'")]
    NotAuthenticated(String),

    /// The Mojang session server is unreachable.
    #[error("authentication servers are unavailable: {0}")]
    ServerUnavailable(String),

    /// Failed to parse the session server response.
    #[error("invalid session server response: {0}")]
    InvalidResponse(String),
}

/// A player profile returned by the Mojang session server.
#[derive(Debug, Clone, Deserialize)]
pub struct GameProfile {
    /// UUID as a hex string without dashes (e.g. "550e8400e29b41d4a716446655440000").
    id: String,
    /// Player's display name.
    name: String,
    /// Profile properties (textures, cape, etc.).
    #[serde(default)]
    properties: Vec<ProfileProperty>,
}

impl GameProfile {
    /// Returns the player's UUID parsed from the hex string.
    ///
    /// # Errors
    ///
    /// Returns `None` if the UUID string is malformed.
    pub fn uuid(&self) -> Option<Uuid> {
        Uuid::parse_str(&self.id).ok()
    }

    /// Returns the player's display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the profile properties (textures, cape, etc.).
    pub fn properties(&self) -> &[ProfileProperty] {
        &self.properties
    }
}

/// A single property in a player profile.
#[derive(Debug, Clone, Deserialize)]
pub struct ProfileProperty {
    /// Property name (e.g. "textures").
    name: String,
    /// Base64-encoded property value.
    value: String,
    /// Optional RSA signature (base64-encoded).
    signature: Option<String>,
}

impl ProfileProperty {
    /// Returns the property name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the base64-encoded property value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the optional base64-encoded RSA signature.
    pub fn signature(&self) -> Option<&str> {
        self.signature.as_deref()
    }
}

/// Verifies a player's session with the Mojang session server.
///
/// Sends a GET request to:
/// ```text
/// https://sessionserver.mojang.com/session/minecraft/hasJoined
///   ?username=<username>&serverId=<server_hash>[&ip=<client_ip>]
/// ```
///
/// # Arguments
///
/// * `client` - An HTTP client (reuse across requests for connection pooling)
/// * `username` - The player's username
/// * `server_hash` - The auth hash from [`crate::crypto::minecraft_digest`]
/// * `client_ip` - Optional client IP (for `prevent-proxy-connections`)
///
/// # Errors
///
/// Returns [`AuthError`] if the player is not authenticated, the
/// session server is unreachable, or the response is malformed.
pub async fn has_joined(
    client: &reqwest::Client,
    username: &str,
    server_hash: &str,
    client_ip: Option<&str>,
) -> Result<GameProfile, AuthError> {
    // Build URL with proper encoding to prevent parameter injection.
    let mut url = format!(
        "{}?username={}&serverId={}",
        SESSION_SERVER_URL,
        urlencoding::encode(username),
        urlencoding::encode(server_hash),
    );
    if let Some(ip) = client_ip {
        url.push_str("&ip=");
        url.push_str(&urlencoding::encode(ip));
    }

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AuthError::ServerUnavailable(e.to_string()))?;

    let status = response.status();

    if status == reqwest::StatusCode::NO_CONTENT || status == reqwest::StatusCode::NOT_FOUND {
        return Err(AuthError::NotAuthenticated(username.to_string()));
    }

    if !status.is_success() {
        return Err(AuthError::ServerUnavailable(format!(
            "unexpected status: {status}"
        )));
    }

    let profile: GameProfile = response
        .json()
        .await
        .map_err(|e| AuthError::InvalidResponse(e.to_string()))?;

    Ok(profile)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_game_profile_deserialize() {
        let json = r#"{
            "id": "550e8400e29b41d4a716446655440000",
            "name": "Steve",
            "properties": [
                {
                    "name": "textures",
                    "value": "dGV4dHVyZXM=",
                    "signature": "c2lnbmF0dXJl"
                }
            ]
        }"#;

        let profile: GameProfile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.name(), "Steve");
        assert_eq!(
            profile.uuid().unwrap().to_string(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
        assert_eq!(profile.properties().len(), 1);
        assert_eq!(profile.properties()[0].name(), "textures");
        assert_eq!(profile.properties()[0].value(), "dGV4dHVyZXM=");
        assert_eq!(profile.properties()[0].signature(), Some("c2lnbmF0dXJl"));
    }

    #[test]
    fn test_game_profile_no_properties() {
        let json = r#"{"id": "550e8400e29b41d4a716446655440000", "name": "Alex"}"#;
        let profile: GameProfile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.name(), "Alex");
        assert!(profile.properties().is_empty());
    }

    #[test]
    fn test_game_profile_invalid_uuid() {
        let json = r#"{"id": "not-a-uuid", "name": "Bad"}"#;
        let profile: GameProfile = serde_json::from_str(json).unwrap();
        assert!(profile.uuid().is_none());
    }

    #[test]
    fn test_property_without_signature() {
        let json = r#"{"name": "textures", "value": "dGV4dHVyZXM="}"#;
        let prop: ProfileProperty = serde_json::from_str(json).unwrap();
        assert!(prop.signature().is_none());
    }
}
