//! Core ECS component types for entity representation.
//!
//! These components decompose vanilla's entity class hierarchy into
//! individual `bevy_ecs` components per ADR-018 (Entity System). Each field in vanilla's
//! `Entity`, `LivingEntity`, and `Player` classes maps to a named component.

use bevy_ecs::prelude::*;
use glam::DVec3;
use uuid::Uuid;

use crate::entity::synched_data::SynchedEntityData;
use crate::player::GameMode;
use oxidized_protocol::auth::GameProfile;
use oxidized_protocol::types::BlockPos;
use oxidized_protocol::types::aabb::Aabb;
use oxidized_protocol::types::resource_location::ResourceLocation;

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

impl EntityFlags {
    /// Returns `true` if the given flag bit is set.
    ///
    /// # Panics
    ///
    /// Debug-asserts that `bit < 8`.
    pub fn get(self, bit: u8) -> bool {
        debug_assert!(bit < 8, "flag bit index {bit} out of range 0..8");
        self.0 & (1 << bit) != 0
    }

    /// Sets or clears the given flag bit.
    ///
    /// # Panics
    ///
    /// Debug-asserts that `bit < 8`.
    pub fn set(&mut self, bit: u8, value: bool) {
        debug_assert!(bit < 8, "flag bit index {bit} out of range 0..8");
        if value {
            self.0 |= 1 << bit;
        } else {
            self.0 &= !(1 << bit);
        }
    }
}

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

// ---------------------------------------------------------------------------
// Entity identity (from Entity / ServerPlayer)
// ---------------------------------------------------------------------------

/// Network entity ID (unique per session, never recycled).
///
/// All entities get this — used in every network packet that references
/// an entity by ID.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkId(pub i32);

/// Entity UUID (persistent across sessions for players).
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct EntityUuid(pub Uuid);

/// Entity type (e.g., `minecraft:player`, `minecraft:zombie`).
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct EntityTypeName(pub ResourceLocation);

/// Player's authenticated game profile (name + UUID + properties).
#[derive(Component, Debug, Clone)]
pub struct Profile(pub GameProfile);

// ---------------------------------------------------------------------------
// Player game state (from ServerPlayer sub-structs)
// ---------------------------------------------------------------------------

/// Player's game mode (survival, creative, adventure, spectator).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameModeComponent {
    /// Current active game mode.
    pub current: GameMode,
    /// Previous game mode (for F3+N toggle).
    pub previous: Option<GameMode>,
}

/// Player abilities derived from game mode.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Abilities(pub crate::player::abilities::PlayerAbilities);

/// Player inventory (46 protocol slots).
#[derive(Component, Debug, Clone)]
pub struct Inventory(pub crate::player::inventory::PlayerInventory);

/// Combat stats: food, saturation, score, last death location.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct CombatData {
    /// Food level (0–20).
    pub food_level: i32,
    /// Saturation (hidden hunger buffer).
    pub food_saturation: f32,
    /// Player score (displayed on death screen).
    pub score: i32,
    /// Last death location (dimension, packed block pos).
    pub last_death_location: Option<(ResourceLocation, i64)>,
}

/// Player spawn point and current dimension.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct SpawnData {
    /// Dimension the player is in.
    pub dimension: ResourceLocation,
    /// Spawn point block position.
    pub spawn_pos: BlockPos,
    /// Spawn point yaw angle.
    pub spawn_angle: f32,
}

/// Skin model customisation byte (visible parts bitmask).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelCustomisation(pub u8);

// ---------------------------------------------------------------------------
// Entity physics (replacing Entity struct fields)
// ---------------------------------------------------------------------------

/// Axis-aligned bounding box for collision.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox(pub Aabb);

/// Entity hitbox width and height.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Dimensions {
    /// Width of the hitbox (meters).
    pub width: f32,
    /// Height of the hitbox (meters).
    pub height: f32,
}

/// Dirty-tracked entity data slots for network sync.
#[derive(Component)]
pub struct SynchedData(pub SynchedEntityData);

impl std::fmt::Debug for SynchedData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SynchedData")
            .field(&format_args!("({} slots)", self.0.len()))
            .finish()
    }
}

/// Marker indicating the entity has been scheduled for removal.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Removed;

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
    fn test_network_id_insert_and_query() {
        let mut world = World::new();
        let entity = world.spawn(NetworkId(42)).id();
        assert_eq!(world.get::<NetworkId>(entity).unwrap().0, 42);
    }

    #[test]
    fn test_entity_uuid_insert_and_query() {
        let mut world = World::new();
        let id = Uuid::new_v4();
        let entity = world.spawn(EntityUuid(id)).id();
        assert_eq!(world.get::<EntityUuid>(entity).unwrap().0, id);
    }

    #[test]
    fn test_game_mode_component() {
        let gm = GameModeComponent {
            current: GameMode::Survival,
            previous: Some(GameMode::Creative),
        };
        assert_eq!(gm.current, GameMode::Survival);
        assert_eq!(gm.previous, Some(GameMode::Creative));
    }

    #[test]
    fn test_combat_data_defaults() {
        let cd = CombatData {
            food_level: 20,
            food_saturation: 5.0,
            score: 0,
            last_death_location: None,
        };
        assert_eq!(cd.food_level, 20);
        assert!(cd.last_death_location.is_none());
    }

    #[test]
    fn test_bounding_box_component() {
        use oxidized_protocol::types::aabb::Aabb;
        let bbox = BoundingBox(Aabb::from_center(0.0, 0.0, 0.0, 0.6, 1.8));
        let mut world = World::new();
        let entity = world.spawn(bbox).id();
        assert!(world.get::<BoundingBox>(entity).is_some());
    }

    #[test]
    fn test_dimensions_component() {
        let dims = Dimensions {
            width: 0.6,
            height: 1.8,
        };
        assert!((dims.width - 0.6).abs() < 1e-6);
        assert!((dims.height - 1.8).abs() < 1e-6);
    }

    #[test]
    fn test_removed_marker() {
        let mut world = World::new();
        let entity = world.spawn(Removed).id();
        assert!(world.get::<Removed>(entity).is_some());
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
