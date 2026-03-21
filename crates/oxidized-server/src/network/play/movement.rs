//! Movement packet handling and chunk boundary tracking.
//!
//! Processes position/rotation updates from the client, validates them,
//! and sends/forgets chunks when the player crosses chunk boundaries.

use oxidized_game::player::movement::validate_movement;
use oxidized_protocol::codec::Packet;
use oxidized_protocol::connection::ConnectionError;
use oxidized_protocol::packets::play::{
    ClientboundChunkBatchFinishedPacket, ClientboundChunkBatchStartPacket,
    ClientboundForgetLevelChunkPacket, ClientboundLevelChunkWithLightPacket,
    ClientboundPlayerPositionPacket, ClientboundSetChunkCacheCenterPacket, RelativeFlags,
    ServerboundMovePlayerPacket, ServerboundMovePlayerPosPacket, ServerboundMovePlayerPosRotPacket,
    ServerboundMovePlayerRotPacket, ServerboundMovePlayerStatusOnlyPacket,
};
use oxidized_world::chunk::{ChunkPos, LevelChunk};
use tracing::{debug, trace};

use oxidized_game::net::chunk_serializer::build_chunk_packet;

use super::PlayContext;

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

    let result = {
        let p = ctx.player.read();
        validate_movement(
            p.pos,
            p.yaw,
            p.pitch,
            move_pkt.x,
            move_pkt.y,
            move_pkt.z,
            move_pkt.yaw,
            move_pkt.pitch,
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

        // Check if player crossed a chunk boundary.
        if move_pkt.has_pos() {
            let new_chunk = ChunkPos::from_block(
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
            let chunk = LevelChunk::new(*pos);
            let chunk_pkt = build_chunk_packet(&chunk);
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
