//! Clientbound level chunk with light packet.
//!
//! Sends a full chunk column (all sections, heightmaps, block entities) plus
//! light data to the client. This is the main chunk packet.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundLevelChunkWithLightPacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::packet::{Packet, PacketDecodeError};
use crate::codec::types::{TypeError, read_i32, read_i64};
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
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkPacketData {
    /// Heightmap entries: `(type_id, long_array)`.
    pub heightmaps: Vec<HeightmapEntry>,
    /// Concatenated serialized sections.
    pub buffer: Vec<u8>,
}

/// A single heightmap entry for wire serialization.
#[derive(Debug, Clone, PartialEq)]
pub struct HeightmapEntry {
    /// Heightmap type ID (1 = WORLD_SURFACE, 4 = MOTION_BLOCKING).
    pub type_id: i32,
    /// Packed heightmap longs.
    pub data: Vec<i64>,
}

/// Light update data for the chunk packet.
#[derive(Debug, Clone, PartialEq)]
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

impl Packet for ClientboundLevelChunkWithLightPacket {
    const PACKET_ID: i32 = 0x2D;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let chunk_x = read_i32(&mut data)?;
        let chunk_z = read_i32(&mut data)?;
        let chunk_data = ChunkPacketData::read_from(&mut data)?;
        let light_data = LightUpdateData::read_from(&mut data)?;
        Ok(Self {
            chunk_x,
            chunk_z,
            chunk_data,
            light_data,
        })
    }

    fn encode(&self) -> BytesMut {
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

    /// Reads chunk data from the buffer.
    ///
    /// # Errors
    ///
    /// Returns [`PacketDecodeError`] if the buffer is too short or contains
    /// unsupported block entity data.
    pub fn read_from(buf: &mut Bytes) -> Result<Self, PacketDecodeError> {
        // Heightmaps: VarInt(count) then for each: VarInt(type_id) VarInt(longs_count) i64[]
        let hm_count = read_non_negative_varint(buf, "heightmap count")?;
        let mut heightmaps = Vec::with_capacity(hm_count);
        for _ in 0..hm_count {
            let type_id = varint::read_varint_buf(buf)?;
            let longs_count = read_non_negative_varint(buf, "heightmap longs count")?;
            let mut data = Vec::with_capacity(longs_count);
            for _ in 0..longs_count {
                data.push(read_i64(buf)?);
            }
            heightmaps.push(HeightmapEntry { type_id, data });
        }

        // Buffer: VarInt(length) + raw bytes
        let buffer_len = read_non_negative_varint(buf, "chunk buffer length")?;
        if buf.remaining() < buffer_len {
            return Err(PacketDecodeError::Type(TypeError::UnexpectedEof {
                need: buffer_len,
                have: buf.remaining(),
            }));
        }
        let buffer = buf.copy_to_bytes(buffer_len).to_vec();

        // Block entities: VarInt(count) — skip for now
        let block_entity_count = varint::read_varint_buf(buf)?;
        if block_entity_count != 0 {
            return Err(PacketDecodeError::InvalidData(
                "block entity decoding not yet implemented".into(),
            ));
        }

        Ok(Self { heightmaps, buffer })
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

    /// Reads light data from the buffer.
    ///
    /// # Errors
    ///
    /// Returns [`PacketDecodeError`] if the buffer is too short.
    pub fn read_from(buf: &mut Bytes) -> Result<Self, PacketDecodeError> {
        let sky_y_mask = read_bitset(buf)?;
        let block_y_mask = read_bitset(buf)?;
        let empty_sky_y_mask = read_bitset(buf)?;
        let empty_block_y_mask = read_bitset(buf)?;
        let sky_updates = read_byte_arrays(buf)?;
        let block_updates = read_byte_arrays(buf)?;
        Ok(Self {
            sky_y_mask,
            block_y_mask,
            empty_sky_y_mask,
            empty_block_y_mask,
            sky_updates,
            block_updates,
        })
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

/// Reads a VarInt, validates it is non-negative, and returns it as `usize`.
fn read_non_negative_varint(buf: &mut Bytes, field_name: &str) -> Result<usize, PacketDecodeError> {
    let value = varint::read_varint_buf(buf)?;
    if value < 0 {
        return Err(PacketDecodeError::InvalidData(format!(
            "negative {field_name}: {value}"
        )));
    }
    Ok(value as usize)
}

/// Reads a BitSet as `VarInt(longs_count) long[]`.
fn read_bitset(buf: &mut Bytes) -> Result<Vec<i64>, PacketDecodeError> {
    let count = read_non_negative_varint(buf, "bitset length")?;
    let mut longs = Vec::with_capacity(count);
    for _ in 0..count {
        longs.push(read_i64(buf)?);
    }
    Ok(longs)
}

/// Reads a list of byte arrays as `VarInt(count) [VarInt(len) bytes]...`.
fn read_byte_arrays(buf: &mut Bytes) -> Result<Vec<Vec<u8>>, PacketDecodeError> {
    let count = read_non_negative_varint(buf, "byte array list count")?;
    let mut arrays = Vec::with_capacity(count);
    for _ in 0..count {
        let len = read_non_negative_varint(buf, "byte array length")?;
        if buf.remaining() < len {
            return Err(PacketDecodeError::Type(TypeError::UnexpectedEof {
                need: len,
                have: buf.remaining(),
            }));
        }
        arrays.push(buf.copy_to_bytes(len).to_vec());
    }
    Ok(arrays)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::codec::packet::Packet;

    #[test]
    fn test_empty_chunk_roundtrip() {
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
        let decoded = ClientboundLevelChunkWithLightPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_chunk_with_heightmaps_roundtrip() {
        let pkt = ClientboundLevelChunkWithLightPacket {
            chunk_x: 5,
            chunk_z: -3,
            chunk_data: ChunkPacketData {
                heightmaps: vec![HeightmapEntry {
                    type_id: 4,
                    data: vec![0i64; 37],
                }],
                buffer: vec![0u8; 100],
            },
            light_data: LightUpdateData::empty(),
        };
        let encoded = pkt.encode();
        let decoded = ClientboundLevelChunkWithLightPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_chunk_with_light_data_roundtrip() {
        let pkt = ClientboundLevelChunkWithLightPacket {
            chunk_x: -100,
            chunk_z: 200,
            chunk_data: ChunkPacketData {
                heightmaps: vec![],
                buffer: vec![1, 2, 3, 4, 5],
            },
            light_data: LightUpdateData {
                sky_y_mask: vec![0x02],
                block_y_mask: vec![],
                empty_sky_y_mask: vec![0xFD],
                empty_block_y_mask: vec![0xFF],
                sky_updates: vec![vec![0xFF; 2048]],
                block_updates: vec![],
            },
        };
        let encoded = pkt.encode();
        let decoded = ClientboundLevelChunkWithLightPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_chunk_decode_truncated_fails() {
        let data = Bytes::from_static(&[0, 0, 0, 1]); // only chunk_x, missing chunk_z
        let result = ClientboundLevelChunkWithLightPacket::decode(data);
        assert!(result.is_err());
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(
            <ClientboundLevelChunkWithLightPacket as Packet>::PACKET_ID,
            0x2D,
        );
    }

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
        assert!(encoded.len() >= 17);
    }

    #[test]
    fn test_encode_with_heightmaps() {
        let pkt = ClientboundLevelChunkWithLightPacket {
            chunk_x: 5,
            chunk_z: -3,
            chunk_data: ChunkPacketData {
                heightmaps: vec![HeightmapEntry {
                    type_id: 4,
                    data: vec![0i64; 37],
                }],
                buffer: vec![0u8; 100],
            },
            light_data: LightUpdateData::empty(),
        };
        let encoded = pkt.encode();
        assert_eq!(i32::from_be_bytes(encoded[0..4].try_into().unwrap()), 5);
        assert_eq!(i32::from_be_bytes(encoded[4..8].try_into().unwrap()), -3);
    }

    #[test]
    fn test_light_data_with_sections() {
        let light = LightUpdateData {
            sky_y_mask: vec![0x02],
            block_y_mask: vec![],
            empty_sky_y_mask: vec![0xFD],
            empty_block_y_mask: vec![0xFF],
            sky_updates: vec![vec![0xFF; 2048]],
            block_updates: vec![],
        };
        let mut buf = BytesMut::new();
        light.write_to(&mut buf);
        assert!(!buf.is_empty());
    }
}
