//! Spawn template bundles for common entity archetypes.
//!
//! Bundles group the components that every entity of a given type must
//! have at spawn time. Using `world.spawn(ZombieBundle::new(...))` ensures
//! all required components are present without forgetting any.
//!
//! **This is scaffolding only.** Bundles use placeholder defaults.
//! Full field initialisation (from vanilla spawn logic) will be added
//! in feature phases.

use bevy_ecs::prelude::*;
use glam::DVec3;

use super::components::{
    AbsorptionAmount, ArmorValue, EntityFlags, Equipment, ExperienceData, FallDistance, Health,
    OnGround, PlayerMarker, Position, Rotation, SelectedSlot, TickCount, Velocity,
};
use super::markers::{CowMarker, CreeperMarker, SkeletonMarker, ZombieMarker};

/// Components shared by every entity type.
///
/// Mirrors the fields defined in vanilla's `Entity` base class.
#[derive(Bundle)]
pub struct BaseEntityBundle {
    /// World position.
    pub position: Position,
    /// Velocity in blocks/tick.
    pub velocity: Velocity,
    /// Yaw/pitch rotation.
    pub rotation: Rotation,
    /// Whether touching the ground.
    pub on_ground: OnGround,
    /// Accumulated fall distance.
    pub fall_distance: FallDistance,
    /// Packed shared flags byte.
    pub flags: EntityFlags,
    /// Server tick counter.
    pub tick_count: TickCount,
}

impl BaseEntityBundle {
    /// Creates a base bundle at the given position with default state.
    pub fn new(pos: DVec3) -> Self {
        Self {
            position: Position(pos),
            velocity: Velocity(DVec3::ZERO),
            rotation: Rotation {
                yaw: 0.0,
                pitch: 0.0,
            },
            on_ground: OnGround(false),
            fall_distance: FallDistance(0.0),
            flags: EntityFlags(0),
            tick_count: TickCount(0),
        }
    }
}

/// Components for all living entities (mobs, players).
///
/// Extends [`BaseEntityBundle`] with health, armor, and absorption.
/// Mirrors vanilla's `LivingEntity` class fields.
#[derive(Bundle)]
pub struct LivingEntityBundle {
    /// Base entity components.
    pub base: BaseEntityBundle,
    /// Current and max health.
    pub health: Health,
    /// Armor defense value.
    pub armor: ArmorValue,
    /// Absorption (yellow hearts).
    pub absorption: AbsorptionAmount,
    /// Equipment slots.
    pub equipment: Equipment,
}

impl LivingEntityBundle {
    /// Creates a living entity bundle with the given position and max health.
    pub fn new(pos: DVec3, max_health: f32) -> Self {
        Self {
            base: BaseEntityBundle::new(pos),
            health: Health {
                current: max_health,
                max: max_health,
            },
            armor: ArmorValue(0.0),
            absorption: AbsorptionAmount(0.0),
            equipment: Equipment::default(),
        }
    }
}

/// Spawn bundle for zombie entities (20 HP).
#[derive(Bundle)]
pub struct ZombieBundle {
    /// Living entity components.
    pub living: LivingEntityBundle,
    /// Zombie type marker.
    pub marker: ZombieMarker,
}

impl ZombieBundle {
    /// Creates a zombie bundle at the given position.
    pub fn new(pos: DVec3) -> Self {
        Self {
            living: LivingEntityBundle::new(pos, 20.0),
            marker: ZombieMarker,
        }
    }
}

/// Spawn bundle for skeleton entities (20 HP).
#[derive(Bundle)]
pub struct SkeletonBundle {
    /// Living entity components.
    pub living: LivingEntityBundle,
    /// Skeleton type marker.
    pub marker: SkeletonMarker,
}

impl SkeletonBundle {
    /// Creates a skeleton bundle at the given position.
    pub fn new(pos: DVec3) -> Self {
        Self {
            living: LivingEntityBundle::new(pos, 20.0),
            marker: SkeletonMarker,
        }
    }
}

/// Spawn bundle for creeper entities (20 HP).
#[derive(Bundle)]
pub struct CreeperBundle {
    /// Living entity components.
    pub living: LivingEntityBundle,
    /// Creeper type marker.
    pub marker: CreeperMarker,
}

impl CreeperBundle {
    /// Creates a creeper bundle at the given position.
    pub fn new(pos: DVec3) -> Self {
        Self {
            living: LivingEntityBundle::new(pos, 20.0),
            marker: CreeperMarker,
        }
    }
}

/// Spawn bundle for cow entities (10 HP).
#[derive(Bundle)]
pub struct CowBundle {
    /// Living entity components.
    pub living: LivingEntityBundle,
    /// Cow type marker.
    pub marker: CowMarker,
}

impl CowBundle {
    /// Creates a cow bundle at the given position.
    pub fn new(pos: DVec3) -> Self {
        Self {
            living: LivingEntityBundle::new(pos, 10.0),
            marker: CowMarker,
        }
    }
}

/// Spawn bundle for player entities (20 HP).
#[derive(Bundle)]
pub struct PlayerBundle {
    /// Living entity components.
    pub living: LivingEntityBundle,
    /// Player type marker.
    pub marker: PlayerMarker,
    /// Selected hotbar slot.
    pub selected_slot: SelectedSlot,
    /// Experience data.
    pub experience: ExperienceData,
}

