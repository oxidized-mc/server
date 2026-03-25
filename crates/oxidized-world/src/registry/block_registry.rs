//! Block registry: O(1) lookup of block states and blocks backed by compile-time
//! generated static data.

use super::block::{BlockDef, BlockStateId};
use super::error::RegistryError;
use super::generated;

/// Registry of all block types and block states.
///
/// All data is generated at compile time in the `generated` module.  This struct
/// is zero-sized and acts as a convenient handle with the same API surface that
/// consumers already use.  Passing `Arc<BlockRegistry>` around is cheap (it
/// holds no data) and avoids changing call-sites.
pub struct BlockRegistry;

impl BlockRegistry {
    /// Create a new block registry.
    ///
    /// This is a no-op — all data is static.
    pub fn new() -> Self {
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

    /// Returns the block definition for the given name, using binary search.
    pub fn get_block_def(&self, name: &str) -> Option<&'static BlockDef> {
        lookup_by_name(name)
    }

    /// Returns the default state ID for a block by its registry name.
    pub fn default_state(&self, name: &str) -> Option<BlockStateId> {
        lookup_by_name(name).map(|b| BlockStateId(b.default_state))
    }

    /// Returns the block name for a given state ID.
    pub fn block_name_from_state_id(&self, state_id: u32) -> Option<&'static str> {
        let entry = generated::BLOCK_STATE_DATA.get(state_id as usize)?;
        let block = generated::BLOCK_DEFS.get(entry.block_type as usize)?;
        Some(block.name)
    }

    /// Total number of block types in the registry.
    pub fn block_count(&self) -> usize {
        generated::BLOCK_COUNT
    }

    /// Total number of block states in the registry.
    pub fn state_count(&self) -> usize {
        generated::STATE_COUNT
    }

    /// Length of the internal state array.
    ///
    /// Use this to allocate dense arrays indexed by state ID.
    pub fn state_array_size(&self) -> usize {
        generated::STATE_COUNT
    }

    /// Gets a block definition by its type index.
    pub fn get_block_def_by_index(&self, index: u16) -> Option<&'static BlockDef> {
        generated::BLOCK_DEFS.get(index as usize)
    }

    /// Computes property key-value pairs for a state (on the fly via strides).
    pub fn state_properties(&self, id: BlockStateId) -> Vec<(&'static str, &'static str)> {
        id.properties()
    }

    /// Finds a state by block name and property key-value pairs.
    ///
    /// Returns the default state if properties is empty or the block has no
    /// properties.  Returns `None` if the block name is unknown or no state
    /// matches all the given properties.
    pub fn find_state(&self, name: &str, properties: &[(&str, &str)]) -> Option<BlockStateId> {
        let def = lookup_by_name(name)?;
        if properties.is_empty() || def.prop_count == 0 {
            return Some(BlockStateId(def.default_state));
        }
        // Start from default and apply each property
        let mut state = BlockStateId(def.default_state);
        for &(key, value) in properties {
            state = state.with_property(key, value)?;
        }
        Some(state)
    }
}

