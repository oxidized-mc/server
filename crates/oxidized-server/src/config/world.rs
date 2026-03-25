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
    pub is_generating_structures: bool,
    /// Chunk view distance (default `10`).
    pub view_distance: u32,
    /// Simulation distance in chunks (default `10`).
    pub simulation_distance: u32,
    /// Region file compression algorithm (default `"deflate"`).
    pub region_file_compression: String,
    /// Maximum chunks kept in the in-memory cache (default `1024`).
    pub chunk_cache_size: usize,
    /// Maximum concurrent chunk generation tasks (default `64`).
    pub max_concurrent_chunk_generations: usize,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            name: "world".to_string(),
            seed: String::new(),
            is_generating_structures: true,
            view_distance: 10,
            simulation_distance: 10,
            region_file_compression: "deflate".to_string(),
            chunk_cache_size: 1024,
            max_concurrent_chunk_generations: 64,
        }
    }
}
