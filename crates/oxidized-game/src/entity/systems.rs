//! ECS systems for each tick phase.
//!
//! These systems are registered in the appropriate [`PhaseSchedule`](super::phases::PhaseSchedule)
//! and run automatically each tick. Systems within a phase may execute
//! in parallel; phases execute sequentially.

use bevy_ecs::prelude::*;

use super::components::{
    BoundingBox, Dimensions, NetworkId, NoGravity, OnGround, Position, SynchedData, TickCount,
    Velocity,
};
use oxidized_protocol::packets::play::{ClientboundSetEntityDataPacket, EntityDataEntry};
use oxidized_protocol::types::aabb::Aabb;

use super::components::PlayerMarker;

/// Resource: outbound entity packets to be broadcast after the schedule runs.
///
/// Systems append `(entity_network_id, raw_packet_bytes)` tuples during
/// `NetworkSync`. The tick loop drains this after all phases complete.
#[derive(Resource, Default)]
pub struct OutboundEntityPackets(pub Vec<OutboundEntityPacket>);

/// A single outbound entity packet to broadcast.
#[derive(Debug)]
pub struct OutboundEntityPacket {
    /// Network entity ID this packet refers to.
    pub entity_id: i32,
    /// Pre-encoded packet bytes.
    pub data: bytes::Bytes,
    /// Packet ID for the broadcast message.
    pub packet_id: i32,
}

/// PreTick: increment tick count for all entities.
pub fn tick_count_system(mut query: Query<&mut TickCount>) {
    for mut tc in &mut query {
        tc.0 = tc.0.wrapping_add(1);
    }
}

/// Vanilla gravity constant (blocks/tick²).
const GRAVITY: f64 = 0.08;

/// Physics: apply gravity to all non-player entities without `NoGravity`.
///
/// Player entities skip gravity because their position is
/// server-authoritative (comes from client packets, not simulation).
pub fn gravity_system(
    mut query: Query<&mut Velocity, (Without<NoGravity>, Without<PlayerMarker>)>,
) {
    for mut vel in &mut query {
        vel.0.y -= GRAVITY;
    }
}

/// Physics: apply velocity to position (simplified — full collision
/// replaces this in later phases).
pub fn velocity_apply_system(
    mut query: Query<(&mut Position, &Velocity, &mut OnGround), Without<PlayerMarker>>,
) {
    for (mut pos, vel, mut on_ground) in &mut query {
        pos.0 += vel.0;
        // Simplified ground check — will be replaced with full AABB sweep.
        if pos.0.y < 0.0 {
            pos.0.y = 0.0;
            on_ground.0 = true;
        }
    }
}

/// PostTick: recalculate bounding box from position + dimensions.
///
/// Only runs on entities whose [`Position`] changed this tick.
pub fn bounding_box_update_system(
    mut query: Query<(&Position, &Dimensions, &mut BoundingBox), Changed<Position>>,
) {
    for (pos, dims, mut bbox) in &mut query {
        bbox.0 = Aabb::from_center(
            pos.0.x,
            pos.0.y,
            pos.0.z,
            f64::from(dims.width),
            f64::from(dims.height),
        );
    }
}

