//! Movement packet handling and chunk boundary tracking.
//!
//! Processes position/rotation updates from the client, validates them,
//! broadcasts position changes to other players, and sends/forgets chunks
//! when the player crosses chunk boundaries.

use std::sync::Arc;

use oxidized_game::player::movement::validate_movement;
use oxidized_protocol::codec::Packet;
use oxidized_protocol::connection::ConnectionError;
use oxidized_protocol::packets::play::{
    ClientboundChunkBatchFinishedPacket, ClientboundChunkBatchStartPacket,
    ClientboundForgetLevelChunkPacket, ClientboundLevelChunkWithLightPacket,
    ClientboundMoveEntityPosPacket, ClientboundMoveEntityPosRotPacket,
    ClientboundMoveEntityRotPacket, ClientboundPlayerPositionPacket, ClientboundRotateHeadPacket,
    ClientboundSetChunkCacheCenterPacket, RelativeFlags, ServerboundMovePlayerPacket,
    ServerboundMovePlayerPosPacket, ServerboundMovePlayerPosRotPacket,
    ServerboundMovePlayerRotPacket, ServerboundMovePlayerStatusOnlyPacket,
};
use oxidized_world::chunk::ChunkPos;
use parking_lot::RwLock;
use tracing::{debug, trace};

use oxidized_game::net::chunk_serializer::build_chunk_packet;
use oxidized_game::net::entity_movement::{EntityMoveKind, classify_move, pack_degrees};

use super::PlayContext;
use crate::network::BroadcastMessage;

/// Handles a movement packet (position, rotation, or both).
pub async fn handle_movement(
    ctx: &mut PlayContext<'_>,
    packet_id: i32,
    data: bytes::Bytes,
) -> Result<(), ConnectionError> {
    let decode_result: Result<ServerboundMovePlayerPacket, _> = match packet_id {
        ServerboundMovePlayerPosPacket::PACKET_ID => {
            ServerboundMovePlayerPosPacket::decode(data).map(Into::into)
        },
        ServerboundMovePlayerPosRotPacket::PACKET_ID => {
            ServerboundMovePlayerPosRotPacket::decode(data).map(Into::into)
        },
        ServerboundMovePlayerRotPacket::PACKET_ID => {
            ServerboundMovePlayerRotPacket::decode(data).map(Into::into)
        },
        _ => ServerboundMovePlayerStatusOnlyPacket::decode(data).map(Into::into),
    };

    let move_pkt = match decode_result {
        Ok(pkt) => pkt,
        Err(e) => {
            debug!(
                peer = %ctx.addr,
                name = %ctx.player_name,
                error = %e,
                "Failed to decode MovePlayer",
            );
            return Ok(());
        },
    };

    if move_pkt.contains_invalid_values() {
        debug!(peer = %ctx.addr, name = %ctx.player_name, "Movement packet contains invalid values");
        return Ok(());
    }

    let (old_pos, old_yaw, old_pitch, entity_id) = {
        let p = ctx.player.read();
        (p.pos, p.yaw, p.pitch, p.entity_id)
    };

    let result = {
        validate_movement(
            old_pos,
            old_yaw,
            old_pitch,
            move_pkt.x,
            move_pkt.y,
            move_pkt.z,
            move_pkt.yaw,
            move_pkt.pitch,
            false, // TODO: pass actual elytra state once tracked
        )
    };

    if result.needs_correction {
        let correction = {
            let mut p = ctx.player.write();
            let teleport_id = p.next_teleport_id();
            p.pending_teleports.push_back(teleport_id);
            ClientboundPlayerPositionPacket {
                teleport_id,
                x: p.pos.x,
                y: p.pos.y,
                z: p.pos.z,
                dx: 0.0,
                dy: 0.0,
                dz: 0.0,
                yaw: p.yaw,
                pitch: p.pitch,
                relative_flags: RelativeFlags::empty(),
            }
        };
        ctx.conn.send_packet(&correction).await?;
        debug!(peer = %ctx.addr, name = %ctx.player_name, "Position correction sent");
    } else {
        {
            let mut p = ctx.player.write();
            p.pos = result.new_pos;
            p.yaw = result.new_yaw;
            p.pitch = result.new_pitch;
            p.on_ground = move_pkt.on_ground;
        }

        trace!(
            peer = %ctx.addr,
            name = %ctx.player_name,
            x = result.new_pos.x,
            y = result.new_pos.y,
            z = result.new_pos.z,
            on_ground = move_pkt.on_ground,
            "Position updated",
        );

        // Broadcast movement to other players.
        let has_pos = move_pkt.has_pos();
        let has_rot = move_pkt.has_rot();
        broadcast_movement(
            ctx,
            entity_id,
            old_pos,
            result.new_pos,
            old_yaw,
            old_pitch,
            result.new_yaw,
            result.new_pitch,
            move_pkt.on_ground,
            has_pos,
            has_rot,
        );

        // Check if player crossed a chunk boundary.
        if has_pos {
            let new_chunk = ChunkPos::from_block_coords(
                result.new_pos.x.floor() as i32,
                result.new_pos.z.floor() as i32,
            );
            let (to_load, to_unload) = ctx.chunk_tracker.update_center(new_chunk);

            if !to_load.is_empty() || !to_unload.is_empty() {
                send_chunk_updates(ctx, new_chunk, &to_load, &to_unload).await?;
            }
        }
    }

    Ok(())
}

