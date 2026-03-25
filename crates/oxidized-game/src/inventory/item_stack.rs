//! Item stack and data component representation.
//!
//! An [`ItemStack`] represents one or more items of the same type in an
//! inventory slot. Each stack carries a [`DataComponentPatch`] that stores
//! only the components that differ from the item's default prototype
//! (enchantments, custom name, damage, lore, etc.).
//!
//! # Wire format (1.20.5+)
//!
//! On the network, item stacks use the "optional item" codec:
//! `VarInt(count)` — 0 = empty, then `VarInt(item_id)` + `DataComponentPatch`.
//!
//! # NBT format (persistence)
//!
//! ```text
//! {id:"minecraft:diamond_sword", count:1b, components:{...}}
//! ```

use std::collections::{HashMap, HashSet};

use oxidized_nbt::{NbtCompound, NbtTag};
use thiserror::Error;

/// Errors that can occur when deserializing inventory data from NBT.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ItemError {
    /// Missing required field in NBT.
    #[error("missing '{0}' field in ItemStack NBT")]
    MissingField(&'static str),
}

/// Identifies an item type by its namespaced resource key.
///
/// # Examples
///
/// ```
/// use oxidized_game::inventory::ItemId;
///
/// let id = ItemId("minecraft:diamond_sword".into());
/// assert_eq!(id.0, "minecraft:diamond_sword");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ItemId(pub String);

/// Block name for air, used to avoid magic strings in item identity checks.
const AIR_ITEM_NAME: &str = "minecraft:air";

impl ItemId {
    /// Returns `true` if this is an empty/air item ID.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty() || self.0 == AIR_ITEM_NAME
    }
}

/// Sparse map of data components that differ from the item's default prototype.
///
/// In Minecraft 1.20.5+, item data (enchantments, damage, custom name, lore,
/// etc.) is stored as typed components rather than free-form NBT. A
/// `DataComponentPatch` records only the *differences* from the item type's
/// default set of components.
///
/// # Examples
///
/// ```
/// use oxidized_game::inventory::DataComponentPatch;
///
/// let patch = DataComponentPatch::default();
/// assert!(patch.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DataComponentPatch {
    /// Components added or modified from the default.
    pub added: HashMap<String, NbtTag>,
    /// Component type keys removed from the default.
    pub removed: HashSet<String>,
}

impl DataComponentPatch {
    /// Returns `true` if no components are modified.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }

    /// Serializes the patch to an NBT compound for disk storage.
    pub fn to_nbt(&self) -> NbtCompound {
        let mut compound = NbtCompound::new();
        for (key, value) in &self.added {
            compound.put(key.clone(), value.clone());
        }
        if !self.removed.is_empty() {
            // Store removed keys as a compound with empty markers
            let mut removed_compound = NbtCompound::new();
            for key in &self.removed {
                removed_compound.put(key.clone(), NbtTag::Byte(0));
            }
            compound.put("!removed", NbtTag::Compound(removed_compound));
        }
        compound
    }

    /// Deserializes a patch from an NBT compound.
    pub fn from_nbt(compound: &NbtCompound) -> Self {
        let mut added = HashMap::new();
        let mut removed = HashSet::new();

        for (key, value) in compound.iter() {
            if key == "!removed" {
                if let NbtTag::Compound(removed_compound) = value {
                    for (rkey, _) in removed_compound.iter() {
                        removed.insert(rkey.to_string());
                    }
                }
            } else {
                added.insert(key.to_string(), value.clone());
            }
        }

        Self { added, removed }
    }
}

/// A stack of items in an inventory slot.
///
/// # Examples
///
/// ```
/// use oxidized_game::inventory::ItemStack;
///
/// let stack = ItemStack::new("minecraft:stone", 64);
/// assert!(!stack.is_empty());
/// assert_eq!(stack.count, 64);
///
/// let empty = ItemStack::empty();
/// assert!(empty.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ItemStack {
    /// The item type identifier.
    pub item: ItemId,
    /// Number of items in the stack. ≤ 0 means empty.
    pub count: i32,
    /// Components that differ from the item's default prototype.
    pub components: DataComponentPatch,
}

impl ItemStack {
    /// Creates a new item stack with the given item and count.
    pub fn new(item: impl Into<String>, count: i32) -> Self {
        Self {
            item: ItemId(item.into()),
            count,
            components: DataComponentPatch::default(),
        }
    }