impl Default for BlockRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Binary search the sorted name table for a block definition.
fn lookup_by_name(name: &str) -> Option<&'static BlockDef> {
    let idx = generated::BLOCK_NAMES_SORTED
        .binary_search_by_key(&name, |&(n, _)| n)
        .ok()?;
    let type_idx = generated::BLOCK_NAMES_SORTED[idx].1;
    generated::BLOCK_DEFS.get(type_idx as usize)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::registry::generated;

    fn registry() -> BlockRegistry {
        BlockRegistry::load().expect("failed to load block registry")
    }

    #[test]
    fn test_load_blocks() {
        let _ = registry();
    }

    #[test]
    fn test_block_count() {
        let reg = registry();
        assert_eq!(reg.block_count(), 1168);
    }

    #[test]
    fn test_state_count() {
        let reg = registry();
        assert_eq!(reg.state_count(), 29873);
    }

    #[test]
    fn test_air_is_state_zero() {
        let id = BlockStateId(0);
        assert_eq!(id.block_name(), "minecraft:air");
        assert!(id.is_default());
        assert!(id.is_air());
    }

    #[test]
    fn test_stone_is_state_one() {
        let id = BlockStateId(1);
        assert_eq!(id.block_name(), "minecraft:stone");
    }

    #[test]
    fn test_get_block_by_name() {
        let reg = registry();
        let def = reg
            .get_block_def("minecraft:grass_block")
            .expect("grass_block missing");
        assert_eq!(def.name, "minecraft:grass_block");
        assert!(def.prop_count > 0);
    }

    #[test]
    fn test_default_state() {
        let reg = registry();
        let default = reg
            .default_state("minecraft:grass_block")
            .expect("grass_block default missing");
        assert!(default.is_default());
    }

    #[test]
    fn test_grass_block_has_snowy_property() {
        let def = BlockRegistry
            .get_block_def("minecraft:grass_block")
            .expect("grass_block missing");
        let props_start = def.props_offset as usize;
        let props_end = props_start + def.prop_count as usize;
        let props = &generated::PROPERTY_DEFS[props_start..props_end];
        let snowy = props
            .iter()
            .find(|p| p.name == "snowy")
            .expect("snowy missing");
        let vals_start = snowy.values_offset as usize;
        let vals = &generated::PROPERTY_VALUES[vals_start..vals_start + snowy.num_values as usize];
        assert!(vals.contains(&"false"));
        assert!(vals.contains(&"true"));
    }

    #[test]
    fn test_oak_log_states() {
        let def = BlockRegistry
            .get_block_def("minecraft:oak_log")
            .expect("oak_log missing");
        assert_eq!(def.state_count, 3);
    }

    #[test]
    fn test_unknown_block_returns_none() {
        let reg = registry();
        assert!(reg.get_block_def("minecraft:not_a_block").is_none());
    }

    #[test]
    fn test_state_roundtrip() {
        for (i, entry) in generated::BLOCK_STATE_DATA.iter().enumerate() {
            let def = &generated::BLOCK_DEFS[entry.block_type as usize];
            let first = def.first_state as usize;
            let last = first + def.state_count as usize;
            assert!(
                i >= first && i < last,
                "state {i} claims block_type={} ({}) but is outside range {}..{}",
                entry.block_type,
                def.name,
                first,
                last,
            );
        }
    }

    #[test]
    fn test_all_blocks_have_default_state() {
        for def in &generated::BLOCK_DEFS {
            let id = BlockStateId(def.default_state);
            assert!(
                id.is_default(),
                "block {} default state {} not marked as default",
                def.name,
                def.default_state,
            );
        }
    }

    #[test]
    fn test_state_properties_roundtrip() {
        let reg = registry();
        // Verify a known block's property values
        let def = reg
            .get_block_def("minecraft:oak_stairs")
            .expect("oak_stairs missing");
        let default_id = BlockStateId(def.default_state);
        let props = default_id.properties();
        assert!(!props.is_empty(), "oak_stairs should have properties");
        // Should have facing, half, shape, waterlogged
        let keys: Vec<&str> = props.iter().map(|&(k, _)| k).collect();
        assert!(keys.contains(&"facing"));
        assert!(keys.contains(&"half"));
        assert!(keys.contains(&"shape"));
        assert!(keys.contains(&"waterlogged"));
    }

    #[test]
    fn test_with_property() {
        let def = BlockRegistry
            .get_block_def("minecraft:oak_stairs")
            .expect("oak_stairs missing");
        let default_id = BlockStateId(def.default_state);

        // Change facing to south
        let south = default_id
            .with_property("facing", "south")
            .expect("failed to set facing=south");
        let south_props = south.properties();
        let facing = south_props
            .iter()
            .find(|&&(k, _)| k == "facing")
            .expect("facing missing");
        assert_eq!(facing.1, "south");

        // Verify other properties unchanged
        let default_props = default_id.properties();
        for &(key, val) in &south_props {
            if key != "facing" {
                let original = default_props
                    .iter()
                    .find(|&&(k, _)| k == key)
                    .expect("property missing");
                assert_eq!(val, original.1, "property {key} changed unexpectedly");
            }
        }
    }

    #[test]
    fn test_with_property_invalid() {
        let id = BlockStateId(1); // stone, no properties
        assert!(id.with_property("facing", "north").is_none());
    }

    #[test]
    fn test_find_state() {
        let reg = registry();
        let state = reg
            .find_state(
                "minecraft:oak_stairs",
                &[("facing", "south"), ("half", "bottom")],
            )
            .expect("find_state failed");
        let props = state.properties();
        assert_eq!(
            props.iter().find(|&&(k, _)| k == "facing").unwrap().1,
            "south"
        );
        assert_eq!(
            props.iter().find(|&&(k, _)| k == "half").unwrap().1,
            "bottom"
        );
    }

    #[test]
    fn test_block_name_from_state_id() {
        let reg = registry();
        assert_eq!(reg.block_name_from_state_id(0), Some("minecraft:air"));
        assert_eq!(reg.block_name_from_state_id(1), Some("minecraft:stone"));
    }

    // ── R5.1: BlockStateFlags enrichment verification ──────────────────

    #[test]
    fn test_air_flags() {
        let air = BlockStateId(0);
        assert!(air.is_air(), "air should be IS_AIR");
        assert!(!air.has_collision(), "air should not have collision");
        assert!(!air.is_solid(), "air should not be solid");
        assert!(air.is_replaceable(), "air should be replaceable");

        // cave_air and void_air
        let cave_air = BlockRegistry
            .default_state("minecraft:cave_air")
            .expect("cave_air missing");
        assert!(cave_air.is_air());
        let void_air = BlockRegistry
            .default_state("minecraft:void_air")
            .expect("void_air missing");
        assert!(void_air.is_air());
    }

    #[test]
    fn test_liquid_flags() {
        let water = BlockRegistry
            .default_state("minecraft:water")
            .expect("water missing");
        assert!(water.is_liquid(), "water should be IS_LIQUID");
        assert!(water.is_replaceable(), "water should be replaceable");
        assert!(!water.has_collision(), "water should not have collision");

        let lava = BlockRegistry
            .default_state("minecraft:lava")
            .expect("lava missing");
        assert!(lava.is_liquid(), "lava should be IS_LIQUID");
    }

    #[test]
    fn test_stone_flags() {
        let stone = BlockStateId(1);
        assert!(stone.is_solid(), "stone should be solid");
        assert!(stone.has_collision(), "stone should have collision");
        assert!(stone.is_opaque(), "stone should be opaque");
        assert!(stone.requires_tool(), "stone should require tool");
        assert!(!stone.is_air(), "stone should not be air");
        assert!(!stone.is_liquid(), "stone should not be liquid");
        assert!(!stone.is_replaceable(), "stone should not be replaceable");
        assert!(!stone.is_interactable(), "stone should not be interactable");
    }

    #[test]
    fn test_interactable_blocks() {
        let interactable = [
            "minecraft:crafting_table",
            "minecraft:chest",
            "minecraft:furnace",
            "minecraft:anvil",
            "minecraft:enchanting_table",
            "minecraft:barrel",
            "minecraft:brewing_stand",
            "minecraft:hopper",
            "minecraft:lever",
            "minecraft:oak_door",
            "minecraft:oak_button",
            "minecraft:oak_fence_gate",
        ];
        for name in &interactable {
            let state = BlockRegistry
                .default_state(name)
                .unwrap_or_else(|| panic!("{name} missing"));
            assert!(state.is_interactable(), "{name} should be interactable");
        }
    }

    #[test]
    fn test_non_interactable_blocks() {
        let non_interactable = [
            "minecraft:stone",
            "minecraft:dirt",
            "minecraft:grass_block",
            "minecraft:oak_planks",
            "minecraft:cobblestone",
            "minecraft:oak_log",
        ];
        for name in &non_interactable {
            let state = BlockRegistry
                .default_state(name)
                .unwrap_or_else(|| panic!("{name} missing"));
            assert!(
                !state.is_interactable(),
                "{name} should not be interactable"
            );
        }
    }

    #[test]
    fn test_block_entity_blocks() {
        let with_be = [
            "minecraft:chest",
            "minecraft:furnace",
            "minecraft:hopper",
            "minecraft:beacon",
            "minecraft:brewing_stand",
            "minecraft:spawner",
        ];
        for name in &with_be {
            let state = BlockRegistry
                .default_state(name)
                .unwrap_or_else(|| panic!("{name} missing"));
            assert!(state.has_block_entity(), "{name} should have block entity");
        }
    }

    #[test]
    fn test_replaceable_matches_known_set() {
        // Verify that all blocks from the old is_replaceable_block() in
        // placement.rs are marked replaceable via the new flag.
        let expected_replaceable = [
            "minecraft:air",
            "minecraft:cave_air",
            "minecraft:void_air",
            "minecraft:water",
            "minecraft:lava",
            "minecraft:short_grass",
            "minecraft:tall_grass",
            "minecraft:seagrass",
            "minecraft:tall_seagrass",
            "minecraft:fire",
            "minecraft:soul_fire",
            "minecraft:snow",
            "minecraft:vine",
            "minecraft:dead_bush",
            "minecraft:fern",
            "minecraft:large_fern",
            "minecraft:structure_void",
            "minecraft:light",
            "minecraft:crimson_roots",
            "minecraft:warped_roots",
            "minecraft:nether_sprouts",
            "minecraft:hanging_roots",
            "minecraft:glow_lichen",
        ];
        for name in &expected_replaceable {
            let state = BlockRegistry
                .default_state(name)
                .unwrap_or_else(|| panic!("{name} missing"));
            assert!(state.is_replaceable(), "{name} should be replaceable");
        }
    }

    #[test]
    fn test_flammable_blocks() {
        let flammable = [
            "minecraft:oak_planks",
            "minecraft:oak_log",
            "minecraft:oak_leaves",
        ];
        for name in &flammable {
            let state = BlockRegistry
                .default_state(name)
                .unwrap_or_else(|| panic!("{name} missing"));
            assert!(state.is_flammable(), "{name} should be flammable");
        }

        // Stone and iron should not be flammable
        let stone = BlockStateId(1);
        assert!(!stone.is_flammable(), "stone should not be flammable");
    }

    #[test]
    fn test_random_ticking_blocks() {
        let ticking = ["minecraft:grass_block", "minecraft:ice"];
        for name in &ticking {
            let state = BlockRegistry
                .default_state(name)
                .unwrap_or_else(|| panic!("{name} missing"));
            assert!(state.ticks_randomly(), "{name} should tick randomly");
        }

        // Stone should not tick randomly
        assert!(!BlockStateId(1).ticks_randomly());
    }

    #[test]
    fn test_glass_transparency() {
        let glass = BlockRegistry
            .default_state("minecraft:glass")
            .expect("glass missing");
        assert!(!glass.is_opaque(), "glass should not be opaque");
        assert!(glass.has_collision(), "glass should have collision");
    }

    // --- R5.2 property value spot-checks ---

    /// Helper: assert a float is within epsilon of the expected value.
    fn assert_approx(actual: f64, expected: f64, label: &str) {
        let eps = 0.011; // fixed-point ×100 gives ±0.01 precision
        assert!(
            (actual - expected).abs() < eps,
            "{label}: expected {expected}, got {actual}"
        );
    }

    /// Helper: assert a float is within epsilon for ×10000 encoded values.
    fn assert_approx_fine(actual: f64, expected: f64, label: &str) {
        let eps = 0.0002; // fixed-point ×10000 gives ±0.0001 precision
        assert!(
            (actual - expected).abs() < eps,
            "{label}: expected {expected}, got {actual}"
        );
    }

    fn default(name: &str) -> BlockStateId {
        BlockRegistry
            .default_state(name)
            .unwrap_or_else(|| panic!("{name} missing from registry"))
    }

    #[test]
    fn test_stone_properties() {
        let s = default("minecraft:stone");
        assert_approx(s.hardness(), 1.5, "stone hardness");
        assert_approx(s.explosion_resistance(), 6.0, "stone resistance");
        assert_approx_fine(s.friction(), 0.6, "stone friction");
        assert_approx_fine(s.speed_factor(), 1.0, "stone speed_factor");
        assert_approx_fine(s.jump_factor(), 1.0, "stone jump_factor");
        assert_eq!(s.light_emission(), 0);
    }

    #[test]
    fn test_ice_properties() {
        let s = default("minecraft:ice");
        assert_approx(s.hardness(), 0.5, "ice hardness");
        assert_approx_fine(s.friction(), 0.98, "ice friction");
        assert_approx_fine(s.speed_factor(), 1.0, "ice speed_factor");
    }

    #[test]
    fn test_blue_ice_properties() {
        let s = default("minecraft:blue_ice");
        assert_approx(s.hardness(), 2.8, "blue_ice hardness");
        assert_approx_fine(s.friction(), 0.989, "blue_ice friction");
    }

    #[test]
    fn test_soul_sand_properties() {
        let s = default("minecraft:soul_sand");
        assert_approx_fine(s.speed_factor(), 0.4, "soul_sand speed_factor");
        assert_approx_fine(s.jump_factor(), 1.0, "soul_sand jump_factor");
    }

    #[test]
    fn test_honey_block_properties() {
        let s = default("minecraft:honey_block");
        assert_approx_fine(s.speed_factor(), 0.4, "honey speed_factor");
        assert_approx_fine(s.jump_factor(), 0.5, "honey jump_factor");
    }

    #[test]
    fn test_glowstone_properties() {
        let s = default("minecraft:glowstone");
        assert_eq!(s.light_emission(), 15, "glowstone light_emission");
        assert_approx(s.hardness(), 0.3, "glowstone hardness");
    }

    #[test]
    fn test_obsidian_properties() {
        let s = default("minecraft:obsidian");
        assert_approx(s.hardness(), 50.0, "obsidian hardness");
        // Explosion resistance 1200.0 exceeds u16×100 range (max 655.35), clamped.
        assert_approx(
            s.explosion_resistance(),
            655.35,
            "obsidian resistance (clamped)",
        );
    }

    #[test]
    fn test_bedrock_unbreakable() {
        let s = default("minecraft:bedrock");
        assert_eq!(s.hardness(), -1.0, "bedrock hardness should be -1.0");
    }

    #[test]
    fn test_torch_light_emission() {
        let s = default("minecraft:torch");
        assert_eq!(s.light_emission(), 14, "torch light_emission");
    }

    #[test]
    fn test_redstone_torch_light() {
        let s = default("minecraft:redstone_torch");
        assert_eq!(s.light_emission(), 7, "redstone_torch light_emission");
    }

    #[test]
    fn test_slime_block_properties() {
        let s = default("minecraft:slime_block");
        assert_approx_fine(s.friction(), 0.8, "slime friction");
        assert_approx(s.hardness(), 0.0, "slime hardness");
    }

    #[test]
    fn test_powder_snow_properties() {
        let s = default("minecraft:powder_snow");
        // Powder snow's speed reduction comes from PowderSnowBlock behavior at
        // runtime, NOT the block property speedFactor (which is default 1.0).
        assert_approx_fine(s.speed_factor(), 1.0, "powder_snow speed_factor");
        assert_approx(s.hardness(), 0.25, "powder_snow hardness");
    }

    #[test]
    fn test_packed_ice_properties() {
        let s = default("minecraft:packed_ice");
        assert_approx_fine(s.friction(), 0.98, "packed_ice friction");
        assert_approx(s.hardness(), 0.5, "packed_ice hardness");
    }

    #[test]
    fn test_dirt_properties() {
        let s = default("minecraft:dirt");
        assert_approx(s.hardness(), 0.5, "dirt hardness");
        assert_approx(s.explosion_resistance(), 0.5, "dirt resistance");
        assert_approx_fine(s.friction(), 0.6, "dirt friction");
    }

    #[test]
    fn test_oak_planks_properties() {
        let s = default("minecraft:oak_planks");
        assert_approx(s.hardness(), 2.0, "oak_planks hardness");
        assert_approx(s.explosion_resistance(), 3.0, "oak_planks resistance");
    }

    #[test]
    fn test_iron_block_properties() {
        let s = default("minecraft:iron_block");
        assert_approx(s.hardness(), 5.0, "iron_block hardness");
        assert_approx(s.explosion_resistance(), 6.0, "iron_block resistance");
    }

    #[test]
    fn test_diamond_block_properties() {
        let s = default("minecraft:diamond_block");
        assert_approx(s.hardness(), 5.0, "diamond_block hardness");
        assert_approx(s.explosion_resistance(), 6.0, "diamond_block resistance");
    }

    #[test]
    fn test_sea_lantern_light() {
        let s = default("minecraft:sea_lantern");
        assert_eq!(s.light_emission(), 15, "sea_lantern light_emission");
    }

    #[test]
    fn test_water_properties() {
        let s = default("minecraft:water");
        // Light opacity heuristic: liquids → 1
        assert_eq!(s.light_opacity(), 1, "water light_opacity");
        assert!(s.is_liquid());
        assert!(s.is_replaceable());
        assert_eq!(s.map_color(), 12, "water map_color=WATER");
    }

    #[test]
    fn test_cobweb_properties() {
        let s = default("minecraft:cobweb");
        assert_approx(s.hardness(), 4.0, "cobweb hardness");
        assert!(!s.has_collision(), "cobweb should not have collision");
        // Cobweb has forceSolidOn + noCollision → solid but no collision
        assert_eq!(s.push_reaction(), 1, "cobweb push=DESTROY");
    }

    #[test]
    fn test_push_reaction_values() {
        // Moving piston cannot be pushed
        assert_eq!(
            default("minecraft:moving_piston").push_reaction(),
            2,
            "moving_piston push=BLOCK"
        );
        // Torch is destroyed when pushed
        assert_eq!(
            default("minecraft:torch").push_reaction(),
            1,
            "torch push=DESTROY"
        );
        // Stone is pushable normally
        assert_eq!(
            default("minecraft:stone").push_reaction(),
            0,
            "stone push=NORMAL"
        );
        // Obsidian uses default NORMAL (piston code prevents pushing via hardness)
        assert_eq!(
            default("minecraft:obsidian").push_reaction(),
            0,
            "obsidian push=NORMAL"
        );
    }

    #[test]
    fn test_fixed_point_roundtrip_precision() {
        // Verify fixed-point encoding preserves values within acceptable error.
        // ×100 fields: hardness, explosion_resistance — precision ±0.01
        // ×10000 fields: friction, speed_factor, jump_factor — precision ±0.0001
        let cases = [
            ("minecraft:stone", 1.5_f64, 6.0, 0.6, 1.0, 1.0),
            ("minecraft:ice", 0.5, 0.5, 0.98, 1.0, 1.0),
            ("minecraft:blue_ice", 2.8, 2.8, 0.989, 1.0, 1.0),
            ("minecraft:soul_sand", 0.5, 0.5, 0.6, 0.4, 1.0),
            ("minecraft:honey_block", 0.0, 0.0, 0.6, 0.4, 0.5),
        ];
        for (name, hard, resist, fric, speed, jump) in &cases {
            let s = default(name);
            assert_approx(s.hardness(), *hard, &format!("{name} hardness roundtrip"));
            assert_approx(
                s.explosion_resistance(),
                *resist,
                &format!("{name} resistance roundtrip"),
            );
            assert_approx_fine(s.friction(), *fric, &format!("{name} friction roundtrip"));
            assert_approx_fine(s.speed_factor(), *speed, &format!("{name} speed roundtrip"));
            assert_approx_fine(s.jump_factor(), *jump, &format!("{name} jump roundtrip"));
        }
    }

    #[test]
    fn test_map_color_values() {
        assert_eq!(default("minecraft:stone").map_color(), 11, "stone=STONE");
        assert_eq!(
            default("minecraft:grass_block").map_color(),
            1,
            "grass=GRASS"
        );
        assert_eq!(default("minecraft:dirt").map_color(), 10, "dirt=DIRT");
        assert_eq!(
            default("minecraft:oak_planks").map_color(),
            13,
            "oak_planks=WOOD"
        );
        assert_eq!(default("minecraft:water").map_color(), 12, "water=WATER");
        assert_eq!(
            default("minecraft:obsidian").map_color(),
            29,
            "obsidian=COLOR_BLACK"
        );
        assert_eq!(
            default("minecraft:white_wool").map_color(),
            8,
            "white_wool=SNOW"
        );
        assert_eq!(
            default("minecraft:gold_block").map_color(),
            30,
            "gold_block=GOLD"
        );
    }

    #[test]
    fn test_light_opacity_heuristic() {
        // Full opaque solid blocks → 15
        assert_eq!(
            default("minecraft:stone").light_opacity(),
            15,
            "stone=opaque"
        );
        assert_eq!(default("minecraft:dirt").light_opacity(), 15, "dirt=opaque");
        // Transparent blocks → 0
        assert_eq!(
            default("minecraft:glass").light_opacity(),
            0,
            "glass=transparent"
        );
        assert_eq!(
            default("minecraft:air").light_opacity(),
            0,
            "air=transparent"
        );
        // Liquids → 1
        assert_eq!(
            default("minecraft:water").light_opacity(),
            1,
            "water=liquid"
        );
        assert_eq!(default("minecraft:lava").light_opacity(), 1, "lava=liquid");
    }
}
