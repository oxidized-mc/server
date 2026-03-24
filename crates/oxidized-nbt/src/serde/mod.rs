//! Serde integration for NBT compounds.
//!
//! Provides [`to_compound`] to serialize a Rust struct into an
//! [`NbtCompound`], and [`from_compound`] to deserialize one back.

mod de;
mod ser;

use std::fmt;

use ::serde::Serialize;

use crate::compound::NbtCompound;
use crate::error::NbtError;
use crate::tag::NbtTag;

// ── Public API ──────────────────────────────────────────────────────────

/// Deserializes a Rust value from an [`NbtCompound`].
///
/// # Examples
///
/// ```
/// use oxidized_nbt::{NbtCompound, from_compound, to_compound};
/// use serde::Deserialize;
///
/// #[derive(Deserialize, PartialEq, Debug)]
/// struct Player { health: i32, name: String }
///
/// let mut compound = NbtCompound::new();
/// compound.put_int("health", 20);
/// compound.put_string("name", "Steve");
///
/// let player: Player = from_compound(&compound).unwrap();
/// assert_eq!(player.health, 20);
/// assert_eq!(player.name, "Steve");
/// ```
///
/// # Errors
///
/// Returns [`NbtError::SerdeError`] if the compound structure does not
/// match the target type.
pub fn from_compound<T: ::serde::de::DeserializeOwned>(
    compound: &NbtCompound,
) -> Result<T, NbtError> {
    let deserializer = self::de::CompoundDeserializer(compound);
    T::deserialize(deserializer).map_err(|e| NbtError::SerdeError(e.to_string()))
}

/// Serializes a Rust value into an [`NbtCompound`].
///
/// # Examples
///
/// ```
/// use oxidized_nbt::to_compound;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Player { health: i32, name: String }
///
/// let player = Player { health: 20, name: "Steve".into() };
/// let compound = to_compound(&player).unwrap();
/// assert_eq!(compound.get_int("health"), Some(20));
/// assert_eq!(compound.get_string("name"), Some("Steve"));
/// ```
///
/// # Errors
///
/// Returns [`NbtError::SerdeError`] if the value cannot be represented as
/// an NBT compound (e.g. a bare integer at the top level).
pub fn to_compound<T: Serialize>(value: &T) -> Result<NbtCompound, NbtError> {
    let tag = value
        .serialize(self::ser::NbtSerializer)
        .map_err(|e| NbtError::SerdeError(e.to_string()))?;
    match tag {
        NbtTag::Compound(c) => Ok(c),
        other => Err(NbtError::SerdeError(format!(
            "expected compound, got {}",
            other.type_name()
        ))),
    }
}

// ── Serde error bridge ──────────────────────────────────────────────────

/// Internal serde error type.
#[derive(Debug)]
struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl ::serde::ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error(msg.to_string())
    }
}

impl ::serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error(msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::collections::HashMap;

    use ::serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct PlayerData {
        health: f32,
        level: i32,
        name: String,
        position: Vec<f64>,
        is_on_ground: bool,
    }

    #[test]
    fn test_serde_roundtrip_struct() {
        let player = PlayerData {
            health: 20.0,
            level: 5,
            name: "Steve".into(),
            position: vec![1.0, 64.0, 1.0],
            is_on_ground: true,
        };
        let compound = to_compound(&player).unwrap();
        assert_eq!(compound.get_float("health"), Some(20.0));
        assert_eq!(compound.get_int("level"), Some(5));
        assert_eq!(compound.get_string("name"), Some("Steve"));

        let back: PlayerData = from_compound(&compound).unwrap();
        assert_eq!(player, back);
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Position {
        x: f64,
        y: f64,
        z: f64,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Entity {
        id: String,
        pos: Position,
    }

    #[test]
    fn test_serde_nested_struct() {
        let entity = Entity {
            id: "minecraft:zombie".into(),
            pos: Position {
                x: 1.0,
                y: 64.0,
                z: -3.5,
            },
        };
        let compound = to_compound(&entity).unwrap();
        let back: Entity = from_compound(&compound).unwrap();
        assert_eq!(entity, back);
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct OptionalFields {
        name: String,
        nickname: Option<String>,
    }

    #[test]
    fn test_serde_optional_field_present() {
        let data = OptionalFields {
            name: "Alex".into(),
            nickname: Some("Lex".into()),
        };
        let compound = to_compound(&data).unwrap();
        let back: OptionalFields = from_compound(&compound).unwrap();
        assert_eq!(data, back);
    }

    #[test]
    fn test_serde_map() {
        let mut map = HashMap::new();
        map.insert("health".to_owned(), 20);
        map.insert("level".to_owned(), 5);
        let compound = to_compound(&map).unwrap();
        let back: HashMap<String, i32> = from_compound(&compound).unwrap();
        assert_eq!(map, back);
    }

    #[test]
    fn test_serde_primitive_values() {
        // Verify individual type mappings via compound
        let mut c = NbtCompound::new();
        c.put_byte("b", 42);
        c.put_short("s", 1000);
        c.put_int("i", 100_000);
        c.put_long("l", 9_999_999_999i64);
        c.put_float("f", 1.5);
        c.put_double("d", 2.5);
        c.put_string("str", "hello");

        #[derive(Deserialize, Debug, PartialEq)]
        struct Prims {
            b: i8,
            s: i16,
            i: i32,
            l: i64,
            f: f32,
            d: f64,
            str: String,
        }

        let p: Prims = from_compound(&c).unwrap();
        assert_eq!(p.b, 42);
        assert_eq!(p.s, 1000);
        assert_eq!(p.i, 100_000);
        assert_eq!(p.l, 9_999_999_999);
        assert_eq!(p.f, 1.5);
        assert_eq!(p.d, 2.5);
        assert_eq!(p.str, "hello");
    }

    #[test]
    fn test_serde_bool_roundtrip() {
        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct Flags {
            active: bool,
            visible: bool,
        }

        let flags = Flags {
            active: true,
            visible: false,
        };
        let compound = to_compound(&flags).unwrap();
        assert_eq!(compound.get_byte("active"), Some(1));
        assert_eq!(compound.get_byte("visible"), Some(0));
        let back: Flags = from_compound(&compound).unwrap();
        assert_eq!(flags, back);
    }

    #[test]
    fn test_serde_empty_struct() {
        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct Empty {}

        let empty = Empty {};
        let compound = to_compound(&empty).unwrap();
        assert!(compound.is_empty());
        let back: Empty = from_compound(&compound).unwrap();
        assert_eq!(empty, back);
    }

    #[test]
    fn test_serde_unit_variant_enum() {
        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        enum Color {
            Red,
            Green,
            Blue,
        }

        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct WithEnum {
            color: Color,
        }

        let data = WithEnum {
            color: Color::Green,
        };
        let compound = to_compound(&data).unwrap();
        assert_eq!(compound.get_string("color"), Some("Green"));
        let back: WithEnum = from_compound(&compound).unwrap();
        assert_eq!(data, back);
    }
}
