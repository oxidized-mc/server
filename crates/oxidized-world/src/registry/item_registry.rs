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
            let max_stack_size = value
                .get("max_stack_size")
                .and_then(|v| v.as_u64())
                .unwrap_or(64) as u8;

            let max_damage = value
                .get("max_damage")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u16;

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
}
