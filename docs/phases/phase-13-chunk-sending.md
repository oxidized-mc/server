# Phase 13 — Chunk Sending

**Status:** ✅ Complete  
**Crate:** `oxidized-game`  
**Reward:** A player who joins the world can see it: chunks render correctly
in the vanilla client. The full `ClientboundLevelChunkWithLightPacket` is
constructed and serialized from the in-memory `LevelChunk`, including all
sections, heightmaps, block entities, and light data.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-006: Network I/O](../adr/adr-006-network-io.md) — per-connection task pairs with mpsc channels
- [ADR-014: Chunk Storage](../adr/adr-014-chunk-storage.md) — DashMap + per-section RwLock for concurrent access
- [ADR-017: Lighting](../adr/adr-017-lighting.md) — batched BFS with parallel section processing


## Goal

Serialize `LevelChunk` → `ClientboundLevelChunkWithLightPacket` in the exact
binary format the vanilla client expects. Implement the chunk-batch protocol,
view-distance spiral iteration, and the send/unload lifecycle.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Chunk + light packet | `ClientboundLevelChunkWithLightPacket` | `net.minecraft.network.protocol.game.ClientboundLevelChunkWithLightPacket` |
| Chunk packet data | `ClientboundLevelChunkPacketData` | `net.minecraft.network.protocol.game.ClientboundLevelChunkPacketData` |
| Light update data | `ClientboundLightUpdatePacketData` | `net.minecraft.network.protocol.game.ClientboundLightUpdatePacketData` |
| Light update packet | `ClientboundLightUpdatePacket` | `net.minecraft.network.protocol.game.ClientboundLightUpdatePacket` |
| Chunk batch start | `ClientboundChunkBatchStartPacket` | `net.minecraft.network.protocol.game.ClientboundChunkBatchStartPacket` |
| Chunk batch finish | `ClientboundChunkBatchFinishedPacket` | `net.minecraft.network.protocol.game.ClientboundChunkBatchFinishedPacket` |
| Chunk batch received | `ServerboundChunkBatchReceivedPacket` | `net.minecraft.network.protocol.game.ServerboundChunkBatchReceivedPacket` |
| Cache center | `ClientboundSetChunkCacheCenterPacket` | `net.minecraft.network.protocol.game.ClientboundSetChunkCacheCenterPacket` |
| Cache radius | `ClientboundSetChunkCacheRadiusPacket` | `net.minecraft.network.protocol.game.ClientboundSetChunkCacheRadiusPacket` |
| Forget chunk | `ClientboundForgetLevelChunkPacket` | `net.minecraft.network.protocol.game.ClientboundForgetLevelChunkPacket` |

---

## Tasks

### 13.1 — Section wire format

Each section serializes as:

```
[non_empty_block_count: i16]  ← Java writes TWO i16s; fluid_count first (blocks first in 1.20+)
[block_states PalettedContainer]
[biomes PalettedContainer]
```

> **Note:** As of 1.20, `LevelChunkSection.write` writes **two** `i16` fields:
> `nonEmptyBlockCount` followed by `fluidCount`. Both must be included.

```rust
// crates/oxidized-game/src/net/chunk_serializer.rs

use oxidized_world::chunk::{LevelChunk, LevelChunkSection, SECTION_COUNT};
use oxidized_protocol::io::PacketBuf;

/// Serialize all 24 sections into a flat byte buffer.
/// This is the `buffer` field of `ClientboundLevelChunkPacketData`.
pub fn serialize_chunk_sections(chunk: &LevelChunk) -> Vec<u8> {
    let mut buf = Vec::with_capacity(estimate_chunk_size(chunk));
    for section in chunk.sections.iter() {
        serialize_section(section, &mut buf);
    }
    buf
}

fn serialize_section(section: &LevelChunkSection, buf: &mut Vec<u8>) {
    buf.extend_from_slice(&section.non_empty_block_count.to_be_bytes());
    buf.extend_from_slice(&section.fluid_count.to_be_bytes());
    section.block_states.write_to_buf(buf);
    section.biomes.write_to_buf(buf);
}

fn estimate_chunk_size(chunk: &LevelChunk) -> usize {
    chunk.sections.iter().map(|s| s.serialized_size()).sum()
}
```

### 13.2 — Heightmap serialization

Only `WORLD_SURFACE` and `MOTION_BLOCKING` are sent to the client. Both are
packed as `LongArray` inside a `CompoundTag` (NBT).

```rust
pub fn serialize_heightmaps(chunk: &LevelChunk) -> NbtCompound {
    let mut nbt = NbtCompound::new();
    for ht in [HeightmapType::WorldSurface, HeightmapType::MotionBlocking] {
        if let Some(hm) = chunk.heightmaps.get(&ht) {
            let key = match ht {
                HeightmapType::WorldSurface => "WORLD_SURFACE",
                HeightmapType::MotionBlocking => "MOTION_BLOCKING",
                _ => continue,
            };
            nbt.put_long_array(key, hm.raw_data().to_vec());
        }
    }
    nbt
}
```

