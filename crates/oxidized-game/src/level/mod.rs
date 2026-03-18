//! Server level module: block access, chunk caching, and dimension management.
//!
//! This module provides the runtime world representation. The key types are:
//!
//! - [`BlockFlags`] — bitflag set for `set_block_state` update control
//! - [`BlockGetter`] — trait for read-only block access
//! - [`LevelWriter`] — trait for read-write block access
//! - [`DimensionType`] — static dimension properties (height, lighting, etc.)
//! - [`ChunkCache`] — LRU cache of loaded chunks
//! - [`ServerLevel`] — runtime world, owns `ChunkCache` + `AsyncChunkLoader`
//! - [`DimensionManager`] — manages multiple dimensions

pub mod chunk_cache;
pub mod dimension;
pub mod dimension_manager;
pub mod error;
pub mod flags;
pub mod server_level;
pub mod traits;

pub use chunk_cache::ChunkCache;
pub use dimension::DimensionType;
pub use dimension_manager::DimensionManager;
pub use error::LevelError;
pub use flags::BlockFlags;
pub use server_level::ServerLevel;
pub use traits::{AIR_STATE_ID, BlockGetter, LevelWriter};
