//! Item registry: lookup of items by name.

use std::io::Read;

use ahash::AHashMap;
use flate2::read::GzDecoder;

use super::error::RegistryError;
use super::item::Item;

/// The compressed `items.json` data, embedded at compile time.
const ITEMS_DATA_GZ: &[u8] = include_bytes!("../data/items.json.gz");

/// Registry of all item types.
///
/// Provides O(1) lookup of item definitions by name.
pub struct ItemRegistry {
    /// All items in registration order.
    items: Vec<Item>,
    /// Name → item index lookup.
    by_name: AHashMap<String, usize>,
}

impl ItemRegistry {
    /// Load the item registry from the embedded compressed JSON data.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::Decompress`] if decompression fails, or
    /// [`RegistryError::Json`] if the JSON is malformed.
    pub fn load() -> Result<Self, RegistryError> {
        let mut decoder = GzDecoder::new(ITEMS_DATA_GZ);
        let mut json_str = String::new();
        decoder.read_to_string(&mut json_str)?;

        let root: serde_json::Value = serde_json::from_str(&json_str)?;
        let empty_map = serde_json::Map::new();
        let obj = root.as_object().unwrap_or(&empty_map);

        let mut items = Vec::with_capacity(obj.len());
        let mut by_name = AHashMap::with_capacity(obj.len());

        for (name, value) in obj {
            let raw_stack = value
                .get("max_stack_size")
                .and_then(|v| v.as_u64())
                .unwrap_or(64);
            let max_stack_size = u8::try_from(raw_stack).map_err(|_| {
                RegistryError::InvalidItemProperty(name.clone(), "max_stack_size", raw_stack)
            })?;

            let raw_damage = value
                .get("max_damage")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let max_damage = u16::try_from(raw_damage).map_err(|_| {
                RegistryError::InvalidItemProperty(name.clone(), "max_damage", raw_damage)
            })?;

            let idx = items.len();
            by_name.insert(name.clone(), idx);

            items.push(Item {
                name: name.clone(),
                max_stack_size,
                max_damage,
            });
        }

        Ok(Self { items, by_name })
    }

    /// Get an item definition by its registry name (e.g., `"minecraft:stone"`).
    pub fn get(&self, name: &str) -> Option<&Item> {
        self.by_name.get(name).map(|&idx| &self.items[idx])
    }

    /// Returns the protocol numeric ID for an item name, or `None` if unknown.
    ///
    /// The ID is the item's index in the vanilla registration-order list
    /// (alphabetical). Empty strings return `None`.
    pub fn name_to_id(&self, name: &str) -> Option<i32> {
        self.by_name.get(name).map(|&idx| idx as i32)
    }

    /// Returns the item name for a protocol numeric ID, or `None` if out of range.
    pub fn id_to_name(&self, id: i32) -> Option<&str> {
        if id < 0 {
            return None;
        }
        self.items.get(id as usize).map(|item| item.name.as_str())
    }

    /// Returns the maximum stack size for an item by name.
    ///
    /// Returns `64` (the vanilla default) if the item is not found.
    pub fn max_stack_size(&self, name: &str) -> u8 {
        self.get(name).map_or(64, |item| item.max_stack_size)
    }

    /// Total number of items in the registry.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn registry() -> ItemRegistry {
        ItemRegistry::load().expect("failed to load item registry")
    }

    #[test]
    fn test_load_items() {
        let _ = registry();
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
    fn test_name_to_id_alphabetical_order() {
        let reg = registry();
        // Items are in alphabetical order; acacia_boat should be first (index 0).
        assert_eq!(reg.name_to_id("minecraft:acacia_boat"), Some(0));
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
}
