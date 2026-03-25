//! Biome registry: maps biome names ↔ protocol IDs.
//!
//! Vanilla assigns biome IDs alphabetically. This module provides the
//! canonical mapping used by the protocol, chunk serialization, and
//! world generation.

/// All 65 vanilla biome registry names, sorted alphabetically.
/// Index = protocol ID (data-driven registries assign IDs in alphabetical order).
static BIOME_NAMES: &[&str] = &[
    "minecraft:badlands",
    "minecraft:bamboo_jungle",
    "minecraft:basalt_deltas",
    "minecraft:beach",
    "minecraft:birch_forest",
    "minecraft:cherry_grove",
    "minecraft:cold_ocean",
    "minecraft:crimson_forest",
    "minecraft:dark_forest",
    "minecraft:deep_cold_ocean",
    "minecraft:deep_dark",
    "minecraft:deep_frozen_ocean",
    "minecraft:deep_lukewarm_ocean",
    "minecraft:deep_ocean",
    "minecraft:desert",
    "minecraft:dripstone_caves",
    "minecraft:end_barrens",
    "minecraft:end_highlands",
    "minecraft:end_midlands",
    "minecraft:eroded_badlands",
    "minecraft:flower_forest",
    "minecraft:forest",
    "minecraft:frozen_ocean",
    "minecraft:frozen_peaks",
    "minecraft:frozen_river",
    "minecraft:grove",
    "minecraft:ice_spikes",
    "minecraft:jagged_peaks",
    "minecraft:jungle",
    "minecraft:lukewarm_ocean",
    "minecraft:lush_caves",
    "minecraft:mangrove_swamp",
    "minecraft:meadow",
    "minecraft:mushroom_fields",
    "minecraft:nether_wastes",
    "minecraft:ocean",
    "minecraft:old_growth_birch_forest",
    "minecraft:old_growth_pine_taiga",
    "minecraft:old_growth_spruce_taiga",
    "minecraft:pale_garden",
    "minecraft:plains",
    "minecraft:river",
    "minecraft:savanna",
    "minecraft:savanna_plateau",
    "minecraft:small_end_islands",
    "minecraft:snowy_beach",
    "minecraft:snowy_plains",
    "minecraft:snowy_slopes",
    "minecraft:snowy_taiga",
    "minecraft:soul_sand_valley",
    "minecraft:sparse_jungle",
    "minecraft:stony_peaks",
    "minecraft:stony_shore",
    "minecraft:sunflower_plains",
    "minecraft:swamp",
    "minecraft:taiga",
    "minecraft:the_end",
    "minecraft:the_void",
    "minecraft:warm_ocean",
    "minecraft:warped_forest",
    "minecraft:windswept_forest",
    "minecraft:windswept_gravelly_hills",
    "minecraft:windswept_hills",
    "minecraft:windswept_savanna",
    "minecraft:wooded_badlands",
];

/// Protocol ID for `minecraft:plains` (alphabetical index 40).
pub const PLAINS_BIOME_ID: u32 = 40;

/// Returns the protocol ID for a biome resource name, or `None` if unknown.
///
/// Uses binary search on the alphabetically-sorted biome list for O(log n)
/// lookup.
///
/// # Examples
///
/// ```
/// use oxidized_world::registry::biome_name_to_id;
///
/// assert_eq!(biome_name_to_id("minecraft:plains"), Some(40));
/// assert_eq!(biome_name_to_id("minecraft:desert"), Some(14));
/// assert_eq!(biome_name_to_id("minecraft:nonexistent"), None);
/// ```
pub fn biome_name_to_id(name: &str) -> Option<u32> {
    BIOME_NAMES
        .binary_search(&name)
        .ok()
        .map(|idx| idx as u32)
}

/// Returns the biome resource name for a protocol ID, or `None` if out of range.
///
/// # Examples
///
/// ```
/// use oxidized_world::registry::biome_id_to_name;
///
/// assert_eq!(biome_id_to_name(40), Some("minecraft:plains"));
/// assert_eq!(biome_id_to_name(999), None);
/// ```
pub fn biome_id_to_name(id: u32) -> Option<&'static str> {
    BIOME_NAMES.get(id as usize).copied()
}

/// Returns the total number of registered biomes (65 in vanilla 26.1).
#[must_use]
pub fn biome_count() -> usize {
    BIOME_NAMES.len()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_plains_biome_id_constant() {
        assert_eq!(biome_name_to_id("minecraft:plains"), Some(PLAINS_BIOME_ID));
    }

    #[test]
    fn test_biome_name_to_id_known_biomes() {
        assert_eq!(biome_name_to_id("minecraft:plains"), Some(40));
        assert_eq!(biome_name_to_id("minecraft:desert"), Some(14));
        assert_eq!(biome_name_to_id("minecraft:the_void"), Some(57));
        assert_eq!(biome_name_to_id("minecraft:snowy_plains"), Some(46));
        assert_eq!(biome_name_to_id("minecraft:mushroom_fields"), Some(33));
        assert_eq!(biome_name_to_id("minecraft:badlands"), Some(0));
        assert_eq!(biome_name_to_id("minecraft:wooded_badlands"), Some(64));
    }

    #[test]
    fn test_biome_name_to_id_unknown() {
        assert_eq!(biome_name_to_id("minecraft:nonexistent"), None);
        assert_eq!(biome_name_to_id(""), None);
        assert_eq!(biome_name_to_id("plains"), None);
    }

    #[test]
    fn test_biome_id_to_name_roundtrip() {
        for id in 0..biome_count() as u32 {
            let name = biome_id_to_name(id).unwrap();
            assert_eq!(biome_name_to_id(name), Some(id));
        }
    }

    #[test]
    fn test_biome_id_to_name_out_of_range() {
        assert_eq!(biome_id_to_name(65), None);
        assert_eq!(biome_id_to_name(999), None);
    }

    #[test]
    fn test_biome_count() {
        assert_eq!(biome_count(), 65);
    }

    #[test]
    fn test_biome_names_sorted() {
        for window in BIOME_NAMES.windows(2) {
            assert!(
                window[0] < window[1],
                "BIOME_NAMES not sorted: {:?} >= {:?}",
                window[0],
                window[1]
            );
        }
    }

    #[test]
    fn test_all_64_vanilla_biomes_resolve() {
        // Verify all 65 biomes have valid roundtrip
        assert_eq!(biome_name_to_id("minecraft:badlands"), Some(0));
        assert_eq!(biome_name_to_id("minecraft:bamboo_jungle"), Some(1));
        assert_eq!(biome_name_to_id("minecraft:cherry_grove"), Some(5));
        assert_eq!(biome_name_to_id("minecraft:deep_ocean"), Some(13));
        assert_eq!(biome_name_to_id("minecraft:forest"), Some(21));
        assert_eq!(biome_name_to_id("minecraft:jungle"), Some(28));
        assert_eq!(biome_name_to_id("minecraft:ocean"), Some(35));
        assert_eq!(biome_name_to_id("minecraft:pale_garden"), Some(39));
        assert_eq!(biome_name_to_id("minecraft:savanna"), Some(42));
        assert_eq!(biome_name_to_id("minecraft:taiga"), Some(55));
        assert_eq!(biome_name_to_id("minecraft:the_end"), Some(56));
        assert_eq!(biome_name_to_id("minecraft:warm_ocean"), Some(58));
    }
}
