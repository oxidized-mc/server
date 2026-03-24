//! Block registry: O(1) lookup of block states and blocks backed by compile-time
//! generated static data.

use super::block::{BlockDef, BlockStateId};
use super::error::RegistryError;
use super::generated;

/// Registry of all block types and block states.
///
/// All data is generated at compile time in [`super::generated`].  This struct
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
}
