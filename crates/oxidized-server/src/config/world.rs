//! World generation and storage configuration.

use serde::{Deserialize, Serialize};

/// World generation and storage settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WorldConfig {
    /// Name of the world folder (default `"world"`).
    pub name: String,
    /// World seed; empty means random (default `""`).
    pub seed: String,
    /// Generate structures such as villages (default `true`).
    pub generate_structures: bool,
    /// Chunk view distance (default `10`).
    pub view_distance: u32,
    /// Simulation distance in chunks (default `10`).
    pub simulation_distance: u32,
    /// Synchronous chunk writes for data safety (default `true`).
    pub sync_chunk_writes: bool,
    /// Region file compression algorithm (default `"deflate"`).
    pub region_file_compression: String,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            name: "world".to_string(),
            seed: String::new(),
            generate_structures: true,
            view_distance: 10,
            simulation_distance: 10,
            sync_chunk_writes: true,
            region_file_compression: "deflate".to_string(),
        }
    }
}
