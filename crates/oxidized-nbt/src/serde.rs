//! Serde integration for NBT compounds.
//!
//! Provides [`to_compound`] to serialize a Rust struct into an
//! [`NbtCompound`], and [`from_compound`] to deserialize one back.

use std::fmt;

use serde::Serialize;
use serde::de::{self, DeserializeSeed, IntoDeserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{self, SerializeMap, SerializeSeq, SerializeStruct};

use crate::compound::NbtCompound;
use crate::error::NbtError;
use crate::list::NbtList;
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
pub fn from_compound<T: de::DeserializeOwned>(compound: &NbtCompound) -> Result<T, NbtError> {
    let tag = NbtTag::Compound(compound.clone());
    let deserializer = NbtDeserializer(&tag);
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
        .serialize(NbtSerializer)
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

impl ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error(msg.to_string())
    }
}

impl de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error(msg.to_string())
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Serializer
// ═══════════════════════════════════════════════════════════════════════

/// Serde [`Serializer`] that produces [`NbtTag`] values.
struct NbtSerializer;

impl ser::Serializer for NbtSerializer {
    type Ok = NbtTag;
    type Error = Error;

    type SerializeSeq = NbtSeqSerializer;
    type SerializeTuple = NbtSeqSerializer;
    type SerializeTupleStruct = NbtSeqSerializer;
    type SerializeTupleVariant = NbtSeqSerializer;
    type SerializeMap = NbtMapSerializer;
    type SerializeStruct = NbtStructSerializer;
    type SerializeStructVariant = NbtStructSerializer;

    fn serialize_bool(self, v: bool) -> Result<NbtTag, Error> {
        Ok(NbtTag::Byte(i8::from(v)))
    }

    fn serialize_i8(self, v: i8) -> Result<NbtTag, Error> {
        Ok(NbtTag::Byte(v))
    }

    fn serialize_i16(self, v: i16) -> Result<NbtTag, Error> {
        Ok(NbtTag::Short(v))
    }

    fn serialize_i32(self, v: i32) -> Result<NbtTag, Error> {
        Ok(NbtTag::Int(v))
    }

    fn serialize_i64(self, v: i64) -> Result<NbtTag, Error> {
        Ok(NbtTag::Long(v))
    }

    fn serialize_u8(self, v: u8) -> Result<NbtTag, Error> {
        Ok(NbtTag::Byte(v as i8))
    }

    fn serialize_u16(self, v: u16) -> Result<NbtTag, Error> {
        Ok(NbtTag::Short(v as i16))
    }

    fn serialize_u32(self, v: u32) -> Result<NbtTag, Error> {
        Ok(NbtTag::Int(v as i32))
    }

    fn serialize_u64(self, v: u64) -> Result<NbtTag, Error> {
        Ok(NbtTag::Long(v as i64))
    }

    fn serialize_f32(self, v: f32) -> Result<NbtTag, Error> {
        Ok(NbtTag::Float(v))
    }

    fn serialize_f64(self, v: f64) -> Result<NbtTag, Error> {
        Ok(NbtTag::Double(v))
    }

    fn serialize_char(self, v: char) -> Result<NbtTag, Error> {
        Ok(NbtTag::String(v.to_string()))
    }

