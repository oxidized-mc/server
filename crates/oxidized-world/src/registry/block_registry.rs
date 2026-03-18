//! Block registry: O(1) lookup of block states by numeric ID and blocks by name.

use std::io::Read;

use ahash::AHashMap;
use flate2::read::GzDecoder;

use super::block::{Block, BlockProperty, BlockState, BlockStateId};
use super::error::RegistryError;

/// The compressed `blocks.json` data, embedded at compile time.
const BLOCKS_DATA_GZ: &[u8] = include_bytes!("../data/blocks.json.gz");

/// Registry of all block types and block states.
///
/// Provides O(1) lookup of block states by [`BlockStateId`] and block
/// definitions by name.
pub struct BlockRegistry {
    /// O(1) lookup: `state_id` → [`BlockState`]. Indexed by `state_id` as `usize`.
    states: Vec<Option<BlockState>>,
    /// All blocks in registration order.
    blocks: Vec<Block>,
    /// Name → block index lookup.
    by_name: AHashMap<String, u16>,
}

impl BlockRegistry {
    /// Load the block registry from the embedded compressed JSON data.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::Decompress`] if decompression fails, or
    /// [`RegistryError::Json`] if the JSON is malformed.
    pub fn load() -> Result<Self, RegistryError> {
        let mut decoder = GzDecoder::new(BLOCKS_DATA_GZ);
        let mut json_str = String::new();
        decoder.read_to_string(&mut json_str)?;

        let root: serde_json::Value = serde_json::from_str(&json_str)?;
        let empty_map = serde_json::Map::new();
        let obj = root.as_object().unwrap_or(&empty_map);

        // First pass: determine max state ID for pre-allocation.
        let mut max_id: u16 = 0;
        for block_value in obj.values() {
            if let Some(states_arr) = block_value.get("states").and_then(|s| s.as_array()) {
                for state_val in states_arr {
                    if let Some(id) = state_val.get("id").and_then(|v| v.as_u64()) {
                        let id16 = id as u16;
                        if id16 > max_id {
                            max_id = id16;
                        }
                    }
                }
            }
        }

        let mut states: Vec<Option<BlockState>> = vec![None; (max_id as usize) + 1];
        let mut blocks: Vec<Block> = Vec::with_capacity(obj.len());
        let mut by_name: AHashMap<String, u16> = AHashMap::with_capacity(obj.len());

        for (block_name, block_value) in obj {
            let block_index = blocks.len() as u16;

            // Parse properties.
            let properties = parse_properties(block_value);

            // Parse states.
            let mut block_state_ids = Vec::new();
            let mut default_state = BlockStateId(0);

            if let Some(states_arr) = block_value.get("states").and_then(|s| s.as_array()) {
                for state_val in states_arr {
                    let id = state_val.get("id").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
                    let is_default = state_val
                        .get("default")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    let state_props = parse_state_properties(state_val);

                    let state_id = BlockStateId(id);
                    block_state_ids.push(state_id);

                    if is_default {
                        default_state = state_id;
                    }

                    let block_state = BlockState {
                        id: state_id,
                        block_index,
                        is_default,
                        properties: state_props,
                    };

                    if (id as usize) < states.len() {
                        states[id as usize] = Some(block_state);
                    }
                }
            }

            by_name.insert(block_name.clone(), block_index);

            blocks.push(Block {
                name: block_name.clone(),
                index: block_index,
                properties,
                default_state,
                states: block_state_ids,
            });
        }

        // Count actual populated states.
        Ok(Self {
            states,
            blocks,
            by_name,
        })
    }

    /// Get a block state by its flat numeric ID.
    pub fn get_state(&self, id: BlockStateId) -> Option<&BlockState> {
        self.states.get(id.0 as usize).and_then(|s| s.as_ref())
    }

    /// Get a block definition by its registry name (e.g., `"minecraft:stone"`).
    pub fn get_block(&self, name: &str) -> Option<&Block> {
        self.by_name
            .get(name)
            .map(|&idx| &self.blocks[idx as usize])
    }

