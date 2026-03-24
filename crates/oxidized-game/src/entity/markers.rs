//! Entity-type marker components.
//!
//! Each vanilla entity type gets a zero-sized marker component so that
//! systems can filter queries with `With<ZombieMarker>`, `With<CowMarker>`,
//! etc. This replaces vanilla's class hierarchy for type dispatch.
//!
//! **This is scaffolding only.** Marker components are defined here for
//! the most common vanilla entity types. Additional markers will be added
//! as entity types are implemented in feature phases.

use bevy_ecs::prelude::*;

// ---------------------------------------------------------------------------
// Hostile mobs
// ---------------------------------------------------------------------------

/// Marker for zombie entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZombieMarker;

/// Marker for skeleton entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkeletonMarker;

/// Marker for creeper entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreeperMarker;

/// Marker for spider entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpiderMarker;

/// Marker for enderman entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndermanMarker;

/// Marker for slime entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlimeMarker;

/// Marker for phantom entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhantomMarker;

/// Marker for drowned entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrownedMarker;

/// Marker for witch entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WitchMarker;

// ---------------------------------------------------------------------------
// Passive mobs
// ---------------------------------------------------------------------------

/// Marker for villager entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct VillagerMarker;

/// Marker for chicken entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChickenMarker;

/// Marker for cow entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CowMarker;

/// Marker for pig entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PigMarker;

/// Marker for sheep entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SheepMarker;

/// Marker for horse entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct HorseMarker;

/// Marker for wolf entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WolfMarker;

/// Marker for cat entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatMarker;

/// Marker for rabbit entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RabbitMarker;

/// Marker for iron golem entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct IronGolemMarker;

// ---------------------------------------------------------------------------
// Misc entities
// ---------------------------------------------------------------------------

/// Marker for item entities (dropped items).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemEntityMarker;

/// Marker for experience orb entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExperienceOrbMarker;

/// Marker for arrow entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrowMarker;

/// Marker for falling block entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FallingBlockMarker;

/// Marker for TNT entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TntMarker;

/// Marker for boat entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoatMarker;

/// Marker for minecart entities.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinecartMarker;

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::entity::components::{Health, OnGround, Position};
    use glam::DVec3;

    #[test]
    fn test_marker_filters_correct_subset() {
        let mut world = World::new();

        // Spawn 2 zombies and 1 cow
        world.spawn((ZombieMarker, Position(DVec3::ZERO)));
        world.spawn((ZombieMarker, Position(DVec3::new(10.0, 0.0, 0.0))));
        world.spawn((CowMarker, Position(DVec3::new(20.0, 0.0, 0.0))));

        let mut zombie_q = world.query_filtered::<&Position, With<ZombieMarker>>();
        let zombies: Vec<_> = zombie_q.iter(&world).collect();
        assert_eq!(zombies.len(), 2);

        let mut cow_q = world.query_filtered::<&Position, With<CowMarker>>();
        let cows: Vec<_> = cow_q.iter(&world).collect();
        assert_eq!(cows.len(), 1);
    }

    #[test]
    fn test_without_filter_excludes_marker() {
        let mut world = World::new();
        world.spawn((ZombieMarker, OnGround(true)));
        world.spawn((CowMarker, OnGround(true)));
        world.spawn((SkeletonMarker, OnGround(false)));

        let mut q = world.query_filtered::<&OnGround, Without<ZombieMarker>>();
        let non_zombies: Vec<_> = q.iter(&world).collect();
        assert_eq!(non_zombies.len(), 2);
    }

    #[test]
    fn test_marker_with_living_components() {
        let mut world = World::new();
        let entity = world
            .spawn((
                ZombieMarker,
                Position(DVec3::ZERO),
                Health {
                    current: 20.0,
                    max: 20.0,
                },
            ))
            .id();

        assert!(world.get::<ZombieMarker>(entity).is_some());
        assert!(world.get::<Health>(entity).is_some());
    }

    #[test]
    fn test_multiple_markers_are_distinct_archetypes() {
        let mut world = World::new();
        world.spawn(ZombieMarker);
        world.spawn(SkeletonMarker);
        world.spawn(CreeperMarker);
        world.spawn(SpiderMarker);

        let mut q = world.query_filtered::<Entity, With<ZombieMarker>>();
        assert_eq!(q.iter(&world).count(), 1);

        let mut q = world.query_filtered::<Entity, With<SkeletonMarker>>();
        assert_eq!(q.iter(&world).count(), 1);
    }
}
