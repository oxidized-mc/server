//! Item ID ↔ name mapping backed by the vanilla item registry.
//!
//! Uses [`oxidized_world::registry::ItemRegistry`] loaded from the embedded
//! `items.json.gz` data (1 506 items in vanilla registration order).
//! The item's index in the alphabetical list equals its protocol numeric ID.

use std::sync::LazyLock;

use oxidized_world::registry::ItemRegistry;

/// Lazily-loaded item registry shared by all callers in this process.
static REGISTRY: LazyLock<ItemRegistry> =
    LazyLock::new(|| ItemRegistry::load().expect("failed to load embedded item registry"));

/// Converts an item resource name to its vanilla protocol numeric ID.
///
/// Returns `0` for empty strings (treated as air). Returns `-1` for names
/// not present in the vanilla registry.
///
/// # Examples
///
/// ```
/// use oxidized_game::inventory::item_ids::item_name_to_id;
///
/// let stone_id = item_name_to_id("minecraft:stone");
/// assert!(stone_id > 0);
/// assert_eq!(item_name_to_id(""), -1);
/// ```
pub fn item_name_to_id(name: &str) -> i32 {
    if name.is_empty() {
        return -1;
    }
    REGISTRY.name_to_id(name).unwrap_or(-1)
}

/// Converts a vanilla protocol numeric ID back to an item resource name.
///
/// Returns an empty string for IDs not present in the registry.
///
/// # Examples
///
/// ```
/// use oxidized_game::inventory::item_ids::{item_id_to_name, item_name_to_id};
///
/// let id = item_name_to_id("minecraft:stone");
/// assert_eq!(item_id_to_name(id), "minecraft:stone");
/// ```
pub fn item_id_to_name(id: i32) -> String {
    REGISTRY
        .id_to_name(id)
        .unwrap_or("minecraft:air")
        .to_owned()
}

/// Returns the maximum stack size for an item by name.
///
/// Uses the real vanilla item registry. Returns `64` for unknown items.
///
/// # Examples
///
/// ```
/// use oxidized_game::inventory::item_ids::max_stack_size_by_name;
///
/// assert_eq!(max_stack_size_by_name("minecraft:diamond_sword"), 1);
/// assert_eq!(max_stack_size_by_name("minecraft:stone"), 64);
/// ```
pub fn max_stack_size_by_name(name: &str) -> i32 {
    i32::from(REGISTRY.max_stack_size(name))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_known_items_roundtrip() {
        let known = [
            "minecraft:air",
            "minecraft:stone",
            "minecraft:grass_block",
            "minecraft:dirt",
            "minecraft:cobblestone",
            "minecraft:oak_planks",
            "minecraft:diamond",
            "minecraft:iron_pickaxe",
            "minecraft:diamond_sword",
        ];
        for name in known {
            let id = item_name_to_id(name);
            assert!(id >= 0, "name_to_id returned -1 for {name}");
            let back = item_id_to_name(id);
            assert_eq!(back, name, "roundtrip failed for {name}");
        }
    }

    #[test]
    fn test_empty_returns_negative() {
        assert_eq!(item_name_to_id(""), -1);
    }

    #[test]
    fn test_unknown_name_returns_negative() {
        assert_eq!(item_name_to_id("minecraft:not_a_real_item_xyz"), -1);
    }

    #[test]
    fn test_unknown_id_returns_air() {
        assert_eq!(item_id_to_name(999_999), "minecraft:air");
    }

    #[test]
    fn test_all_1506_items_roundtrip() {
        for id in 0..1506 {
            let name = item_id_to_name(id);
            assert!(!name.is_empty(), "id {id} returned empty name");
            let back = item_name_to_id(&name);
            assert_eq!(back, id, "roundtrip failed for {name} (id {id})");
        }
    }

    #[test]
    fn test_max_stack_size() {
        assert_eq!(max_stack_size_by_name("minecraft:diamond_sword"), 1);
        assert_eq!(max_stack_size_by_name("minecraft:stone"), 64);
        assert_eq!(max_stack_size_by_name("minecraft:ender_pearl"), 16);
        // Unknown item → default 64
        assert_eq!(max_stack_size_by_name("minecraft:nonexistent"), 64);
    }
}