/// Broadcasts a player's movement to all other players.
///
/// Uses delta-encoded movement packets when the delta fits in `i16`
/// (~8 blocks). For larger movements, falls back to a full entity
/// position sync. Head rotation is always sent when yaw changes.
#[allow(clippy::too_many_arguments)]
fn broadcast_movement(
    ctx: &PlayContext<'_>,
    entity_id: i32,
    old_pos: oxidized_protocol::types::Vec3,
    new_pos: oxidized_protocol::types::Vec3,
    _old_yaw: f32,
    _old_pitch: f32,
    new_yaw: f32,
    new_pitch: f32,
    on_ground: bool,
    has_pos: bool,
    has_rot: bool,
) {
    let pos_changed =
        has_pos && (old_pos.x != new_pos.x || old_pos.y != new_pos.y || old_pos.z != new_pos.z);
    let rot_changed = has_rot;

    if !pos_changed && !rot_changed {
        return;
    }

    let packed_yaw = pack_degrees(new_yaw);
    let packed_pitch = pack_degrees(new_pitch);

    if pos_changed && rot_changed {
        let move_kind = classify_move(
            old_pos.x, old_pos.y, old_pos.z, new_pos.x, new_pos.y, new_pos.z,
        );
        match move_kind {
            EntityMoveKind::Delta { dx, dy, dz } => {
                let pkt = ClientboundMoveEntityPosRotPacket {
                    entity_id,
                    dx,
                    dy,
                    dz,
                    yaw: packed_yaw,
                    pitch: packed_pitch,
                    on_ground,
                };
                ctx.server_ctx.broadcast(BroadcastMessage {
                    packet_id: ClientboundMoveEntityPosRotPacket::PACKET_ID,
                    data: pkt.encode().freeze(),
                    exclude_entity: Some(entity_id),
                    target_entity: None,
                });
            },
            EntityMoveKind::Sync { x, y, z } => {
                // Delta too large — send full position sync instead.
                let pkt = oxidized_protocol::packets::play::ClientboundEntityPositionSyncPacket {
                    entity_id,
                    x,
                    y,
                    z,
                    vx: 0.0,
                    vy: 0.0,
                    vz: 0.0,
                    yaw: new_yaw,
                    pitch: new_pitch,
                    on_ground,
                };
                ctx.server_ctx.broadcast(BroadcastMessage {
                    packet_id: oxidized_protocol::packets::play::ClientboundEntityPositionSyncPacket::PACKET_ID,
                    data: pkt.encode().freeze(),
                    exclude_entity: Some(entity_id),
                    target_entity: None,
                });
            },
        }
    } else if pos_changed {
        let move_kind = classify_move(
            old_pos.x, old_pos.y, old_pos.z, new_pos.x, new_pos.y, new_pos.z,
        );
        match move_kind {
            EntityMoveKind::Delta { dx, dy, dz } => {
                let pkt = ClientboundMoveEntityPosPacket {
                    entity_id,
                    dx,
                    dy,
                    dz,
                    on_ground,
                };
                ctx.server_ctx.broadcast(BroadcastMessage {
                    packet_id: ClientboundMoveEntityPosPacket::PACKET_ID,
                    data: pkt.encode().freeze(),
                    exclude_entity: Some(entity_id),
                    target_entity: None,
                });
            },
            EntityMoveKind::Sync { x, y, z } => {
                let pkt = oxidized_protocol::packets::play::ClientboundEntityPositionSyncPacket {
                    entity_id,
                    x,
                    y,
                    z,
                    vx: 0.0,
                    vy: 0.0,
                    vz: 0.0,
                    yaw: new_yaw,
                    pitch: new_pitch,
                    on_ground,
                };
                ctx.server_ctx.broadcast(BroadcastMessage {
                    packet_id: oxidized_protocol::packets::play::ClientboundEntityPositionSyncPacket::PACKET_ID,
                    data: pkt.encode().freeze(),
                    exclude_entity: Some(entity_id),
                    target_entity: None,
                });
            },
        }
    } else {
        let pkt = ClientboundMoveEntityRotPacket {
            entity_id,
            yaw: packed_yaw,
            pitch: packed_pitch,
            on_ground,
        };
        ctx.server_ctx.broadcast(BroadcastMessage {
            packet_id: ClientboundMoveEntityRotPacket::PACKET_ID,
            data: pkt.encode().freeze(),
            exclude_entity: Some(entity_id),
            target_entity: None,
        });
    }

    // Head rotation is always sent when yaw changes (for head tracking).
    if rot_changed {
        let head_pkt = ClientboundRotateHeadPacket {
            entity_id,
            head_yaw: packed_yaw,
        };
        ctx.server_ctx.broadcast(BroadcastMessage {
            packet_id: ClientboundRotateHeadPacket::PACKET_ID,
            data: head_pkt.encode().freeze(),
            exclude_entity: Some(entity_id),
            target_entity: None,
        });
    }
}