### 13.3 — `ClientboundLightUpdatePacketData`

Light data uses Java's `BitSet` (64-bit longs). The section bitmasks have one
bit per section in the range `[bottom_section - 1, top_section + 1]` (includes
the two border sections, so 26 bits for the overworld).

```rust
// crates/oxidized-game/src/net/light_serializer.rs

use oxidized_world::chunk::LightData;

pub struct LightUpdateData {
    /// Bit i set → section i has sky light data.
    pub sky_y_mask: u64,
    /// Bit i set → section i has block light data.
    pub block_y_mask: u64,
    /// Bit i set → section i sky light is entirely dark (all zeros).
    pub empty_sky_y_mask: u64,
    /// Bit i set → section i block light is entirely dark.
    pub empty_block_y_mask: u64,
    /// Sky light arrays in section order (only for sections where sky_y_mask bit is set).
    pub sky_updates: Vec<Vec<u8>>,
    /// Block light arrays in section order.
    pub block_updates: Vec<Vec<u8>>,
}

impl LightUpdateData {
    pub fn from_light(light: &LightData) -> Self {
        let section_count = light.sky_light.len();
        let mut sky_mask = 0u64;
        let mut block_mask = 0u64;
        let mut empty_sky = 0u64;
        let mut empty_block = 0u64;
        let mut sky_updates = Vec::new();
        let mut block_updates = Vec::new();

        for i in 0..section_count {
            match &light.sky_light[i] {
                Some(arr) => {
                    if arr.iter().any(|&b| b != 0) {
                        sky_mask |= 1u64 << i;
                        sky_updates.push(arr.to_vec());
                    } else {
                        empty_sky |= 1u64 << i;
                    }
                }
                None => { empty_sky |= 1u64 << i; }
            }
            match &light.block_light[i] {
                Some(arr) => {
                    if arr.iter().any(|&b| b != 0) {
                        block_mask |= 1u64 << i;
                        block_updates.push(arr.to_vec());
                    } else {
                        empty_block |= 1u64 << i;
                    }
                }
                None => { empty_block |= 1u64 << i; }
            }
        }

        Self {
            sky_y_mask: sky_mask,
            block_y_mask: block_mask,
            empty_sky_y_mask: empty_sky,
            empty_block_y_mask: empty_block,
            sky_updates,
            block_updates,
        }
    }

    /// Serialize to packet bytes (as sent in ClientboundLevelChunkWithLightPacket).
    pub fn write_to_buf(&self, buf: &mut Vec<u8>) {
        // sky_y_mask as BitSet: [longs_count: VarInt][long...]
        write_bitset(buf, self.sky_y_mask);
        write_bitset(buf, self.block_y_mask);
        write_bitset(buf, self.empty_sky_y_mask);
        write_bitset(buf, self.empty_block_y_mask);

        // Sky light arrays: [count: VarInt] then each [len: VarInt][bytes...]
        write_varint(buf, self.sky_updates.len() as i32);
        for arr in &self.sky_updates {
            write_varint(buf, arr.len() as i32);
            buf.extend_from_slice(arr);
        }

        // Block light arrays
        write_varint(buf, self.block_updates.len() as i32);
        for arr in &self.block_updates {
            write_varint(buf, arr.len() as i32);
            buf.extend_from_slice(arr);
        }
    }
}

fn write_bitset(buf: &mut Vec<u8>, bits: u64) {
    if bits == 0 {
        write_varint(buf, 0); // zero longs
    } else {
        write_varint(buf, 1);
        buf.extend_from_slice(&bits.to_be_bytes());
    }
}
```

### 13.4 — `ClientboundLevelChunkWithLightPacket`

