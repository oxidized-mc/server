//! Configuration validation errors.

/// Errors that can occur when validating server configuration.
#[derive(Debug, thiserror::Error)]
#[allow(clippy::enum_variant_names)]
#[non_exhaustive]
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

    /// Color char must be a single non-alphanumeric ASCII character (or empty to disable).
    #[error("invalid color_char: \"{0}\" (must be a single non-alphanumeric ASCII char or empty)")]
    InvalidColorChar(String),
}