/// Sends chunk load/unload packets when a player crosses a chunk boundary.
async fn send_chunk_updates(
    ctx: &mut PlayContext<'_>,
    new_center: ChunkPos,
    to_load: &[ChunkPos],
    to_unload: &[ChunkPos],
) -> Result<(), ConnectionError> {
    let center_pkt = ClientboundSetChunkCacheCenterPacket {
        chunk_x: new_center.x,
        chunk_z: new_center.z,
    };
    ctx.conn
        .send_raw(
            ClientboundSetChunkCacheCenterPacket::PACKET_ID,
            &center_pkt.encode(),
        )
        .await?;

    for pos in to_unload {
        let forget = ClientboundForgetLevelChunkPacket {
            chunk_x: pos.x,
            chunk_z: pos.z,
        };
        ctx.conn
            .send_raw(
                ClientboundForgetLevelChunkPacket::PACKET_ID,
                &forget.encode(),
            )
            .await?;
    }

    if !to_load.is_empty() {
        ctx.conn
            .send_raw(
                ClientboundChunkBatchStartPacket::PACKET_ID,
                &ClientboundChunkBatchStartPacket.encode(),
            )
            .await?;

        for pos in to_load {
            // Use the existing chunk from storage if available (preserves block
            // changes), otherwise generate a new one.
            let chunk_ref = ctx
                .server_ctx
                .chunks
                .entry(*pos)
                .or_insert_with(|| {
                    let chunk = ctx.server_ctx.chunk_generator.generate_chunk(*pos);
                    Arc::new(RwLock::new(chunk))
                })
                .clone();

            let chunk_pkt = build_chunk_packet(&chunk_ref.read());
            ctx.conn
                .send_raw(
                    ClientboundLevelChunkWithLightPacket::PACKET_ID,
                    &chunk_pkt.encode(),
                )
                .await?;
        }

        let batch_finished = ClientboundChunkBatchFinishedPacket {
            batch_size: to_load.len() as i32,
        };
        ctx.conn
            .send_raw(
                ClientboundChunkBatchFinishedPacket::PACKET_ID,
                &batch_finished.encode(),
            )
            .await?;
    }

    ctx.conn.flush().await?;

    debug!(
        peer = %ctx.addr,
        name = %ctx.player_name,
        loaded = to_load.len(),
        unloaded = to_unload.len(),
        center_x = new_center.x,
        center_z = new_center.z,
        "Chunk boundary crossed",
    );

    Ok(())
}
