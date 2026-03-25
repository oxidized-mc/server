# World Format

## Overview

This document covers Minecraft's world storage, chunk representation, and the
binary formats Oxidized must read and write.

**Java references:**
- `net.minecraft.world.level.chunk.*`
- `net.minecraft.world.level.storage.*`
- `net.minecraft.nbt.NbtIo`

---

## World Folder Layout

```
<level-name>/
├── level.dat                     # PrimaryLevelData (NBT, GZipped)
├── level.dat_old                 # Previous backup of level.dat
├── session.lock                  # File lock
├── playerdata/
│   └── <uuid>.dat                # Per-player data (NBT, GZipped)
├── advancements/
│   └── <uuid>.json               # Per-player advancement progress
├── stats/
│   └── <uuid>.json               # Per-player statistics
├── data/
│   ├── scoreboard.dat            # Scoreboard data
│   ├── raids.dat                 # Raid data
│   └── map_<id>.dat              # Cartography maps
├── region/                       # Overworld chunks
│   └── r.<rx>.<rz>.mca
├── DIM-1/region/                 # Nether chunks
│   └── r.<rx>.<rz>.mca
└── DIM1/region/                  # The End chunks
    └── r.<rx>.<rz>.mca
```

---

## Anvil Region Format (`.mca`)

Each region file covers a 32×32 chunk area.

### Header (8 KiB total)

```
Offset 0: Location table (4096 bytes)
  1024 entries × 4 bytes:
  [offset: u24 big-endian][sector_count: u8]
  - offset is in 4096-byte sectors from file start
  - 0,0 entry means chunk not present

Offset 4096: Timestamp table (4096 bytes)
  1024 entries × 4 bytes (u32 big-endian): Unix timestamp of last save
```

Chunk index in table: `((x & 31) + (z & 31) * 32)`

### Chunk Data

At `offset * 4096`:

```
[length: u32 big-endian]        ← byte count of what follows (including compression_type byte)
[compression_type: u8]
  1 = GZip    (RFC 1952) — legacy, rarely used
  2 = Zlib    (RFC 1950) — standard
  3 = None    (uncompressed)
  4 = LZ4     (newer)
[compressed_data: bytes]        ← (length - 1) bytes
```

Decompressed data is an NBT `CompoundTag` (the chunk NBT).

Java reference: `net.minecraft.world.level.storage.RegionFile`

---

## Chunk NBT Format

Top-level keys in the chunk `CompoundTag`:

```
DataVersion: Int           ← 4786 for 26.1
xPos: Int
zPos: Int
yPos: Int                  ← bottom section Y index (−4 for overworld)
Status: String             ← ChunkStatus: "minecraft:full", "minecraft:empty", …
LastUpdate: Long
InhabitedTime: Long        ← cumulative tick time players have been in this chunk
isLightOn: Byte (bool)
sections: List[Compound]   ← list of LevelChunkSection NBT (only non-empty)
block_entities: List[Compound]
Heightmaps: Compound {
    MOTION_BLOCKING: LongArray
    MOTION_BLOCKING_NO_LEAVES: LongArray
    OCEAN_FLOOR: LongArray
    WORLD_SURFACE: LongArray
}
fluid_ticks: List[Compound]      ← { i: String, t: Int, p: Int, x/y/z: Int }
block_ticks: List[Compound]
PostProcessing: List[List[Short]]
structures: Compound { ... }
blending_data: Compound (optional)
```

### Section NBT

```
Y: Byte                    ← section Y index (−4 to 19 for overworld)
block_states: Compound {
    palette: List[Compound]   ← each: { Name: String, Properties: Compound }
    data: LongArray           ← packed bit data; absent if palette has 1 entry
}
biomes: Compound {
    palette: List[String]     ← biome ResourceLocations
    data: LongArray           ← absent if palette has 1 entry
}
BlockLight: ByteArray(2048)  ← 4 bits per block, XZY order
SkyLight: ByteArray(2048)
```

Java reference: `net.minecraft.world.level.chunk.storage.ChunkSerializer`

---

## PalettedContainer Binary Format (Network)

Used in `ClientboundLevelChunkWithLightPacket` (not the same as NBT format).

