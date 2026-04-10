//! Spawn template bundles for common entity archetypes.
//!
//! Bundles group the components that every entity of a given type must
//! have at spawn time. Using `world.spawn(ZombieBundle::new(...))` ensures
//! all required components are present without forgetting any.

use bevy_ecs::prelude::*;
use glam::DVec3;
use uuid::Uuid;

use super::components::{
    Abilities, AbsorptionAmount, ArmorValue, BoundingBox, CombatData, Dimensions, EntityFlags,
    EntityTypeName, EntityUuid, Equipment, ExperienceData, FallDistance, GameModeComponent, Health,
    Inventory, ModelCustomisation, NetworkId, OnGround, PlayerMarker, Position, Profile, Rotation,
    SelectedSlot, SpawnData, SynchedData, TickCount, Velocity,
};
use super::data_slots::*;
use super::markers::{CowMarker, CreeperMarker, SkeletonMarker, ZombieMarker};
use super::synched_data::{DataSerializerType, SynchedEntityData};
use oxidized_mc_types::aabb::Aabb;
use oxidized_mc_types::resource_location::ResourceLocation;

/// Creates a base `SynchedEntityData` with the 8 standard entity data
/// slots (matching `Entity.java` constructor).
fn base_synched_data() -> SynchedEntityData {
    let mut data = SynchedEntityData::new();
    data.define(DATA_SHARED_FLAGS, DataSerializerType::Byte, 0u8);
    data.define(DATA_AIR_SUPPLY, DataSerializerType::Int, 300i32);
    data.define(
        DATA_CUSTOM_NAME,
        DataSerializerType::OptionalComponent,
        None::<String>,
    );
    data.define(DATA_CUSTOM_NAME_VISIBLE, DataSerializerType::Boolean, false);
    data.define(DATA_SILENT, DataSerializerType::Boolean, false);
    data.define(DATA_NO_GRAVITY, DataSerializerType::Boolean, false);
    data.define(DATA_POSE, DataSerializerType::Pose, 0i32);
    data.define(DATA_TICKS_FROZEN, DataSerializerType::Int, 0i32);
    data
}

/// Components shared by every entity type.
///
/// Mirrors the fields defined in vanilla's `Entity` base class.
/// Includes identity (`NetworkId`, `EntityUuid`, `EntityTypeName`),
/// spatial (`Position`, `Velocity`, `Rotation`), and metadata components.
#[derive(Bundle)]
pub struct BaseEntityBundle {
    /// Network entity ID.
    pub network_id: NetworkId,
    /// Global UUID.
    pub uuid: EntityUuid,
    /// Entity type identifier.
    pub entity_type: EntityTypeName,
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
    /// Collision bounding box.
    pub bounding_box: BoundingBox,
    /// Hitbox dimensions.
    pub dimensions: Dimensions,
    /// Synched entity data for network serialisation.
    pub synched_data: SynchedData,
}

