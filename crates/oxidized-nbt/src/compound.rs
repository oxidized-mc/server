//! [`NbtCompound`] — an insertion-ordered map of named NBT tags.

use indexmap::IndexMap;

use crate::list::NbtList;
use crate::tag::NbtTag;

/// An insertion-ordered map of named NBT tags.
///
/// Backed by [`IndexMap`] so that iteration order matches insertion order,
/// which is important for deterministic serialization.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NbtCompound {
    entries: IndexMap<String, NbtTag>,
}

impl NbtCompound {
    /// Creates an empty compound.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a reference to the tag stored under `key`, if present.
    pub fn get(&self, key: &str) -> Option<&NbtTag> {
        self.entries.get(key)
    }

    /// Returns a mutable reference to the tag stored under `key`, if present.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut NbtTag> {
        self.entries.get_mut(key)
    }

    // ── Typed getters ───────────────────────────────────────────────────

    /// Returns the byte value for `key`, or `None`.
    pub fn get_byte(&self, key: &str) -> Option<i8> {
        self.entries.get(key).and_then(NbtTag::as_byte)
    }

    /// Returns the short value for `key`, or `None`.
    pub fn get_short(&self, key: &str) -> Option<i16> {
        self.entries.get(key).and_then(NbtTag::as_short)
    }

    /// Returns the int value for `key`, or `None`.
    pub fn get_int(&self, key: &str) -> Option<i32> {
        self.entries.get(key).and_then(NbtTag::as_int)
    }

    /// Returns the long value for `key`, or `None`.
    pub fn get_long(&self, key: &str) -> Option<i64> {
        self.entries.get(key).and_then(NbtTag::as_long)
    }

    /// Returns the float value for `key`, or `None`.
    pub fn get_float(&self, key: &str) -> Option<f32> {
        self.entries.get(key).and_then(NbtTag::as_float)
    }

    /// Returns the double value for `key`, or `None`.
    pub fn get_double(&self, key: &str) -> Option<f64> {
        self.entries.get(key).and_then(NbtTag::as_double)
    }

    /// Returns the string value for `key`, or `None`.
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.entries.get(key).and_then(NbtTag::as_str)
    }

    /// Returns a reference to the compound value for `key`, or `None`.
    pub fn get_compound(&self, key: &str) -> Option<&NbtCompound> {
        self.entries.get(key).and_then(NbtTag::as_compound)
    }

    /// Returns a mutable reference to the compound value for `key`, or `None`.
    pub fn get_compound_mut(&mut self, key: &str) -> Option<&mut NbtCompound> {
        self.entries.get_mut(key).and_then(NbtTag::as_compound_mut)
    }

    /// Returns a reference to the list value for `key`, or `None`.
    pub fn get_list(&self, key: &str) -> Option<&NbtList> {
        self.entries.get(key).and_then(NbtTag::as_list)
    }

    /// Returns a reference to the byte array for `key`, or `None`.
    pub fn get_byte_array(&self, key: &str) -> Option<&[i8]> {
        self.entries.get(key).and_then(NbtTag::as_byte_array)
    }

    /// Returns a reference to the int array for `key`, or `None`.
    pub fn get_int_array(&self, key: &str) -> Option<&[i32]> {
        self.entries.get(key).and_then(NbtTag::as_int_array)
    }

    /// Returns a reference to the long array for `key`, or `None`.
    pub fn get_long_array(&self, key: &str) -> Option<&[i64]> {
        self.entries.get(key).and_then(NbtTag::as_long_array)
    }

    // ── Insertion ───────────────────────────────────────────────────────

    /// Inserts a key-value pair, returning the previous value if the key existed.
    pub fn put(&mut self, key: impl Into<String>, value: impl Into<NbtTag>) -> Option<NbtTag> {
        self.entries.insert(key.into(), value.into())
    }

    /// Inserts a byte value.
    pub fn put_byte(&mut self, key: impl Into<String>, value: i8) -> Option<NbtTag> {
        self.put(key, NbtTag::Byte(value))
    }

    /// Inserts a short value.
    pub fn put_short(&mut self, key: impl Into<String>, value: i16) -> Option<NbtTag> {
        self.put(key, NbtTag::Short(value))
    }

    /// Inserts an int value.
    pub fn put_int(&mut self, key: impl Into<String>, value: i32) -> Option<NbtTag> {
        self.put(key, NbtTag::Int(value))
    }

    /// Inserts a long value.
    pub fn put_long(&mut self, key: impl Into<String>, value: i64) -> Option<NbtTag> {
        self.put(key, NbtTag::Long(value))
    }

    /// Inserts a float value.
    pub fn put_float(&mut self, key: impl Into<String>, value: f32) -> Option<NbtTag> {
        self.put(key, NbtTag::Float(value))
    }

    /// Inserts a double value.
    pub fn put_double(&mut self, key: impl Into<String>, value: f64) -> Option<NbtTag> {
        self.put(key, NbtTag::Double(value))
    }

    /// Inserts a string value.
    pub fn put_string(
        &mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Option<NbtTag> {
        self.put(key, NbtTag::String(value.into()))
    }

    // ── Query / mutation ────────────────────────────────────────────────

    /// Returns `true` if the compound contains the given key.
    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    /// Removes and returns the tag stored under `key`, if present.
    pub fn remove(&mut self, key: &str) -> Option<NbtTag> {
        self.entries.shift_remove(key)
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the compound has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    // ── Iterators ───────────────────────────────────────────────────────

    /// Iterates over `(key, value)` pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &NbtTag)> {
        self.entries.iter()
    }

    /// Iterates over keys in insertion order.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.keys()
    }

    /// Iterates over values in insertion order.
    pub fn values(&self) -> impl Iterator<Item = &NbtTag> {
        self.entries.values()
    }
}

