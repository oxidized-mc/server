//! Movement packet handling and chunk boundary tracking.
//!
//! Processes position/rotation updates from the client, validates them,
//! broadcasts position changes to other players, and sends/forgets chunks
//! when the player crosses chunk boundaries.

use std::time::Instant;

use oxidized_game::level::game_rules::GameRuleKey;
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
use tracing::{debug, trace, warn};

use oxidized_game::net::chunk_serializer::build_chunk_packet;
use oxidized_game::net::entity_movement::{EntityMoveKind, classify_move, pack_degrees};

use super::PlayContext;
use crate::network::BroadcastMessage;

/// Determines whether speed validation should be skipped.
///
/// Vanilla skips speed checks when:
/// - The player is in creative or spectator mode
/// - The `player_movement_check` game rule is `false`
/// - The player is elytra-flying and `elytra_movement_check` is `false`
///
/// NaN/Infinity rejection and coordinate clamping are never skipped.
fn should_skip_speed_check(
    is_creative_or_spectator: bool,
    player_movement_check: bool,
    elytra_movement_check: bool,
    is_fall_flying: bool,
) -> bool {
    is_creative_or_spectator || !player_movement_check || (is_fall_flying && !elytra_movement_check)
}

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

    // Vanilla disconnects on NaN/Infinity — never silently drop.
    if move_pkt.contains_invalid_values() {
        warn!(peer = %ctx.addr, name = %ctx.player_name, "Movement packet contains NaN/Infinity — disconnecting");
        return Err(ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid player movement",
        )));
    }

    // Rate-limit movement packets: max 120/second (6 per tick × 20 ticks).
    {
        let mut p = ctx.player.write();
        let now = Instant::now();
        if now.duration_since(p.movement_rate.1).as_millis() >= 1000 {
            p.movement_rate = (0, now);
        }
        p.movement_rate.0 += 1;
        if p.movement_rate.0 > 120 {
            debug!(peer = %ctx.addr, name = %ctx.player_name, "Movement packet throttled");
            return Ok(());
        }
    }

    let (old_pos, old_yaw, old_pitch, entity_id, is_fall_flying, game_mode) = {
        let p = ctx.player.read();
        (
            p.pos,
            p.yaw,
            p.pitch,
            p.entity_id,
            p.is_fall_flying,
            p.game_mode,
        )
    };

    let (player_movement_check, elytra_movement_check) = {
        let rules = ctx.server_ctx.game_rules.read();
        (
            rules.get_bool(GameRuleKey::PlayerMovementCheck),
            rules.get_bool(GameRuleKey::ElytraMovementCheck),
        )
    };

    let skip_speed_check = should_skip_speed_check(
        game_mode.is_creative_or_spectator(),
        player_movement_check,
        elytra_movement_check,
        is_fall_flying,
    );

    let mut result = {
        validate_movement(
            old_pos,
            old_yaw,
            old_pitch,
            move_pkt.x,
            move_pkt.y,
            move_pkt.z,
            move_pkt.yaw,
            move_pkt.pitch,
            is_fall_flying,
        )
    };

    // Skip speed limits but still reject NaN/Infinity and apply coordinate clamping.
    if skip_speed_check && !result.has_invalid_values {
        result.is_correction_needed = false;
        result.is_accepted = true;
    }

    if result.is_correction_needed {
        let correction = {
            let mut p = ctx.player.write();
            let teleport_id = p.next_teleport_id();
            let pos = p.pos;
            p.pending_teleports
                .push_back((teleport_id, pos, Instant::now()));
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
        ctx.conn_handle.send_packet(&correction).await?;
        debug!(peer = %ctx.addr, name = %ctx.player_name, "Position correction sent");
    } else {
        {
            let mut p = ctx.player.write();
            p.pos = result.new_pos;
            p.yaw = result.new_yaw;
            p.pitch = result.new_pitch;
            p.is_on_ground = move_pkt.is_on_ground;
            // Vanilla: elytra flight ends when the player touches the ground.
            if move_pkt.is_on_ground {
                p.is_fall_flying = false;
            }
        }

        trace!(
            peer = %ctx.addr,
            name = %ctx.player_name,
            x = result.new_pos.x,
            y = result.new_pos.y,
            z = result.new_pos.z,
            is_on_ground = move_pkt.is_on_ground,
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
            move_pkt.is_on_ground,
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
    is_on_ground: bool,
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
                    is_on_ground,
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
                    is_on_ground,
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
                    is_on_ground,
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
                    is_on_ground,
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
            is_on_ground,
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
pub(super) async fn send_chunk_updates(
    ctx: &mut PlayContext<'_>,
    new_center: ChunkPos,
    to_load: &[ChunkPos],
    to_unload: &[ChunkPos],
) -> Result<(), ConnectionError> {
    let center_pkt = ClientboundSetChunkCacheCenterPacket {
        chunk_x: new_center.x,
        chunk_z: new_center.z,
    };
    ctx.conn_handle
        .send_raw(
            ClientboundSetChunkCacheCenterPacket::PACKET_ID,
            center_pkt.encode().freeze(),
        )
        .await?;

    for pos in to_unload {
        let forget = ClientboundForgetLevelChunkPacket {
            chunk_x: pos.x,
            chunk_z: pos.z,
        };
        ctx.conn_handle
            .send_raw(
                ClientboundForgetLevelChunkPacket::PACKET_ID,
                forget.encode().freeze(),
            )
            .await?;
    }

    if !to_load.is_empty() {
        ctx.conn_handle
            .send_raw(
                ClientboundChunkBatchStartPacket::PACKET_ID,
                ClientboundChunkBatchStartPacket.encode().freeze(),
            )
            .await?;

        for pos in to_load {
            // Load from disk or generate, preserving in-memory block changes.
            let chunk_ref =
                super::helpers::get_or_create_chunk(ctx.server_ctx, *pos).await;

            let chunk_pkt = build_chunk_packet(&chunk_ref.read());
            ctx.conn_handle
                .send_raw(
                    ClientboundLevelChunkWithLightPacket::PACKET_ID,
                    chunk_pkt.encode().freeze(),
                )
                .await?;
        }

        let batch_finished = ClientboundChunkBatchFinishedPacket {
            batch_size: to_load.len() as i32,
        };
        ctx.conn_handle
            .send_raw(
                ClientboundChunkBatchFinishedPacket::PACKET_ID,
                batch_finished.encode().freeze(),
            )
            .await?;
    }

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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::should_skip_speed_check;

    #[test]
    fn test_skip_when_creative_or_spectator() {
        assert!(should_skip_speed_check(true, true, true, false));
    }

    #[test]
    fn test_no_skip_in_survival_with_defaults() {
        assert!(!should_skip_speed_check(false, true, true, false));
    }

    #[test]
    fn test_skip_when_player_movement_check_disabled() {
        assert!(should_skip_speed_check(false, false, true, false));
        assert!(should_skip_speed_check(false, false, true, true));
        assert!(should_skip_speed_check(false, false, false, false));
    }

    #[test]
    fn test_skip_elytra_when_elytra_check_disabled_and_flying() {
        assert!(should_skip_speed_check(false, true, false, true));
    }

    #[test]
    fn test_no_skip_elytra_check_disabled_but_not_flying() {
        assert!(!should_skip_speed_check(false, true, false, false));
    }

    #[test]
    fn test_all_rules_disabled_still_skips() {
        assert!(should_skip_speed_check(false, false, false, true));
        assert!(should_skip_speed_check(false, false, false, false));
    }

    #[test]
    fn test_creative_overrides_regardless_of_rules() {
        assert!(should_skip_speed_check(true, false, false, false));
        assert!(should_skip_speed_check(true, true, false, true));
    }
}