    fn serialize_str(self, v: &str) -> Result<NbtTag, Error> {
        Ok(NbtTag::String(v.to_owned()))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<NbtTag, Error> {
        let mut list = NbtList::empty();
        for &b in v {
            list.push(NbtTag::Byte(b as i8))
                .map_err(|e| Error(e.to_string()))?;
        }
        Ok(NbtTag::List(list))
    }

    fn serialize_none(self) -> Result<NbtTag, Error> {
        // Represent None as Byte(0) — the caller (struct serializer)
        // will handle omission of Option fields.
        Ok(NbtTag::Byte(0))
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<NbtTag, Error> {
        value.serialize(NbtSerializer)
    }

    fn serialize_unit(self) -> Result<NbtTag, Error> {
        Ok(NbtTag::Compound(NbtCompound::new()))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<NbtTag, Error> {
        Ok(NbtTag::Compound(NbtCompound::new()))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<NbtTag, Error> {
        Ok(NbtTag::String(variant.to_owned()))
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<NbtTag, Error> {
        value.serialize(NbtSerializer)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<NbtTag, Error> {
        let inner = value.serialize(NbtSerializer)?;
        let mut compound = NbtCompound::new();
        compound.put(variant, inner);
        Ok(NbtTag::Compound(compound))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<NbtSeqSerializer, Error> {
        Ok(NbtSeqSerializer {
            list: NbtList::empty(),
        })
    }

    fn serialize_tuple(self, _len: usize) -> Result<NbtSeqSerializer, Error> {
        Ok(NbtSeqSerializer {
            list: NbtList::empty(),
        })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<NbtSeqSerializer, Error> {
        Ok(NbtSeqSerializer {
            list: NbtList::empty(),
        })
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<NbtSeqSerializer, Error> {
        Ok(NbtSeqSerializer {
            list: NbtList::empty(),
        })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<NbtMapSerializer, Error> {
        Ok(NbtMapSerializer {
            compound: NbtCompound::new(),
            current_key: None,
        })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<NbtStructSerializer, Error> {
        Ok(NbtStructSerializer {
            compound: NbtCompound::new(),
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<NbtStructSerializer, Error> {
        Ok(NbtStructSerializer {
            compound: NbtCompound::new(),
        })
    }
}

// ── Sequence serializer ─────────────────────────────────────────────────

struct NbtSeqSerializer {
    list: NbtList,
}

impl SerializeSeq for NbtSeqSerializer {
    type Ok = NbtTag;
    type Error = Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        let tag = value.serialize(NbtSerializer)?;
        self.list.push(tag).map_err(|e| Error(e.to_string()))?;
        Ok(())
    }

    fn end(self) -> Result<NbtTag, Error> {
        Ok(NbtTag::List(self.list))
    }
}

impl ser::SerializeTuple for NbtSeqSerializer {
    type Ok = NbtTag;
    type Error = Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<NbtTag, Error> {
        SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleStruct for NbtSeqSerializer {
    type Ok = NbtTag;
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<NbtTag, Error> {
        SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleVariant for NbtSeqSerializer {
    type Ok = NbtTag;
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<NbtTag, Error> {
        SerializeSeq::end(self)
    }
}

// ── Map serializer ──────────────────────────────────────────────────────

struct NbtMapSerializer {
    compound: NbtCompound,
    current_key: Option<String>,
}

impl SerializeMap for NbtMapSerializer {
    type Ok = NbtTag;
    type Error = Error;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Error> {
        let tag = key.serialize(NbtSerializer)?;
        match tag {
            NbtTag::String(s) => {
                self.current_key = Some(s);
                Ok(())
            },
            _ => Err(Error("map keys must be strings".to_owned())),
        }
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        let key = self
            .current_key
            .take()
            .ok_or_else(|| Error("serialize_value called without serialize_key".to_owned()))?;
        let tag = value.serialize(NbtSerializer)?;
        self.compound.put(key, tag);
        Ok(())
    }

    fn end(self) -> Result<NbtTag, Error> {
        Ok(NbtTag::Compound(self.compound))
    }
}

// ── Struct serializer ───────────────────────────────────────────────────

struct NbtStructSerializer {
    compound: NbtCompound,
}

impl SerializeStruct for NbtStructSerializer {
    type Ok = NbtTag;
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        let tag = value.serialize(NbtSerializer)?;
        self.compound.put(key, tag);
        Ok(())
    }

    fn end(self) -> Result<NbtTag, Error> {
        Ok(NbtTag::Compound(self.compound))
    }
}

impl ser::SerializeStructVariant for NbtStructSerializer {
    type Ok = NbtTag;
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        SerializeStruct::serialize_field(self, key, value)
    }

    fn end(self) -> Result<NbtTag, Error> {
        SerializeStruct::end(self)
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Deserializer
// ═══════════════════════════════════════════════════════════════════════

/// Serde [`Deserializer`] wrapping a reference to an [`NbtTag`].
struct NbtDeserializer<'a>(&'a NbtTag);

impl<'de, 'a> de::Deserializer<'de> for NbtDeserializer<'a> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            NbtTag::Byte(v) => visitor.visit_i8(*v),
            NbtTag::Short(v) => visitor.visit_i16(*v),
            NbtTag::Int(v) => visitor.visit_i32(*v),
            NbtTag::Long(v) => visitor.visit_i64(*v),
            NbtTag::Float(v) => visitor.visit_f32(*v),
            NbtTag::Double(v) => visitor.visit_f64(*v),
            NbtTag::String(v) => visitor.visit_str(v),
            NbtTag::List(list) => {
                let seq = NbtSeqAccess {
                    iter: list.iter(),
                    len: list.len(),
                };
                visitor.visit_seq(seq)
            },
            NbtTag::Compound(compound) => {
                let map = NbtMapAccess::new(compound);
                visitor.visit_map(map)
            },
            NbtTag::ByteArray(arr) => {
                let tags: Vec<NbtTag> = arr.iter().map(|&b| NbtTag::Byte(b)).collect();
                let seq = NbtOwnedSeqAccess {
                    iter: tags.into_iter(),
                    len: arr.len(),
                };
                visitor.visit_seq(seq)
            },
            NbtTag::IntArray(arr) => {
                let tags: Vec<NbtTag> = arr.iter().map(|&v| NbtTag::Int(v)).collect();
                let seq = NbtOwnedSeqAccess {
                    iter: tags.into_iter(),
                    len: arr.len(),
                };
                visitor.visit_seq(seq)
            },
            NbtTag::LongArray(arr) => {
                let tags: Vec<NbtTag> = arr.iter().map(|&v| NbtTag::Long(v)).collect();
                let seq = NbtOwnedSeqAccess {
                    iter: tags.into_iter(),
                    len: arr.len(),
                };
                visitor.visit_seq(seq)
            },
        }
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            NbtTag::Byte(v) => visitor.visit_bool(*v != 0),
            _ => Err(Error(format!(
                "expected byte for bool, got {}",
                self.0.type_name()
            ))),
        }
    }

    fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            NbtTag::Byte(v) => visitor.visit_i8(*v),
            _ => Err(Error(format!("expected byte, got {}", self.0.type_name()))),
        }
    }

    fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            NbtTag::Short(v) => visitor.visit_i16(*v),
            _ => Err(Error(format!("expected short, got {}", self.0.type_name()))),
        }
    }

    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            NbtTag::Int(v) => visitor.visit_i32(*v),
            _ => Err(Error(format!("expected int, got {}", self.0.type_name()))),
        }
    }

    fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            NbtTag::Long(v) => visitor.visit_i64(*v),
            _ => Err(Error(format!("expected long, got {}", self.0.type_name()))),
        }
    }

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            NbtTag::Byte(v) => visitor.visit_u8(*v as u8),
            _ => Err(Error(format!("expected byte, got {}", self.0.type_name()))),
        }
    }

    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            NbtTag::Short(v) => visitor.visit_u16(*v as u16),
            _ => Err(Error(format!("expected short, got {}", self.0.type_name()))),
        }
    }

    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            NbtTag::Int(v) => visitor.visit_u32(*v as u32),
            _ => Err(Error(format!("expected int, got {}", self.0.type_name()))),
        }
    }

    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            NbtTag::Long(v) => visitor.visit_u64(*v as u64),
            _ => Err(Error(format!("expected long, got {}", self.0.type_name()))),
        }
    }

    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            NbtTag::Float(v) => visitor.visit_f32(*v),
            _ => Err(Error(format!("expected float, got {}", self.0.type_name()))),
        }
    }

    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            NbtTag::Double(v) => visitor.visit_f64(*v),
            _ => Err(Error(format!(
                "expected double, got {}",
                self.0.type_name()
            ))),
        }
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            NbtTag::String(s) => {
                let mut chars = s.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => visitor.visit_char(c),
                    _ => Err(Error("expected single character string".to_owned())),
                }
            },
            _ => Err(Error(format!(
                "expected string, got {}",
                self.0.type_name()
            ))),
        }
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            NbtTag::String(s) => visitor.visit_str(s),
            _ => Err(Error(format!(
                "expected string, got {}",
                self.0.type_name()
            ))),
        }
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_any(visitor)
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_any(visitor)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        // If we reached here, the key exists so the value is Some.
        visitor.visit_some(self)
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            NbtTag::List(list) => {
                let seq = NbtSeqAccess {
                    iter: list.iter(),
                    len: list.len(),
                };
                visitor.visit_seq(seq)
            },
            NbtTag::ByteArray(arr) => {
                let tags: Vec<NbtTag> = arr.iter().map(|&b| NbtTag::Byte(b)).collect();
                let len = tags.len();
                let seq = NbtOwnedSeqAccess {
                    iter: tags.into_iter(),
                    len,
                };
                visitor.visit_seq(seq)
            },
            NbtTag::IntArray(arr) => {
                let tags: Vec<NbtTag> = arr.iter().map(|&v| NbtTag::Int(v)).collect();
                let len = tags.len();
                let seq = NbtOwnedSeqAccess {
                    iter: tags.into_iter(),
                    len,
                };
                visitor.visit_seq(seq)
            },
            NbtTag::LongArray(arr) => {
                let tags: Vec<NbtTag> = arr.iter().map(|&v| NbtTag::Long(v)).collect();
                let len = tags.len();
                let seq = NbtOwnedSeqAccess {
                    iter: tags.into_iter(),
                    len,
                };
                visitor.visit_seq(seq)
            },
            _ => Err(Error(format!(
                "expected list/array, got {}",
                self.0.type_name()
            ))),
        }
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            NbtTag::Compound(compound) => {
                let map = NbtMapAccess::new(compound);
                visitor.visit_map(map)
            },
            _ => Err(Error(format!(
                "expected compound, got {}",
                self.0.type_name()
            ))),
        }
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        match self.0 {
            NbtTag::String(s) => visitor.visit_enum(s.as_str().into_deserializer()),
            NbtTag::Compound(compound) => {
                if let Some((key, value)) = compound.iter().next() {
                    visitor.visit_enum(NbtEnumAccess {
                        variant: key.clone(),
                        value,
                    })
                } else {
                    Err(Error("expected non-empty compound for enum".to_owned()))
                }
            },
            _ => Err(Error(format!(
                "expected string or compound for enum, got {}",
                self.0.type_name()
            ))),
        }
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }
}

