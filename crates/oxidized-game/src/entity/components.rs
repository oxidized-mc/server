//! Core ECS component types for entity representation.
//!
//! These components decompose vanilla's entity class hierarchy into
//! individual `bevy_ecs` components per [ADR-018]. Each field in vanilla's
//! `Entity`, `LivingEntity`, and `Player` classes maps to a named component.
//!
//! **This is scaffolding only.** The monolithic [`super::Entity`] struct
//! remains in use. These types will be adopted incrementally during
//! feature phases (P15/P24/P25/P27).
//!
//! [ADR-018]: ../../../docs/adr/adr-018-entity-system.md

use bevy_ecs::prelude::*;
use glam::DVec3;

// ---------------------------------------------------------------------------
// Entity base (vanilla Entity.java fields)
// ---------------------------------------------------------------------------

/// World position as a double-precision 3D vector.
///
/// Mirrors `Entity.position` in vanilla.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Position(pub DVec3);

/// Velocity in blocks per tick.
///
/// Mirrors `Entity.deltaMovement` in vanilla.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Velocity(pub DVec3);

/// Yaw and pitch rotation in degrees.
///
/// Mirrors `Entity.yRot` and `Entity.xRot` in vanilla.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Rotation {
    /// Horizontal rotation (degrees). 0 = south, 90 = west.
    pub yaw: f32,
    /// Vertical rotation (degrees). Negative = up, positive = down.
    pub pitch: f32,
}

/// Whether the entity is touching the ground.
///
/// Mirrors `Entity.onGround` in vanilla.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnGround(pub bool);

/// Accumulated fall distance in blocks for fall damage calculation.
///
/// Increases while falling, resets to `0.0` on landing.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct FallDistance(pub f32);

/// Packed entity flags byte (on_fire, crouching, sprinting, etc.).
///
/// Mirrors `Entity.DATA_SHARED_FLAGS` in vanilla. Bit layout:
/// - 0: on fire
/// - 1: crouching
/// - 3: sprinting
/// - 4: swimming
/// - 5: invisible
/// - 6: glowing
/// - 7: fall flying (elytra)
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityFlags(pub u8);

/// Marker: entity is unaffected by gravity.
///
/// Mirrors `Entity.noGravity` in vanilla.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoGravity;

/// Marker: entity produces no sounds.
///
/// Mirrors `Entity.silent` in vanilla.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Silent;

/// Tick counter incremented each server tick.
///
/// Mirrors `Entity.tickCount` in vanilla.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickCount(pub u32);

// ---------------------------------------------------------------------------
// LivingEntity fields
// ---------------------------------------------------------------------------

/// Current and maximum health.
///
/// Mirrors `LivingEntity.health` and `getMaxHealth()` in vanilla.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Health {
    /// Current health points.
    pub current: f32,
    /// Maximum health points (from attribute).
    pub max: f32,
}

/// Equipment slot contents placeholder.
///
/// The full item-stack representation will be implemented in feature phases.
/// For now this tracks which equipment slots are occupied.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Equipment {
    /// Number of occupied equipment slots (placeholder).
    pub slot_count: u8,
}

/// Armor defense value.
///
/// Mirrors `LivingEntity.getArmorValue()` in vanilla.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ArmorValue(pub f32);

/// Absorption hearts (yellow hearts above health bar).
///
/// Mirrors `LivingEntity.absorptionAmount` in vanilla.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AbsorptionAmount(pub f32);

// ---------------------------------------------------------------------------
// Player-specific components
// ---------------------------------------------------------------------------

/// Marker indicating this entity is a player.
///
/// Used with `With<PlayerMarker>` queries to filter player entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerMarker;

/// Currently selected hotbar slot (0–8).
///
/// Mirrors `Inventory.selected` in vanilla.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedSlot(pub u8);

/// Player experience data.
///
/// Mirrors `Player.experienceLevel`, `experienceProgress`, and
/// `totalExperience` in vanilla.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ExperienceData {
    /// Current experience level.
    pub level: i32,
    /// Progress toward the next level (0.0–1.0).
    pub progress: f32,
    /// Total lifetime experience points.
    pub total: i32,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_position_insert_and_query() {
        let mut world = World::new();
        let entity = world.spawn(Position(DVec3::new(1.0, 64.0, -3.0))).id();

        let pos = world.get::<Position>(entity).unwrap();
        assert_eq!(pos.0, DVec3::new(1.0, 64.0, -3.0));
    }

    #[test]
    fn test_velocity_insert_and_query() {
        let mut world = World::new();
        let entity = world.spawn(Velocity(DVec3::new(0.1, -0.08, 0.0))).id();

        let vel = world.get::<Velocity>(entity).unwrap();
        assert!((vel.0.y - (-0.08)).abs() < 1e-10);
    }

    #[test]
    fn test_rotation_fields() {
        let mut world = World::new();
        let entity = world
            .spawn(Rotation {
                yaw: 90.0,
                pitch: -45.0,
            })
            .id();

        let rot = world.get::<Rotation>(entity).unwrap();
        assert!((rot.yaw - 90.0).abs() < 1e-6);
        assert!((rot.pitch - (-45.0)).abs() < 1e-6);
    }

    #[test]
    fn test_health_component() {
        let mut world = World::new();
        let entity = world
            .spawn(Health {
                current: 20.0,
                max: 20.0,
            })
            .id();

        let hp = world.get::<Health>(entity).unwrap();
        assert!((hp.current - 20.0).abs() < 1e-6);
        assert!((hp.max - 20.0).abs() < 1e-6);
    }

    #[test]
    fn test_player_marker_query() {
        let mut world = World::new();
        world.spawn(PlayerMarker);
        world.spawn(PlayerMarker);
        world.spawn(OnGround(true));

        let mut query = world.query_filtered::<Entity, With<PlayerMarker>>();
        let players: Vec<_> = query.iter(&world).collect();
        assert_eq!(players.len(), 2);
    }

    #[test]
    fn test_experience_data_defaults() {
        let xp = ExperienceData {
            level: 0,
            progress: 0.0,
            total: 0,
        };
        assert_eq!(xp.level, 0);
        assert!((xp.progress).abs() < 1e-6);
        assert_eq!(xp.total, 0);
    }

    #[test]
    fn test_entity_flags_bitwise() {
        let flags = EntityFlags(0b0000_1001); // on_fire + sprinting
        assert_eq!(flags.0 & 1, 1); // bit 0: on_fire
        assert_eq!(flags.0 & (1 << 3), 8); // bit 3: sprinting
        assert_eq!(flags.0 & (1 << 1), 0); // bit 1: crouching = off
    }

    #[test]
    fn test_no_gravity_is_unit_component() {
        let mut world = World::new();
        let entity = world.spawn(NoGravity).id();
        assert!(world.get::<NoGravity>(entity).is_some());
    }

    #[test]
    fn test_multiple_components_on_entity() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Position(DVec3::ZERO),
                Velocity(DVec3::ZERO),
                Rotation {
                    yaw: 0.0,
                    pitch: 0.0,
                },
                OnGround(true),
                FallDistance(0.0),
                EntityFlags(0),
            ))
            .id();

        assert!(world.get::<Position>(entity).is_some());
        assert!(world.get::<Velocity>(entity).is_some());
        assert!(world.get::<Rotation>(entity).is_some());
        assert!(world.get::<OnGround>(entity).is_some());
        assert!(world.get::<FallDistance>(entity).is_some());
        assert!(world.get::<EntityFlags>(entity).is_some());
    }
}
