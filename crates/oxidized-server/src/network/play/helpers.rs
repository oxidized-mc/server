//! Play-state utility functions.
//!
//! Provides [`send_initial_chunks`] for the player join sequence.

use std::sync::Arc;

use oxidized_game::chunk::view_distance::spiral_chunks;
use oxidized_game::net::chunk_serializer::build_chunk_packet;
use oxidized_protocol::codec::Packet;
use oxidized_protocol::connection::ConnectionError;
use oxidized_protocol::handle::ConnectionHandle;
use oxidized_protocol::packets::play::{
    ClientboundChunkBatchFinishedPacket, ClientboundChunkBatchStartPacket,
    ClientboundLevelChunkWithLightPacket,
};
use oxidized_world::chunk::{ChunkPos, LevelChunk};
use parking_lot::RwLock;
use tracing::warn;

use crate::network::ServerContext;

/// Loads a chunk from disk or generates a new one, inserting it into
/// the shared chunk storage.
///
/// Checks the in-memory `DashMap` first, then tries disk, then generates.
pub(super) async fn get_or_create_chunk(
    server_ctx: &ServerContext,
    pos: ChunkPos,
) -> Arc<RwLock<LevelChunk>> {
    // Fast path: already in memory.
    if let Some(existing) = server_ctx.world.chunks.get(&pos) {
        return existing.clone();
    }

    // Try loading from disk.
    match server_ctx.world.chunk_loader.load_chunk(pos.x, pos.z).await {
        Ok(Some(chunk)) => {
            return server_ctx
                .world.chunks
                .entry(pos)
                .or_insert_with(|| Arc::new(RwLock::new(chunk)))
                .clone();
        },
        Ok(None) => {
            // Not on disk — will generate below.
        },
        Err(e) => {
            warn!(chunk_x = pos.x, chunk_z = pos.z, error = %e, "Failed to load chunk from disk");
        },
    }

    // Generate a new chunk.
    server_ctx
        .world.chunks
        .entry(pos)
        .or_insert_with(|| {
            let chunk = server_ctx.world.chunk_generator.generate_chunk(pos);
            Arc::new(RwLock::new(chunk))
        })
        .clone()
}

/// Sends the initial chunk batch for a player joining the world.
///
/// Loads chunks from disk or generates them using the [`ChunkGenerator`]
/// in a spiral pattern around the player, sending them wrapped in
/// `ChunkBatchStart` / `ChunkBatchFinished` framing via the outbound channel.
///
/// Each chunk is also registered in the shared chunk storage so that
/// play-state handlers (block breaking/placing) can read and modify blocks.
///
/// Returns the number of chunks sent.
pub async fn send_initial_chunks(
    conn_handle: &ConnectionHandle,
    center: ChunkPos,
    view_distance: i32,
    server_ctx: &ServerContext,
) -> Result<i32, ConnectionError> {
    // Start the chunk batch.
    conn_handle
        .send_raw(
            ClientboundChunkBatchStartPacket::PACKET_ID,
            ClientboundChunkBatchStartPacket.encode().freeze(),
        )
        .await?;

    let mut count: i32 = 0;
    for chunk_pos in spiral_chunks(center, view_distance) {
        let chunk_ref = get_or_create_chunk(server_ctx, chunk_pos).await;

        let pkt = build_chunk_packet(&chunk_ref.read());
        conn_handle
            .send_raw(
                ClientboundLevelChunkWithLightPacket::PACKET_ID,
                pkt.encode().freeze(),
            )
            .await?;

        count += 1;
    }

    // Finish the chunk batch.
    let finished = ClientboundChunkBatchFinishedPacket { batch_size: count };
    conn_handle
        .send_raw(
            ClientboundChunkBatchFinishedPacket::PACKET_ID,
            finished.encode().freeze(),
        )
        .await?;

    Ok(count)
}
