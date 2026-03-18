//! Block type and block state definitions.

/// Opaque block state identifier. Maps 1:1 to vanilla's flat state ID.
///
/// Range: 0..29872 for 26.1-pre-3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockStateId(pub u16);

/// A block type definition (e.g., `"minecraft:oak_log"`).
#[derive(Debug, Clone)]
pub struct Block {
    /// Registry name (e.g., `"minecraft:stone"`).
    pub name: String,
    /// Index in [`BlockRegistry`](super::BlockRegistry)'s block list.
    pub index: u16,
    /// Property definitions: name → list of possible values.
    pub properties: Vec<BlockProperty>,
    /// The default block state ID for this block.
    pub default_state: BlockStateId,
    /// All state IDs for this block.
    pub states: Vec<BlockStateId>,
}

/// A property definition for a block (e.g., `"axis"` with values `["x", "y", "z"]`).
#[derive(Debug, Clone)]
pub struct BlockProperty {
    /// Property name (e.g., `"snowy"`, `"axis"`).
    pub name: String,
    /// All possible values for this property.
    pub values: Vec<String>,
}

/// A specific block state: one combination of property values.
#[derive(Debug, Clone)]
pub struct BlockState {
    /// Flat state ID.
    pub id: BlockStateId,
    /// Index of the owning [`Block`] in the registry.
    pub block_index: u16,
    /// Whether this is the default state for its block.
    pub is_default: bool,
    /// Property values for this state (name → value as strings).
    ///
    /// Empty for blocks with no properties.
    pub properties: Vec<(String, String)>,
}
