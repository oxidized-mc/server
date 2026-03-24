//! NBT (Named Binary Tag) read/write implementation for Minecraft.
//!
//! Supports all 13 tag types, Modified UTF-8, memory accounting,
//! GZIP/zlib compression, and the unnamed root tag format used by
//! both disk files and network packets.

mod accounter;
mod compound;
mod error;
mod io;
mod list;
mod mutf8;
mod reader;
mod serde;
mod snbt;
mod tag;
mod writer;

pub use self::serde::{from_compound, to_compound};
pub use accounter::NbtAccounter;
pub use compound::NbtCompound;
pub use error::*;
pub use io::*;
pub use list::NbtList;
pub use mutf8::{decode_modified_utf8, encode_modified_utf8};
pub use reader::{read_named_tag, read_nbt, read_network_nbt};
pub use snbt::{format_snbt, format_snbt_pretty, parse_snbt};
pub use tag::NbtTag;
pub use writer::{write_named_tag, write_nbt, write_network_nbt};
