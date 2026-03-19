//! Serializes chunk data for network packets.
//!
//! Builds a [`ClientboundLevelChunkWithLightPacket`] from a [`LevelChunk`],
//! including heightmap encoding, section buffer assembly, and light data.

use oxidized_protocol::packets::play::{
    ChunkPacketData, ClientboundLevelChunkWithLightPacket, HeightmapEntry,
};
use oxidized_world::chunk::heightmap::HeightmapType;
use oxidized_world::chunk::LevelChunk;

use super::light_serializer::build_light_data;

/// Heightmap type IDs matching Java's `Heightmap.Types` enum ordinals.
const HEIGHTMAP_TYPE_ID_WORLD_SURFACE: i32 = 1;
const HEIGHTMAP_TYPE_ID_MOTION_BLOCKING: i32 = 4;

/// Builds a full chunk packet from a [`LevelChunk`].
#[must_use]
pub fn build_chunk_packet(chunk: &LevelChunk) -> ClientboundLevelChunkWithLightPacket {
    ClientboundLevelChunkWithLightPacket {
        chunk_x: chunk.pos.x,
        chunk_z: chunk.pos.z,
        chunk_data: build_chunk_data(chunk),
        light_data: build_light_data(chunk.sky_light_layers(), chunk.block_light_layers()),
    }
}

/// Builds the chunk data portion (heightmaps + section buffer).
fn build_chunk_data(chunk: &LevelChunk) -> ChunkPacketData {
    ChunkPacketData {
        heightmaps: build_heightmap_entries(chunk),
        buffer: chunk.write_sections_to_bytes(),
    }
}

/// Serializes client-visible heightmaps as binary map entries.
///
/// Only `MOTION_BLOCKING` and `WORLD_SURFACE` are sent to the client,
/// matching Java's `Heightmap.Types.sendToClient()` filter.
fn build_heightmap_entries(chunk: &LevelChunk) -> Vec<HeightmapEntry> {
    let mut entries = Vec::with_capacity(2);

    for &htype in HeightmapType::CLIENT_TYPES {
        if let Some(hm) = chunk.heightmap(htype) {
            let type_id = match htype {
                HeightmapType::WorldSurface => HEIGHTMAP_TYPE_ID_WORLD_SURFACE,
                HeightmapType::MotionBlocking => HEIGHTMAP_TYPE_ID_MOTION_BLOCKING,
                _ => continue,
            };
            entries.push(HeightmapEntry {
                type_id,
                data: hm.to_nbt_longs(),
            });
        }
    }

    entries
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use oxidized_world::chunk::heightmap::{Heightmap, HeightmapType};
    use oxidized_world::chunk::level_chunk::{ChunkPos, OVERWORLD_HEIGHT};

    #[test]
    fn test_empty_chunk_packet() {
        let chunk = LevelChunk::new(ChunkPos::new(0, 0));
        let pkt = build_chunk_packet(&chunk);
        assert_eq!(pkt.chunk_x, 0);
        assert_eq!(pkt.chunk_z, 0);
        // No heightmaps set → empty entries
        assert!(pkt.chunk_data.heightmaps.is_empty());
        // Section buffer should be non-empty (24 sections, each with headers)
        assert!(!pkt.chunk_data.buffer.is_empty());
    }

    #[test]
    fn test_chunk_with_heightmaps() {
        let mut chunk = LevelChunk::new(ChunkPos::new(5, -3));
        let hm = Heightmap::new(HeightmapType::MotionBlocking, OVERWORLD_HEIGHT).unwrap();
        chunk.set_heightmap(hm);
        let hm2 = Heightmap::new(HeightmapType::WorldSurface, OVERWORLD_HEIGHT).unwrap();
        chunk.set_heightmap(hm2);

        let pkt = build_chunk_packet(&chunk);
        assert_eq!(pkt.chunk_x, 5);
        assert_eq!(pkt.chunk_z, -3);
        assert_eq!(pkt.chunk_data.heightmaps.len(), 2);

        // Check type IDs
        let type_ids: Vec<i32> = pkt.chunk_data.heightmaps.iter().map(|e| e.type_id).collect();
        assert!(type_ids.contains(&HEIGHTMAP_TYPE_ID_MOTION_BLOCKING));
        assert!(type_ids.contains(&HEIGHTMAP_TYPE_ID_WORLD_SURFACE));
    }

    #[test]
    fn test_empty_chunk_section_wire_format() {
        let chunk = LevelChunk::new(ChunkPos::new(0, 0));
        let buf = chunk.write_sections_to_bytes();

        // Each empty section: 2 bytes (non_empty=0) + 2 bytes (fluid=0)
        // + block states (at least 1 byte bits_per_entry=0 + varint palette entry)
        // + biomes (at least 1 byte bits_per_entry=0 + varint palette entry)
        // So the buffer has data for 24 sections.
        assert!(buf.len() > 24 * 4);

        // First section starts with non_empty_block_count = 0
        assert_eq!(buf[0], 0);
        assert_eq!(buf[1], 0);
        // fluid_count = 0
        assert_eq!(buf[2], 0);
        assert_eq!(buf[3], 0);
    }

    #[test]
    fn test_chunk_packet_encodes() {
        let chunk = LevelChunk::new(ChunkPos::new(10, -20));
        let pkt = build_chunk_packet(&chunk);
        let encoded = pkt.encode();
        // Should at minimum have chunk coords (8 bytes) + data
        assert!(encoded.len() > 8);
        // Verify coordinates
        assert_eq!(
            i32::from_be_bytes(encoded[0..4].try_into().unwrap()),
            10
        );
        assert_eq!(
            i32::from_be_bytes(encoded[4..8].try_into().unwrap()),
            -20
        );
    }

    #[test]
    fn test_server_only_heightmaps_excluded() {
        let mut chunk = LevelChunk::new(ChunkPos::new(0, 0));
        // Set an OCEAN_FLOOR heightmap (server-only, not sent to client)
        let hm = Heightmap::new(HeightmapType::OceanFloor, OVERWORLD_HEIGHT).unwrap();
        chunk.set_heightmap(hm);

        let pkt = build_chunk_packet(&chunk);
        // OCEAN_FLOOR should not appear
        assert!(pkt.chunk_data.heightmaps.is_empty());
    }
}
