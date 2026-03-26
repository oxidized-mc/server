//! Block type and block state definitions.
//!
//! Types here are used by both the compile-time code generator (`build.rs`)
//! and the runtime block registry.  Block state data is generated at compile
//! time as dense static arrays — see [`super::generated`].

use bitflags::bitflags;

use super::generated;

/// Opaque block state identifier. Maps 1:1 to vanilla's flat state ID.
///
/// Range: 0..29 872 for 26.1.
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

    /// Returns `true` if this block has a full solid collision shape.
    #[inline]
    pub fn is_solid(self) -> bool {
        self.data().flags.contains(BlockStateFlags::IS_SOLID)
    }

    /// Returns `true` if this block has collision geometry.
    #[inline]
    pub fn has_collision(self) -> bool {
        self.data().flags.contains(BlockStateFlags::HAS_COLLISION)
    }

    /// Returns `true` if this block occludes adjacent faces.
    #[inline]
    pub fn is_opaque(self) -> bool {
        self.data().flags.contains(BlockStateFlags::IS_OPAQUE)
    }

    /// Returns `true` if this block can be replaced by placing another block.
    #[inline]
    pub fn is_replaceable(self) -> bool {
        self.data().flags.contains(BlockStateFlags::IS_REPLACEABLE)
    }

    /// Returns `true` if this block has a block entity (tile entity).
    #[inline]
    pub fn has_block_entity(self) -> bool {
        self.data()
            .flags
            .contains(BlockStateFlags::HAS_BLOCK_ENTITY)
    }

    /// Returns `true` if this block ticks randomly.
    #[inline]
    pub fn ticks_randomly(self) -> bool {
        self.data().flags.contains(BlockStateFlags::TICKS_RANDOMLY)
    }

    /// Returns `true` if this block requires the correct tool to drop items.
    #[inline]
    pub fn requires_tool(self) -> bool {
        self.data().flags.contains(BlockStateFlags::REQUIRES_TOOL)
    }

    /// Returns `true` if this block can be ignited by lava.
    #[inline]
    pub fn is_flammable(self) -> bool {
        self.data().flags.contains(BlockStateFlags::IS_FLAMMABLE)
    }

    /// Returns `true` if this block opens a UI or changes state on right-click.
    #[inline]
    pub fn is_interactable(self) -> bool {
        self.data().flags.contains(BlockStateFlags::IS_INTERACTABLE)
    }

    /// Returns `true` if this block uses its shape for directional light occlusion.
    ///
    /// When `true`, the lighting engine checks per-face occlusion data rather
    /// than scalar `light_opacity()` to determine if light passes through a face.
    #[inline]
    pub fn use_shape_for_light_occlusion(self) -> bool {
        self.data()
            .flags
            .contains(BlockStateFlags::USE_SHAPE_FOR_LIGHT_OCCLUSION)
    }

    /// Returns `true` if this block state's shape is "empty" for light occlusion.
    ///
    /// Matches vanilla's `isEmptyShape()`: a shape is empty if the block cannot
    /// occlude OR does not use shape-based light occlusion. When empty, only
    /// scalar `light_opacity()` is used.
    #[inline]
    pub fn is_empty_shape(self) -> bool {
        !self.is_opaque() || !self.use_shape_for_light_occlusion()
    }

    /// Returns `true` if the given face fully occludes light.
    ///
    /// `face_index` maps to [`Direction`] ordinals: 0=Down, 1=Up, 2=North,
    /// 3=South, 4=West, 5=East.
    #[inline]
    pub fn occlusion_face(self, face_index: u8) -> bool {
        debug_assert!(face_index < 6, "face_index must be 0..6");
        self.data().occlusion_faces & (1 << face_index) != 0
    }

    /// Returns the light level this block emits (0–15).
    #[inline]
    pub fn light_emission(self) -> u8 {
        self.data().light_emission
    }

    /// Returns how much light this block absorbs (0–15).
    #[inline]
    pub fn light_opacity(self) -> u8 {
        self.data().light_opacity
    }

    /// Returns the hardness of this block in seconds.
    /// Returns -1.0 for unbreakable blocks (bedrock).
    #[inline]
    pub fn hardness(self) -> f64 {
        let raw = self.data().hardness;
        if raw == 0xFFFF {
            -1.0
        } else {
            f64::from(raw) / 100.0
        }
    }

    /// Returns the explosion resistance of this block.
    #[inline]
    pub fn explosion_resistance(self) -> f64 {
        f64::from(self.data().explosion_resistance) / 100.0
    }

    /// Returns the friction (slide speed multiplier) for this block.
    /// Default (no friction): 0.6.
    #[inline]
    pub fn friction(self) -> f64 {
        f64::from(self.data().friction) / 10_000.0
    }

    /// Returns the speed factor for movement on this block.
    /// Default (no slowdown): 1.0.
    #[inline]
    pub fn speed_factor(self) -> f64 {
        f64::from(self.data().speed_factor) / 10_000.0
    }

    /// Returns the jump height factor for this block.
    /// Default (no change): 1.0.
    #[inline]
    pub fn jump_factor(self) -> f64 {
        f64::from(self.data().jump_factor) / 10_000.0
    }

    /// Returns the map color index for this block (0–63).
    #[inline]
    pub fn map_color(self) -> u8 {
        self.data().map_color
    }

    /// Returns the push reaction for this block.
    /// 0 = NORMAL, 1 = DESTROY, 2 = BLOCK, 3 = PUSH_ONLY
    #[inline]
    pub fn push_reaction(self) -> u8 {
        self.data().push_reaction
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

/// Per-state block data: all immutable properties.
/// Generated at compile time and stored in a dense static array.
///
/// Property values are **not** stored inline — they are computed on demand
/// from the state's offset within its block using stride arithmetic.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct BlockStateEntry {
    /// Index into `BLOCK_DEFS`.
    pub block_type: u16,
    /// Bitflags for commonly queried properties (13 flags, u16).
    pub flags: BlockStateFlags,
    /// Light level emission (0–15). From vanilla `luminance`.
    pub light_emission: u8,
    /// Light opacity (0–15). How much light this block absorbs.
    pub light_opacity: u8,
    /// Per-face occlusion bitmask.
    ///
    /// Bit N is set if face N (matching [`Direction`] ordinal: 0=Down, 1=Up,
    /// 2=North, 3=South, 4=West, 5=East) is fully occluding for light.
    /// Only meaningful when both `IS_OPAQUE` and `USE_SHAPE_FOR_LIGHT_OCCLUSION`
    /// flags are set.
    pub occlusion_faces: u8,
    /// Hardness in fixed-point ×100. 0xFFFF = unbreakable (bedrock).
    pub hardness: u16,
    /// Explosion resistance in fixed-point ×100.
    pub explosion_resistance: u16,
    /// Friction (slide speed) in fixed-point ×10000. Vanilla default: 6000.
    pub friction: u16,
    /// Speed factor in fixed-point ×10000. Vanilla default: 10000 (no slowdown).
    pub speed_factor: u16,
    /// Jump height factor in fixed-point ×10000. Vanilla default: 10000.
    pub jump_factor: u16,
    /// Map color index (0–63). Used for filled maps.
    pub map_color: u8,
    /// Push reaction: 0=NORMAL, 1=DESTROY, 2=BLOCK, 3=PUSH_ONLY.
    pub push_reaction: u8,
}

