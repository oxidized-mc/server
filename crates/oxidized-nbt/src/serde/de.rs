//! NBT deserializer — converts [`NbtTag`] trees back into Rust values.

use ::serde::de::{self, DeserializeSeed, IntoDeserializer, MapAccess, SeqAccess, Visitor};

use crate::compound::NbtCompound;
use crate::tag::NbtTag;

use super::Error;

/// Serde [`Deserializer`] that borrows an [`NbtCompound`] directly,
/// avoiding the clone that would be needed to wrap it in [`NbtTag::Compound`].
///
/// Used by [`super::from_compound`] as the entry-point deserializer.
pub(super) struct CompoundDeserializer<'a>(pub(super) &'a NbtCompound);

impl<'de, 'a> de::Deserializer<'de> for CompoundDeserializer<'a> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_map(NbtMapAccess::new(self.0))
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_map(NbtMapAccess::new(self.0))
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
        if let Some((key, value)) = self.0.iter().next() {
            visitor.visit_enum(NbtEnumAccess {
                variant: key.clone(),
                value,
            })
        } else {
            Err(Error("expected non-empty compound for enum".to_owned()))
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes
        byte_buf option unit unit_struct seq tuple tuple_struct identifier
    }
}

/// Serde [`Deserializer`] wrapping a reference to an [`NbtTag`].
struct NbtDeserializer<'a>(&'a NbtTag);

/// Generates a primitive `deserialize_*` trait method that matches a single
/// [`NbtTag`] variant and forwards to the corresponding visitor method.
macro_rules! deserialize_prim {
    ($method:ident, $variant:ident, $visit:ident, $ty_name:literal) => {
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
            match self.0 {
                NbtTag::$variant(v) => visitor.$visit(*v),
                _ => Err(Error(format!(
                    concat!("expected ", $ty_name, ", got {}"),
                    self.0.type_name()
                ))),
            }
        }
    };
    ($method:ident, $variant:ident, $visit:ident as $cast:ty, $ty_name:literal) => {
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
            match self.0 {
                NbtTag::$variant(v) => visitor.$visit(*v as $cast),
                _ => Err(Error(format!(
                    concat!("expected ", $ty_name, ", got {}"),
                    self.0.type_name()
                ))),
            }
        }
    };
}

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

    deserialize_prim!(deserialize_i8, Byte, visit_i8, "byte");
    deserialize_prim!(deserialize_i16, Short, visit_i16, "short");
    deserialize_prim!(deserialize_i32, Int, visit_i32, "int");
    deserialize_prim!(deserialize_i64, Long, visit_i64, "long");
    deserialize_prim!(deserialize_u8, Byte, visit_u8 as u8, "byte");
    deserialize_prim!(deserialize_u16, Short, visit_u16 as u16, "short");
    deserialize_prim!(deserialize_u32, Int, visit_u32 as u32, "int");
    deserialize_prim!(deserialize_u64, Long, visit_u64 as u64, "long");
    deserialize_prim!(deserialize_f32, Float, visit_f32, "float");
    deserialize_prim!(deserialize_f64, Double, visit_f64, "double");

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