// ── Sequence access ─────────────────────────────────────────────────────

struct NbtSeqAccess<'a> {
    iter: std::slice::Iter<'a, NbtTag>,
    len: usize,
}

impl<'de, 'a> SeqAccess<'de> for NbtSeqAccess<'a> {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        match self.iter.next() {
            Some(tag) => seed.deserialize(NbtDeserializer(tag)).map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.len)
    }
}

/// Owned sequence access for arrays that need conversion.
struct NbtOwnedSeqAccess {
    iter: std::vec::IntoIter<NbtTag>,
    len: usize,
}

impl<'de> SeqAccess<'de> for NbtOwnedSeqAccess {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        match self.iter.next() {
            Some(ref tag) => seed.deserialize(NbtDeserializer(tag)).map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.len)
    }
}

// ── Map access ──────────────────────────────────────────────────────────

struct NbtMapAccess<'a> {
    entries: Vec<(&'a String, &'a NbtTag)>,
    index: usize,
    pending_value: Option<&'a NbtTag>,
}

impl<'a> NbtMapAccess<'a> {
    fn new(compound: &'a NbtCompound) -> Self {
        Self {
            entries: compound.iter().collect(),
            index: 0,
            pending_value: None,
        }
    }
}

impl<'de, 'a> MapAccess<'de> for NbtMapAccess<'a> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        if self.index >= self.entries.len() {
            return Ok(None);
        }
        let (key, value) = self.entries[self.index];
        self.index += 1;
        self.pending_value = Some(value);
        let key_tag = NbtTag::String(key.clone());
        seed.deserialize(NbtDeserializer(&key_tag)).map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
        let value = self
            .pending_value
            .take()
            .ok_or_else(|| Error("next_value_seed called without next_key_seed".to_owned()))?;
        seed.deserialize(NbtDeserializer(value))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.entries.len() - self.index)
    }
}

// ── Enum access ─────────────────────────────────────────────────────────

struct NbtEnumAccess<'a> {
    variant: String,
    value: &'a NbtTag,
}

impl<'de, 'a> de::EnumAccess<'de> for NbtEnumAccess<'a> {
    type Error = Error;
    type Variant = NbtVariantAccess<'a>;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Error> {
        let variant_tag = NbtTag::String(self.variant);
        let val = seed.deserialize(NbtDeserializer(&variant_tag))?;
        Ok((val, NbtVariantAccess { value: self.value }))
    }
}

struct NbtVariantAccess<'a> {
    value: &'a NbtTag,
}

impl<'de, 'a> de::VariantAccess<'de> for NbtVariantAccess<'a> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
        seed.deserialize(NbtDeserializer(self.value))
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value, Error> {
        de::Deserializer::deserialize_seq(NbtDeserializer(self.value), visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        de::Deserializer::deserialize_map(NbtDeserializer(self.value), visitor)
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
        on_ground: bool,
    }

    #[test]
    fn test_serde_roundtrip_struct() {
        let player = PlayerData {
            health: 20.0,
            level: 5,
            name: "Steve".into(),
            position: vec![1.0, 64.0, 1.0],
            on_ground: true,
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
