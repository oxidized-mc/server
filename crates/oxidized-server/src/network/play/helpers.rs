//! Play-state utility functions.
//!
//! Provides [`send_initial_chunks`] for the player join sequence.

use std::sync::Arc;

use dashmap::DashMap;
use oxidized_game::chunk::view_distance::spiral_chunks;
use oxidized_game::net::chunk_serializer::build_chunk_packet;
use oxidized_game::worldgen::ChunkGenerator;
use oxidized_protocol::codec::Packet;
use oxidized_protocol::connection::{Connection, ConnectionError};
use oxidized_protocol::packets::play::{
    ClientboundChunkBatchFinishedPacket, ClientboundChunkBatchStartPacket,
    ClientboundLevelChunkWithLightPacket,
};
use oxidized_world::chunk::{ChunkPos, LevelChunk};
use parking_lot::RwLock;

/// Sends the initial chunk batch for a player joining the world.
///
/// Generates chunks using the provided [`ChunkGenerator`] in a spiral pattern
/// around the player and sends them wrapped in `ChunkBatchStart` /
/// `ChunkBatchFinished` framing.
///
/// Each chunk is also registered in the shared `chunk_storage` map so that
/// play-state handlers (block breaking/placing) can read and modify blocks.
///
/// Returns the number of chunks sent.
pub async fn send_initial_chunks(
    conn: &mut Connection,
    center: ChunkPos,
    view_distance: i32,
    chunk_storage: &DashMap<ChunkPos, Arc<RwLock<LevelChunk>>>,
    chunk_generator: &dyn ChunkGenerator,
) -> Result<i32, ConnectionError> {
    // Start the chunk batch.
    conn.send_raw(
        ClientboundChunkBatchStartPacket::PACKET_ID,
        &ClientboundChunkBatchStartPacket.encode(),
    )
    .await?;

    let mut count: i32 = 0;
    for chunk_pos in spiral_chunks(center, view_distance) {
        let chunk = chunk_generator.generate_chunk(chunk_pos);
        let pkt = build_chunk_packet(&chunk);
        conn.send_raw(
            ClientboundLevelChunkWithLightPacket::PACKET_ID,
            &pkt.encode(),
        )
        .await?;

        // Register the chunk in shared storage for block interaction.
        chunk_storage
            .entry(chunk_pos)
            .or_insert_with(|| Arc::new(RwLock::new(chunk)));

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
