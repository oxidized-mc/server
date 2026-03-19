//! Error types and NBT tag type constants.

use thiserror::Error;

/// NBT tag type ID for End (compound terminator).
pub const TAG_END: u8 = 0;
/// NBT tag type ID for Byte (`i8`).
pub const TAG_BYTE: u8 = 1;
/// NBT tag type ID for Short (`i16`).
pub const TAG_SHORT: u8 = 2;
/// NBT tag type ID for Int (`i32`).
pub const TAG_INT: u8 = 3;
/// NBT tag type ID for Long (`i64`).
pub const TAG_LONG: u8 = 4;
/// NBT tag type ID for Float (`f32`).
pub const TAG_FLOAT: u8 = 5;
/// NBT tag type ID for Double (`f64`).
pub const TAG_DOUBLE: u8 = 6;
/// NBT tag type ID for ByteArray (`Vec<i8>`).
pub const TAG_BYTE_ARRAY: u8 = 7;
/// NBT tag type ID for String.
pub const TAG_STRING: u8 = 8;
/// NBT tag type ID for List.
pub const TAG_LIST: u8 = 9;
/// NBT tag type ID for Compound.
pub const TAG_COMPOUND: u8 = 10;
/// NBT tag type ID for IntArray (`Vec<i32>`).
pub const TAG_INT_ARRAY: u8 = 11;
/// NBT tag type ID for LongArray (`Vec<i64>`).
pub const TAG_LONG_ARRAY: u8 = 12;

/// Default memory quota for network NBT (2 MiB) — matches vanilla.
pub const DEFAULT_QUOTA: usize = 2_097_152;
/// Memory quota for uncompressed disk NBT (100 MiB) — matches vanilla.
pub const UNCOMPRESSED_QUOTA: usize = 104_857_600;
/// Maximum nesting depth — matches vanilla.
pub const MAX_DEPTH: usize = 512;

/// Errors that can occur during NBT reading, writing, or manipulation.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum NbtError {
    /// The tag type ID is not in the valid range 0–12.
    #[error("invalid tag type ID: {0}")]
    InvalidTagType(u8),

    /// The cumulative memory accounting exceeded the quota.
    #[error("NBT size limit exceeded: used {used} bytes, quota {quota} bytes")]
    SizeLimit {
        /// Bytes consumed so far (including the allocation that triggered the error).
        used: usize,
        /// Maximum allowed bytes.
        quota: usize,
    },

    /// The nesting depth of compounds/lists exceeded the maximum.
    #[error("NBT nesting depth exceeded: depth {depth}, max {max}")]
    DepthLimit {
        /// Current depth when the error was raised.
        depth: usize,
        /// Configured maximum depth.
        max: usize,
    },

    /// Modified UTF-8 data could not be decoded.
    #[error("invalid Modified UTF-8 data")]
    InvalidUtf8,

    /// The reader ran out of bytes before a complete tag was read.
    #[error("unexpected end of NBT data")]
    UnexpectedEnd,

    /// A structural violation in the binary data.
    #[error("invalid NBT format: {0}")]
    InvalidFormat(String),

    /// A tag pushed into a list does not match the list's element type.
    #[error("list type mismatch: expected tag type {expected}, got {got}")]
    ListTypeMismatch {
        /// The element type the list was declared with.
        expected: u8,
        /// The tag type that was actually provided.
        got: u8,
    },

    /// SNBT parse error at a specific position.
    #[error("SNBT parse error at position {pos}: {message}")]
    SnbtParse {
        /// Byte offset in the input where the error occurred.
        pos: usize,
        /// Human-readable description of the parse error.
        message: String,
    },

    /// Serde serialization or deserialization error.
    #[error("serde error: {0}")]
    SerdeError(String),

    /// An underlying I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_error_display_snapshots() {
        insta::assert_snapshot!(
            "invalid_tag_type",
            format!("{}", NbtError::InvalidTagType(99))
        );
        insta::assert_snapshot!(
            "size_limit",
            format!(
                "{}",
                NbtError::SizeLimit {
                    used: 3_000_000,
                    quota: 2_097_152,
                }
            )
        );
        insta::assert_snapshot!(
            "depth_limit",
            format!(
                "{}",
                NbtError::DepthLimit {
                    depth: 513,
                    max: 512,
                }
            )
        );
        insta::assert_snapshot!("invalid_utf8", format!("{}", NbtError::InvalidUtf8));
        insta::assert_snapshot!("unexpected_end", format!("{}", NbtError::UnexpectedEnd));
        insta::assert_snapshot!(
            "invalid_format",
            format!(
                "{}",
                NbtError::InvalidFormat("missing root compound tag".into())
            )
        );
        insta::assert_snapshot!(
            "list_type_mismatch",
            format!(
                "{}",
                NbtError::ListTypeMismatch {
                    expected: 3,
                    got: 8,
                }
            )
        );
        insta::assert_snapshot!(
            "snbt_parse",
            format!(
                "{}",
                NbtError::SnbtParse {
                    pos: 42,
                    message: "unexpected character".into(),
                }
            )
        );
        insta::assert_snapshot!(
            "serde_error",
            format!("{}", NbtError::SerdeError("field not found".into()))
        );
        insta::assert_snapshot!(
            "io_error",
            format!(
                "{}",
                NbtError::Io(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
            )
        );
    }
}