    /// Returns the canonical empty item stack (air, count 0).
    pub fn empty() -> Self {
        Self {
            item: ItemId(String::new()),
            count: 0,
            components: DataComponentPatch::default(),
        }
    }

    /// Returns `true` if this stack is empty (count ≤ 0 or item is air/empty).
    pub fn is_empty(&self) -> bool {
        self.count <= 0 || self.item.is_empty()
    }

    /// Returns a copy with the given count.
    pub fn with_count(mut self, count: i32) -> Self {
        self.count = count;
        self
    }

    /// Splits off `amount` items from this stack and returns them as a new stack.
    ///
    /// The original stack's count is reduced by the amount taken.
    /// If `amount` exceeds the current count, takes all remaining items.
    pub fn split(&mut self, amount: i32) -> Self {
        let taken = amount.min(self.count);
        self.count -= taken;
        Self {
            item: self.item.clone(),
            count: taken,
            components: self.components.clone(),
        }
    }

    /// Returns `true` if this stack can be merged with `other`
    /// (same item type and compatible components).
    pub fn is_stackable_with(&self, other: &ItemStack) -> bool {
        self.item == other.item && self.components == other.components
    }

    /// Returns `true` if this stack has enchantments applied.
    ///
    /// Used by [`crate::player::PlayerInventory::suitable_hotbar_slot`] to
    /// prefer replacing non-enchanted items when all hotbar slots are occupied.
    pub fn is_enchanted(&self) -> bool {
        self.components.added.contains_key("minecraft:enchantments")
    }

    /// Serializes to NBT for disk persistence.
    ///
    /// Returns `None` if the stack is empty (matching vanilla behavior —
    /// empty slots are not stored in the inventory NBT list).
    pub fn to_nbt(&self) -> Option<NbtCompound> {
        if self.is_empty() {
            return None;
        }
        let mut tag = NbtCompound::new();
        tag.put_string("id", &self.item.0);
        tag.put_int("count", self.count);
        if !self.components.is_empty() {
            let patch_tag = self.components.to_nbt();
            tag.put("components", NbtTag::Compound(patch_tag));
        }
        Some(tag)
    }

    /// Deserializes from an NBT compound (disk persistence format).
    ///
    /// # Errors
    ///
    /// Returns [`ItemError::MissingField`] if the `id` field is absent.
    pub fn from_nbt(tag: &NbtCompound) -> Result<Self, ItemError> {
        let item = tag
            .get_string("id")
            .ok_or(ItemError::MissingField("id"))?
            .to_string();
        // Vanilla 26.1 stores count as IntTag. Accept ByteTag too for
        // backward compatibility with older saves.
        let count = tag
            .get_int("count")
            .or_else(|| tag.get_byte("count").map(|b| b as i32))
            .unwrap_or(1);
        let components = if let Some(NbtTag::Compound(c)) = tag.get("components") {
            DataComponentPatch::from_nbt(c)
        } else {
            DataComponentPatch::default()
        };
        Ok(Self {
            item: ItemId(item),
            count,
            components,
        })
    }
}

impl Default for ItemStack {
    fn default() -> Self {
        Self::empty()
    }
}

