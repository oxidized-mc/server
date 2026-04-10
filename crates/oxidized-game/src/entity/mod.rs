//! Entity framework — base types, ID allocation, data sync, and tracking.
//!
//! This module provides the core entity infrastructure:
//!
//! - [`id::next_entity_id()`] — global atomic entity ID counter
//! - [`synched_data::SynchedEntityData`] — per-entity dirty-tracked data slots
//! - [`data_slots`] — base entity data slot index constants
//! - [`Aabb`] — axis-aligned bounding box (from `oxidized-protocol`)
//! - [`Entity`] — base entity struct combining ID, position, and metadata
//! - [`tracker::EntityTracker`] — tracks which players see each entity
//!
//! Corresponds to `net.minecraft.world.entity.Entity` and related classes.

pub mod bundles;
pub mod commands;
pub mod components;
pub mod data_slots;
pub mod id;
pub mod markers;
pub mod phases;
pub mod synched_data;
pub mod systems;
pub mod tracker;

use uuid::Uuid;

use self::data_slots::*;
use self::id::next_entity_id;
use self::synched_data::{DataSerializerType, SynchedEntityData};
use oxidized_mc_types::Vec3;
use oxidized_mc_types::aabb::Aabb;
use oxidized_mc_types::resource_location::ResourceLocation;
use oxidized_mc_types::EntityDimensions;

/// Entity rotation in degrees (yaw, pitch, and head yaw).
///
/// Body yaw and pitch are sent together; head yaw may differ when
/// the entity looks around without turning its body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityRotation {
    /// Body yaw in degrees.
    pub yaw: f32,
    /// Pitch in degrees.
    pub pitch: f32,
    /// Head yaw in degrees (independent of body yaw).
    pub head_yaw: f32,
}

impl EntityRotation {
    /// All-zero rotation.
    pub const ZERO: Self = Self {
        yaw: 0.0,
        pitch: 0.0,
        head_yaw: 0.0,
    };
}

/// Base entity containing all fields common to every entity type.
///
/// **Deprecated (phase 23b):** This monolithic struct is superseded by
/// individual ECS components in [`components`]. Physics code now takes
/// component references directly. This struct is retained for existing
/// tests and will be removed in a future phase.
///
/// Mirrors `net.minecraft.world.entity.Entity`.
///
/// # Examples
///
/// ```
/// use oxidized_game::entity::Entity;
/// use oxidized_mc_types::resource_location::ResourceLocation;
///
/// let mut entity = Entity::new(
///     ResourceLocation::minecraft("cow"),
///     0.9,  // width
///     1.4,  // height
/// );
/// entity.set_pos(10.0, 64.0, 10.0);
/// assert!((entity.pos.x - 10.0).abs() < 1e-10);
/// ```
pub struct Entity {
    /// Network entity ID (unique per server session).
    pub id: i32,
    /// Entity UUID (unique globally).
    pub uuid: Uuid,
    /// Entity type identifier (e.g., `minecraft:cow`).
    pub entity_type: ResourceLocation,

    /// World position (double precision).
    pub pos: Vec3,
    /// Rotation in degrees.
    pub rotation: EntityRotation,
    /// Velocity in blocks per tick.
    pub velocity: Vec3,

    /// Whether the entity is on the ground.
    pub is_on_ground: bool,
    /// Whether the entity has been removed from the world.
    pub is_removed: bool,

    /// Collision bounding box.
    pub bounding_box: Aabb,
    /// Synchronised entity data for network serialisation.
    pub synched_data: SynchedEntityData,

    /// Hitbox dimensions.
    pub dimensions: EntityDimensions,

    /// Accumulated fall distance (blocks) for fall damage calculation.
    ///
    /// Increases while falling, resets to 0.0 on landing.
    pub fall_distance: f32,
}

impl Entity {
    /// Creates a new entity with the given type and hitbox dimensions.
    ///
    /// Allocates a unique entity ID, generates a random UUID, and
    /// defines the base entity data slots (flags, air supply, pose, etc.).
    pub fn new(entity_type: ResourceLocation, width: f32, height: f32) -> Self {
        let id = next_entity_id();
        let mut synched_data = SynchedEntityData::new();

        // Define base Entity data slots (matches Entity.java constructor).
        synched_data.define(DATA_SHARED_FLAGS, DataSerializerType::Byte, 0u8);
        synched_data.define(DATA_AIR_SUPPLY, DataSerializerType::Int, 300i32);
        synched_data.define(
            DATA_CUSTOM_NAME,
            DataSerializerType::OptionalComponent,
            None::<String>,
        );
        synched_data.define(DATA_CUSTOM_NAME_VISIBLE, DataSerializerType::Boolean, false);
        synched_data.define(DATA_SILENT, DataSerializerType::Boolean, false);
        synched_data.define(DATA_NO_GRAVITY, DataSerializerType::Boolean, false);
        synched_data.define(DATA_POSE, DataSerializerType::Pose, 0i32);
        synched_data.define(DATA_TICKS_FROZEN, DataSerializerType::Int, 0i32);

        Self {
            id,
            uuid: Uuid::new_v4(),
            entity_type,
            pos: Vec3::ZERO,
            rotation: EntityRotation::ZERO,
            velocity: Vec3::ZERO,
            is_on_ground: false,
            is_removed: false,
            bounding_box: Aabb::from_center(0.0, 0.0, 0.0, f64::from(width), f64::from(height)),
            synched_data,
            dimensions: EntityDimensions { width, height },
            fall_distance: 0.0,
        }
    }

