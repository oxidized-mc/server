//! NBT serializer — converts Rust values into [`NbtTag`] trees.

use ::serde::Serialize;
use ::serde::ser::{self, SerializeMap, SerializeSeq, SerializeStruct};

use crate::compound::NbtCompound;
use crate::list::NbtList;
use crate::tag::NbtTag;

use super::Error;

/// Serde [`Serializer`] that produces [`NbtTag`] values.
pub(super) struct NbtSerializer;

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

pub(super) struct NbtSeqSerializer {
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

pub(super) struct NbtMapSerializer {
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

pub(super) struct NbtStructSerializer {
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
