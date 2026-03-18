//! [`ResourceLocation`] — a namespaced identifier like `minecraft:stone`.
//!
//! Used throughout the Minecraft protocol for registry keys, block/item IDs,
//! dimension types, biome names, and many other identifiers.

use std::fmt;

use bytes::{Bytes, BytesMut};
use thiserror::Error;

use crate::codec::types::{self, TypeError};

/// The default namespace used when no colon is present in a resource location
/// string (e.g. `"stone"` becomes `"minecraft:stone"`).
pub const DEFAULT_NAMESPACE: &str = "minecraft";

/// Maximum length of a resource location string on the wire (namespace + `:` + path).
const MAX_RESOURCE_LOCATION_LENGTH: usize = 32767;

/// Errors that can occur when constructing or parsing a [`ResourceLocation`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ResourceLocationError {
    /// The namespace contains invalid characters.
    #[error("invalid namespace '{0}': must match [a-z0-9_.-] and not contain \"..\"")]
    InvalidNamespace(String),

    /// The path contains invalid characters.
    #[error("invalid path '{0}': must match [a-z0-9/._-]")]
    InvalidPath(String),

    /// The namespace is empty.
    #[error("namespace must not be empty")]
    EmptyNamespace,

    /// The path is empty.
    #[error("path must not be empty")]
    EmptyPath,

    /// Wire decode failure.
    #[error("type error: {0}")]
    Type(#[from] TypeError),
}

/// A namespaced identifier like `minecraft:stone`.
///
/// Resource locations are used throughout the Minecraft protocol to identify
/// registries, blocks, items, dimensions, biomes, and many other resources.
///
/// # Format
///
/// - **Namespace:** `[a-z0-9_.-]`, no `".."` sequences
/// - **Path:** `[a-z0-9/._-]`
/// - **Wire format:** VarInt-prefixed UTF-8 string `"namespace:path"`
///
/// If no colon is present when parsing from a string, the default namespace
/// `"minecraft"` is used.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceLocation {
    namespace: String,
    path: String,
}

/// Returns `true` if every character matches `[a-z0-9_.-]`.
fn is_valid_namespace_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.' || c == '-'
}

/// Returns `true` if every character matches `[a-z0-9/._-]`.
fn is_valid_path_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || c == '/' || c == '.' || c == '_' || c == '-'
}

impl ResourceLocation {
    /// Creates a new `ResourceLocation` with explicit namespace and path.
    ///
    /// # Errors
    ///
    /// Returns [`ResourceLocationError`] if either component is empty or
    /// contains invalid characters.
    pub fn new(
        namespace: impl Into<String>,
        path: impl Into<String>,
    ) -> Result<Self, ResourceLocationError> {
        let namespace = namespace.into();
        let path = path.into();

        if namespace.is_empty() {
            return Err(ResourceLocationError::EmptyNamespace);
        }
        if path.is_empty() {
            return Err(ResourceLocationError::EmptyPath);
        }
        if !namespace.chars().all(is_valid_namespace_char) || namespace.contains("..") {
            return Err(ResourceLocationError::InvalidNamespace(namespace));
        }
        if !path.chars().all(is_valid_path_char) {
            return Err(ResourceLocationError::InvalidPath(path));
        }

        Ok(Self { namespace, path })
    }

    /// Parses a `ResourceLocation` from a string like `"minecraft:stone"`.
    ///
    /// If no colon is present, the default namespace `"minecraft"` is used.
    ///
    /// # Errors
    ///
    /// Returns [`ResourceLocationError`] if the components are invalid.
    pub fn from_string(s: &str) -> Result<Self, ResourceLocationError> {
        if let Some((ns, path)) = s.split_once(':') {
            Self::new(ns, path)
        } else {
            Self::new(DEFAULT_NAMESPACE, s)
        }
    }