impl PlayerBundle {
    /// Creates a player bundle at the given position.
    pub fn new(pos: DVec3) -> Self {
        Self {
            living: LivingEntityBundle::new(pos, 20.0),
            marker: PlayerMarker,
            selected_slot: SelectedSlot(0),
            experience: ExperienceData {
                level: 0,
                progress: 0.0,
                total: 0,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::entity::markers::CowMarker;

    #[test]
    fn test_base_bundle_creates_all_components() {
        let mut world = World::new();
        let entity = world
            .spawn(BaseEntityBundle::new(DVec3::new(1.0, 2.0, 3.0)))
            .id();

        assert!(world.get::<Position>(entity).is_some());
        assert!(world.get::<Velocity>(entity).is_some());
        assert!(world.get::<Rotation>(entity).is_some());
        assert!(world.get::<OnGround>(entity).is_some());
        assert!(world.get::<FallDistance>(entity).is_some());
        assert!(world.get::<EntityFlags>(entity).is_some());
        assert!(world.get::<TickCount>(entity).is_some());

        let pos = world.get::<Position>(entity).unwrap();
        assert_eq!(pos.0, DVec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_living_bundle_includes_base_and_health() {
        let mut world = World::new();
        let entity = world.spawn(LivingEntityBundle::new(DVec3::ZERO, 20.0)).id();

        // Base components
        assert!(world.get::<Position>(entity).is_some());
        assert!(world.get::<Velocity>(entity).is_some());

        // Living-specific
        let hp = world.get::<Health>(entity).unwrap();
        assert!((hp.current - 20.0).abs() < 1e-6);
        assert!((hp.max - 20.0).abs() < 1e-6);

        assert!(world.get::<ArmorValue>(entity).is_some());
        assert!(world.get::<AbsorptionAmount>(entity).is_some());
        assert!(world.get::<Equipment>(entity).is_some());
    }

    #[test]
    fn test_zombie_bundle_has_marker_and_living_components() {
        let mut world = World::new();
        let entity = world
            .spawn(ZombieBundle::new(DVec3::new(10.0, 64.0, 10.0)))
            .id();

        assert!(world.get::<ZombieMarker>(entity).is_some());
        assert!(world.get::<Position>(entity).is_some());
        assert!(world.get::<Health>(entity).is_some());

        let hp = world.get::<Health>(entity).unwrap();
        assert!((hp.current - 20.0).abs() < 1e-6);
    }

    #[test]
    fn test_cow_bundle_has_10_hp() {
        let mut world = World::new();
        let entity = world.spawn(CowBundle::new(DVec3::ZERO)).id();

        assert!(world.get::<CowMarker>(entity).is_some());

        let hp = world.get::<Health>(entity).unwrap();
        assert!((hp.current - 10.0).abs() < 1e-6);
        assert!((hp.max - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_player_bundle_has_player_components() {
        let mut world = World::new();
        let entity = world.spawn(PlayerBundle::new(DVec3::ZERO)).id();

        assert!(world.get::<PlayerMarker>(entity).is_some());
        assert!(world.get::<SelectedSlot>(entity).is_some());
        assert!(world.get::<ExperienceData>(entity).is_some());
        assert!(world.get::<Health>(entity).is_some());

        let slot = world.get::<SelectedSlot>(entity).unwrap();
        assert_eq!(slot.0, 0);
    }

    #[test]
    fn test_bundle_queries_with_markers() {
        let mut world = World::new();
        world.spawn(ZombieBundle::new(DVec3::ZERO));
        world.spawn(ZombieBundle::new(DVec3::new(5.0, 0.0, 0.0)));
        world.spawn(CowBundle::new(DVec3::new(10.0, 0.0, 0.0)));
        world.spawn(PlayerBundle::new(DVec3::new(0.0, 64.0, 0.0)));

        let mut zombie_q = world.query_filtered::<&Health, With<ZombieMarker>>();
        assert_eq!(zombie_q.iter(&world).count(), 2);

        let mut cow_q = world.query_filtered::<&Health, With<CowMarker>>();
        assert_eq!(cow_q.iter(&world).count(), 1);

        let mut player_q = world.query_filtered::<&Health, With<PlayerMarker>>();
        assert_eq!(player_q.iter(&world).count(), 1);

        // All living entities have health
        let mut all_health = world.query::<&Health>();
        assert_eq!(all_health.iter(&world).count(), 4);
    }

    #[test]
    fn test_skeleton_bundle() {
        let mut world = World::new();
        let entity = world
            .spawn(SkeletonBundle::new(DVec3::new(0.0, 70.0, 0.0)))
            .id();

        assert!(world.get::<SkeletonMarker>(entity).is_some());
        assert!(world.get::<Health>(entity).is_some());
    }

    #[test]
    fn test_creeper_bundle() {
        let mut world = World::new();
        let entity = world.spawn(CreeperBundle::new(DVec3::ZERO)).id();

        assert!(world.get::<CreeperMarker>(entity).is_some());
        assert!(world.get::<Health>(entity).is_some());
    }
}