    /// Get the default state ID for a block by its registry name.
    pub fn default_state(&self, name: &str) -> Option<BlockStateId> {
        self.get_block(name).map(|b| b.default_state)
    }

    /// Total number of block types in the registry.
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Total number of block states in the registry.
    pub fn state_count(&self) -> usize {
        self.states.iter().filter(|s| s.is_some()).count()
    }
}

/// Parse property definitions from a block JSON value.
fn parse_properties(block_value: &serde_json::Value) -> Vec<BlockProperty> {
    let Some(props_obj) = block_value.get("properties").and_then(|p| p.as_object()) else {
        return Vec::new();
    };

    props_obj
        .iter()
        .map(|(name, values)| {
            let vals = values
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            BlockProperty {
                name: name.clone(),
                values: vals,
            }
        })
        .collect()
}

/// Parse property values from a state JSON value.
fn parse_state_properties(state_val: &serde_json::Value) -> Vec<(String, String)> {
    let Some(props_obj) = state_val.get("properties").and_then(|p| p.as_object()) else {
        return Vec::new();
    };

    props_obj
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_owned()))
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

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
        let reg = registry();
        let state = reg.get_state(BlockStateId(0)).expect("state 0 missing");
        let block = &reg.blocks[state.block_index as usize];
        assert_eq!(block.name, "minecraft:air");
        assert!(state.is_default);
    }

    #[test]
    fn test_stone_is_state_one() {
        let reg = registry();
        let state = reg.get_state(BlockStateId(1)).expect("state 1 missing");
        let block = &reg.blocks[state.block_index as usize];
        assert_eq!(block.name, "minecraft:stone");
    }

    #[test]
    fn test_get_block_by_name() {
        let reg = registry();
        let block = reg
            .get_block("minecraft:grass_block")
            .expect("grass_block missing");
        assert_eq!(block.name, "minecraft:grass_block");
        assert!(!block.properties.is_empty());
    }

    #[test]
    fn test_default_state() {
        let reg = registry();
        let default = reg
            .default_state("minecraft:grass_block")
            .expect("grass_block default missing");
        let state = reg.get_state(default).expect("default state missing");
        assert!(state.is_default);
    }

    #[test]
    fn test_grass_block_has_snowy_property() {
        let reg = registry();
        let block = reg
            .get_block("minecraft:grass_block")
            .expect("grass_block missing");
        let snowy = block
            .properties
            .iter()
            .find(|p| p.name == "snowy")
            .expect("snowy property missing");
        assert!(snowy.values.contains(&"false".to_owned()));
        assert!(snowy.values.contains(&"true".to_owned()));
    }

    #[test]
    fn test_oak_log_states() {
        let reg = registry();
        let block = reg.get_block("minecraft:oak_log").expect("oak_log missing");
        // oak_log has "axis" property with values ["x", "y", "z"] = 3 states
        assert_eq!(block.states.len(), 3);
        let axis = block
            .properties
            .iter()
            .find(|p| p.name == "axis")
            .expect("axis property missing");
        assert_eq!(axis.values.len(), 3);
    }

    #[test]
    fn test_unknown_block_returns_none() {
        let reg = registry();
        assert!(reg.get_block("minecraft:not_a_block").is_none());
    }

    #[test]
    fn test_state_roundtrip() {
        let reg = registry();
        for (i, slot) in reg.states.iter().enumerate() {
            if let Some(state) = slot {
                assert_eq!(
                    state.id.0 as usize, i,
                    "state at index {i} has mismatched id {}",
                    state.id.0
                );
            }
        }
    }

    #[test]
    fn test_all_blocks_have_default_state() {
        let reg = registry();
        for block in &reg.blocks {
            let state = reg
                .get_state(block.default_state)
                .unwrap_or_else(|| panic!("block {} has invalid default state", block.name));
            assert!(
                state.is_default,
                "block {} default state is not marked as default",
                block.name
            );
        }
    }
}
