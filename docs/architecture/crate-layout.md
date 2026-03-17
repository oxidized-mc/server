# Crate Layout

## Dependency Graph

```
oxidized-nbt
    ▲
    │
    ├──── oxidized-protocol
    │
    ├──── oxidized-world
    │          ▲
    │          │
    └──────────┼──── oxidized-game
                          ▲
                          │
              oxidized-server (binary)
```

**Rule:** No crate may import a crate that is higher in the graph.
Circular dependencies are forbidden.

---

## Crate Responsibilities

### `oxidized-nbt`
**No internal workspace dependencies.**

Responsible for:
- All 13 NBT tag types (End, Byte, Short, Int, Long, Float, Double, ByteArray,
  String, List, Compound, IntArray, LongArray)
- Binary serialization: big-endian, named root tag
- Named Binary Tag I/O: GZIP-compressed, zlib-compressed, uncompressed
- SNBT (Stringified NBT): parser + printer
- `NbtAccounter`: memory budget enforcement (default cap: 67 MB)
- Serde integration: derive `NbtSerialize`/`NbtDeserialize` for Rust structs

Java reference: `net.minecraft.nbt.*`

---

### `oxidized-protocol`
**Depends on:** `oxidized-nbt`

Responsible for:
- TCP framing: VarInt length-prefixed frames
- `Connection` struct: Tokio read/write halves, send queue, protocol state
- AES-128-CFB8 encryption pipeline (`CipherEncoder`/`CipherDecoder`)
- zlib compression pipeline (`CompressionEncoder`/`CompressionDecoder`)
- All packet types (185 total across 5 states):
  - `handshaking`: 1 serverbound
  - `status`: 2 clientbound + 2 serverbound
  - `login`: 5 clientbound + 5 serverbound
  - `configuration`: 6 clientbound + 6 serverbound
  - `play`: 127 clientbound + 58 serverbound
- `FriendlyByteBuf`: typed read/write helpers on top of `bytes::BytesMut`
- `RegistryFriendlyByteBuf`: registry-aware buffer for data-driven packets
- Protocol state machine: `Handshaking → Status/Login → Configuration → Play`
- Mojang session server auth (HTTP POST to `sessionserver.mojang.com`)
- RSA-1024 key pair management

Java reference: `net.minecraft.network.*`

---

### `oxidized-world`
**Depends on:** `oxidized-nbt`

Responsible for:
- Core coordinate types: `BlockPos`, `ChunkPos`, `SectionPos`, `Vec3`, `AABB`
- Block registry: `Block`, `BlockState`, `PalettedContainer`, global palette (from `blocks.json`)
- Item registry: `Item`, `ItemStack`, `DataComponentMap` (from `items.json`)
- Chunk data structures: `LevelChunkSection`, `LevelChunk`, heightmaps, light data
- Anvil region format: read/write `.mca` files
- `LevelStorageSource`: resolve world folder, `level.dat`
- `PrimaryLevelData`: spawn point, game rules, time, weather, data version
- Biome registry (from `worldgen/biome/*.json`)
- World generation trait: `ChunkGenerator`, `FlatLevelSource`, `NoiseBasedChunkGenerator`
- Lighting engine: `BlockLightEngine`, `SkyLightEngine`
- Physics helpers: AABB intersection, block collision shapes

Java reference: `net.minecraft.world.*`, `net.minecraft.core.*`

---

### `oxidized-game`
**Depends on:** `oxidized-nbt`, `oxidized-protocol`, `oxidized-world`

Responsible for:
- `ServerLevel`: per-dimension world with tick loop, entity tracking, chunk management
- `Entity` base + full hierarchy: `LivingEntity`, `Mob`, `Player`, `Animal`, `Monster`
- `SynchedEntityData`: entity metadata sync to clients
- AI system: `GoalSelector`, `PathfinderGoal`, `PathNavigation` (A*), `Brain`/`BehaviorControl`
- `ServerPlayer`: player state, inventory, game mode, experience
- `PlayerList`: login/logout, player tracking, tab list
- Combat: damage sources, `LivingEntity::hurt()`, death/respawn
- Inventory: `PlayerInventory`, `AbstractContainerMenu`, all container types
- Crafting: recipe loading, `ShapedRecipe`, `ShapelessRecipe`, recipe matching
- Commands: Brigadier dispatcher, all 96 vanilla commands
- Advancements: trigger system, per-player progress
- Scoreboard + teams + boss bars
- Loot tables
- Enchantments + potion effects
- Block entities (chest, furnace, sign, spawner, …)
- Mob AI goals (zombies, skeletons, creepers, villagers, …)

Java reference: `net.minecraft.server.*`, `net.minecraft.world.entity.*`,
`net.minecraft.world.inventory.*`, `net.minecraft.server.commands.*`

---

### `oxidized-server`
**Depends on:** all crates

