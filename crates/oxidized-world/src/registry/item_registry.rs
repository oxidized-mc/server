//! Item registry: O(1) lookup of items backed by compile-time generated static data.

use super::error::RegistryError;
use super::item::Item;
use super::item_generated;

/// Registry of all item types.
///
/// All data is generated at compile time in the `item_generated` module. This
/// struct is zero-sized and acts as a convenient handle with the same API
/// surface that consumers already use.
pub struct ItemRegistry;

impl ItemRegistry {
    /// Create a new item registry.
    ///
    /// This is a no-op — all data is static.
    pub const fn new() -> Self {
        Self
    }

    /// Backward-compatible load method.
    ///
    /// Always succeeds since data is compiled-in.
    ///
    /// # Errors
    ///
    /// Never fails. Signature is kept for API compatibility.
    pub fn load() -> Result<Self, RegistryError> {
        Ok(Self)
    }

    /// Get an item definition by its registry name (e.g., `"minecraft:stone"`).
    pub fn get(&self, name: &str) -> Option<Item> {
        let idx = self.name_to_id_usize(name)?;
        Some(Item {
            name: item_generated::ITEM_NAMES[idx].to_owned(),
            max_stack_size: item_generated::ITEM_MAX_STACK_SIZES[idx],
            max_damage: item_generated::ITEM_MAX_DAMAGES[idx],
        })
    }

    /// Returns the protocol numeric ID for an item name, or `None` if unknown.
    ///
    /// The ID is the item's index in the vanilla registration-order list.
    /// Empty strings return `None`.
    pub fn name_to_id(&self, name: &str) -> Option<i32> {
        self.name_to_id_usize(name).map(|idx| idx as i32)
    }

    /// Returns the item name for a protocol numeric ID, or `None` if out of range.
    pub fn id_to_name(&self, id: i32) -> Option<&'static str> {
        if id < 0 {
            return None;
        }
        item_generated::ITEM_NAMES.get(id as usize).copied()
    }

    /// Returns the maximum stack size for an item by name.
    ///
    /// Returns `64` (the vanilla default) if the item is not found.
    pub fn max_stack_size(&self, name: &str) -> u8 {
        self.name_to_id_usize(name)
            .map_or(64, |idx| item_generated::ITEM_MAX_STACK_SIZES[idx])
    }

    /// Total number of items in the registry.
    pub fn item_count(&self) -> usize {
        item_generated::ITEM_COUNT
    }

    /// Binary search lookup: name → registration-order index.
    fn name_to_id_usize(&self, name: &str) -> Option<usize> {
        item_generated::ITEM_NAMES_SORTED
            .binary_search_by_key(&name, |&(n, _)| n)
            .ok()
            .map(|pos| item_generated::ITEM_NAMES_SORTED[pos].1 as usize)
    }
}

impl Default for ItemRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn registry() -> ItemRegistry {
        ItemRegistry::new()
    }

    #[test]
    fn test_load_items() {
        let _ = ItemRegistry::load().expect("load should never fail");
    }

    #[test]
    fn test_item_count() {
        let reg = registry();
        assert_eq!(reg.item_count(), 1506);
    }

    #[test]
    fn test_diamond_sword() {
        let reg = registry();
        let sword = reg
            .get("minecraft:diamond_sword")
            .expect("diamond_sword missing");
        assert_eq!(sword.max_stack_size, 1);
        assert_eq!(sword.max_damage, 1561);
    }

    #[test]
    fn test_stone_default_stack() {
        let reg = registry();
        let stone = reg.get("minecraft:stone").expect("stone missing");
        assert_eq!(stone.max_stack_size, 64);
    }

    #[test]
    fn test_unknown_item_returns_none() {
        let reg = registry();
        assert!(reg.get("minecraft:not_an_item").is_none());
    }

    #[test]
    fn test_name_to_id_roundtrip() {
        let reg = registry();
        let id = reg.name_to_id("minecraft:stone").expect("stone not found");
        let name = reg.id_to_name(id).expect("id not found");
        assert_eq!(name, "minecraft:stone");
    }

    #[test]
    fn test_name_to_id_registration_order() {
        let reg = registry();
        // Items are in vanilla registration order; air should be first (index 0).
        assert_eq!(reg.name_to_id("minecraft:air"), Some(0));
        assert_eq!(reg.name_to_id("minecraft:stone"), Some(1));
        assert_eq!(reg.name_to_id("minecraft:grass_block"), Some(27));
    }

    #[test]
    fn test_id_to_name_out_of_range() {
        let reg = registry();
        assert!(reg.id_to_name(-1).is_none());
        assert!(reg.id_to_name(999_999).is_none());
    }

    #[test]
    fn test_max_stack_size_lookup() {
        let reg = registry();
        assert_eq!(reg.max_stack_size("minecraft:diamond_sword"), 1);
        assert_eq!(reg.max_stack_size("minecraft:stone"), 64);
        assert_eq!(reg.max_stack_size("minecraft:ender_pearl"), 16);
        // Unknown item → default 64
        assert_eq!(reg.max_stack_size("minecraft:nonexistent"), 64);
    }

    #[test]
    fn test_all_items_roundtrip() {
        let reg = registry();
        for id in 0..reg.item_count() as i32 {
            let name = reg.id_to_name(id).expect("id should be valid");
            let back = reg.name_to_id(name).expect("name should be valid");
            assert_eq!(back, id, "roundtrip failed for {name} (id {id})");
        }
    }

    #[test]
    fn test_spot_check_known_items() {
        let reg = registry();
        // Verify specific well-known items exist and have correct properties
        assert!(reg.get("minecraft:stone").is_some());
        assert!(reg.get("minecraft:diamond_sword").is_some());
        assert!(reg.get("minecraft:ender_pearl").is_some());

        let pearl = reg.get("minecraft:ender_pearl").expect("ender_pearl");
        assert_eq!(pearl.max_stack_size, 16);
        assert_eq!(pearl.max_damage, 0);
    }

    #[test]
    fn test_snapshot_item_names() {
        let names: Vec<&str> = (0..item_generated::ITEM_COUNT)
            .map(|i| item_generated::ITEM_NAMES[i])
            .collect();
        insta::assert_snapshot!(names.join("\n"));
    }
}
