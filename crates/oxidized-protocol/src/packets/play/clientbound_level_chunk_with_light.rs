//! Clientbound level chunk with light packet.
//!
//! Sends a full chunk column (all sections, heightmaps, block entities) plus
//! light data to the client. This is the main chunk packet.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundLevelChunkWithLightPacket`.

use bytes::{BufMut, BytesMut};

use crate::codec::varint;

/// A full chunk + light packet.
///
/// Wire format:
/// ```text
/// chunk_x: i32
/// chunk_z: i32
/// heightmaps: Map<VarInt(type_id), LongArray>
/// buffer_length: VarInt
/// buffer: byte[]
/// block_entities: VarInt(0)  (no block entities yet)
/// sky_y_mask: BitSet
/// block_y_mask: BitSet
/// empty_sky_y_mask: BitSet
/// empty_block_y_mask: BitSet
/// sky_updates: List<byte[2048]>
/// block_updates: List<byte[2048]>
/// ```
#[derive(Debug, Clone)]
pub struct ClientboundLevelChunkWithLightPacket {
    /// Chunk X coordinate.
    pub chunk_x: i32,
    /// Chunk Z coordinate.
    pub chunk_z: i32,
    /// Chunk data (heightmaps + sections + block entities).
    pub chunk_data: ChunkPacketData,
    /// Light data (masks + nibble arrays).
    pub light_data: LightUpdateData,
}

/// Serialized chunk data for the chunk packet.
#[derive(Debug, Clone)]
pub struct ChunkPacketData {
    /// Heightmap entries: `(type_id, long_array)`.
    pub heightmaps: Vec<HeightmapEntry>,
    /// Concatenated serialized sections.
    pub buffer: Vec<u8>,
}

/// A single heightmap entry for wire serialization.
#[derive(Debug, Clone)]
pub struct HeightmapEntry {
    /// Heightmap type ID (1 = WORLD_SURFACE, 4 = MOTION_BLOCKING).
    pub type_id: i32,
    /// Packed heightmap longs.
    pub data: Vec<i64>,
}

/// Light update data for the chunk packet.
#[derive(Debug, Clone)]
pub struct LightUpdateData {
    /// BitSet: which sections have sky light data.
    pub sky_y_mask: Vec<i64>,
    /// BitSet: which sections have block light data.
    pub block_y_mask: Vec<i64>,
    /// BitSet: which sections have empty sky light (all zeros).
    pub empty_sky_y_mask: Vec<i64>,
    /// BitSet: which sections have empty block light.
    pub empty_block_y_mask: Vec<i64>,
    /// Sky light arrays (2048 bytes each), for sections in `sky_y_mask`.
    pub sky_updates: Vec<Vec<u8>>,
    /// Block light arrays (2048 bytes each), for sections in `block_y_mask`.
    pub block_updates: Vec<Vec<u8>>,
}

impl ClientboundLevelChunkWithLightPacket {
    /// Packet ID in the PLAY state.
    pub const PACKET_ID: i32 = 0x2D; // 45

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let estimated = 8 + self.chunk_data.buffer.len() + 256;
        let mut buf = BytesMut::with_capacity(estimated);

        // Chunk coordinates
        buf.put_i32(self.chunk_x);
        buf.put_i32(self.chunk_z);

        // Chunk data
        self.chunk_data.write_to(&mut buf);

        // Light data
        self.light_data.write_to(&mut buf);

        buf
    }
}

impl ChunkPacketData {
    /// Writes chunk data to the buffer.
    pub fn write_to(&self, buf: &mut BytesMut) {
        // Heightmaps: Map<VarInt(type_id), LongArray>
        // VarInt(map_size) then for each: VarInt(key) VarInt(longs_count) long[...]
        varint::write_varint_buf(self.heightmaps.len() as i32, buf);
        for entry in &self.heightmaps {
            varint::write_varint_buf(entry.type_id, buf);
            varint::write_varint_buf(entry.data.len() as i32, buf);
            for &long in &entry.data {
                buf.put_i64(long);
            }
        }

        // Section buffer: VarInt(length) + raw bytes
        varint::write_varint_buf(self.buffer.len() as i32, buf);
        buf.extend_from_slice(&self.buffer);

        // Block entities: VarInt(0) — no block entities yet
        varint::write_varint_buf(0, buf);
    }
}

