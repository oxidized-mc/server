//! Core [`NbtTag`] enum representing all NBT value types.

use crate::compound::NbtCompound;
use crate::error::{
    TAG_BYTE, TAG_BYTE_ARRAY, TAG_COMPOUND, TAG_DOUBLE, TAG_FLOAT, TAG_INT, TAG_INT_ARRAY,
    TAG_LIST, TAG_LONG, TAG_LONG_ARRAY, TAG_SHORT, TAG_STRING,
};
use crate::list::NbtList;

/// A single NBT value.
///
/// There is no `End` variant — the TAG_END byte is a wire-format sentinel
/// used only during serialization of compounds, not a user-facing value.
///
/// # Examples
///
/// ```
/// use oxidized_nbt::NbtTag;
///
/// // Create tags from primitives via From impls
/// let byte_tag: NbtTag = 1i8.into();
/// let int_tag: NbtTag = 42i32.into();
/// let str_tag: NbtTag = "hello".into();
///
/// // Access the inner value with typed accessors
/// assert_eq!(int_tag.as_int(), Some(42));
/// assert_eq!(int_tag.as_byte(), None); // wrong type returns None
/// assert_eq!(str_tag.as_str(), Some("hello"));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum NbtTag {
    /// A signed 8-bit integer.
    Byte(i8),
    /// A signed 16-bit integer.
    Short(i16),
    /// A signed 32-bit integer.
    Int(i32),
    /// A signed 64-bit integer.
    Long(i64),
    /// A 32-bit IEEE 754 floating-point number.
    Float(f32),
    /// A 64-bit IEEE 754 floating-point number.
    Double(f64),
    /// An array of signed bytes.
    ByteArray(Vec<i8>),
    /// A Modified UTF-8 string.
    String(String),
    /// A typed, homogeneous list of tags.
    List(NbtList),
    /// An ordered map of named tags.
    Compound(NbtCompound),
    /// An array of signed 32-bit integers.
    IntArray(Vec<i32>),
    /// An array of signed 64-bit integers.
    LongArray(Vec<i64>),
}