    /// Teleports the entity to `(x, y, z)` and recalculates the
    /// bounding box.
    pub fn set_pos(&mut self, x: f64, y: f64, z: f64) {
        self.pos.x = x;
        self.pos.y = y;
        self.pos.z = z;
        self.bounding_box = Aabb::from_center(
            x,
            y,
            z,
            f64::from(self.dimensions.width),
            f64::from(self.dimensions.height),
        );
    }

    /// Returns the value of a shared flag bit.
    ///
    /// # Panics
    ///
    /// Debug-asserts that `bit < 8`.
    pub fn get_flag(&self, bit: u8) -> bool {
        debug_assert!(bit < 8, "flag bit index {bit} out of range 0..8");
        let flags: u8 = self.synched_data.get(DATA_SHARED_FLAGS);
        flags & (1 << bit) != 0
    }

    /// Sets a shared flag bit.
    ///
    /// # Panics
    ///
    /// Debug-asserts that `bit < 8`.
    pub fn set_flag(&mut self, bit: u8, value: bool) {
        debug_assert!(bit < 8, "flag bit index {bit} out of range 0..8");
        let mut flags: u8 = self.synched_data.get(DATA_SHARED_FLAGS);
        if value {
            flags |= 1 << bit;
        } else {
            flags &= !(1 << bit);
        }
        self.synched_data.set(DATA_SHARED_FLAGS, flags);
    }

    /// Returns `true` if the entity is on fire (flag bit 0).
    pub fn is_on_fire(&self) -> bool {
        self.get_flag(FLAG_ON_FIRE)
    }

    /// Returns `true` if the entity is crouching (flag bit 1).
    pub fn is_crouching(&self) -> bool {
        self.get_flag(FLAG_CROUCHING)
    }

    /// Returns `true` if the entity is sprinting (flag bit 3).
    pub fn is_sprinting(&self) -> bool {
        self.get_flag(FLAG_SPRINTING)
    }

    /// Returns `true` if the entity is swimming (flag bit 4).
    pub fn is_swimming(&self) -> bool {
        self.get_flag(FLAG_SWIMMING)
    }

    /// Returns `true` if the entity is invisible (flag bit 5).
    pub fn is_invisible(&self) -> bool {
        self.get_flag(FLAG_INVISIBLE)
    }

    /// Returns `true` if the entity is glowing (flag bit 6).
    pub fn is_glowing(&self) -> bool {
        self.get_flag(FLAG_GLOWING)
    }

    /// Returns `true` if the entity is fall-flying / using elytra (flag
    /// bit 7).
    pub fn is_fall_flying(&self) -> bool {
        self.get_flag(FLAG_FALL_FLYING)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn make_entity() -> Entity {
        Entity::new(ResourceLocation::minecraft("cow"), 0.9, 1.4)
    }

    #[test]
    fn test_entity_has_unique_id() {
        let e1 = make_entity();
        let e2 = make_entity();
        assert_ne!(e1.id, e2.id);
    }

    #[test]
    fn test_entity_has_unique_uuid() {
        let e1 = make_entity();
        let e2 = make_entity();
        assert_ne!(e1.uuid, e2.uuid);
    }

    #[test]
    fn test_entity_default_position_zero() {
        let e = make_entity();
        assert!((e.pos.x).abs() < 1e-10);
        assert!((e.pos.y).abs() < 1e-10);
        assert!((e.pos.z).abs() < 1e-10);
    }

    #[test]
    fn test_set_pos_updates_bbox() {
        let mut e = make_entity();
        e.set_pos(100.0, 64.0, 200.0);
        assert!((e.pos.x - 100.0).abs() < 1e-10);
        assert!((e.pos.y - 64.0).abs() < 1e-10);
        assert!((e.pos.z - 200.0).abs() < 1e-10);
        assert!(e.bounding_box.contains(100.0, 64.5, 200.0));
    }

    #[test]
    fn test_shared_flags_default_zero() {
        let e = make_entity();
        assert!(!e.is_on_fire());
        assert!(!e.is_crouching());
        assert!(!e.is_sprinting());
        assert!(!e.is_invisible());
    }

    #[test]
    fn test_set_on_fire_flag() {
        let mut e = make_entity();
        e.set_flag(FLAG_ON_FIRE, true);
        assert!(e.is_on_fire());
        assert!(!e.is_crouching());
        e.set_flag(FLAG_ON_FIRE, false);
        assert!(!e.is_on_fire());
    }

    #[test]
    fn test_multiple_flags_independent() {
        let mut e = make_entity();
        e.set_flag(FLAG_ON_FIRE, true);
        e.set_flag(FLAG_SPRINTING, true);
        assert!(e.is_on_fire());
        assert!(e.is_sprinting());
        assert!(!e.is_crouching());

        e.set_flag(FLAG_ON_FIRE, false);
        assert!(!e.is_on_fire());
        assert!(e.is_sprinting());
    }

    #[test]
    fn test_synched_data_has_8_base_slots() {
        let e = make_entity();
        assert_eq!(e.synched_data.len(), 8);
    }

    #[test]
    fn test_air_supply_default() {
        let e = make_entity();
        assert_eq!(e.synched_data.get::<i32>(DATA_AIR_SUPPLY), 300);
    }

    #[test]
    fn test_entity_dimensions() {
        let e = make_entity();
        assert!((e.dimensions.width - 0.9).abs() < 1e-6);
        assert!((e.dimensions.height - 1.4).abs() < 1e-6);
    }
}
