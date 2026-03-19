//! Dense lookup table for per-block physics properties.
//!
//! [`PhysicsBlockProperties`] maps block state IDs to their friction,
//! speed factor, and jump factor values. Built once from the
//! [`BlockRegistry`] at startup, it provides O(1) lookups used by
//! the physics tick and slow-block systems.
//!
//! # Why a dense table?
//!
//! Block state IDs are small integers (0..≈30 000 in 26.1-pre-3), so
//! a `Vec<f64>` indexed by state ID is cache-friendly and avoids hash
//! lookups in the hot physics path. ADR-021 recommends this approach.

use oxidized_world::registry::BlockRegistry;

use super::constants::*;

/// Physics properties for all block states, indexed by state ID.
///
/// Built from [`BlockRegistry`] via [`PhysicsBlockProperties::from_registry`].
/// All lookups are O(1) array index operations.
///
/// # Examples
///
/// ```no_run
/// use oxidized_game::physics::block_properties::PhysicsBlockProperties;
/// use oxidized_world::registry::BlockRegistry;
///
/// let registry = BlockRegistry::load().unwrap();
/// let physics = PhysicsBlockProperties::from_registry(&registry);
///
/// // Ice has 0.98 friction, not the default 0.6
/// let ice_default = registry.default_state("minecraft:ice").unwrap();
/// assert!((physics.friction(ice_default.0 as u32) - 0.98).abs() < 0.001);
/// ```
pub struct PhysicsBlockProperties {
    /// Friction per state ID (default: 0.6).
    friction: Vec<f64>,
    /// Horizontal speed factor per state ID (default: 1.0).
    speed_factor: Vec<f64>,
    /// Jump factor per state ID (default: 1.0).
    jump_factor: Vec<f64>,
    /// Whether the block is a slime block (for bounce).
    is_slime: Vec<bool>,
}

/// A block name and its physics property overrides.
struct PhysicsOverride {
    name: &'static str,
    friction: f64,
    speed_factor: f64,
    jump_factor: f64,
    is_slime: bool,
}

/// All blocks with non-default physics properties.
///
/// Values are sourced from `Blocks.java` in the vanilla 26.1-pre-3 server.
const PHYSICS_OVERRIDES: &[PhysicsOverride] = &[
    // Ice variants: friction 0.98
    PhysicsOverride {
        name: "minecraft:ice",
        friction: ICE_FRICTION,
        speed_factor: 1.0,
        jump_factor: 1.0,
        is_slime: false,
    },
    PhysicsOverride {
        name: "minecraft:packed_ice",
        friction: ICE_FRICTION,
        speed_factor: 1.0,
        jump_factor: 1.0,
        is_slime: false,
    },
    PhysicsOverride {
        name: "minecraft:frosted_ice",
        friction: ICE_FRICTION,
        speed_factor: 1.0,
        jump_factor: 1.0,
        is_slime: false,
    },
    // Blue ice: extra slippery
    PhysicsOverride {
        name: "minecraft:blue_ice",
        friction: BLUE_ICE_FRICTION,
        speed_factor: 1.0,
        jump_factor: 1.0,
        is_slime: false,
    },
    // Slime block: special friction + bounce
    PhysicsOverride {
        name: "minecraft:slime_block",
        friction: SLIME_FRICTION,
        speed_factor: 1.0,
        jump_factor: 1.0,
        is_slime: true,
    },
    // Soul sand: slows movement
    PhysicsOverride {
        name: "minecraft:soul_sand",
        friction: BLOCK_FRICTION_DEFAULT,
        speed_factor: SOUL_SAND_SPEED_FACTOR,
        jump_factor: 1.0,
        is_slime: false,
    },
    // Honey block: slows movement + reduces jump
    PhysicsOverride {
        name: "minecraft:honey_block",
        friction: BLOCK_FRICTION_DEFAULT,
        speed_factor: HONEY_BLOCK_SPEED_FACTOR,
        jump_factor: HONEY_BLOCK_JUMP_FACTOR,
        is_slime: false,
    },
    // Powder snow: slows movement (via makeStuckInBlock)
    PhysicsOverride {
        name: "minecraft:powder_snow",
        friction: BLOCK_FRICTION_DEFAULT,
        speed_factor: POWDER_SNOW_SPEED_FACTOR,
        jump_factor: 1.0,
        is_slime: false,
    },
];