```rust
// crates/oxidized-game/src/net/packets/clientbound.rs

pub struct ClientboundLevelChunkWithLightPacket {
    pub chunk_x: i32,
    pub chunk_z: i32,
    /// Heightmaps NBT + section buffer + block entities.
    pub chunk_data: ChunkPacketData,
    pub light_data: LightUpdateData,
}

pub struct ChunkPacketData {
    /// NBT CompoundTag with WORLD_SURFACE and MOTION_BLOCKING.
    pub heightmaps: NbtCompound,
    /// Concatenated serialized sections.
    pub buffer: Vec<u8>,
    /// Block entity summaries.
    pub block_entities: Vec<BlockEntityInfo>,
}

pub struct BlockEntityInfo {
    /// Packed XZ within chunk: (z << 4) | x
    pub packed_xz: u8,
    pub y: i16,
    pub block_entity_type: ResourceLocation,
    pub tag: Option<NbtCompound>,
}

impl ClientboundLevelChunkWithLightPacket {
    pub fn from_chunk(chunk: &LevelChunk) -> Self {
        Self {
            chunk_x: chunk.chunk_x,
            chunk_z: chunk.chunk_z,
            chunk_data: ChunkPacketData {
                heightmaps: serialize_heightmaps(chunk),
                buffer: serialize_chunk_sections(chunk),
                block_entities: collect_block_entities(chunk),
            },
            light_data: LightUpdateData::from_light(&chunk.light),
        }
    }

    pub fn encode(&self, buf: &mut Vec<u8>) {
        write_i32(buf, self.chunk_x);
        write_i32(buf, self.chunk_z);
        // ChunkPacketData
        write_nbt(buf, &self.chunk_data.heightmaps);
        write_varint(buf, self.chunk_data.buffer.len() as i32);
        buf.extend_from_slice(&self.chunk_data.buffer);
        write_block_entities(buf, &self.chunk_data.block_entities);
        // LightUpdateData
        self.light_data.write_to_buf(buf);
    }
}
```

### 13.5 — View-distance spiral iteration

Chunks are sent in a spiral from the player's current chunk position, closest
first. This is the same strategy used by vanilla's `ChunkMap`.

```rust
// crates/oxidized-game/src/chunk/view_distance.rs

use oxidized_world::chunk::ChunkPos;

/// Iterate chunk positions in a square spiral from `center`, out to `radius`.
/// Yields at most `(2*radius+1)^2` positions.
pub fn spiral_chunks(center: ChunkPos, radius: i32) -> impl Iterator<Item = ChunkPos> {
    let mut result = Vec::with_capacity(((2 * radius + 1) * (2 * radius + 1)) as usize);
    // Manhattan-distance shell by shell, then sort by Chebyshev distance.
    for r in 0..=radius {
        for dx in -r..=r {
            for dz in -r..=r {
                if dx.abs() == r || dz.abs() == r {
                    result.push(ChunkPos::new(center.x + dx, center.z + dz));
                }
            }
        }
    }
    result.into_iter()
}

/// Returns the set of chunks that a player at `new_center` needs but a player
/// at `old_center` did not, given `radius`.
pub fn chunks_to_load(
    old_center: ChunkPos,
    new_center: ChunkPos,
    radius: i32,
) -> Vec<ChunkPos> {
    spiral_chunks(new_center, radius)
        .filter(|&pos| chebyshev(pos, old_center) > radius)
        .collect()
}

/// Returns chunks in old view that are no longer in the new view.
pub fn chunks_to_unload(
    old_center: ChunkPos,
    new_center: ChunkPos,
    radius: i32,
) -> Vec<ChunkPos> {
    spiral_chunks(old_center, radius)
        .filter(|&pos| chebyshev(pos, new_center) > radius)
        .collect()
}

fn chebyshev(a: ChunkPos, b: ChunkPos) -> i32 {
    (a.x - b.x).abs().max((a.z - b.z).abs())
}
```

### 13.6 — Chunk-batch protocol

```rust
// crates/oxidized-game/src/chunk/chunk_sender.rs

/// Send a batch of chunks to the player, bracketed by batch start/finish packets.
/// After the batch, the client will send ServerboundChunkBatchReceivedPacket.
pub async fn send_chunk_batch(
    conn: &mut PlayerConnection,
    chunks: &[&LevelChunk],
) -> anyhow::Result<()> {
    // Signal start of batch (empty packet — no fields).
    conn.send(ClientboundChunkBatchStartPacket {}).await?;

    for chunk in chunks {
        let pkt = ClientboundLevelChunkWithLightPacket::from_chunk(chunk);
        conn.send(pkt).await?;
    }

    // Signal end of batch with the count of chunks sent.
    conn.send(ClientboundChunkBatchFinishedPacket {
        batch_size: chunks.len() as i32,
    }).await?;

    Ok(())
}

/// Handle ServerboundChunkBatchReceivedPacket from client.
/// `desired_chunks_per_tick` is a f32 the client sends as its preferred rate.
pub fn handle_chunk_batch_received(
    player: &mut ServerPlayer,
    desired_chunks_per_tick: f32,
) {
    // Adjust sending rate (vanilla uses this to throttle chunk sending).
    // For now, clamp to a sane range.
    player.chunk_send_rate = desired_chunks_per_tick.clamp(0.1, 64.0);
}
```

### 13.7 — Chunk unload packet

```rust
/// Send ClientboundForgetLevelChunkPacket when a chunk leaves a player's view.
pub async fn send_forget_chunk(
    conn: &mut PlayerConnection,
    chunk_x: i32,
    chunk_z: i32,
) -> anyhow::Result<()> {
    conn.send(ClientboundForgetLevelChunkPacket { chunk_x, chunk_z }).await
}
```