/// NetworkSync: serialize dirty entity data into outbound packets.
///
/// Checks each entity's [`SynchedData`] for dirty slots, packs them
/// into [`ClientboundSetEntityDataPacket`]s, and appends to the
/// [`OutboundEntityPackets`] resource for broadcast.
pub fn entity_data_sync_system(
    mut query: Query<(&NetworkId, &mut SynchedData)>,
    mut outbound: ResMut<OutboundEntityPackets>,
) {
    use oxidized_protocol::codec::Packet;

    for (net_id, mut synched) in &mut query {
        if synched.0.is_dirty() {
            let dirty_values = synched.0.pack_dirty();
            if dirty_values.is_empty() {
                continue;
            }
            let entries: Vec<EntityDataEntry> = dirty_values
                .iter()
                .map(|dv| EntityDataEntry {
                    slot: dv.slot,
                    serializer_type: dv.serializer_type as i32,
                    value_bytes: dv.encode_value(),
                })
                .collect();
            let pkt = ClientboundSetEntityDataPacket {
                entity_id: net_id.0,
                entries,
            };
            let encoded = pkt.encode();
            outbound.0.push(OutboundEntityPacket {
                entity_id: net_id.0,
                data: encoded.freeze(),
                packet_id: ClientboundSetEntityDataPacket::PACKET_ID,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use bevy_ecs::schedule::Schedule;
    use glam::DVec3;

    #[test]
    fn test_tick_count_increments() {
        let mut world = World::new();
        let entity = world.spawn(TickCount(0)).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(tick_count_system);
        schedule.run(&mut world);

        assert_eq!(world.get::<TickCount>(entity).unwrap().0, 1);
    }

    #[test]
    fn test_tick_count_wraps() {
        let mut world = World::new();
        let entity = world.spawn(TickCount(u32::MAX)).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(tick_count_system);
        schedule.run(&mut world);

        assert_eq!(world.get::<TickCount>(entity).unwrap().0, 0);
    }

    #[test]
    fn test_gravity_reduces_y_velocity() {
        let mut world = World::new();
        let entity = world.spawn(Velocity(DVec3::ZERO)).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(gravity_system);
        schedule.run(&mut world);

        let vel = world.get::<Velocity>(entity).unwrap();
        assert!((vel.0.y - (-GRAVITY)).abs() < 1e-10);
    }

    #[test]
    fn test_gravity_skips_no_gravity() {
        let mut world = World::new();
        let entity = world.spawn((Velocity(DVec3::ZERO), NoGravity)).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(gravity_system);
        schedule.run(&mut world);

        let vel = world.get::<Velocity>(entity).unwrap();
        assert!(
            (vel.0.y).abs() < 1e-10,
            "NoGravity entities should not be affected"
        );
    }

    #[test]
    fn test_gravity_skips_players() {
        let mut world = World::new();
        let entity = world.spawn((Velocity(DVec3::ZERO), PlayerMarker)).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(gravity_system);
        schedule.run(&mut world);

        let vel = world.get::<Velocity>(entity).unwrap();
        assert!(
            (vel.0.y).abs() < 1e-10,
            "Player entities should not have gravity applied"
        );
    }

    #[test]
    fn test_velocity_apply_moves_position() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Position(DVec3::new(0.0, 100.0, 0.0)),
                Velocity(DVec3::new(1.0, -0.08, 0.5)),
                OnGround(false),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(velocity_apply_system);
        schedule.run(&mut world);

        let pos = world.get::<Position>(entity).unwrap();
        assert!((pos.0.x - 1.0).abs() < 1e-10);
        assert!((pos.0.y - 99.92).abs() < 1e-10);
        assert!((pos.0.z - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_velocity_apply_ground_clamp() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Position(DVec3::new(0.0, 0.01, 0.0)),
                Velocity(DVec3::new(0.0, -1.0, 0.0)),
                OnGround(false),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(velocity_apply_system);
        schedule.run(&mut world);

        let pos = world.get::<Position>(entity).unwrap();
        assert!((pos.0.y).abs() < 1e-10, "Should clamp to y=0");
        assert!(world.get::<OnGround>(entity).unwrap().0);
    }

    #[test]
    fn test_bounding_box_update_on_position_change() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Position(DVec3::new(0.0, 64.0, 0.0)),
                Dimensions {
                    width: 0.6,
                    height: 1.8,
                },
                BoundingBox(Aabb::from_center(0.0, 64.0, 0.0, 0.6, 1.8)),
            ))
            .id();

        // First run to clear change detection flags.
        let mut schedule = Schedule::default();
        schedule.add_systems(bounding_box_update_system);
        schedule.run(&mut world);

        // Mutate position.
        world.get_mut::<Position>(entity).unwrap().0 = DVec3::new(10.0, 70.0, -5.0);

        schedule.run(&mut world);

        let bbox = world.get::<BoundingBox>(entity).unwrap();
        assert!(
            bbox.0.contains(10.0, 70.5, -5.0),
            "Bounding box should be re-centered on new position"
        );
    }

    #[test]
    fn test_full_pretick_physics_posttick_cycle() {
        let mut world = World::new();
        let entity = world
            .spawn((
                TickCount(0),
                Position(DVec3::new(0.0, 100.0, 0.0)),
                Velocity(DVec3::new(0.0, 0.0, 0.0)),
                OnGround(false),
                Dimensions {
                    width: 0.6,
                    height: 1.8,
                },
                BoundingBox(Aabb::from_center(0.0, 100.0, 0.0, 0.6, 1.8)),
            ))
            .id();

        // PreTick
        let mut pretick = Schedule::default();
        pretick.add_systems(tick_count_system);
        pretick.run(&mut world);

        // Physics
        let mut physics = Schedule::default();
        physics.add_systems((gravity_system, velocity_apply_system).chain());
        physics.run(&mut world);

        // PostTick
        let mut posttick = Schedule::default();
        posttick.add_systems(bounding_box_update_system);
        posttick.run(&mut world);

        assert_eq!(world.get::<TickCount>(entity).unwrap().0, 1);
        let pos = world.get::<Position>(entity).unwrap();
        assert!(pos.0.y < 100.0, "Entity should have fallen due to gravity");
    }

    #[test]
    fn test_entity_data_sync_dirty() {
        use crate::entity::synched_data::{DataSerializerType, SynchedEntityData};

        let mut world = World::new();
        world.init_resource::<OutboundEntityPackets>();

        let mut synched = SynchedEntityData::new();
        synched.define(0, DataSerializerType::Byte, 0u8);
        synched.set(0u8, 42u8); // mark dirty

        world.spawn((NetworkId(1), SynchedData(synched)));

        let mut schedule = Schedule::default();
        schedule.add_systems(entity_data_sync_system);
        schedule.run(&mut world);

        let outbound = world.resource::<OutboundEntityPackets>();
        assert_eq!(outbound.0.len(), 1);
        assert_eq!(outbound.0[0].entity_id, 1);
    }

    #[test]
    fn test_entity_data_sync_clean() {
        use crate::entity::synched_data::{DataSerializerType, SynchedEntityData};

        let mut world = World::new();
        world.init_resource::<OutboundEntityPackets>();

        let mut synched = SynchedEntityData::new();
        synched.define(0, DataSerializerType::Byte, 0u8);
        // Not dirty — default value

        world.spawn((NetworkId(1), SynchedData(synched)));

        let mut schedule = Schedule::default();
        schedule.add_systems(entity_data_sync_system);
        schedule.run(&mut world);

        let outbound = world.resource::<OutboundEntityPackets>();
        assert!(
            outbound.0.is_empty(),
            "Clean data should produce no packets"
        );
    }
}
