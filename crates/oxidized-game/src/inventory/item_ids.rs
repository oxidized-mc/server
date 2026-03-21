//! Placeholder item ID ↔ name mapping.
//!
//! A proper item registry will be built in Phase 22+. Until then, this module
//! provides a small hardcoded mapping for common items plus a deterministic
//! hash-based fallback for unknown items.
//!
//! Both the login sequence and the play-state inventory handlers share this
//! mapping to ensure consistency.

/// Converts an item resource name to a numeric registry ID.
///
/// Uses a hardcoded mapping for common items plus a hash-based fallback
/// for unknown items. The fallback is deterministic (same name → same ID)
/// but will not match vanilla registry IDs.
///
/// # Examples
///
/// ```
/// use oxidized_game::inventory::item_ids::item_name_to_id;
///
/// assert_eq!(item_name_to_id("minecraft:stone"), 1);
/// assert_eq!(item_name_to_id("minecraft:air"), 0);
/// assert_eq!(item_name_to_id(""), 0);
/// ```
// TODO(Phase 22+): Replace with proper item registry lookup.
pub fn item_name_to_id(name: &str) -> i32 {
    match name {
        "minecraft:air" | "" => 0,
        "minecraft:stone" => 1,
        "minecraft:grass_block" => 8,
        "minecraft:dirt" => 10,
        "minecraft:cobblestone" => 14,
        "minecraft:oak_planks" => 15,
        "minecraft:diamond" => 802,
        "minecraft:iron_pickaxe" => 813,
        "minecraft:diamond_sword" => 824,
        _ => {
            // Deterministic hash-based fallback for unknown items.
            let mut hash: i32 = 0;
            for b in name.bytes() {
                hash = hash.wrapping_mul(31).wrapping_add(b as i32);
            }
            hash.abs() % 2000 + 100
        },
    }
}

/// Converts a numeric registry ID back to an item resource name.
///
/// Only recognizes the hardcoded IDs from [`item_name_to_id`]; all others
/// return `"minecraft:unknown_{id}"`.
///
/// # Examples
///
/// ```
/// use oxidized_game::inventory::item_ids::item_id_to_name;
///
/// assert_eq!(item_id_to_name(1), "minecraft:stone");
/// assert_eq!(item_id_to_name(0), "minecraft:air");
/// ```
// TODO(Phase 22+): Replace with proper item registry lookup.
pub fn item_id_to_name(id: i32) -> String {
    match id {
        0 => "minecraft:air".to_string(),
        1 => "minecraft:stone".to_string(),
        8 => "minecraft:grass_block".to_string(),
        10 => "minecraft:dirt".to_string(),
        14 => "minecraft:cobblestone".to_string(),
        15 => "minecraft:oak_planks".to_string(),
        802 => "minecraft:diamond".to_string(),
        813 => "minecraft:iron_pickaxe".to_string(),
        824 => "minecraft:diamond_sword".to_string(),
        _ => format!("minecraft:unknown_{id}"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_known_items_roundtrip() {
        let known = [
            ("minecraft:air", 0),
            ("minecraft:stone", 1),
            ("minecraft:grass_block", 8),
            ("minecraft:dirt", 10),
            ("minecraft:cobblestone", 14),
            ("minecraft:oak_planks", 15),
            ("minecraft:diamond", 802),
            ("minecraft:iron_pickaxe", 813),
            ("minecraft:diamond_sword", 824),
        ];
        for (name, expected_id) in known {
            let id = item_name_to_id(name);
            assert_eq!(id, expected_id, "name_to_id failed for {name}");
            let back = item_id_to_name(id);
            assert_eq!(back, name, "id_to_name failed for {expected_id}");
        }
    }

    #[test]
    fn test_empty_maps_to_air() {
        assert_eq!(item_name_to_id(""), 0);
        assert_eq!(item_id_to_name(0), "minecraft:air");
    }

    #[test]
    fn test_unknown_name_gives_stable_id() {
        let id1 = item_name_to_id("minecraft:emerald");
        let id2 = item_name_to_id("minecraft:emerald");
        assert_eq!(id1, id2, "hash must be deterministic");
        assert!(id1 >= 100, "fallback IDs start at 100");
    }

    #[test]
    fn test_unknown_id_gives_placeholder_name() {
        let name = item_id_to_name(9999);
        assert_eq!(name, "minecraft:unknown_9999");
    }
}