impl BaseEntityBundle {
    /// Creates a base bundle at the given position with default state.
    ///
    /// Allocates a new entity ID, generates a random UUID, and defines
    /// the 8 base synched data slots.
    pub fn new(entity_type: ResourceLocation, pos: DVec3, width: f32, height: f32) -> Self {
        Self {
            network_id: NetworkId(super::id::next_entity_id()),
            uuid: EntityUuid(Uuid::new_v4()),
            entity_type: EntityTypeName(entity_type),
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
            bounding_box: BoundingBox(Aabb::from_center(
                pos.x,
                pos.y,
                pos.z,
                f64::from(width),
                f64::from(height),
            )),
            dimensions: Dimensions { width, height },
            synched_data: SynchedData(base_synched_data()),
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
    /// Creates a living entity bundle with the given type, position, dimensions, and max health.
    pub fn new(
        entity_type: ResourceLocation,
        pos: DVec3,
        width: f32,
        height: f32,
        max_health: f32,
    ) -> Self {
        Self {
            base: BaseEntityBundle::new(entity_type, pos, width, height),
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
            living: LivingEntityBundle::new(
                ResourceLocation::minecraft("zombie"),
                pos,
                0.6,
                1.95,
                20.0,
            ),
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
            living: LivingEntityBundle::new(
                ResourceLocation::minecraft("skeleton"),
                pos,
                0.6,
                1.99,
                20.0,
            ),
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
            living: LivingEntityBundle::new(
                ResourceLocation::minecraft("creeper"),
                pos,
                0.6,
                1.7,
                20.0,
            ),
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
            living: LivingEntityBundle::new(
                ResourceLocation::minecraft("cow"),
                pos,
                0.9,
                1.4,
                10.0,
            ),
            marker: CowMarker,
        }
    }
}

/// Spawn bundle for player entities (20 HP).
///
/// Contains every component a connected player needs. After spawning,
/// the entity's [`NetworkId`] and [`EntityUuid`] are used by the
/// network bridge to route packets.
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
    /// Authenticated game profile.
    pub profile: Profile,
    /// Current game mode.
    pub game_mode: GameModeComponent,
    /// Game-mode-derived abilities.
    pub abilities: Abilities,
    /// Player inventory.
    pub inventory: Inventory,
    /// Combat stats (food, saturation, score).
    pub combat: CombatData,
    /// Spawn point data.
    pub spawn_data: SpawnData,
    /// Skin model customisation bitmask.
    pub model_customisation: ModelCustomisation,
}

impl PlayerBundle {
    /// Creates a player bundle at the given position with default state.
    pub fn new(pos: DVec3) -> Self {
        use crate::player::GameType;
        use crate::player::abilities::PlayerAbilities;
        use crate::player::inventory::PlayerInventory;
        use oxidized_auth::GameProfile;
        use oxidized_mc_types::BlockPos;

        Self {
            living: LivingEntityBundle::new(
                ResourceLocation::minecraft("player"),
                pos,
                0.6,
                1.8,
                20.0,
            ),
            marker: PlayerMarker,
            selected_slot: SelectedSlot(0),
            experience: ExperienceData {
                level: 0,
                progress: 0.0,
                total: 0,
            },
            profile: Profile(GameProfile::new(Uuid::new_v4(), String::new())),
            game_mode: GameModeComponent {
                current: GameType::Survival,
                previous: None,
            },
            abilities: Abilities(PlayerAbilities::for_game_mode(GameType::Survival)),
            inventory: Inventory(PlayerInventory::new()),
            combat: CombatData {
                food_level: 20,
                food_saturation: 5.0,
                score: 0,
                last_death_location: None,
            },
            spawn_data: SpawnData {
                dimension: ResourceLocation::minecraft("overworld"),
                spawn_pos: BlockPos::new(0, 64, 0),
                spawn_angle: 0.0,
            },
            model_customisation: ModelCustomisation(0),
        }
    }

    /// Creates a player bundle with a specific network ID and UUID.
    ///
    /// Used during player join when we already have the allocated entity
    /// ID and authenticated profile UUID.
    pub fn with_identity(pos: DVec3, network_id: i32, uuid: Uuid) -> Self {
        let mut bundle = Self::new(pos);
        bundle.living.base.network_id = NetworkId(network_id);
        bundle.living.base.uuid = EntityUuid(uuid);
        bundle
    }

