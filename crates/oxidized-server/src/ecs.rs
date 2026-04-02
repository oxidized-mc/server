//! ECS runtime context — owns the `bevy_ecs::World` and phase schedules.
//!
//! The [`EcsContext`] is created on startup and moved to the tick thread,
//! which has exclusive ownership. Network tasks communicate with it via
//! the [`EntityCommandSender`](oxidized_game::entity::commands::EntityCommandSender)
//! channel (ADR-020).
//!
//! `bevy_ecs::World` is `!Sync` — it cannot be shared behind `Arc`.
//! This matches ADR-019 (dedicated tick thread) and ADR-020 (channel bridge).

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use oxidized_game::entity::commands::EntityCommandReceiver;
use oxidized_game::entity::phases::TickPhase;
use oxidized_game::entity::systems::OutboundEntityPackets;
use oxidized_game::entity::tracker::EntityTracker;
use std::collections::HashMap;

/// ECS world and per-phase schedules, owned exclusively by the tick thread.
///
/// Created on startup, passed to [`run_tick_loop()`](crate::tick::run_tick_loop).
/// Network tasks never access this directly — they post commands via
/// [`EntityCommandSender`](oxidized_game::entity::commands::EntityCommandSender).
pub struct EcsContext {
    /// The single `bevy_ecs` world holding all entities.
    pub world: World,
    /// One schedule per tick phase, run sequentially.
    pub schedules: Vec<Schedule>,
}

impl EcsContext {
    /// Creates a new ECS context with an empty world and 7 phase schedules.
    ///
    /// Call this once at server startup, then pass to the tick thread.
    pub fn new(cmd_rx: EntityCommandReceiver) -> Self {
        let mut world = World::new();

        // Insert shared resources.
        world.insert_resource(CommandQueue(cmd_rx));
        world.insert_resource(PlayerEntityMap::default());
        world.insert_resource(OutboundEntityPackets::default());
        world.insert_resource(TrackerResource::default());

        let schedules = TickPhase::ALL.iter().map(|_| Schedule::default()).collect();

        Self { world, schedules }
    }

    /// Returns a mutable reference to the schedule for the given phase.
    pub fn schedule_mut(&mut self, phase: TickPhase) -> &mut Schedule {
        &mut self.schedules[phase as usize]
    }

    /// Runs all 7 phase schedules sequentially against the world.
    pub fn run_tick(&mut self) {
        for schedule in &mut self.schedules {
            schedule.run(&mut self.world);
        }
    }
}

/// Resource: holds the receiver end of the entity command channel.
#[derive(Resource)]
pub struct CommandQueue(pub EntityCommandReceiver);

/// Resource: maps player UUID → bevy Entity for fast lookup.
///
/// Populated by the `drain_entity_commands` system when `SpawnPlayer`
/// commands are processed. Used by movement sync and despawn.
#[derive(Resource, Default)]
pub struct PlayerEntityMap(pub HashMap<uuid::Uuid, Entity>);

/// Resource: wraps the existing [`EntityTracker`] for use in ECS systems.
///
/// Registered/unregistered in `drain_entity_commands` when entities
/// spawn/despawn. Updated per-tick in the `PostTick` phase.
#[derive(Resource)]
pub struct TrackerResource(pub EntityTracker);

impl Default for TrackerResource {
    fn default() -> Self {
        Self(EntityTracker::new())
    }
}