impl PhysicsBlockProperties {
    /// Builds the physics lookup table from a block registry.
    ///
    /// Iterates known physics-affecting blocks and assigns their
    /// properties to all of their state IDs. Blocks not listed in
    /// [`PHYSICS_OVERRIDES`] get default values.
    pub fn from_registry(registry: &BlockRegistry) -> Self {
        let size = registry.state_array_size();
        let mut friction = vec![BLOCK_FRICTION_DEFAULT; size];
        let mut speed_factor = vec![1.0; size];
        let mut jump_factor = vec![1.0; size];
        let mut is_slime = vec![false; size];

        for entry in PHYSICS_OVERRIDES {
            if let Some(block) = registry.get_block(entry.name) {
                for &state_id in &block.states {
                    let idx = state_id.0 as usize;
                    friction[idx] = entry.friction;
                    speed_factor[idx] = entry.speed_factor;
                    jump_factor[idx] = entry.jump_factor;
                    is_slime[idx] = entry.is_slime;
                }
            }
        }

        Self {
            friction,
            speed_factor,
            jump_factor,
            is_slime,
        }
    }

    /// Creates a default lookup where all blocks have standard physics.
    ///
    /// Useful for testing when no block registry is available.
    pub fn defaults() -> Self {
        Self {
            friction: Vec::new(),
            speed_factor: Vec::new(),
            jump_factor: Vec::new(),
            is_slime: Vec::new(),
        }
    }

    /// Returns the friction value for the given block state ID.
    ///
    /// Returns [`BLOCK_FRICTION_DEFAULT`] (0.6) if the state ID is
    /// out of range or has no special friction.
    pub fn friction(&self, state_id: u32) -> f64 {
        self.friction
            .get(state_id as usize)
            .copied()
            .unwrap_or(BLOCK_FRICTION_DEFAULT)
    }

    /// Returns the horizontal speed factor for the given block state ID.
    ///
    /// Returns 1.0 for normal blocks, < 1.0 for slow blocks like
    /// soul sand or honey.
    pub fn speed_factor(&self, state_id: u32) -> f64 {
        self.speed_factor
            .get(state_id as usize)
            .copied()
            .unwrap_or(1.0)
    }

    /// Returns the jump factor for the given block state ID.
    ///
    /// Returns 1.0 for normal blocks, 0.5 for honey blocks.
    pub fn jump_factor(&self, state_id: u32) -> f64 {
        self.jump_factor
            .get(state_id as usize)
            .copied()
            .unwrap_or(1.0)
    }

