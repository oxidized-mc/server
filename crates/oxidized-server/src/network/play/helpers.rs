//! Play-state utility functions.
//!
//! Provides [`send_initial_chunks`] for the player join sequence.

use oxidized_game::chunk::view_distance::spiral_chunks;
use oxidized_game::net::chunk_serializer::build_chunk_packet;
use oxidized_protocol::connection::{Connection, ConnectionError};
use oxidized_protocol::packets::play::{
    ClientboundChunkBatchFinishedPacket, ClientboundChunkBatchStartPacket,
    ClientboundLevelChunkWithLightPacket,
};
use oxidized_world::chunk::{ChunkPos, LevelChunk};

/// Sends the initial chunk batch for a player joining the world.
///
/// Creates empty air chunks in a spiral pattern around the player and sends
/// them wrapped in `ChunkBatchStart` / `ChunkBatchFinished` framing.
///
/// Real chunk loading from disk or worldgen is not yet implemented — this
/// sends purely air so the client has valid chunk data and renders the world.
///
/// Returns the number of chunks sent.
pub async fn send_initial_chunks(
    conn: &mut Connection,
    center: ChunkPos,
    view_distance: i32,
) -> Result<i32, ConnectionError> {
    // Start the chunk batch.
    conn.send_raw(
        ClientboundChunkBatchStartPacket::PACKET_ID,
        &ClientboundChunkBatchStartPacket.encode(),
    )
    .await?;

    let mut count: i32 = 0;
    for chunk_pos in spiral_chunks(center, view_distance) {
        let chunk = LevelChunk::new(chunk_pos);
        let pkt = build_chunk_packet(&chunk);
        conn.send_raw(
            ClientboundLevelChunkWithLightPacket::PACKET_ID,
            &pkt.encode(),
        )
        .await?;
        count += 1;
    }

    // Finish the chunk batch.
    let finished = ClientboundChunkBatchFinishedPacket { batch_size: count };
    conn.send_raw(
        ClientboundChunkBatchFinishedPacket::PACKET_ID,
        &finished.encode(),
    )
    .await?;
    conn.flush().await?;

    Ok(count)
}