impl IntoIterator for NbtCompound {
    type Item = (String, NbtTag);
    type IntoIter = indexmap::map::IntoIter<String, NbtTag>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<'a> IntoIterator for &'a NbtCompound {
    type Item = (&'a String, &'a NbtTag);
    type IntoIter = indexmap::map::Iter<'a, String, NbtTag>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_new_is_empty() {
        let c = NbtCompound::new();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn test_insertion_order_preserved() {
        let mut c = NbtCompound::new();
        c.put_string("z", "last");
        c.put_string("a", "first");
        c.put_string("m", "middle");

        let keys: Vec<&String> = c.keys().collect();
        assert_eq!(keys, &["z", "a", "m"]);
    }

    #[test]
    fn test_typed_getters() {
        let mut c = NbtCompound::new();
        c.put_byte("b", 1);
        c.put_short("s", 2);
        c.put_int("i", 3);
        c.put_long("l", 4);
        c.put_float("f", 5.0);
        c.put_double("d", 6.0);
        c.put_string("str", "hello");

        assert_eq!(c.get_byte("b"), Some(1));
        assert_eq!(c.get_short("s"), Some(2));
        assert_eq!(c.get_int("i"), Some(3));
        assert_eq!(c.get_long("l"), Some(4));
        assert_eq!(c.get_float("f"), Some(5.0));
        assert_eq!(c.get_double("d"), Some(6.0));
        assert_eq!(c.get_string("str"), Some("hello"));
    }

    #[test]
    fn test_typed_getters_wrong_type_returns_none() {
        let mut c = NbtCompound::new();
        c.put_byte("b", 1);
        assert_eq!(c.get_int("b"), None);
        assert_eq!(c.get_string("b"), None);
    }

    #[test]
    fn test_typed_getters_missing_key_returns_none() {
        let c = NbtCompound::new();
        assert_eq!(c.get_int("missing"), None);
    }

    #[test]
    fn test_put_returns_old_value() {
        let mut c = NbtCompound::new();
        assert!(c.put_int("x", 1).is_none());
        let old = c.put_int("x", 2);
        assert_eq!(old, Some(NbtTag::Int(1)));
        assert_eq!(c.get_int("x"), Some(2));
    }

    #[test]
    fn test_contains_key() {
        let mut c = NbtCompound::new();
        c.put_int("present", 1);
        assert!(c.contains_key("present"));
        assert!(!c.contains_key("absent"));
    }

    #[test]
    fn test_remove() {
        let mut c = NbtCompound::new();
        c.put_int("x", 42);
        let removed = c.remove("x");
        assert_eq!(removed, Some(NbtTag::Int(42)));
        assert!(c.is_empty());
        assert!(c.remove("x").is_none());
    }

    #[test]
    fn test_get_mut() {
        let mut c = NbtCompound::new();
        c.put_int("x", 1);
        if let Some(tag) = c.get_mut("x") {
            *tag = NbtTag::Int(99);
        }
        assert_eq!(c.get_int("x"), Some(99));
    }

    #[test]
    fn test_get_compound_and_mut() {
        let mut c = NbtCompound::new();
        let mut inner = NbtCompound::new();
        inner.put_int("y", 10);
        c.put("nested", inner);

        assert_eq!(c.get_compound("nested").unwrap().get_int("y"), Some(10));

        c.get_compound_mut("nested").unwrap().put_int("y", 20);
        assert_eq!(c.get_compound("nested").unwrap().get_int("y"), Some(20));
    }

    #[test]
    fn test_get_list() {
        let mut c = NbtCompound::new();
        let mut list = NbtList::new(crate::error::TAG_INT);
        list.push(NbtTag::Int(1)).unwrap();
        c.put("nums", list);

        let l = c.get_list("nums").unwrap();
        assert_eq!(l.len(), 1);
    }

    #[test]
    fn test_get_arrays() {
        let mut c = NbtCompound::new();
        c.put("ba", NbtTag::ByteArray(vec![1, 2]));
        c.put("ia", NbtTag::IntArray(vec![3, 4]));
        c.put("la", NbtTag::LongArray(vec![5, 6]));

        assert_eq!(c.get_byte_array("ba"), Some(&[1i8, 2][..]));
        assert_eq!(c.get_int_array("ia"), Some(&[3i32, 4][..]));
        assert_eq!(c.get_long_array("la"), Some(&[5i64, 6][..]));
    }

    #[test]
    fn test_iterate_in_order() {
        let mut c = NbtCompound::new();
        c.put_int("a", 1);
        c.put_int("b", 2);
        c.put_int("c", 3);

        let pairs: Vec<_> = c.iter().map(|(k, v)| (k.as_str(), v.as_int())).collect();
        assert_eq!(pairs, vec![("a", Some(1)), ("b", Some(2)), ("c", Some(3))]);
    }

    #[test]
    fn test_into_iterator_owned() {
        let mut c = NbtCompound::new();
        c.put_int("x", 10);
        c.put_int("y", 20);

        let pairs: Vec<_> = c.into_iter().collect();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("x".to_string(), NbtTag::Int(10)));
        assert_eq!(pairs[1], ("y".to_string(), NbtTag::Int(20)));
    }

    #[test]
    fn test_into_iterator_ref() {
        let mut c = NbtCompound::new();
        c.put_int("a", 1);

        let mut count = 0;
        for (k, v) in &c {
            assert_eq!(k, "a");
            assert_eq!(v, &NbtTag::Int(1));
            count += 1;
        }
        assert_eq!(count, 1);
    }

    #[test]
    fn test_len_and_is_empty() {
        let mut c = NbtCompound::new();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);

        c.put_int("a", 1);
        assert!(!c.is_empty());
        assert_eq!(c.len(), 1);

        c.put_int("b", 2);
        assert_eq!(c.len(), 2);

        c.remove("a");
        assert_eq!(c.len(), 1);
    }
}