    /// Returns the namespace component (e.g. `"minecraft"`).
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Returns the path component (e.g. `"stone"`).
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Creates a `ResourceLocation` with the `minecraft` namespace.
    ///
    /// This is a convenience constructor for the common case of creating
    /// resource locations in the default `minecraft` namespace.
    ///
    /// # Panics
    ///
    /// Panics if `path` contains invalid characters or is empty. Only use
    /// with compile-time-known valid paths.
    #[must_use]
    #[allow(clippy::expect_used)]
    pub fn minecraft(path: impl Into<String>) -> Self {
        Self::new(DEFAULT_NAMESPACE, path).expect("invalid minecraft resource location path")
    }

    /// Reads a `ResourceLocation` from a wire buffer.
    ///
    /// # Errors
    ///
    /// Returns [`ResourceLocationError`] if the string is malformed or the
    /// buffer is truncated.
    pub fn read(buf: &mut Bytes) -> Result<Self, ResourceLocationError> {
        let s = types::read_string(buf, MAX_RESOURCE_LOCATION_LENGTH)?;
        Self::from_string(&s)
    }

    /// Writes this `ResourceLocation` to a wire buffer as a VarInt-prefixed
    /// UTF-8 string `"namespace:path"`.
    pub fn write(&self, buf: &mut BytesMut) {
        types::write_string(buf, &self.to_string());
    }
}

