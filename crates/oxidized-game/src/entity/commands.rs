//! Entity command channel — network ↔ tick thread bridge.
//!
//! Network tasks **never** access `bevy_ecs::World` directly. Instead
//! they enqueue [`EntityCommand`] values which the tick thread drains
//! at the start of each tick during the [`PreTick`](super::phases::TickPhase::PreTick) phase.
//!
//! See [ADR-020](../../../docs/adr/adr-020-player-session.md) for the
//! split architecture rationale.

use glam::DVec3;
use tokio::sync::mpsc;
use uuid::Uuid;

use oxidized_protocol::auth::GameProfile;

use super::components::{ExperienceData, SpawnData};
use crate::player::GameMode;
use crate::player::inventory::PlayerInventory;

/// Commands sent from network tasks to the tick thread's ECS world.
///
/// Each variant represents a mutation that must be applied to the
/// `bevy_ecs::World` on the tick thread. The tick thread drains
/// the command channel once per tick in the `PreTick` phase.
#[derive(Debug)]
pub enum EntityCommand {
    /// A player connected — spawn their ECS entity.
    SpawnPlayer {
        /// Network entity ID (unique per session).
        network_id: i32,
        /// Player UUID.
        uuid: Uuid,
        /// Authenticated game profile.
        profile: GameProfile,
        /// Initial world position.
        position: DVec3,
        /// Initial (yaw, pitch).
        rotation: (f32, f32),
        /// Starting game mode.
        game_mode: GameMode,
        /// Player's inventory.
        inventory: Box<PlayerInventory>,
        /// Starting health.
        health: f32,
        /// Starting food level.
        food_level: i32,
        /// Experience data.
        experience: ExperienceData,
        /// Spawn point data.
        spawn_data: SpawnData,
    },
    /// A player disconnected — despawn their ECS entity.
    DespawnPlayer {
        /// Player UUID to despawn.
        uuid: Uuid,
    },
    /// Player moved (from `ServerboundMovePlayerPacket`).
    PlayerMoved {
        /// Player UUID.
        uuid: Uuid,
        /// New world position.
        position: DVec3,
        /// New yaw.
        yaw: f32,
        /// New pitch.
        pitch: f32,
        /// Whether the player is on the ground.
        on_ground: bool,
    },
    /// Player changed game state (sneak, sprint, etc.).
    PlayerAction {
        /// Player UUID.
        uuid: Uuid,
        /// New entity flags byte.
        flags: u8,
    },
    /// Player changed selected hotbar slot.
    SlotChanged {
        /// Player UUID.
        uuid: Uuid,
        /// New slot index (0–8).
        slot: u8,
    },
}

/// Sender half given to network tasks.
pub type EntityCommandSender = mpsc::Sender<EntityCommand>;

/// Receiver half owned by the tick thread.
pub type EntityCommandReceiver = mpsc::Receiver<EntityCommand>;

/// Default channel capacity (shared across all connections).
pub const DEFAULT_COMMAND_CHANNEL_CAPACITY: usize = 1024;

/// Creates a new entity command channel with the given capacity.
///
/// The [`EntityCommandSender`] is cloned into each connection's play loop.
/// The [`EntityCommandReceiver`] is owned by the tick thread and drained
/// in the `PreTick` phase.
pub fn entity_command_channel(capacity: usize) -> (EntityCommandSender, EntityCommandReceiver) {
    mpsc::channel(capacity)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use oxidized_protocol::auth::GameProfile;
    use oxidized_protocol::types::BlockPos;

    #[test]
    fn test_spawn_player_send_receive() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.block_on(async {
            let (tx, mut rx) = entity_command_channel(16);
            tx.send(EntityCommand::SpawnPlayer {
                network_id: 1,
                uuid: Uuid::nil(),
                profile: GameProfile::new(Uuid::nil(), "test".into()),
                position: DVec3::new(0.0, 64.0, 0.0),
                rotation: (0.0, 0.0),
                game_mode: GameMode::Survival,
                inventory: Box::new(PlayerInventory::new()),
                health: 20.0,
                food_level: 20,
                experience: ExperienceData {
                    level: 0,
                    progress: 0.0,
                    total: 0,
                },
                spawn_data: SpawnData {
                    dimension:
                        oxidized_protocol::types::resource_location::ResourceLocation::minecraft(
                            "overworld",
                        ),
                    spawn_pos: BlockPos::new(0, 64, 0),
                    spawn_angle: 0.0,
                },
            })
            .await
            .unwrap();

            let cmd = rx.recv().await.unwrap();
            assert!(matches!(
                cmd,
                EntityCommand::SpawnPlayer { network_id: 1, .. }
            ));
        });
    }

    #[test]
    fn test_all_command_variants_are_send() {
        fn assert_send<T: Send>() {}
        assert_send::<EntityCommand>();
    }

    #[test]
    fn test_channel_backpressure() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.block_on(async {
            let (tx, _rx) = entity_command_channel(1);
            // First send succeeds.
            tx.send(EntityCommand::DespawnPlayer { uuid: Uuid::nil() })
                .await
                .unwrap();
            // Second send would block (channel full) — use try_send.
            let result = tx.try_send(EntityCommand::DespawnPlayer { uuid: Uuid::nil() });
            assert!(result.is_err(), "Channel should be full");
        });
    }

    #[test]
    fn test_player_moved_command() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.block_on(async {
            let (tx, mut rx) = entity_command_channel(16);
            let uuid = Uuid::new_v4();
            tx.send(EntityCommand::PlayerMoved {
                uuid,
                position: DVec3::new(10.0, 65.0, -3.0),
                yaw: 90.0,
                pitch: -15.0,
                on_ground: true,
            })
            .await
            .unwrap();

            match rx.recv().await.unwrap() {
                EntityCommand::PlayerMoved {
                    uuid: recv_uuid,
                    position,
                    on_ground,
                    ..
                } => {
                    assert_eq!(recv_uuid, uuid);
                    assert!((position.x - 10.0).abs() < 1e-10);
                    assert!(on_ground);
                },
                _ => panic!("Expected PlayerMoved"),
            }
        });
    }
}