/// Generates `as_*` copy-value accessors on `NbtTag`.
macro_rules! impl_nbt_primitive_accessor {
    ($($(#[$meta:meta])* $fn_name:ident -> $ty:ty, $variant:ident);+ $(;)?) => {
        $(
            $(#[$meta])*
            pub fn $fn_name(&self) -> Option<$ty> {
                match self {
                    NbtTag::$variant(v) => Some(*v),
                    _ => None,
                }
            }
        )+
    };
}

/// Generates `as_*` reference accessors on `NbtTag`.
macro_rules! impl_nbt_ref_accessor {
    ($($(#[$meta:meta])* $fn_name:ident -> $ty:ty, $variant:ident);+ $(;)?) => {
        $(
            $(#[$meta])*
            pub fn $fn_name(&self) -> Option<$ty> {
                match self {
                    NbtTag::$variant(v) => Some(v),
                    _ => None,
                }
            }
        )+
    };
}

impl NbtTag {
    /// Returns the NBT tag type ID for this value.
    pub fn type_id(&self) -> u8 {
        match self {
            NbtTag::Byte(_) => TAG_BYTE,
            NbtTag::Short(_) => TAG_SHORT,
            NbtTag::Int(_) => TAG_INT,
            NbtTag::Long(_) => TAG_LONG,
            NbtTag::Float(_) => TAG_FLOAT,
            NbtTag::Double(_) => TAG_DOUBLE,
            NbtTag::ByteArray(_) => TAG_BYTE_ARRAY,
            NbtTag::String(_) => TAG_STRING,
            NbtTag::List(_) => TAG_LIST,
            NbtTag::Compound(_) => TAG_COMPOUND,
            NbtTag::IntArray(_) => TAG_INT_ARRAY,
            NbtTag::LongArray(_) => TAG_LONG_ARRAY,
        }
    }

    /// Returns a human-readable name for the tag type.
    pub fn type_name(&self) -> &'static str {
        match self {
            NbtTag::Byte(_) => "Byte",
            NbtTag::Short(_) => "Short",
            NbtTag::Int(_) => "Int",
            NbtTag::Long(_) => "Long",
            NbtTag::Float(_) => "Float",
            NbtTag::Double(_) => "Double",
            NbtTag::ByteArray(_) => "ByteArray",
            NbtTag::String(_) => "String",
            NbtTag::List(_) => "List",
            NbtTag::Compound(_) => "Compound",
            NbtTag::IntArray(_) => "IntArray",
            NbtTag::LongArray(_) => "LongArray",
        }
    }

    impl_nbt_primitive_accessor! {
        /// Returns the byte value, or `None` if not a `Byte` tag.
        as_byte -> i8, Byte;
        /// Returns the short value, or `None` if not a `Short` tag.
        as_short -> i16, Short;
        /// Returns the int value, or `None` if not an `Int` tag.
        as_int -> i32, Int;
        /// Returns the long value, or `None` if not a `Long` tag.
        as_long -> i64, Long;
        /// Returns the float value, or `None` if not a `Float` tag.
        as_float -> f32, Float;
        /// Returns the double value, or `None` if not a `Double` tag.
        as_double -> f64, Double
    }

    impl_nbt_ref_accessor! {
        /// Returns the string value as a `&str`, or `None` if not a `String` tag.
        as_str -> &str, String;
        /// Returns a reference to the byte array, or `None` if not a `ByteArray` tag.
        as_byte_array -> &[i8], ByteArray;
        /// Returns a reference to the int array, or `None` if not an `IntArray` tag.
        as_int_array -> &[i32], IntArray;
        /// Returns a reference to the long array, or `None` if not a `LongArray` tag.
        as_long_array -> &[i64], LongArray;
        /// Returns a reference to the compound, or `None` if not a `Compound` tag.
        as_compound -> &NbtCompound, Compound;
        /// Returns a reference to the list, or `None` if not a `List` tag.
        as_list -> &NbtList, List
    }

    /// Returns a mutable reference to the compound, or `None`.
    pub fn as_compound_mut(&mut self) -> Option<&mut NbtCompound> {
        match self {
            NbtTag::Compound(c) => Some(c),
            _ => None,
        }
    }
}

// ── From impls ──────────────────────────────────────────────────────────

/// Generates `From<$ty> for NbtTag` wrapping in the given variant.
macro_rules! impl_nbt_from {
    ($($ty:ty => $variant:ident),+ $(,)?) => {
        $(
            impl From<$ty> for NbtTag {
                fn from(v: $ty) -> Self {
                    NbtTag::$variant(v)
                }
            }
        )+
    };
}

impl_nbt_from! {
    i8     => Byte,
    i16    => Short,
    i32    => Int,
    i64    => Long,
    f32    => Float,
    f64    => Double,
    String => String,
}

impl From<&str> for NbtTag {
    fn from(v: &str) -> Self {
        NbtTag::String(v.to_owned())
    }
}

impl From<NbtCompound> for NbtTag {
    fn from(v: NbtCompound) -> Self {
        NbtTag::Compound(v)
    }
}

impl From<NbtList> for NbtTag {
    fn from(v: NbtList) -> Self {
        NbtTag::List(v)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_type_id_returns_correct_id() {
        assert_eq!(NbtTag::Byte(0).type_id(), TAG_BYTE);
        assert_eq!(NbtTag::Short(0).type_id(), TAG_SHORT);
        assert_eq!(NbtTag::Int(0).type_id(), TAG_INT);
        assert_eq!(NbtTag::Long(0).type_id(), TAG_LONG);
        assert_eq!(NbtTag::Float(0.0).type_id(), TAG_FLOAT);
        assert_eq!(NbtTag::Double(0.0).type_id(), TAG_DOUBLE);
        assert_eq!(NbtTag::ByteArray(vec![]).type_id(), TAG_BYTE_ARRAY);
        assert_eq!(NbtTag::String(String::new()).type_id(), TAG_STRING);
        assert_eq!(NbtTag::List(NbtList::empty()).type_id(), TAG_LIST);
        assert_eq!(NbtTag::Compound(NbtCompound::new()).type_id(), TAG_COMPOUND);
        assert_eq!(NbtTag::IntArray(vec![]).type_id(), TAG_INT_ARRAY);
        assert_eq!(NbtTag::LongArray(vec![]).type_id(), TAG_LONG_ARRAY);
    }

    #[test]
    fn test_type_name() {
        assert_eq!(NbtTag::Byte(0).type_name(), "Byte");
        assert_eq!(NbtTag::Short(0).type_name(), "Short");
        assert_eq!(NbtTag::Int(0).type_name(), "Int");
        assert_eq!(NbtTag::Long(0).type_name(), "Long");
        assert_eq!(NbtTag::Float(0.0).type_name(), "Float");
        assert_eq!(NbtTag::Double(0.0).type_name(), "Double");
        assert_eq!(NbtTag::ByteArray(vec![]).type_name(), "ByteArray");
        assert_eq!(NbtTag::String(String::new()).type_name(), "String");
        assert_eq!(NbtTag::List(NbtList::empty()).type_name(), "List");
        assert_eq!(NbtTag::Compound(NbtCompound::new()).type_name(), "Compound");
        assert_eq!(NbtTag::IntArray(vec![]).type_name(), "IntArray");
        assert_eq!(NbtTag::LongArray(vec![]).type_name(), "LongArray");
    }

    #[test]
    fn test_as_byte() {
        assert_eq!(NbtTag::Byte(42).as_byte(), Some(42));
        assert_eq!(NbtTag::Int(42).as_byte(), None);
    }

    #[test]
    fn test_as_short() {
        assert_eq!(NbtTag::Short(1000).as_short(), Some(1000));
        assert_eq!(NbtTag::Byte(1).as_short(), None);
    }

    #[test]
    fn test_as_int() {
        assert_eq!(NbtTag::Int(100_000).as_int(), Some(100_000));
        assert_eq!(NbtTag::Long(1).as_int(), None);
    }

    #[test]
    fn test_as_long() {
        assert_eq!(NbtTag::Long(i64::MAX).as_long(), Some(i64::MAX));
        assert_eq!(NbtTag::Int(1).as_long(), None);
    }

    #[test]
    fn test_as_float() {
        assert_eq!(NbtTag::Float(1.5).as_float(), Some(1.5));
        assert_eq!(NbtTag::Double(1.5).as_float(), None);
    }

    #[test]
    fn test_as_double() {
        assert_eq!(NbtTag::Double(1.25).as_double(), Some(1.25));
        assert_eq!(NbtTag::Float(1.0).as_double(), None);
    }

    #[test]
    fn test_as_str() {
        let tag = NbtTag::String("hello".into());
        assert_eq!(tag.as_str(), Some("hello"));
        assert_eq!(NbtTag::Int(0).as_str(), None);
    }

    #[test]
    fn test_as_compound() {
        let mut c = NbtCompound::new();
        c.put_int("x", 1);
        let tag = NbtTag::Compound(c);
        assert!(tag.as_compound().is_some());
        assert_eq!(tag.as_compound().unwrap().get_int("x"), Some(1));
        assert!(NbtTag::Int(0).as_compound().is_none());
    }

    #[test]
    fn test_as_compound_mut() {
        let mut tag = NbtTag::Compound(NbtCompound::new());
        tag.as_compound_mut().unwrap().put_int("x", 5);
        assert_eq!(tag.as_compound().unwrap().get_int("x"), Some(5));
    }

    #[test]
    fn test_as_list() {
        let tag = NbtTag::List(NbtList::empty());
        assert!(tag.as_list().is_some());
        assert!(NbtTag::Int(0).as_list().is_none());
    }

    #[test]
    fn test_as_byte_array() {
        let tag = NbtTag::ByteArray(vec![1, 2, 3]);
        assert_eq!(tag.as_byte_array(), Some(&[1i8, 2, 3][..]));
        assert!(NbtTag::Int(0).as_byte_array().is_none());
    }

    #[test]
    fn test_as_int_array() {
        let tag = NbtTag::IntArray(vec![10, 20]);
        assert_eq!(tag.as_int_array(), Some(&[10i32, 20][..]));
    }

    #[test]
    fn test_as_long_array() {
        let tag = NbtTag::LongArray(vec![100, 200]);
        assert_eq!(tag.as_long_array(), Some(&[100i64, 200][..]));
    }

    #[test]
    fn test_from_i8() {
        let tag: NbtTag = 42i8.into();
        assert_eq!(tag, NbtTag::Byte(42));
    }

    #[test]
    fn test_from_i16() {
        let tag: NbtTag = 1000i16.into();
        assert_eq!(tag, NbtTag::Short(1000));
    }

    #[test]
    fn test_from_i32() {
        let tag: NbtTag = 100_000i32.into();
        assert_eq!(tag, NbtTag::Int(100_000));
    }

    #[test]
    fn test_from_i64() {
        let tag: NbtTag = 1_000_000i64.into();
        assert_eq!(tag, NbtTag::Long(1_000_000));
    }

    #[test]
    fn test_from_f32() {
        let tag: NbtTag = 1.5f32.into();
        assert_eq!(tag, NbtTag::Float(1.5));
    }

    #[test]
    fn test_from_f64() {
        let tag: NbtTag = 1.25f64.into();
        assert_eq!(tag, NbtTag::Double(1.25));
    }

    #[test]
    fn test_from_string() {
        let tag: NbtTag = String::from("hello").into();
        assert_eq!(tag, NbtTag::String("hello".into()));
    }

    #[test]
    fn test_from_str_ref() {
        let tag: NbtTag = "world".into();
        assert_eq!(tag, NbtTag::String("world".into()));
    }

    #[test]
    fn test_from_compound() {
        let c = NbtCompound::new();
        let tag: NbtTag = c.into();
        assert!(matches!(tag, NbtTag::Compound(_)));
    }

    #[test]
    fn test_from_list() {
        let l = NbtList::empty();
        let tag: NbtTag = l.into();
        assert!(matches!(tag, NbtTag::List(_)));
    }
}
