//! Chunk data structures: sections, palettes, bit storage, heightmaps, and light data.
//!
//! Mirrors the vanilla Java implementation with idiomatic Rust types.
//! All structures support network serialization matching the Minecraft wire format.
//!
//! # Coordinate System
//!
//! Chunk sections use Minecraft's coordinate system:
//! - **X**: 0–15 (west to east)
//! - **Y**: 0–15 (within section; world Y = `min_y + section_idx * 16 + local_y`)
//! - **Z**: 0–15 (north to south)
//! - **Index**: `(y * 16 + z) * 16 + x`
//!
//! Block-state containers (`PalettedContainer<BlockStates>`) use 16³ = 4096 entries.
//! Biome containers (`PalettedContainer<Biomes>`) use 4³ = 64 entries (one biome per
//! 4×4×4 sub-section).

pub mod bit_storage;
pub mod data_layer;
pub mod heightmap;
pub mod level_chunk;
pub mod palette;
mod palette_codec;
pub mod paletted_container;
pub mod section;
pub mod sky_light_sources;

pub use bit_storage::BitStorage;
pub use data_layer::DataLayer;
pub use heightmap::{Heightmap, HeightmapType};
pub use level_chunk::LevelChunk;
pub use paletted_container::PalettedContainer;
pub use section::LevelChunkSection;
pub use sky_light_sources::ChunkSkyLightSources;
