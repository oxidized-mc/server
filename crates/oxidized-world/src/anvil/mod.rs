//! Anvil world storage format: region files, chunk loading/saving, and compression.
//!
//! Implements reading and writing of the Anvil `.mca` region file format,
//! serialization/deserialization of chunk NBT data, and zlib compression.

mod chunk_loader;
mod chunk_serializer;
mod compression;
mod error;
mod region;

pub use chunk_loader::{AnvilChunkLoader, AsyncChunkLoader};
pub use chunk_serializer::ChunkSerializer;
pub use compression::{CompressionType, compress_zlib, compress_zlib_level};
pub use error::AnvilError;
pub use region::{HEADER_BYTES, OffsetEntry, REGION_SIZE, RegionFile, SECTOR_BYTES, SECTOR_INTS};