/// PreTick system: drains the command queue and applies entity mutations.
///
/// Processes all pending [`EntityCommand`]s from the channel, spawning/
/// despawning entities and updating component state. Uses `try_recv`
/// to drain without blocking.
#[allow(clippy::too_many_arguments)]
pub fn drain_entity_commands(
    mut commands: Commands,
    mut queue: ResMut<CommandQueue>,
    mut player_map: ResMut<PlayerEntityMap>,
    mut tracker: ResMut<TrackerResource>,
    network_ids: Query<&oxidized_game::entity::components::NetworkId>,
    mut positions: Query<&mut oxidized_game::entity::components::Position>,
    mut rotations: Query<&mut oxidized_game::entity::components::Rotation>,
    mut on_grounds: Query<&mut oxidized_game::entity::components::OnGround>,
    mut flags: Query<&mut oxidized_game::entity::components::EntityFlags>,
    mut selected_slots: Query<&mut oxidized_game::entity::components::SelectedSlot>,
) {
    use oxidized_game::entity::bundles::PlayerBundle;
    use oxidized_game::entity::commands::EntityCommand;
    use oxidized_game::entity::tracker::TRACKING_RANGE_PLAYER;

    while let Ok(cmd) = queue.0.try_recv() {
        match cmd {
            EntityCommand::SpawnPlayer {
                network_id,
                uuid,
                profile,
                position,
                rotation,
                game_mode,
                inventory,
                health,
                food_level,
                experience,
                spawn_data,
            } => {
                let bundle = PlayerBundle::from_spawn_data(
                    network_id, uuid, profile, position, rotation, game_mode, *inventory, health,
                    food_level, experience, spawn_data,
                );
                let entity = commands.spawn(bundle).id();
                player_map.0.insert(uuid, entity);
                tracker.0.register(network_id, TRACKING_RANGE_PLAYER);
                tracing::debug!(
                    %uuid, network_id, ?position,
                    "ECS: spawned player entity"
                );
            },
            EntityCommand::DespawnPlayer { uuid } => {
                if let Some(entity) = player_map.0.remove(&uuid) {
                    if let Ok(net_id) = network_ids.get(entity) {
                        tracker.0.unregister(net_id.0);
                    }
                    commands.entity(entity).despawn();
                    tracing::debug!(%uuid, "ECS: despawned player entity");
                }
            },
            EntityCommand::PlayerMoved {
                uuid,
                position,
                yaw,
                pitch,
                on_ground,
            } => {
                if let Some(&entity) = player_map.0.get(&uuid) {
                    if let Ok(mut pos) = positions.get_mut(entity) {
                        pos.0 = position;
                    }
                    if let Ok(mut rot) = rotations.get_mut(entity) {
                        rot.yaw = yaw;
                        rot.pitch = pitch;
                    }
                    if let Ok(mut og) = on_grounds.get_mut(entity) {
                        og.0 = on_ground;
                    }
                }
            },
            EntityCommand::PlayerAction {
                uuid,
                flags: new_flags,
            } => {
                if let Some(&entity) = player_map.0.get(&uuid) {
                    if let Ok(mut f) = flags.get_mut(entity) {
                        f.0 = new_flags;
                    }
                }
            },
            EntityCommand::SlotChanged { uuid, slot } => {
                if let Some(&entity) = player_map.0.get(&uuid) {
                    if let Ok(mut s) = selected_slots.get_mut(entity) {
                        s.0 = slot;
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use oxidized_game::entity::commands::entity_command_channel;
    use oxidized_game::entity::phases::TickPhase;

    #[test]
    fn test_ecs_context_creates_empty_world() {
        let (_tx, rx) = entity_command_channel(16);
        let ecs = EcsContext::new(rx);
        assert_eq!(ecs.world.entities().len(), 0);
    }

    #[test]
    fn test_ecs_context_has_7_schedules() {
        let (_tx, rx) = entity_command_channel(16);
        let ecs = EcsContext::new(rx);
        assert_eq!(ecs.schedules.len(), TickPhase::ALL.len());
    }

    #[test]
    fn test_ecs_context_has_resources() {
        let (_tx, rx) = entity_command_channel(16);
        let ecs = EcsContext::new(rx);
        assert!(ecs.world.get_resource::<CommandQueue>().is_some());
        assert!(ecs.world.get_resource::<PlayerEntityMap>().is_some());
        assert!(ecs.world.get_resource::<OutboundEntityPackets>().is_some());
    }

    #[test]
    fn test_run_tick_empty_world_no_panic() {
        let (_tx, rx) = entity_command_channel(16);
        let mut ecs = EcsContext::new(rx);
        ecs.run_tick(); // Should complete without panic.
    }

    #[test]
    fn test_drain_spawn_player() {
        use oxidized_game::entity::commands::EntityCommand;
        use oxidized_game::entity::components::{NetworkId, PlayerMarker, Position};

        let (tx, rx) = entity_command_channel(16);
        let mut ecs = EcsContext::new(rx);
        ecs.schedule_mut(TickPhase::PreTick)
            .add_systems(drain_entity_commands);

        let uuid = uuid::Uuid::new_v4();
        tx.try_send(EntityCommand::SpawnPlayer {
            network_id: 42,
            uuid,
            profile: oxidized_auth::GameProfile::new(uuid, "test".into()),
            position: glam::DVec3::new(0.0, 64.0, 0.0),
            rotation: (90.0, 0.0),
            game_mode: oxidized_game::player::GameMode::Survival,
            inventory: Box::new(oxidized_game::player::inventory::PlayerInventory::new()),
            health: 20.0,
            food_level: 20,
            experience: oxidized_game::entity::components::ExperienceData {
                level: 0,
                progress: 0.0,
                total: 0,
            },
            spawn_data: oxidized_game::entity::components::SpawnData {
                dimension: oxidized_mc_types::resource_location::ResourceLocation::minecraft(
                    "overworld",
                ),
                spawn_pos: oxidized_mc_types::BlockPos::new(0, 64, 0),
                spawn_angle: 0.0,
            },
        })
        .unwrap();

        ecs.run_tick();

        // Verify entity was spawned with all components populated.
        let map = ecs.world.resource::<PlayerEntityMap>();
        assert!(map.0.contains_key(&uuid));
        let entity = map.0[&uuid];
        assert_eq!(ecs.world.get::<NetworkId>(entity).unwrap().0, 42);
        assert!(ecs.world.get::<PlayerMarker>(entity).is_some());
        let pos = ecs.world.get::<Position>(entity).unwrap();
        assert!((pos.0.y - 64.0).abs() < 1e-10);

        // Verify player-specific components from SpawnPlayer command.
        let rot = ecs
            .world
            .get::<oxidized_game::entity::components::Rotation>(entity)
            .unwrap();
        assert!((rot.yaw - 90.0).abs() < 1e-6);

        let gm = ecs
            .world
            .get::<oxidized_game::entity::components::GameModeComponent>(entity)
            .unwrap();
        assert_eq!(gm.current, oxidized_game::player::GameMode::Survival);

        assert!(
            ecs.world
                .get::<oxidized_game::entity::components::Profile>(entity)
                .is_some()
        );
        assert!(
            ecs.world
                .get::<oxidized_game::entity::components::Inventory>(entity)
                .is_some()
        );
        assert!(
            ecs.world
                .get::<oxidized_game::entity::components::CombatData>(entity)
                .is_some()
        );
        assert!(
            ecs.world
                .get::<oxidized_game::entity::components::SpawnData>(entity)
                .is_some()
        );
    }

    #[test]
    fn test_drain_despawn_player() {
        use oxidized_game::entity::commands::EntityCommand;

        let (tx, rx) = entity_command_channel(16);
        let mut ecs = EcsContext::new(rx);
        ecs.schedule_mut(TickPhase::PreTick)
            .add_systems(drain_entity_commands);

        let uuid = uuid::Uuid::new_v4();

        // Spawn first.
        tx.try_send(EntityCommand::SpawnPlayer {
            network_id: 1,
            uuid,
            profile: oxidized_auth::GameProfile::new(uuid, "test".into()),
            position: glam::DVec3::ZERO,
            rotation: (0.0, 0.0),
            game_mode: oxidized_game::player::GameMode::Survival,
            inventory: Box::new(oxidized_game::player::inventory::PlayerInventory::new()),
            health: 20.0,
            food_level: 20,
            experience: oxidized_game::entity::components::ExperienceData {
                level: 0,
                progress: 0.0,
                total: 0,
            },
            spawn_data: oxidized_game::entity::components::SpawnData {
                dimension: oxidized_mc_types::resource_location::ResourceLocation::minecraft(
                    "overworld",
                ),
                spawn_pos: oxidized_mc_types::BlockPos::new(0, 64, 0),
                spawn_angle: 0.0,
            },
        })
        .unwrap();
        ecs.run_tick();

        assert!(
            ecs.world
                .resource::<PlayerEntityMap>()
                .0
                .contains_key(&uuid)
        );

        // Despawn.
        tx.try_send(EntityCommand::DespawnPlayer { uuid }).unwrap();
        ecs.run_tick();

        assert!(
            !ecs.world
                .resource::<PlayerEntityMap>()
                .0
                .contains_key(&uuid)
        );
    }

    #[test]
    fn test_drain_player_moved() {
        use oxidized_game::entity::commands::EntityCommand;
        use oxidized_game::entity::components::Position;

        let (tx, rx) = entity_command_channel(16);
        let mut ecs = EcsContext::new(rx);
        ecs.schedule_mut(TickPhase::PreTick)
            .add_systems(drain_entity_commands);

        let uuid = uuid::Uuid::new_v4();

        // Spawn player.
        tx.try_send(EntityCommand::SpawnPlayer {
            network_id: 1,
            uuid,
            profile: oxidized_auth::GameProfile::new(uuid, "test".into()),
            position: glam::DVec3::ZERO,
            rotation: (0.0, 0.0),
            game_mode: oxidized_game::player::GameMode::Survival,
            inventory: Box::new(oxidized_game::player::inventory::PlayerInventory::new()),
            health: 20.0,
            food_level: 20,
            experience: oxidized_game::entity::components::ExperienceData {
                level: 0,
                progress: 0.0,
                total: 0,
            },
            spawn_data: oxidized_game::entity::components::SpawnData {
                dimension: oxidized_mc_types::resource_location::ResourceLocation::minecraft(
                    "overworld",
                ),
                spawn_pos: oxidized_mc_types::BlockPos::new(0, 64, 0),
                spawn_angle: 0.0,
            },
        })
        .unwrap();
        ecs.run_tick();

        // Move player.
        tx.try_send(EntityCommand::PlayerMoved {
            uuid,
            position: glam::DVec3::new(100.0, 72.0, -50.0),
            yaw: 45.0,
            pitch: -10.0,
            on_ground: true,
        })
        .unwrap();
        ecs.run_tick();

        let entity = ecs.world.resource::<PlayerEntityMap>().0[&uuid];
        let pos = ecs.world.get::<Position>(entity).unwrap();
        assert!((pos.0.x - 100.0).abs() < 1e-10);
        assert!((pos.0.y - 72.0).abs() < 1e-10);
    }

    #[test]
    fn test_drain_empty_queue_noop() {
        let (_tx, rx) = entity_command_channel(16);
        let mut ecs = EcsContext::new(rx);
        ecs.schedule_mut(TickPhase::PreTick)
            .add_systems(drain_entity_commands);
        ecs.run_tick(); // No commands, should be no-op.
        assert_eq!(ecs.world.entities().len(), 0);
    }
}
