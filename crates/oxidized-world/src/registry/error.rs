//! Registry error types.

use thiserror::Error;

/// Errors that can occur when loading or querying registries.
#[derive(Debug, Error)]
pub enum RegistryError {
    /// Failed to decompress gzipped registry data.
    #[error("failed to decompress registry data: {0}")]
    Decompress(#[from] std::io::Error),

    /// Failed to parse registry JSON.
    #[error("failed to parse registry JSON: {0}")]
    Json(#[from] serde_json::Error),

    /// A block state ID was out of the valid u16 range.
    #[error("block state ID {0} exceeds u16 range")]
    InvalidStateId(u64),

    /// A state entry in the JSON is missing a required "id" field.
    #[error("block state missing 'id' field in block {0}")]
    MissingStateId(String),

    /// An item property value exceeded the valid range.
    #[error("item '{0}' has invalid {1} value: {2}")]
    InvalidItemProperty(String, &'static str, u64),

    /// The requested block name was not found in the registry.
    #[error("unknown block: {0}")]
    UnknownBlock(String),

    /// The requested item name was not found in the registry.
    #[error("unknown item: {0}")]
    UnknownItem(String),
}