    /// Creates a player bundle from all the data in a `SpawnPlayer` command.
    ///
    /// This is the primary constructor used by [`drain_entity_commands`] when
    /// processing a `SpawnPlayer` command from the network layer.
    #[allow(clippy::too_many_arguments)]
    pub fn from_spawn_data(
        network_id: i32,
        uuid: Uuid,
        profile: oxidized_auth::GameProfile,
        position: DVec3,
        rotation: (f32, f32),
        game_mode: crate::player::GameType,
        inventory: crate::player::inventory::PlayerInventory,
        health: f32,
        food_level: i32,
        experience: ExperienceData,
        spawn_data: SpawnData,
    ) -> Self {
        use crate::player::abilities::PlayerAbilities;

        let mut bundle = Self::new(position);
        bundle.living.base.network_id = NetworkId(network_id);
        bundle.living.base.uuid = EntityUuid(uuid);
        bundle.living.base.rotation = Rotation {
            yaw: rotation.0,
            pitch: rotation.1,
        };
        bundle.living.health = Health {
            current: health,
            max: 20.0,
        };
        bundle.profile = Profile(profile);
        bundle.game_mode = GameModeComponent {
            current: game_mode,
            previous: None,
        };
        bundle.abilities = Abilities(PlayerAbilities::for_game_mode(game_mode));
        bundle.inventory = Inventory(inventory);
        bundle.combat = CombatData {
            food_level,
            food_saturation: 5.0,
            score: 0,
            last_death_location: None,
        };
        bundle.experience = experience;
        bundle.spawn_data = spawn_data;
        bundle
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::entity::markers::CowMarker;
    use oxidized_mc_types::resource_location::ResourceLocation;

    #[test]
    fn test_base_bundle_creates_all_components() {
        let mut world = World::new();
        let entity = world
            .spawn(BaseEntityBundle::new(
                ResourceLocation::minecraft("cow"),
                DVec3::new(1.0, 2.0, 3.0),
                0.9,
                1.4,
            ))
            .id();

        assert!(world.get::<Position>(entity).is_some());
        assert!(world.get::<Velocity>(entity).is_some());
        assert!(world.get::<Rotation>(entity).is_some());
        assert!(world.get::<OnGround>(entity).is_some());
        assert!(world.get::<FallDistance>(entity).is_some());
        assert!(world.get::<EntityFlags>(entity).is_some());
        assert!(world.get::<TickCount>(entity).is_some());
        assert!(world.get::<NetworkId>(entity).is_some());
        assert!(world.get::<EntityUuid>(entity).is_some());
        assert!(world.get::<EntityTypeName>(entity).is_some());
        assert!(world.get::<BoundingBox>(entity).is_some());
        assert!(world.get::<Dimensions>(entity).is_some());
        assert!(world.get::<SynchedData>(entity).is_some());

        let pos = world.get::<Position>(entity).unwrap();
        assert_eq!(pos.0, DVec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_living_bundle_includes_base_and_health() {
        let mut world = World::new();
        let entity = world
            .spawn(LivingEntityBundle::new(
                ResourceLocation::minecraft("zombie"),
                DVec3::ZERO,
                0.6,
                1.95,
                20.0,
            ))
            .id();

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
        assert!(world.get::<Profile>(entity).is_some());
        assert!(world.get::<GameModeComponent>(entity).is_some());
        assert!(world.get::<Abilities>(entity).is_some());
        assert!(world.get::<Inventory>(entity).is_some());
        assert!(world.get::<CombatData>(entity).is_some());
        assert!(world.get::<SpawnData>(entity).is_some());
        assert!(world.get::<ModelCustomisation>(entity).is_some());

        let slot = world.get::<SelectedSlot>(entity).unwrap();
        assert_eq!(slot.0, 0);

        let gm = world.get::<GameModeComponent>(entity).unwrap();
        assert_eq!(gm.current, crate::player::GameType::Survival);
        assert!(gm.previous.is_none());
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

    #[test]
    fn test_player_with_identity() {
        let mut world = World::new();
        let uuid = uuid::Uuid::new_v4();
        let entity = world
            .spawn(PlayerBundle::with_identity(
                DVec3::new(0.0, 64.0, 0.0),
                42,
                uuid,
            ))
            .id();

        assert_eq!(world.get::<NetworkId>(entity).unwrap().0, 42);
        assert_eq!(world.get::<EntityUuid>(entity).unwrap().0, uuid);
        assert!(world.get::<PlayerMarker>(entity).is_some());
    }

    #[test]
    fn test_base_bundle_has_synched_data_with_8_slots() {
        let mut world = World::new();
        let entity = world
            .spawn(BaseEntityBundle::new(
                ResourceLocation::minecraft("pig"),
                DVec3::ZERO,
                0.6,
                0.9,
            ))
            .id();

        let data = world.get::<SynchedData>(entity).unwrap();
        assert_eq!(data.0.len(), 8);
    }

    #[test]
    fn test_player_from_spawn_data() {
        use crate::player::GameType;
        use crate::player::inventory::PlayerInventory;
        use oxidized_auth::GameProfile;
        use oxidized_mc_types::BlockPos;

        let mut world = World::new();
        let uuid = uuid::Uuid::new_v4();
        let profile = GameProfile::new(uuid, "Steve".into());
        let entity = world
            .spawn(PlayerBundle::from_spawn_data(
                42,
                uuid,
                profile,
                DVec3::new(100.0, 72.0, -50.0),
                (90.0, -15.0),
                GameType::Creative,
                PlayerInventory::new(),
                18.5,
                15,
                ExperienceData {
                    level: 5,
                    progress: 0.3,
                    total: 250,
                },
                SpawnData {
                    dimension: ResourceLocation::minecraft("overworld"),
                    spawn_pos: BlockPos::new(0, 64, 0),
                    spawn_angle: 0.0,
                },
            ))
            .id();

        assert_eq!(world.get::<NetworkId>(entity).unwrap().0, 42);
        assert_eq!(world.get::<EntityUuid>(entity).unwrap().0, uuid);

        let rot = world.get::<Rotation>(entity).unwrap();
        assert!((rot.yaw - 90.0).abs() < 1e-6);
        assert!((rot.pitch - (-15.0)).abs() < 1e-6);

        let hp = world.get::<Health>(entity).unwrap();
        assert!((hp.current - 18.5).abs() < 1e-6);

        let gm = world.get::<GameModeComponent>(entity).unwrap();
        assert_eq!(gm.current, GameType::Creative);

        let ab = world.get::<Abilities>(entity).unwrap();
        assert!(ab.0.can_fly);
        assert!(ab.0.is_instabuild);

        let xp = world.get::<ExperienceData>(entity).unwrap();
        assert_eq!(xp.level, 5);
        assert_eq!(xp.total, 250);

        let combat = world.get::<CombatData>(entity).unwrap();
        assert_eq!(combat.food_level, 15);

        let p = world.get::<Profile>(entity).unwrap();
        assert_eq!(p.0.name(), "Steve");
    }
}
