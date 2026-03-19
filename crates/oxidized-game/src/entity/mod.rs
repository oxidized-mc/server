//! Entity framework — base types, ID allocation, data sync, and tracking.
//!
//! This module provides the core entity infrastructure:
//!
//! - [`id::next_entity_id()`] — global atomic entity ID counter
//! - [`synched_data::SynchedEntityData`] — per-entity dirty-tracked data slots
//! - [`data_slots`] — base entity data slot index constants
//! - [`Aabb`](oxidized_protocol::types::aabb::Aabb) — axis-aligned bounding box (from `oxidized-protocol`)
//! - [`Entity`] — base entity struct combining ID, position, and metadata
//! - [`tracker::EntityTracker`] — tracks which players see each entity
//!
//! Corresponds to `net.minecraft.world.entity.Entity` and related classes.

pub mod data_slots;
pub mod id;
pub mod synched_data;
pub mod tracker;

use uuid::Uuid;

use self::data_slots::*;
use self::id::next_entity_id;
use self::synched_data::{DataSerializerType, SynchedEntityData};
use oxidized_protocol::types::aabb::Aabb;
use oxidized_protocol::types::resource_location::ResourceLocation;

/// Base entity containing all fields common to every entity type.
///
/// Mirrors `net.minecraft.world.entity.Entity`. In the future ECS
/// architecture (ADR-018), these fields will be decomposed into
/// individual `bevy_ecs` components. For now this struct serves as
/// the data container.
///
/// # Examples
///
/// ```
/// use oxidized_game::entity::Entity;
/// use oxidized_protocol::types::resource_location::ResourceLocation;
///
/// let mut entity = Entity::new(
///     ResourceLocation::minecraft("cow"),
///     0.9,  // width
///     1.4,  // height
/// );
/// entity.set_pos(10.0, 64.0, 10.0);
/// assert!((entity.x - 10.0).abs() < 1e-10);
/// ```
pub struct Entity {
    /// Network entity ID (unique per server session).
    pub id: i32,
    /// Entity UUID (unique globally).
    pub uuid: Uuid,
    /// Entity type identifier (e.g., `minecraft:cow`).
    pub entity_type: ResourceLocation,

    /// X position (world coordinates).
    pub x: f64,
    /// Y position (world coordinates, feet level).
    pub y: f64,
    /// Z position (world coordinates).
    pub z: f64,
    /// Yaw rotation in degrees.
    pub yaw: f32,
    /// Pitch rotation in degrees.
    pub pitch: f32,
    /// Head yaw rotation in degrees (independent of body yaw).
    pub head_yaw: f32,

    /// X velocity (blocks per tick).
    pub vx: f64,
    /// Y velocity (blocks per tick).
    pub vy: f64,
    /// Z velocity (blocks per tick).
    pub vz: f64,

    /// Whether the entity is on the ground.
    pub on_ground: bool,
    /// Whether the entity has been removed from the world.
    pub removed: bool,

    /// Collision bounding box.
    pub bounding_box: Aabb,
    /// Synchronised entity data for network serialisation.
    pub synched_data: SynchedEntityData,

    /// Width of entity's hitbox (meters).
    pub width: f32,
    /// Height of entity's hitbox (meters).
    pub height: f32,
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
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            head_yaw: 0.0,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            on_ground: false,
            removed: false,
            bounding_box: Aabb::from_center(0.0, 0.0, 0.0, f64::from(width), f64::from(height)),
            synched_data,
            width,
            height,
        }
    }

    /// Teleports the entity to `(x, y, z)` and recalculates the
    /// bounding box.
    pub fn set_pos(&mut self, x: f64, y: f64, z: f64) {
        self.x = x;
        self.y = y;
        self.z = z;
        self.bounding_box =
            Aabb::from_center(x, y, z, f64::from(self.width), f64::from(self.height));
    }

    /// Returns the value of a shared flag bit.
    pub fn get_flag(&self, bit: u8) -> bool {
        let flags: u8 = self.synched_data.get(DATA_SHARED_FLAGS);
        flags & (1 << bit) != 0
    }

    /// Sets a shared flag bit.
    pub fn set_flag(&mut self, bit: u8, value: bool) {
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
        assert!((e.x).abs() < 1e-10);
        assert!((e.y).abs() < 1e-10);
        assert!((e.z).abs() < 1e-10);
    }

    #[test]
    fn test_set_pos_updates_bbox() {
        let mut e = make_entity();
        e.set_pos(100.0, 64.0, 200.0);
        assert!((e.x - 100.0).abs() < 1e-10);
        assert!((e.y - 64.0).abs() < 1e-10);
        assert!((e.z - 200.0).abs() < 1e-10);
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
        assert!((e.width - 0.9).abs() < 1e-6);
        assert!((e.height - 1.4).abs() < 1e-6);
    }
}