impl fmt::Display for ResourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.path)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ── Construction ─────────────────────────────────────────────────────

    #[test]
    fn test_new_valid() {
        let rl = ResourceLocation::new("minecraft", "stone").unwrap();
        assert_eq!(rl.namespace(), "minecraft");
        assert_eq!(rl.path(), "stone");
    }

    #[test]
    fn test_minecraft_convenience() {
        let rl = ResourceLocation::minecraft("overworld");
        assert_eq!(rl.namespace(), "minecraft");
        assert_eq!(rl.path(), "overworld");
        assert_eq!(rl.to_string(), "minecraft:overworld");
    }

    #[test]
    fn test_minecraft_nested_path() {
        let rl = ResourceLocation::minecraft("worldgen/biome");
        assert_eq!(rl.path(), "worldgen/biome");
    }

    #[test]
    #[should_panic(expected = "invalid minecraft resource location path")]
    fn test_minecraft_invalid_path_panics() {
        let _ = ResourceLocation::minecraft("Invalid Path");
    }

    #[test]
    fn test_new_with_dots_and_hyphens() {
        let rl = ResourceLocation::new("my-mod.v2", "blocks/custom_stone").unwrap();
        assert_eq!(rl.namespace(), "my-mod.v2");
        assert_eq!(rl.path(), "blocks/custom_stone");
    }

    #[test]
    fn test_new_empty_namespace() {
        let err = ResourceLocation::new("", "stone").unwrap_err();
        assert!(matches!(err, ResourceLocationError::EmptyNamespace));
    }

    #[test]
    fn test_new_empty_path() {
        let err = ResourceLocation::new("minecraft", "").unwrap_err();
        assert!(matches!(err, ResourceLocationError::EmptyPath));
    }

    #[test]
    fn test_new_invalid_namespace_uppercase() {
        let err = ResourceLocation::new("Minecraft", "stone").unwrap_err();
        assert!(matches!(err, ResourceLocationError::InvalidNamespace(_)));
    }

    #[test]
    fn test_new_invalid_namespace_space() {
        let err = ResourceLocation::new("my mod", "stone").unwrap_err();
        assert!(matches!(err, ResourceLocationError::InvalidNamespace(_)));
    }

    #[test]
    fn test_new_invalid_namespace_double_dot() {
        let err = ResourceLocation::new("my..mod", "stone").unwrap_err();
        assert!(matches!(err, ResourceLocationError::InvalidNamespace(_)));
    }

    #[test]
    fn test_new_invalid_path_uppercase() {
        let err = ResourceLocation::new("minecraft", "Stone").unwrap_err();
        assert!(matches!(err, ResourceLocationError::InvalidPath(_)));
    }

    #[test]
    fn test_new_invalid_path_space() {
        let err = ResourceLocation::new("minecraft", "my stone").unwrap_err();
        assert!(matches!(err, ResourceLocationError::InvalidPath(_)));
    }

    #[test]
    fn test_new_valid_path_with_slashes() {
        let rl = ResourceLocation::new("minecraft", "block/stone/variant").unwrap();
        assert_eq!(rl.path(), "block/stone/variant");
    }

    #[test]
    fn test_new_namespace_with_digits() {
        let rl = ResourceLocation::new("mod123", "item").unwrap();
        assert_eq!(rl.namespace(), "mod123");
    }

    // ── from_string ─────────────────────────────────────────────────────

    #[test]
    fn test_from_string_with_colon() {
        let rl = ResourceLocation::from_string("minecraft:stone").unwrap();
        assert_eq!(rl.namespace(), "minecraft");
        assert_eq!(rl.path(), "stone");
    }

    #[test]
    fn test_from_string_default_namespace() {
        let rl = ResourceLocation::from_string("stone").unwrap();
        assert_eq!(rl.namespace(), "minecraft");
        assert_eq!(rl.path(), "stone");
    }

    #[test]
    fn test_from_string_custom_namespace() {
        let rl = ResourceLocation::from_string("mymod:custom_block").unwrap();
        assert_eq!(rl.namespace(), "mymod");
        assert_eq!(rl.path(), "custom_block");
    }

    #[test]
    fn test_from_string_nested_path() {
        let rl = ResourceLocation::from_string("minecraft:block/stone").unwrap();
        assert_eq!(rl.path(), "block/stone");
    }

    #[test]
    fn test_from_string_invalid() {
        let err = ResourceLocation::from_string("Mod:Stone").unwrap_err();
        assert!(matches!(err, ResourceLocationError::InvalidNamespace(_)));
    }

    // ── Display ─────────────────────────────────────────────────────────

    #[test]
    fn test_display() {
        let rl = ResourceLocation::new("minecraft", "stone").unwrap();
        assert_eq!(rl.to_string(), "minecraft:stone");
    }

    #[test]
    fn test_display_custom() {
        let rl = ResourceLocation::new("mymod", "items/sword").unwrap();
        assert_eq!(rl.to_string(), "mymod:items/sword");
    }

    // ── Wire roundtrip ──────────────────────────────────────────────────

    #[test]
    fn test_wire_roundtrip() {
        let rl = ResourceLocation::new("minecraft", "stone").unwrap();
        let mut buf = BytesMut::new();
        rl.write(&mut buf);
        let mut data = buf.freeze();
        let decoded = ResourceLocation::read(&mut data).unwrap();
        assert_eq!(decoded, rl);
    }

    #[test]
    fn test_wire_roundtrip_nested_path() {
        let rl = ResourceLocation::new("minecraft", "worldgen/biome").unwrap();
        let mut buf = BytesMut::new();
        rl.write(&mut buf);
        let mut data = buf.freeze();
        let decoded = ResourceLocation::read(&mut data).unwrap();
        assert_eq!(decoded, rl);
    }

    #[test]
    fn test_wire_roundtrip_custom_namespace() {
        let rl = ResourceLocation::new("my-mod", "blocks/fancy_stone").unwrap();
        let mut buf = BytesMut::new();
        rl.write(&mut buf);
        let mut data = buf.freeze();
        let decoded = ResourceLocation::read(&mut data).unwrap();
        assert_eq!(decoded, rl);
    }

    // ── Equality / Hash ─────────────────────────────────────────────────

    #[test]
    fn test_equality() {
        let a = ResourceLocation::new("minecraft", "stone").unwrap();
        let b = ResourceLocation::from_string("minecraft:stone").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_inequality() {
        let a = ResourceLocation::new("minecraft", "stone").unwrap();
        let b = ResourceLocation::new("minecraft", "dirt").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn test_hash_consistency() {
        use std::collections::HashSet;
        let a = ResourceLocation::new("minecraft", "stone").unwrap();
        let b = ResourceLocation::from_string("minecraft:stone").unwrap();
        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
    }
}
