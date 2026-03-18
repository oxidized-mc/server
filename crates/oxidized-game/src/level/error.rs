//! [`LevelError`] — typed errors for level operations.

use oxidized_world::chunk::level_chunk::ChunkError;

/// Errors that can occur during level operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LevelError {
    /// The requested chunk is not loaded.
    #[error("chunk not loaded at ({chunk_x}, {chunk_z})")]
    ChunkNotLoaded {
        /// Chunk X coordinate.
        chunk_x: i32,
        /// Chunk Z coordinate.
        chunk_z: i32,
    },

    /// Block position is outside valid world bounds.
    #[error("position out of bounds: ({x}, {y}, {z})")]
    OutOfBounds {
        /// X coordinate.
        x: i32,
        /// Y coordinate.
        y: i32,
        /// Z coordinate.
        z: i32,
    },

    /// Error from the chunk layer.
    #[error("chunk error: {0}")]
    Chunk(#[from] ChunkError),

    /// I/O error during chunk loading.
    #[error("chunk I/O error: {0}")]
    Io(String),
}