---

## Data Structures Summary

```
oxidized-game::net
  ├── ChunkPacketData                    — heightmaps + buffer + block_entities
  ├── ClientboundLevelChunkWithLightPacket — full chunk wire packet
  ├── LightUpdateData                    — bitmask + nibble arrays
  └── BlockEntityInfo                    — packed_xz + y + type + nbt

oxidized-game::chunk
  ├── spiral_chunks(center, radius)      — iterator in send order
  ├── chunks_to_load(old, new, r)        — delta on center change
  ├── chunks_to_unload(old, new, r)      — delta on center change
  └── send_chunk_batch(conn, chunks)     — batch start → N chunks → batch finish
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use oxidized_world::chunk::LevelChunk;

    /// Serialize an all-air chunk and verify:
    ///   - Each section starts with non_empty_block_count = 0 (i16 = 0x00 0x00)
    ///   - Each section has bits_per_entry = 0 for both block_states and biomes
    #[test]
    fn empty_chunk_section_wire_format() {
        let chunk = LevelChunk::new_empty(0, 0, BiomeId::PLAINS);
        let buf = serialize_chunk_sections(&chunk);

        // For each of 24 sections, first 6 bytes should be:
        //   non_empty_block_count: 0x00 0x00 (i16)
        //   fluid_count:           0x00 0x00 (i16)
        //   block_states bits:     0x00 (single-value palette)
        //   biomes bits:           0x00 (single-value palette)
        let mut cursor = 0;
        for section in 0..24 {
            assert_eq!(buf[cursor], 0, "section {section}: non_empty high byte");
            assert_eq!(buf[cursor + 1], 0, "section {section}: non_empty low byte");
            // bits_per_entry for block_states
            let block_bits_offset = cursor + 4;
            assert_eq!(buf[block_bits_offset], 0,
                "section {section}: block bits_per_entry should be 0 (single value)");
            // Skip to next section (varies; just check first 6 bytes per section).
            cursor += section_wire_size(&chunk.sections[section]);
        }
    }

    /// Heightmap serialization includes only WORLD_SURFACE and MOTION_BLOCKING.
    #[test]
    fn heightmap_nbt_contains_correct_keys() {
        let chunk = LevelChunk::new_empty(5, -3, BiomeId::PLAINS);
        let nbt = serialize_heightmaps(&chunk);
        assert!(nbt.contains_key("WORLD_SURFACE"));
        assert!(nbt.contains_key("MOTION_BLOCKING"));
        // Server-only keys must not be sent.
        assert!(!nbt.contains_key("OCEAN_FLOOR"));
        assert!(!nbt.contains_key("MOTION_BLOCKING_NO_LEAVES"));
    }

    /// Light packing: section with no light produces empty mask bits.
    #[test]
    fn light_data_empty_produces_zero_masks() {
        let light = LightData::new(26); // 24 sections + 2 border
        let data = LightUpdateData::from_light(&light);
        assert_eq!(data.sky_y_mask, 0);
        assert_eq!(data.block_y_mask, 0);
        assert!(data.sky_updates.is_empty());
        assert!(data.block_updates.is_empty());
    }

    /// A section with sky light set to 15 everywhere shows up in the mask.
    #[test]
    fn light_data_full_sky_light_section() {
        let mut light = LightData::new(26);
        let arr = Box::new([0xFFu8; 2048]);
        light.sky_light[1] = Some(arr);
        let data = LightUpdateData::from_light(&light);
        assert_eq!(data.sky_y_mask & (1 << 1), 1 << 1);
        assert_eq!(data.sky_updates.len(), 1);
        assert_eq!(data.sky_updates[0].len(), 2048);
    }

    /// Spiral iteration: center (0,0) radius 1 yields 9 chunks.
    #[test]
    fn spiral_chunks_radius_1_count() {
        let chunks: Vec<_> = spiral_chunks(ChunkPos::new(0, 0), 1).collect();
        assert_eq!(chunks.len(), 9);
        assert!(chunks.contains(&ChunkPos::new(0, 0)));
        assert!(chunks.contains(&ChunkPos::new(1, 0)));
        assert!(chunks.contains(&ChunkPos::new(-1, -1)));
    }

    /// chunks_to_load and chunks_to_unload are disjoint and complementary.
    #[test]
    fn load_unload_disjoint() {
        let old = ChunkPos::new(0, 0);
        let new = ChunkPos::new(1, 0);
        let r = 3;
        let to_load = chunks_to_load(old, new, r);
        let to_unload = chunks_to_unload(old, new, r);
        for pos in &to_load {
            assert!(!to_unload.contains(pos),
                "{pos:?} appears in both load and unload");
        }
    }
}
```