    /// Returns `true` if the block is a slime block (bounces entities).
    pub fn is_slime_block(&self, state_id: u32) -> bool {
        self.is_slime
            .get(state_id as usize)
            .copied()
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn registry() -> BlockRegistry {
        BlockRegistry::load().expect("failed to load block registry")
    }

    fn physics() -> PhysicsBlockProperties {
        PhysicsBlockProperties::from_registry(&registry())
    }

    #[test]
    fn test_default_block_has_default_friction() {
        let p = physics();
        let reg = registry();
        let stone = reg.default_state("minecraft:stone").unwrap();
        assert!(
            (p.friction(stone.0 as u32) - BLOCK_FRICTION_DEFAULT).abs() < 1e-10,
            "Stone should have default friction"
        );
    }

    #[test]
    fn test_ice_friction() {
        let p = physics();
        let reg = registry();
        let ice = reg.default_state("minecraft:ice").unwrap();
        assert!(
            (p.friction(ice.0 as u32) - ICE_FRICTION).abs() < 1e-10,
            "Ice friction should be {}",
            ICE_FRICTION
        );
    }

    #[test]
    fn test_packed_ice_friction() {
        let p = physics();
        let reg = registry();
        let packed = reg.default_state("minecraft:packed_ice").unwrap();
        assert!(
            (p.friction(packed.0 as u32) - ICE_FRICTION).abs() < 1e-10,
            "Packed ice friction should be {}",
            ICE_FRICTION
        );
    }

    #[test]
    fn test_blue_ice_friction() {
        let p = physics();
        let reg = registry();
        let blue = reg.default_state("minecraft:blue_ice").unwrap();
        assert!(
            (p.friction(blue.0 as u32) - BLUE_ICE_FRICTION).abs() < 1e-10,
            "Blue ice friction should be {}",
            BLUE_ICE_FRICTION
        );
    }

    #[test]
    fn test_slime_friction() {
        let p = physics();
        let reg = registry();
        let slime = reg.default_state("minecraft:slime_block").unwrap();
        assert!(
            (p.friction(slime.0 as u32) - SLIME_FRICTION).abs() < 1e-10,
            "Slime friction should be {}",
            SLIME_FRICTION
        );
    }

    #[test]
    fn test_slime_is_slime() {
        let p = physics();
        let reg = registry();
        let slime = reg.default_state("minecraft:slime_block").unwrap();
        assert!(p.is_slime_block(slime.0 as u32));
    }

    #[test]
    fn test_soul_sand_speed_factor() {
        let p = physics();
        let reg = registry();
        let soul = reg.default_state("minecraft:soul_sand").unwrap();
        assert!(
            (p.speed_factor(soul.0 as u32) - SOUL_SAND_SPEED_FACTOR).abs() < 1e-10,
            "Soul sand speed should be {}",
            SOUL_SAND_SPEED_FACTOR
        );
    }

    #[test]
    fn test_honey_speed_and_jump() {
        let p = physics();
        let reg = registry();
        let honey = reg.default_state("minecraft:honey_block").unwrap();
        assert!(
            (p.speed_factor(honey.0 as u32) - HONEY_BLOCK_SPEED_FACTOR).abs() < 1e-10,
            "Honey speed should be {}",
            HONEY_BLOCK_SPEED_FACTOR
        );
        assert!(
            (p.jump_factor(honey.0 as u32) - HONEY_BLOCK_JUMP_FACTOR).abs() < 1e-10,
            "Honey jump should be {}",
            HONEY_BLOCK_JUMP_FACTOR
        );
    }

    #[test]
    fn test_powder_snow_speed_factor() {
        let p = physics();
        let reg = registry();
        let powder = reg.default_state("minecraft:powder_snow").unwrap();
        assert!(
            (p.speed_factor(powder.0 as u32) - POWDER_SNOW_SPEED_FACTOR).abs() < 1e-10,
            "Powder snow speed should be {}",
            POWDER_SNOW_SPEED_FACTOR
        );
    }

    #[test]
    fn test_frosted_ice_all_states() {
        let p = physics();
        let reg = registry();
        let block = reg.get_block("minecraft:frosted_ice").unwrap();
        // Frosted ice has 4 states (age=0..3). All should have ice friction.
        assert_eq!(block.states.len(), 4);
        for &state_id in &block.states {
            assert!(
                (p.friction(state_id.0 as u32) - ICE_FRICTION).abs() < 1e-10,
                "Frosted ice state {} should have friction {}",
                state_id.0,
                ICE_FRICTION
            );
        }
    }

    #[test]
    fn test_defaults_returns_standard_values() {
        let p = PhysicsBlockProperties::defaults();
        // Out-of-range lookups return defaults.
        assert!((p.friction(999) - BLOCK_FRICTION_DEFAULT).abs() < 1e-10);
        assert!((p.speed_factor(999) - 1.0).abs() < 1e-10);
        assert!((p.jump_factor(999) - 1.0).abs() < 1e-10);
        assert!(!p.is_slime_block(999));
    }

    #[test]
    fn test_air_has_default_physics() {
        let p = physics();
        assert!((p.friction(0) - BLOCK_FRICTION_DEFAULT).abs() < 1e-10);
        assert!((p.speed_factor(0) - 1.0).abs() < 1e-10);
        assert!((p.jump_factor(0) - 1.0).abs() < 1e-10);
        assert!(!p.is_slime_block(0));
    }
}
