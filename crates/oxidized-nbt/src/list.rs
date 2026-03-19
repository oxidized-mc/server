//! [`NbtList`] — a type-homogeneous list of NBT tags.

use crate::compound::NbtCompound;
use crate::error::{NbtError, TAG_END};
use crate::tag::NbtTag;

/// A type-homogeneous list of NBT tags.
///
/// Every element must share the same tag type ID. An empty list has element
/// type [`TAG_END`] (0); the first push to an empty list sets its element
/// type automatically.
///
/// # Examples
///
/// ```
/// use oxidized_nbt::{NbtList, NbtTag};
///
/// let mut list = NbtList::empty();
/// list.push(NbtTag::Int(10)).unwrap();
/// list.push(NbtTag::Int(20)).unwrap();
///
/// assert_eq!(list.len(), 2);
/// assert_eq!(list.get(0), Some(&NbtTag::Int(10)));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct NbtList {
    element_type: u8,
    elements: Vec<NbtTag>,
}

impl NbtList {
    /// Creates a new list that will contain elements of `element_type`.
    pub fn new(element_type: u8) -> Self {
        Self {
            element_type,
            elements: Vec::new(),
        }
    }

    /// Creates an empty list with element type [`TAG_END`].
    pub fn empty() -> Self {
        Self::new(TAG_END)
    }

    /// Returns the tag type ID shared by all elements.
    pub fn element_type(&self) -> u8 {
        self.element_type
    }

    /// Returns a reference to the element at `index`, or `None`.
    pub fn get(&self, index: usize) -> Option<&NbtTag> {
        self.elements.get(index)
    }

    /// Appends `tag` to the list.
    ///
    /// If the list is empty ([`TAG_END`] element type), the element type is
    /// set to `tag`'s type. Otherwise, the tag's type must match.
    ///
    /// # Errors
    ///
    /// Returns [`NbtError::ListTypeMismatch`] if the tag type does not match
    /// the list's element type.
    pub fn push(&mut self, tag: NbtTag) -> Result<(), NbtError> {
        if self.element_type == TAG_END {
            self.element_type = tag.type_id();
        } else if tag.type_id() != self.element_type {
            return Err(NbtError::ListTypeMismatch {
                expected: self.element_type,
                got: tag.type_id(),
            });
        }
        self.elements.push(tag);
        Ok(())
    }

    /// Returns the number of elements.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Returns `true` if the list has no elements.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Iterates over all elements.
    pub fn iter(&self) -> std::slice::Iter<'_, NbtTag> {
        self.elements.iter()
    }

    /// Convenience iterator over compound elements.
    ///
    /// Non-compound elements are silently skipped (though a well-formed list
    /// should never mix types).
    pub fn compounds(&self) -> impl Iterator<Item = &NbtCompound> {
        self.elements.iter().filter_map(NbtTag::as_compound)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::error::{TAG_BYTE, TAG_INT};

    #[test]
    fn test_empty_list() {
        let list = NbtList::empty();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert_eq!(list.element_type(), TAG_END);
    }

    #[test]
    fn test_push_sets_type_on_empty() {
        let mut list = NbtList::empty();
        list.push(NbtTag::Int(42)).unwrap();
        assert_eq!(list.element_type(), TAG_INT);
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn test_push_wrong_type_errors() {
        let mut list = NbtList::new(TAG_INT);
        list.push(NbtTag::Int(1)).unwrap();
        let result = list.push(NbtTag::Byte(2));
        assert!(result.is_err());
    }

    #[test]
    fn test_push_matching_type_succeeds() {
        let mut list = NbtList::new(TAG_BYTE);
        list.push(NbtTag::Byte(1)).unwrap();
        list.push(NbtTag::Byte(2)).unwrap();
        list.push(NbtTag::Byte(3)).unwrap();
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn test_get() {
        let mut list = NbtList::new(TAG_INT);
        list.push(NbtTag::Int(10)).unwrap();
        list.push(NbtTag::Int(20)).unwrap();

        assert_eq!(list.get(0), Some(&NbtTag::Int(10)));
        assert_eq!(list.get(1), Some(&NbtTag::Int(20)));
        assert_eq!(list.get(2), None);
    }

    #[test]
    fn test_iteration() {
        let mut list = NbtList::new(TAG_INT);
        list.push(NbtTag::Int(1)).unwrap();
        list.push(NbtTag::Int(2)).unwrap();
        list.push(NbtTag::Int(3)).unwrap();

        let values: Vec<i32> = list.iter().filter_map(NbtTag::as_int).collect();
        assert_eq!(values, vec![1, 2, 3]);
    }

    #[test]
    fn test_compounds_iterator() {
        let mut list = NbtList::new(crate::error::TAG_COMPOUND);
        let mut c1 = NbtCompound::new();
        c1.put_int("id", 1);
        let mut c2 = NbtCompound::new();
        c2.put_int("id", 2);

        list.push(NbtTag::Compound(c1)).unwrap();
        list.push(NbtTag::Compound(c2)).unwrap();

        let ids: Vec<i32> = list.compounds().filter_map(|c| c.get_int("id")).collect();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn test_new_with_specific_type() {
        let list = NbtList::new(TAG_BYTE);
        assert!(list.is_empty());
        assert_eq!(list.element_type(), TAG_BYTE);
    }
}