```
[bits_per_entry: u8]
[palette data]            ← depends on bits_per_entry
[data_array_length: VarInt]
[data: VarInt[]]          ← packed 64-bit longs
```

### Palette types by bits_per_entry

| bits_per_entry | Palette type | Palette data |
|---|---|---|
| 0 | SingleValue | `[single_value: VarInt]` |
| 1–4 | Linear (indirect) | `[count: VarInt][entry: VarInt × count]` |
| 5–8 | HashMap (indirect) | `[count: VarInt][entry: VarInt × count]` |
| ≥9 (blocks) / ≥4 (biomes) | Global (direct) | _(no palette data)_ |

### Bit packing

```
Values are packed into 64-bit longs, starting from the LSB.
Each long contains floor(64 / bits_per_entry) entries.
Values do NOT span long boundaries.

Example: bits_per_entry = 5, 12 entries per long
  long[0] bits 0–4: entry[0]
  long[0] bits 5–9: entry[1]
  …
  long[0] bits 55–59: entry[11]
  long[0] bits 60–63: unused (0)
  long[1] bits 0–4: entry[12]
  …
```

Java reference: `net.minecraft.world.level.chunk.storage.PackedBitStorage`,
`net.minecraft.world.level.chunk.PalettedContainer`

---

## Heightmap Format

Each heightmap is stored as a `LongArray` with 256 entries (16×16 columns)
packed at `ceil(log2(max_height + 1))` bits per entry.

For the overworld (−64 to 320): 9 bits per entry.
`36 longs` hold all 256 columns (256 × 9 = 2304 bits; 36 × 64 = 2304 bits).

Types:
- `WORLD_SURFACE`: highest non-air block
- `OCEAN_FLOOR`: highest non-fluid block
- `MOTION_BLOCKING`: highest block that blocks motion or fluid
- `MOTION_BLOCKING_NO_LEAVES`: same but ignoring leaves

---

## level.dat NBT

Top-level: `CompoundTag { Data: CompoundTag { … } }`

Key fields inside `Data`:

```
DataVersion: Int                   ← 4786
Version: Compound { Id: Int, Name: String, Snapshot: Byte }
LevelName: String
SpawnX: Int
SpawnY: Int
SpawnZ: Int
SpawnAngle: Float
GameType: Int                      ← 0=survival, 1=creative, 2=adventure, 3=spectator
Difficulty: Byte
hardcore: Byte (bool)
allowCommands: Byte (bool)
DayTime: Long                      ← time of day (mod 24000)
Time: Long                         ← total world age in ticks
raining: Byte (bool)
thundering: Byte (bool)
rainTime: Int
thunderTime: Int
clearWeatherTime: Int
generatorName: String              ← "default", "flat", "largeBiomes", etc.
generatorSettings: Compound
GameRules: Compound { key: String, … }
WorldGenSettings: Compound {
    seed: Long
    generate_features: Byte
    dimensions: Compound { … }
}
```

Java reference: `net.minecraft.world.level.storage.PrimaryLevelData`

---

## Player Data NBT (`playerdata/<uuid>.dat`)

```
DataVersion: Int
Pos: List[Double × 3]             ← x, y, z
Rotation: List[Float × 2]         ← yaw, pitch
Motion: List[Double × 3]
Dimension: String                  ← "minecraft:overworld" etc.
OnGround: Byte (bool)
Health: Float
FoodLevel: Int
FoodSaturationLevel: Float
FoodExhaustionLevel: Float
XpTotal: Int
XpLevel: Int
XpP: Float                         ← progress toward next level (0.0–1.0)
Inventory: List[Compound]          ← ItemStack compounds with Slot: Byte
EnderItems: List[Compound]
Score: Int
playerGameType: Int
previousPlayerGameType: Int
Attributes: List[Compound]
ActiveEffects: List[Compound]
```

Java reference: `net.minecraft.world.entity.player.Player`,
`net.minecraft.server.level.ServerPlayer`

---

## Light Data (Network)

In `ClientboundLightUpdatePacketData`:

```
sky_y_mask: BitSet                 ← which Y sections have sky light data
block_y_mask: BitSet
empty_sky_y_mask: BitSet           ← sections with all-zeros sky light
empty_block_y_mask: BitSet
sky_updates: Vec<Vec<u8>>          ← 2048 bytes per set section
block_updates: Vec<Vec<u8>>        ← 2048 bytes per set section
```