impl LightUpdateData {
    /// Creates empty light data (no light sections).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            sky_y_mask: Vec::new(),
            block_y_mask: Vec::new(),
            empty_sky_y_mask: Vec::new(),
            empty_block_y_mask: Vec::new(),
            sky_updates: Vec::new(),
            block_updates: Vec::new(),
        }
    }

    /// Writes light data to the buffer.
    pub fn write_to(&self, buf: &mut BytesMut) {
        // Four BitSets (each as VarInt(longs_count) + long[])
        write_bitset(buf, &self.sky_y_mask);
        write_bitset(buf, &self.block_y_mask);
        write_bitset(buf, &self.empty_sky_y_mask);
        write_bitset(buf, &self.empty_block_y_mask);

        // Sky updates: VarInt(count) then each as VarInt(length) + bytes
        write_byte_arrays(buf, &self.sky_updates);

        // Block updates
        write_byte_arrays(buf, &self.block_updates);
    }
}

/// Writes a BitSet as `VarInt(longs_count) long[]`.
fn write_bitset(buf: &mut BytesMut, longs: &[i64]) {
    varint::write_varint_buf(longs.len() as i32, buf);
    for &long in longs {
        buf.put_i64(long);
    }
}

/// Writes a list of byte arrays as `VarInt(count) [VarInt(len) bytes]...`.
fn write_byte_arrays(buf: &mut BytesMut, arrays: &[Vec<u8>]) {
    varint::write_varint_buf(arrays.len() as i32, buf);
    for arr in arrays {
        varint::write_varint_buf(arr.len() as i32, buf);
        buf.extend_from_slice(arr);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_chunk_encode() {
        let pkt = ClientboundLevelChunkWithLightPacket {
            chunk_x: 0,
            chunk_z: 0,
            chunk_data: ChunkPacketData {
                heightmaps: vec![],
                buffer: vec![],
            },
            light_data: LightUpdateData::empty(),
        };
        let encoded = pkt.encode();
        // Should at least have: 4 (x) + 4 (z) + 1 (heightmap count=0)
        // + 1 (buffer len=0) + 1 (block entities=0)
        // + 4 (empty bitsets) + 2 (empty lists)
        assert!(encoded.len() >= 17);
    }

    #[test]
    fn test_encode_with_heightmaps() {
        let pkt = ClientboundLevelChunkWithLightPacket {
            chunk_x: 5,
            chunk_z: -3,
            chunk_data: ChunkPacketData {
                heightmaps: vec![HeightmapEntry {
                    type_id: 4, // MOTION_BLOCKING
                    data: vec![0i64; 37],
                }],
                buffer: vec![0u8; 100],
            },
            light_data: LightUpdateData::empty(),
        };
        let encoded = pkt.encode();
        // Verify it starts with the chunk coordinates
        assert_eq!(i32::from_be_bytes(encoded[0..4].try_into().unwrap()), 5);
        assert_eq!(i32::from_be_bytes(encoded[4..8].try_into().unwrap()), -3);
    }

    #[test]
    fn test_light_data_with_sections() {
        let light = LightUpdateData {
            sky_y_mask: vec![0x02], // bit 1 set
            block_y_mask: vec![],
            empty_sky_y_mask: vec![0xFD], // all except bit 1
            empty_block_y_mask: vec![0xFF],
            sky_updates: vec![vec![0xFF; 2048]],
            block_updates: vec![],
        };
        let mut buf = BytesMut::new();
        light.write_to(&mut buf);
        // Should contain 4 bitsets + 2 lists
        assert!(!buf.is_empty());
    }
}
