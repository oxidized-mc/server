//! Physics constants matching the vanilla Minecraft server.
//!
//! All values are sourced from `net.minecraft.world.entity.LivingEntity`,
//! `net.minecraft.world.entity.Entity`, and block property definitions in
//! `net.minecraft.world.level.block.Blocks`.

// --- Gravity ---

/// Gravity acceleration (blocks/tick²).
///
/// Applied every tick to non-flying, non-levitating entities.
/// Matches `LivingEntity.DEFAULT_BASE_GRAVITY`.
pub const GRAVITY: f64 = 0.08;

// --- Drag ---

/// Horizontal drag multiplier per tick in air.
///
/// After movement, horizontal velocity is multiplied by
/// `block_friction * HORIZONTAL_DRAG`. In air (friction = 1.0),
/// this equals 0.91.
pub const HORIZONTAL_DRAG: f64 = 0.91;

/// Vertical drag multiplier per tick.
///
/// Applied as `vy *= VERTICAL_DRAG` after gravity and collision.
pub const VERTICAL_DRAG: f64 = 0.98;

// --- Block friction ---

/// Default block friction. Most solid blocks use this value.
///
/// Effective ground drag = `block_friction * HORIZONTAL_DRAG` = 0.6 × 0.91 = 0.546.
pub const BLOCK_FRICTION_DEFAULT: f64 = 0.6;

/// Friction for ice, packed ice, and frosted ice blocks.
pub const ICE_FRICTION: f64 = 0.98;

/// Friction for blue ice blocks (slightly more slippery than regular ice).
pub const BLUE_ICE_FRICTION: f64 = 0.989;

/// Friction for slime blocks.
pub const SLIME_FRICTION: f64 = 0.8;

// --- Jump ---

/// Base jump velocity (blocks/tick).
///
/// Matches `LivingEntity.getJumpPower()` with default attributes.
pub const JUMP_POWER: f64 = 0.42;

/// Additional jump velocity per level of Jump Boost effect.
pub const JUMP_BOOST_PER_LEVEL: f64 = 0.1;

/// Horizontal sprint-jump boost magnitude.
///
/// Applied in the facing direction when sprinting and jumping.
pub const SPRINT_JUMP_BOOST: f64 = 0.2;

// --- Fluid physics ---

/// Upward velocity added per tick when submerged in water.
pub const WATER_BUOYANCY: f64 = 0.014;

/// Upward velocity added per tick when submerged in lava.
pub const LAVA_BUOYANCY: f64 = 0.007;

/// Velocity drag multiplier when in water.
pub const WATER_DRAG: f64 = 0.8;

/// Velocity drag multiplier when in lava.
pub const LAVA_DRAG: f64 = 0.5;

// --- Slow block speed multipliers ---

/// Soul sand horizontal speed multiplier.
pub const SOUL_SAND_SPEED_FACTOR: f64 = 0.4;

/// Honey block horizontal speed multiplier.
pub const HONEY_BLOCK_SPEED_FACTOR: f64 = 0.4;

/// Honey block jump factor (reduces jump height).
pub const HONEY_BLOCK_JUMP_FACTOR: f64 = 0.5;

/// Powder snow stuck-in-block multiplier (horizontal).
///
/// Applied via `makeStuckInBlock(Vec3(0.9, 1.5, 0.9))` in Java.
pub const POWDER_SNOW_SPEED_FACTOR: f64 = 0.9;

// --- Movement speeds (blocks/tick, before friction) ---

/// Default walk speed.
pub const WALK_SPEED: f64 = 0.1;

/// Sprint speed.
pub const SPRINT_SPEED: f64 = 0.13;

/// Sneak speed.
pub const SNEAK_SPEED: f64 = 0.065;

// --- Step-up ---

/// Maximum height (in blocks) an entity can step up without jumping.
///
/// Default for `LivingEntity` via the `STEP_HEIGHT` attribute.
pub const DEFAULT_STEP_HEIGHT: f64 = 0.6;

// --- Collision tolerance ---

/// Epsilon for floating-point collision comparisons.
///
/// Matches `Shapes.EPSILON` in Java.
pub const COLLISION_EPSILON: f64 = 1.0e-7;