Each 2048-byte array: nibble-packed (4 bits per block), order is `Y * 256 + Z * 16 + X`.

Java reference: `net.minecraft.network.protocol.game.ClientboundLightUpdatePacketData`

---

## In-Memory Representation

The on-disk and on-wire formats described above define the **contract with vanilla
clients and world files**. Oxidized's in-memory representation diverges from vanilla
for performance.

### Coordinate Types ([ADR-013](../adr/adr-013-coordinate-types.md))

All coordinates use **newtype wrappers** to prevent mixing x/y/z/chunk/section
coordinates at compile time:

```rust
pub struct BlockPos { pub x: i32, pub y: i32, pub z: i32 }
pub struct ChunkPos { pub x: i32, pub z: i32 }
pub struct SectionPos { pub x: i32, pub y: i32, pub z: i32 }
pub struct RegionPos { pub x: i32, pub z: i32 }
pub struct Vec3 { pub x: f64, pub y: f64, pub z: f64 }
```

Conversions between types are explicit methods (`BlockPos::chunk_pos()`,
`ChunkPos::region_pos()`, etc.).

### Block States ([ADR-012](../adr/adr-012-block-state.md))

Block states use a **flat `u16` state ID** with a dense lookup table:

```rust
pub struct BlockState(u16);  // global palette index

static BLOCK_STATES: &[BlockStateData] = &[ /* generated by build.rs */ ];

impl BlockState {
    pub fn data(&self) -> &BlockStateData { &BLOCK_STATES[self.0 as usize] }
    pub fn is_air(&self) -> bool { self.data().is_air }
    pub fn is_solid(&self) -> bool { self.data().is_solid }
}
```

No `HashMap` lookups in the hot path — property access is a single array index.
The lookup table is generated at compile time from vanilla `blocks.json` via `build.rs`.

### Chunk Storage ([ADR-014](../adr/adr-014-chunk-storage.md))

In memory, chunks use a concurrent map with per-section locking:

```rust
// Level-wide chunk map (lock-free concurrent reads)
type ChunkMap = DashMap<ChunkPos, Arc<ChunkColumn>>;

pub struct ChunkColumn {
    sections: [RwLock<ChunkSection>; 24],  // per-section fine-grained locking
    heightmaps: Heightmaps,
    block_entities: DashMap<BlockPos, BlockEntity>,
    status: ChunkStatus,
}
```

`DashMap` provides concurrent read access from multiple Tokio tasks (chunk sending)
without blocking the tick thread. `RwLock` per section allows parallel reads of
different Y-levels.

### Disk I/O ([ADR-015](../adr/adr-015-disk-io.md))

Chunk loading and saving uses `tokio::task::spawn_blocking` with write coalescing:

- **Loads**: queued by the tick thread, executed on the blocking pool, results polled next tick
- **Saves**: dirty chunks added to a coalescing queue; multiple saves to the same region file
  are batched into a single I/O operation
- **Auto-save**: every 6000 ticks (5 minutes), dirty chunks are flushed

### World Generation ([ADR-016](../adr/adr-016-worldgen-pipeline.md))

Chunk generation runs on a **rayon thread pool** (CPU-bound, not Tokio):

- `FlatLevelSource`: direct generation, no threading needed
- `NoiseBasedChunkGenerator`: rayon `par_iter` over chunk batch, density functions computed in parallel
- Results are sent back to the tick thread via a channel

---

## Related ADRs

- [ADR-012: Block State Representation](../adr/adr-012-block-state.md) — flat u16 IDs
- [ADR-013: Coordinate Types](../adr/adr-013-coordinate-types.md) — newtype wrappers
- [ADR-014: Chunk Storage](../adr/adr-014-chunk-storage.md) — DashMap + per-section RwLock
- [ADR-015: Disk I/O](../adr/adr-015-disk-io.md) — spawn_blocking + write coalescing
- [ADR-016: Worldgen Pipeline](../adr/adr-016-worldgen-pipeline.md) — rayon thread pool
- [ADR-017: Lighting Engine](../adr/adr-017-lighting.md) — propagation algorithm
