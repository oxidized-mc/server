//! Anvil world storage format: region files, chunk loading, and compression.
//!
//! Implements reading of the Anvil `.mca` region file format and
//! deserialization of chunk NBT data into [`LevelChunk`](crate::chunk::LevelChunk)
//! structures.

mod chunk_loader;
mod compression;
mod error;
mod region;

pub use chunk_loader::{AnvilChunkLoader, AsyncChunkLoader};
pub use compression::CompressionType;
pub use error::AnvilError;
pub use region::{HEADER_BYTES, OffsetEntry, REGION_SIZE, RegionFile, SECTOR_BYTES, SECTOR_INTS};
