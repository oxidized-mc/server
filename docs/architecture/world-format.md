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
DataVersion: Int           ← 4782 for 26.1
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
DataVersion: Int                   ← 4782
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
