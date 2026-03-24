//! Block type and block state definitions.
//!
//! Types here are used by both the compile-time code generator (`build.rs`)
//! and the runtime block registry.  Block state data is generated at compile
//! time as dense static arrays — see [`super::generated`].

use bitflags::bitflags;

use super::generated;

/// Opaque block state identifier. Maps 1:1 to vanilla's flat state ID.
///
/// Range: 0..29 872 for 26.1-pre-3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockStateId(pub u16);

impl BlockStateId {
    /// Returns the static data entry for this state.
    #[inline]
    pub fn data(self) -> &'static BlockStateEntry {
        &generated::BLOCK_STATE_DATA[self.0 as usize]
    }

    /// Returns the parent block definition.
    #[inline]
    pub fn block_def(self) -> &'static BlockDef {
        &generated::BLOCK_DEFS[self.data().block_type as usize]
    }

    /// Returns the block registry name (e.g., `"minecraft:stone"`).
    #[inline]
    pub fn block_name(self) -> &'static str {
        self.block_def().name
    }

    /// Returns `true` if this state is air (`air`, `cave_air`, `void_air`).
    #[inline]
    pub fn is_air(self) -> bool {
        self.data().flags.contains(BlockStateFlags::IS_AIR)
    }

    /// Returns `true` if this is the default state for its block.
    #[inline]
    pub fn is_default(self) -> bool {
        self.data().flags.contains(BlockStateFlags::IS_DEFAULT)
    }

    /// Returns `true` if this state is a liquid (water, lava).
    #[inline]
    pub fn is_liquid(self) -> bool {
        self.data().flags.contains(BlockStateFlags::IS_LIQUID)
    }

    /// Computes property key-value pairs for this state on the fly
    /// using stride arithmetic.
    ///
    /// Returns an empty vec for blocks with no properties.
    pub fn properties(self) -> Vec<(&'static str, &'static str)> {
        let def = self.block_def();
        if def.prop_count == 0 {
            return Vec::new();
        }
        let offset = self.0 - def.first_state;
        let props_start = def.props_offset as usize;
        let props_end = props_start + def.prop_count as usize;
        let props = &generated::PROPERTY_DEFS[props_start..props_end];

        props
            .iter()
            .map(|p| {
                let value_idx = (offset / p.stride) % p.num_values as u16;
                let value =
                    generated::PROPERTY_VALUES[p.values_offset as usize + value_idx as usize];
                (p.name, value)
            })
            .collect()
    }

    /// Returns a new state with the given property set to `value`.
    ///
    /// Returns `None` if the property name or value is not valid for this
    /// block type.
    pub fn with_property(self, name: &str, value: &str) -> Option<Self> {
        let def = self.block_def();
        let offset = self.0 - def.first_state;
        let props_start = def.props_offset as usize;
        let props_end = props_start + def.prop_count as usize;
        let props = &generated::PROPERTY_DEFS[props_start..props_end];

        for p in props {
            if p.name != name {
                continue;
            }
            let vals_start = p.values_offset as usize;
            let vals_end = vals_start + p.num_values as usize;
            let values = &generated::PROPERTY_VALUES[vals_start..vals_end];
            let new_idx = values.iter().position(|&v| v == value)? as u16;
            let old_idx = (offset / p.stride) % p.num_values as u16;
            let new_offset = offset - old_idx * p.stride + new_idx * p.stride;
            return Some(BlockStateId(def.first_state + new_offset));
        }
        None
    }
}

// ─── Static data types (populated by build.rs) ─────────────────────────────

/// Static data for a single block state, generated at compile time.
///
/// Property values are **not** stored inline — they are computed on demand
/// from the state's offset within its block using stride arithmetic.
#[derive(Debug, Clone, Copy)]
pub struct BlockStateEntry {
    /// Index into [`BLOCK_DEFS`](super::generated::BLOCK_DEFS).
    pub block_type: u16,
    /// Bitflags for commonly queried properties.
    pub flags: BlockStateFlags,
}

bitflags! {
    /// Flags for a block state, derivable from vanilla data.
    ///
    /// Additional flags (IS_SOLID, HAS_COLLISION, etc.) will be added once
    /// the Java data extraction pipeline provides that metadata.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BlockStateFlags: u8 {
        /// Block is air (`air`, `cave_air`, `void_air`).
        const IS_AIR     = 0x01;
        /// This is the default state for its block type.
        const IS_DEFAULT = 0x02;
        /// Block is a liquid (`water`, `lava`).
        const IS_LIQUID  = 0x04;
    }
}

/// Static definition of a block type, generated at compile time.
///
/// States for this block occupy the contiguous ID range
/// `first_state .. first_state + state_count`.
#[derive(Debug)]
pub struct BlockDef {
    /// Registry name (e.g., `"minecraft:stone"`).
    pub name: &'static str,
    /// First state ID.
    pub first_state: u16,
    /// Number of states.
    pub state_count: u16,
    /// Default state ID (absolute).
    pub default_state: u16,
    /// Number of properties for this block.
    pub prop_count: u8,
    /// Offset into [`PROPERTY_DEFS`](super::generated::PROPERTY_DEFS).
    pub props_offset: u16,
}

/// Static definition of a block property, generated at compile time.
///
/// The `stride` enables O(1) state transitions via arithmetic — see
/// [`BlockStateId::with_property`].
#[derive(Debug)]
pub struct PropertyDef {
    /// Property name (e.g., `"facing"`).
    pub name: &'static str,
    /// Number of possible values.
    pub num_values: u8,
    /// Offset into [`PROPERTY_VALUES`](super::generated::PROPERTY_VALUES).
    pub values_offset: u16,
    /// Stride for this property in state index computation.
    pub stride: u16,
}
