//! Flat world generation.
//!
//! Generates uniform layer-based terrain where every chunk is identical.
//! The default configuration matches vanilla: 1 bedrock + 2 dirt + 1 grass block.

pub mod config;
pub mod generator;

pub use config::{FlatConfigError, FlatLayerInfo, FlatWorldConfig};
pub use generator::FlatChunkGenerator;
