//! Chunk data structures: sections, palettes, bit storage, heightmaps, and light data.
//!
//! Mirrors the vanilla Java implementation with idiomatic Rust types.
//! All structures support network serialization matching the Minecraft wire format.

pub mod bit_storage;
pub mod data_layer;
pub mod heightmap;
pub mod level_chunk;
pub mod palette;
mod palette_codec;
pub mod paletted_container;
pub mod section;

pub use bit_storage::BitStorage;
pub use data_layer::DataLayer;
pub use heightmap::{Heightmap, HeightmapType};
pub use level_chunk::{ChunkPos, LevelChunk};
pub use paletted_container::PalettedContainer;
pub use section::LevelChunkSection;