/// Returns the maximum stack size for a given item.
///
/// Looks up the item in the vanilla item registry. Returns `64` for unknown
/// items.
pub fn max_stack_size(item: &ItemId) -> i32 {
    super::item_ids::max_stack_size_by_name(&item.0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_item_stack_is_empty() {
        let s = ItemStack::empty();
        assert!(s.is_empty());
    }

    #[test]
    fn test_new_item_stack_not_empty() {
        let s = ItemStack::new("minecraft:stone", 64);
        assert!(!s.is_empty());
        assert_eq!(s.count, 64);
        assert_eq!(s.item.0, "minecraft:stone");
    }

    #[test]
    fn test_zero_count_is_empty() {
        let s = ItemStack::new("minecraft:stone", 0);
        assert!(s.is_empty());
    }

    #[test]
    fn test_air_is_empty() {
        let s = ItemStack::new("minecraft:air", 1);
        assert!(s.is_empty());
    }

    #[test]
    fn test_item_stack_split_reduces_count() {
        let mut s = ItemStack::new("minecraft:stone", 64);
        let split = s.split(16);
        assert_eq!(split.count, 16);
        assert_eq!(s.count, 48);
        assert_eq!(split.item, s.item);
    }

    #[test]
    fn test_item_stack_split_does_not_exceed_count() {
        let mut s = ItemStack::new("minecraft:stone", 5);
        let split = s.split(10);
        assert_eq!(split.count, 5);
        assert_eq!(s.count, 0);
    }

    #[test]
    fn test_item_stack_with_count() {
        let s = ItemStack::new("minecraft:stone", 1).with_count(32);
        assert_eq!(s.count, 32);
    }

    #[test]
    fn test_is_stackable_with_same_item() {
        let a = ItemStack::new("minecraft:stone", 32);
        let b = ItemStack::new("minecraft:stone", 16);
        assert!(a.is_stackable_with(&b));
    }

    #[test]
    fn test_is_not_stackable_with_different_item() {
        let a = ItemStack::new("minecraft:stone", 32);
        let b = ItemStack::new("minecraft:dirt", 16);
        assert!(!a.is_stackable_with(&b));
    }

    #[test]
    fn test_item_stack_nbt_roundtrip() {
        let original = ItemStack::new("minecraft:diamond_sword", 1);
        let nbt = original.to_nbt().unwrap();
        let decoded = ItemStack::from_nbt(&nbt).unwrap();
        assert_eq!(decoded.item.0, "minecraft:diamond_sword");
        assert_eq!(decoded.count, 1);
        assert!(decoded.components.is_empty());
    }

    #[test]
    fn test_empty_item_stack_to_nbt_returns_none() {
        let s = ItemStack::empty();
        assert!(s.to_nbt().is_none());
    }

    #[test]
    fn test_item_stack_with_components_nbt_roundtrip() {
        let mut components = DataComponentPatch::default();
        components
            .added
            .insert("minecraft:damage".to_string(), NbtTag::Int(42));

        let original = ItemStack {
            item: ItemId("minecraft:diamond_sword".into()),
            count: 1,
            components,
        };

        let nbt = original.to_nbt().unwrap();
        let decoded = ItemStack::from_nbt(&nbt).unwrap();
        assert_eq!(decoded.item.0, "minecraft:diamond_sword");
        assert_eq!(decoded.count, 1);
        assert_eq!(
            decoded.components.added.get("minecraft:damage"),
            Some(&NbtTag::Int(42))
        );
    }

    #[test]
    fn test_data_component_patch_empty() {
        let patch = DataComponentPatch::default();
        assert!(patch.is_empty());
    }

    #[test]
    fn test_data_component_patch_with_removed() {
        let mut patch = DataComponentPatch::default();
        patch.removed.insert("minecraft:lore".to_string());
        assert!(!patch.is_empty());

        let nbt = patch.to_nbt();
        let decoded = DataComponentPatch::from_nbt(&nbt);
        assert!(decoded.removed.contains("minecraft:lore"));
    }

    // --- is_enchanted ---

    #[test]
    fn test_is_enchanted_false_by_default() {
        let s = ItemStack::new("minecraft:diamond_sword", 1);
        assert!(!s.is_enchanted());
    }

    #[test]
    fn test_is_enchanted_true_with_enchantments() {
        let mut s = ItemStack::new("minecraft:diamond_sword", 1);
        s.components
            .added
            .insert("minecraft:enchantments".to_string(), NbtTag::Int(1));
        assert!(s.is_enchanted());
    }

    #[test]
    fn test_max_stack_size_tools() {
        assert_eq!(max_stack_size(&ItemId("minecraft:diamond_sword".into())), 1);
        assert_eq!(max_stack_size(&ItemId("minecraft:iron_pickaxe".into())), 1);
    }

    #[test]
    fn test_max_stack_size_default() {
        assert_eq!(max_stack_size(&ItemId("minecraft:stone".into())), 64);
        assert_eq!(max_stack_size(&ItemId("minecraft:dirt".into())), 64);
    }

    #[test]
    fn test_max_stack_size_special() {
        assert_eq!(max_stack_size(&ItemId("minecraft:ender_pearl".into())), 16);
        assert_eq!(max_stack_size(&ItemId("minecraft:snowball".into())), 16);
    }

    #[test]
    fn test_max_stack_size_no_false_positives() {
        // "waxed" contains "axe" as substring — must NOT be treated as a tool
        assert_eq!(
            max_stack_size(&ItemId("minecraft:waxed_copper_block".into())),
            64
        );
        assert_eq!(
            max_stack_size(&ItemId("minecraft:waxed_oxidized_copper".into())),
            64
        );
    }

    #[test]
    fn test_default_is_empty() {
        let s = ItemStack::default();
        assert!(s.is_empty());
    }
}
