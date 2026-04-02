//! Basic physics: gravity, collision, drag, fluid buoyancy, and jump.
//!
//! This module implements the per-tick physics pipeline matching vanilla
//! Minecraft's `LivingEntity.travel()` and `Entity.move()`. The core
//! algorithm is a per-axis AABB sweep collision test with
//! movement-dependent axis ordering (see [`collision::collide_with_shapes`]).
//!
//! # Module Layout
//!
//! - [`constants`] — all physics constants (gravity, drag, friction, etc.)
//! - [`voxel_shape`] — block collision geometry representation
//! - [`collision`] — per-axis sweep collision and obstacle collection
//! - [`tick`] — the main per-tick physics update
//! - [`slow_blocks`] — block speed/jump factor modifiers
//! - [`jump`] — jump impulse application
//!
//! Block physics properties (friction, speed factor, jump factor) are read
//! directly from the compile-time block registry via [`BlockStateId`].
//!
//! [`BlockStateId`]: oxidized_registry::BlockStateId

pub mod collision;
pub mod constants;
pub mod jump;
pub mod slow_blocks;
pub mod tick;
pub mod voxel_shape;