bitflags! {
    /// Flags for a block state, derived from vanilla block property data.
    ///
    /// Stored as `u16` per ADR-012 to accommodate current and future flags.
    /// The extraction script (`tools/extract_block_properties.py`) produces the
    /// data that `build.rs` uses to set these flags at compile time.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BlockStateFlags: u16 {
        /// Block is air (`air`, `cave_air`, `void_air`).
        const IS_AIR           = 1 << 0;
        /// This is the default state for its block type.
        const IS_DEFAULT       = 1 << 1;
        /// Block is a liquid (`water`, `lava`).
        const IS_LIQUID        = 1 << 2;
        /// Block has a full solid collision shape.
        const IS_SOLID         = 1 << 3;
        /// Block has collision geometry (mobs/players cannot walk through).
        const HAS_COLLISION    = 1 << 4;
        /// Block occludes adjacent faces for culling purposes.
        const IS_OPAQUE        = 1 << 5;
        /// Block can be replaced by placing another block on it.
        const IS_REPLACEABLE   = 1 << 6;
        /// Block has an associated block entity (tile entity).
        const HAS_BLOCK_ENTITY = 1 << 7;
        /// Block ticks randomly (crop growth, grass spread, etc.).
        const TICKS_RANDOMLY   = 1 << 8;
        /// Block requires the correct tool to drop items.
        const REQUIRES_TOOL    = 1 << 9;
        /// Block can be ignited by lava.
        const IS_FLAMMABLE     = 1 << 10;
        /// Block opens a UI or changes state on right-click (without item).
        const IS_INTERACTABLE  = 1 << 11;
        /// Block uses its VoxelShape for directional light occlusion.
        ///
        /// When set together with `IS_OPAQUE`, per-face occlusion data in
        /// [`BlockStateEntry::occlusion_faces`] is used by the lighting engine
        /// instead of scalar `light_opacity`.
        const USE_SHAPE_FOR_LIGHT_OCCLUSION = 1 << 12;
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
    /// Offset into `PROPERTY_DEFS`.
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
    /// Offset into `PROPERTY_VALUES`.
    pub values_offset: u16,
    /// Stride for this property in state index computation.
    pub stride: u16,
}