Responsible for:
- Binary entry point (`main.rs`)
- CLI argument parsing
- `server.properties` loading (`DedicatedServerProperties`)
- `MinecraftServer` / `DedicatedServer`: main server struct, startup sequence
- Tick loop: `tokio::time::interval(50ms)` driving `ServerLevel::tick()`
- RCON server (TCP, port 25575)
- Query server (UDP, port 25565)
- JSON-RPC management WebSocket server (new in 26.1)
- Graceful shutdown (Ctrl+C, `/stop`, watchdog)
- `ServerTickRateManager`: sprint mode, pause-when-empty
- Logging setup (`tracing-subscriber` with env filter)

Java reference: `net.minecraft.server.dedicated.*`, `net.minecraft.server.MinecraftServer`

---

## Module Layout (target structure)

```
crates/
├── oxidized-nbt/src/
│   ├── lib.rs
│   ├── tag.rs            # NbtTag enum + all 13 types
│   ├── compound.rs       # CompoundTag operations
│   ├── io.rs             # NbtIo: read/write with compression
│   ├── snbt.rs           # SNBT parser + printer
│   ├── accounter.rs      # NbtAccounter memory budget
│   └── serde.rs          # Serde integration (feature-gated)
│
├── oxidized-protocol/src/
│   ├── lib.rs
│   ├── codec/
│   │   ├── varint.rs     # VarInt/VarLong encode/decode
│   │   ├── frame.rs      # Length-prefix framing codec
│   │   ├── cipher.rs     # AES-CFB8 encoder/decoder
│   │   └── compress.rs   # zlib encoder/decoder
│   ├── connection.rs     # Connection struct, state machine
│   ├── buf.rs            # FriendlyByteBuf
│   └── packets/
│       ├── handshake/
│       ├── status/
│       ├── login/
│       ├── configuration/
│       └── play/
│           ├── clientbound/   # 127 packets
│           └── serverbound/   # 58 packets
│
├── oxidized-world/src/
│   ├── lib.rs
│   ├── core/
│   │   ├── block_pos.rs
│   │   ├── chunk_pos.rs
│   │   ├── section_pos.rs
│   │   ├── vec3.rs
│   │   ├── aabb.rs
│   │   └── direction.rs
│   ├── block/
│   │   ├── registry.rs   # Block + BlockState + global palette
│   │   ├── state.rs      # BlockState properties
│   │   └── properties.rs
│   ├── item/
│   │   ├── registry.rs
│   │   ├── stack.rs
│   │   └── components.rs
│   ├── chunk/
│   │   ├── palette.rs    # PalettedContainer
│   │   ├── section.rs    # LevelChunkSection
│   │   ├── chunk.rs      # LevelChunk
│   │   └── heightmap.rs
│   ├── storage/
│   │   ├── region.rs     # Anvil .mca read/write
│   │   ├── loader.rs     # AnvilChunkLoader
│   │   └── level_data.rs # PrimaryLevelData / level.dat
│   ├── lighting/
│   │   ├── block_light.rs
│   │   └── sky_light.rs
│   └── gen/
│       ├── flat.rs
│       ├── noise.rs
│       └── density.rs
│
├── oxidized-game/src/
│   ├── lib.rs
│   ├── level/
│   │   ├── server_level.rs
│   │   └── chunk_map.rs
│   ├── entity/
│   │   ├── base.rs       # Entity
│   │   ├── living.rs     # LivingEntity
│   │   ├── mob.rs        # Mob
│   │   ├── player.rs     # ServerPlayer
│   │   ├── synced_data.rs
│   │   └── types/        # Zombie, Skeleton, Cow, …
│   ├── ai/
│   │   ├── goal.rs
│   │   ├── pathfinding.rs
│   │   └── brain.rs
│   ├── commands/
│   │   ├── dispatcher.rs
│   │   └── impls/        # 96 command files
│   ├── inventory/
│   │   ├── player_inv.rs
│   │   └── containers/
│   └── crafting/
│       ├── recipe.rs
│       └── shaped.rs
│
└── oxidized-server/src/
    ├── main.rs
    ├── server.rs         # MinecraftServer / DedicatedServer
    ├── config.rs         # server.properties
    ├── tick.rs           # ServerTickRateManager
    ├── rcon.rs
    ├── query.rs
    └── management.rs     # JSON-RPC WebSocket
```

---

## Dependency Rule Enforcement

Add to each `lib.rs` as a compile-time guardrail:

```rust
// oxidized-nbt/src/lib.rs
// No internal deps — enforced by Cargo graph

// oxidized-protocol/src/lib.rs
// Only depends on oxidized-nbt — enforced by Cargo.toml

// oxidized-world/src/lib.rs
// Only depends on oxidized-nbt — enforced by Cargo.toml
```

The CI `cargo deny` check also validates the dependency tree does not accidentally
introduce disallowed transitive dependencies.
