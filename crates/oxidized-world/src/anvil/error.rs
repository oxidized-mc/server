//! Errors for Anvil world storage operations.

use std::path::PathBuf;

use thiserror::Error;

/// Errors that can occur during Anvil file operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AnvilError {
    /// I/O error reading or writing a file.
    #[error("I/O error at {path}: {source}")]
    Io {
        /// The file path involved.
        path: PathBuf,
        /// The underlying I/O error.
        source: std::io::Error,
    },

    /// Unknown compression type byte in a region file.
    #[error("unknown compression type: {0}")]
    UnknownCompression(u8),

    /// Decompression failed.
    #[error("decompression failed: {0}")]
    Decompression(String),

    /// NBT parsing error.
    #[error("NBT error: {0}")]
    Nbt(#[from] oxidized_nbt::NbtError),

    /// Required NBT field is missing.
    #[error("missing NBT field: {field}")]
    MissingField {
        /// The name of the missing field.
        field: &'static str,
    },

    /// NBT field has an unexpected type.
    #[error("wrong type for NBT field '{field}': expected {expected}")]
    WrongFieldType {
        /// The field name.
        field: &'static str,
        /// What was expected.
        expected: &'static str,
    },

    /// Chunk data is corrupted or invalid.
    #[error("corrupted chunk at ({chunk_x}, {chunk_z}): {reason}")]
    CorruptedChunk {
        /// Chunk X coordinate.
        chunk_x: i32,
        /// Chunk Z coordinate.
        chunk_z: i32,
        /// Description of what went wrong.
        reason: String,
    },

    /// Region file header is invalid.
    #[error("invalid region file header: {0}")]
    InvalidHeader(String),

    /// Block name not found in the registry.
    #[error("unknown block: {0}")]
    UnknownBlock(String),

    /// Palette/container error from chunk data structures.
    #[error("container error: {0}")]
    Container(#[from] crate::chunk::paletted_container::PalettedContainerError),

    /// Bit storage error.
    #[error("bit storage error: {0}")]
    BitStorage(#[from] crate::chunk::bit_storage::BitStorageError),

    /// Internal error (mutex poison, task join failure, etc).
    #[error("internal error: {0}")]
    Internal(String),
}

impl AnvilError {
    /// Create an I/O error with associated path context.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
