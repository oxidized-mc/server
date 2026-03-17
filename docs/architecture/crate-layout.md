# Crate Layout

## Dependency Graph

```
oxidized-nbt
    ▲
    ├──── oxidized-protocol ◄── oxidized-macros (proc-macro)
    │
    ├──── oxidized-world
    │          ▲
    └──────────┼──── oxidized-game
                          ▲
              oxidized-server (binary)
```

**6 crates total** (5 library crates + 1 proc-macro crate).

**Rule:** No crate may import a crate that is higher in the graph.
Circular dependencies are forbidden.

---

## Crate Responsibilities

### `oxidized-nbt`
**No internal workspace dependencies.**

Provides three complementary NBT representations optimized for different access
patterns (see ADR-010):

- **Owned tree** — `NbtTag` enum with `NbtCompound(IndexMap<String, NbtTag>)`.
  Full in-place mutation for block entity updates, player data saves, and commands.
  `IndexMap` preserves insertion order (required by protocol hashing).
- **Arena** — `BumpNbt` allocated from a `bumpalo::Bump` arena. All tags for one
  document share a single allocation; the arena is freed in one shot. Used on the
  hot path (chunk deserialization — one arena per chunk).
- **Borrowed** — `BorrowedNbtCompound<'a>` for zero-copy field access over a
  `&'a [u8]` buffer. Lazily parses on demand without building a tree; ideal for
  reading a few known fields from a large payload (e.g. `DataVersion` check).

Additional responsibilities:
- All 13 NBT tag types (End through LongArray)
- Binary serialization: big-endian, named root tag
- Named Binary Tag I/O: GZIP-compressed, zlib-compressed, uncompressed
- SNBT (Stringified NBT): parser + printer
- `NbtAccounter`: memory budget enforcement (default cap: 64 MiB)
- Serde integration: `#[derive(Serialize, Deserialize)]` for Rust structs ↔ NBT
- Modified UTF-8 string encoding/decoding

Java reference: `net.minecraft.nbt.*`

---

### `oxidized-macros`
**No internal workspace dependencies.** Proc-macro crate (`proc-macro = true`).

Implements derive macros consumed by `oxidized-protocol` (see ADR-007):

- `#[derive(McPacket)]` — generates `McRead`, `McWrite`, and `Packet` trait impls
  from a struct definition annotated with `#[packet(id, state, direction)]`
- `McRead` — trait for decoding a type from the Minecraft wire format
- `McWrite` — trait for encoding a type to the Minecraft wire format
- Supports field-level attributes: `#[mc_if]` for conditional fields,
  `#[mc_length_prefix]` for length-prefixed collections

External dependencies: `syn`, `quote`, `proc-macro2`.

---

### `oxidized-protocol`
**Depends on:** `oxidized-nbt`, `oxidized-macros`

Responsible for:
- TCP framing: VarInt length-prefixed frames
- Typestate `Connection<S>` struct: protocol state encoded as a type parameter;
  invalid-state packet handling is a compile-time error (ADR-008). States:
  `Handshaking`, `Status`, `Login`, `Configuration`, `Play`
- Per-connection reader/writer task pair: reader decodes frames and dispatches via
  bounded `mpsc` channels; writer batches and flushes per tick (ADR-006)
- AES-128-CFB8 encryption pipeline (`CipherEncoder`/`CipherDecoder`)
- zlib compression pipeline (`CompressionEncoder`/`CompressionDecoder`)
- All packet types defined via `#[derive(McPacket)]` (~300 across 5 states):
  - `handshaking`: 1 serverbound
  - `status`: 2 clientbound + 2 serverbound
  - `login`: 5 clientbound + 5 serverbound
  - `configuration`: 6 clientbound + 6 serverbound
  - `play`: 127 clientbound + 58 serverbound
- `FriendlyByteBuf`: typed read/write helpers on top of `bytes::BytesMut`
- `RegistryFriendlyByteBuf`: registry-aware buffer for data-driven packets
- Packet registry: `(State, Direction, PacketId) → DecodeFn` dispatch table
- Mojang session server auth (HTTP POST to `sessionserver.mojang.com`)
- RSA-1024 key pair management

Java reference: `net.minecraft.network.*`

---

### `oxidized-world`
**Depends on:** `oxidized-nbt`

Responsible for:

**Coordinate types** (ADR-013) — newtype wrappers with compile-time safety:
- `BlockPos { x: i32, y: i32, z: i32 }` — block-level position, packed i64 for protocol
- `ChunkPos { x: i32, z: i32 }` — chunk column key
- `SectionPos { x: i32, y: i32, z: i32 }` — 16³ section address
- `Vec3 { x: f64, y: f64, z: f64 }` — entity position (double precision)
- `RegionPos { x: i32, z: i32 }` — region file address (32×32 chunks)
- `AABB` — axis-aligned bounding box for physics
- `Direction` enum — six cardinal/vertical directions
- Explicit conversion methods between coordinate systems (e.g. `block_pos.chunk_pos()`)

**Block state** (ADR-012) — flat `u16` state ID with dense lookup table:
- `BlockState(u16)` newtype; ~24 000 states fitting in `u16`
- Static `BLOCK_STATES: &[BlockStateData]` array indexed by state ID — O(1) property
  queries, flag checks, collision shape access, light emission/opacity
- `BlockStateFlags` bitfield, precomputed neighbor transition tables
- Collision shape deduplication (~300 unique `VoxelShape` entries)
- Generated at compile time from `mc-server-ref/generated/` via `build.rs`

**Registries** (ADR-011) — hybrid compiled core + runtime data-driven:
- Core (compiled): `Block`, `Item`, `EntityType`, `BlockEntityType`, `Fluid` —
  generated by `build.rs` from extracted vanilla JSON. Dense `Vec` for ID→entry,
  PHF for name→ID. Integer IDs match vanilla exactly.
- Data-driven (runtime): biomes, enchantments, damage types, trim materials, recipes,
  loot tables, advancements — loaded from JSON at startup with data pack override
  support. Frozen into `Arc<FrozenRegistry<T>>` after loading.
- Common `RegistryAccess<T>` trait for uniform O(1) queries by ID or key
- Tags: `FrozenSet<u32>` membership, loaded from data packs

**Chunk storage** (ADR-014) — `DashMap<ChunkPos, Arc<ChunkColumn>>`:
- Per-section `RwLock<ChunkSection>` for fine-grained concurrency (24 sections per
  overworld chunk column)
- `PalettedContainer<BlockState>`: adaptive storage (single-value, linear palette,
  HashMap palette, direct global IDs)
- Ticket-based chunk lifecycle (entity-ticking → ticking → loaded → unloaded)
- Heightmaps, block entities, dirty tracking, LRU unload queue

**World generation** (ADR-016):
- `ChunkGenerator` trait, `FlatLevelSource`, `NoiseBasedChunkGenerator`
- Rayon thread pool for parallel CPU-bound worldgen
- Status pipeline: `EMPTY → … → FEATURES → LIGHT → LOADED`

**Other**:
- Anvil region format: read/write `.mca` files
- `LevelStorageSource` / `PrimaryLevelData`: world folder, `level.dat`
- Lighting engine: `BlockLightEngine`, `SkyLightEngine`
- Physics helpers: AABB intersection, block collision shapes

Java reference: `net.minecraft.world.*`, `net.minecraft.core.*`

---

### `oxidized-game`
**Depends on:** `oxidized-nbt`, `oxidized-protocol`, `oxidized-world`

Uses **bevy_ecs** as a standalone library for entity representation and processing
(see ADR-018). ECS architecture: entities are opaque IDs, state is decomposed into
components, behavior is implemented as systems.

**Entity components** — every field in vanilla's hierarchy maps to a named component:
- *Core* (`Entity` base): `Position(DVec3)`, `Velocity(DVec3)`,
  `Rotation { yaw, pitch }`, `OnGround(bool)`, `FallDistance(f32)`,
  `EntityFlags(u8)`, `NoGravity`, `Silent`, `CustomName`, `TickCount(u32)`,
  `BoundingBox(AABB)`, `EntityType(ResourceLocation)`
- *Living*: `Health { current, max }`, `Equipment(EquipmentSlots)`,
  `ActiveEffects(HashMap<MobEffect, EffectInstance>)`, `ArmorValue(f32)`,
  `Attributes(AttributeMap)`, `DeathTime(u16)`, `LivingEntityMarker`
- *Mob*: `AiGoals`, `NavigationPath`, marker components per mob type
  (`ZombieMarker`, `SkeletonMarker`, `CreeperMarker`, …)
- *Player*: `PlayerInventory`, `GameMode`, `FoodData`, `ExperienceData`,
  `Abilities(PlayerAbilities)`, `SelectedSlot(u8)`, `PlayerMarker`

**Entity systems** — phased execution per tick:
1. Pre-tick (spawns/despawns, `TickCount` increment)
2. Physics (gravity, velocity, collisions, `OnGround`)
3. AI (`GoalSelector`, pathfinding)
4. Entity behavior (type-specific: zombie burning, creeper timer, …)
5. Status effects (apply/expire potion effects)
6. Post-tick (bounding boxes, chunk section tracking)
7. Network sync (`SynchedEntityData` dirty serialization)

`bevy_ecs` automatically parallelizes non-conflicting systems within each phase.

**Other responsibilities:**
- `ServerLevel`: per-dimension world with tick loop and chunk management
- `PlayerList`: login/logout, player tracking, tab list
- Combat: damage sources, death/respawn
- Inventory: `PlayerInventory`, `AbstractContainerMenu`, container types
- Crafting: recipe loading, recipe matching
- Commands: Brigadier dispatcher, all 96 vanilla commands
- Advancements, scoreboard, teams, boss bars
- Loot tables, enchantments, potion effects
- Block entities (chest, furnace, sign, spawner, …)

Java reference: `net.minecraft.server.*`, `net.minecraft.world.entity.*`,
`net.minecraft.world.inventory.*`, `net.minecraft.server.commands.*`

---

### `oxidized-server`
**Depends on:** all crates

Responsible for:
- Binary entry point (`main.rs`)
- CLI argument parsing
- `oxidized.toml` loading (`DedicatedServerProperties`)
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
│   ├── compound.rs       # NbtCompound (IndexMap-backed)
│   ├── arena.rs          # BumpNbt arena-allocated representation
│   ├── borrowed.rs       # BorrowedNbtCompound<'a> zero-copy reader
│   ├── io.rs             # NbtIo: read/write with compression
│   ├── snbt.rs           # SNBT parser + printer
│   ├── accounter.rs      # NbtAccounter memory budget
│   ├── string.rs         # Modified UTF-8 encode/decode
│   └── serde.rs          # Serde integration (feature-gated)
│
├── oxidized-macros/src/
│   ├── lib.rs            # proc-macro entry points
│   ├── packet.rs         # #[derive(McPacket)] expansion
│   ├── read.rs           # McRead derive logic
│   └── write.rs          # McWrite derive logic
│
├── oxidized-protocol/src/
│   ├── lib.rs
│   ├── codec/
│   │   ├── varint.rs     # VarInt/VarLong encode/decode
│   │   ├── frame.rs      # Length-prefix framing codec
│   │   ├── cipher.rs     # AES-CFB8 encoder/decoder
│   │   └── compress.rs   # zlib encoder/decoder
│   ├── connection.rs     # Connection<S> typestate, reader/writer tasks
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
│   │   ├── region_pos.rs
│   │   ├── vec3.rs
│   │   ├── aabb.rs
│   │   └── direction.rs
│   ├── block/
│   │   ├── state.rs      # BlockState(u16), BlockStateData, dense table
│   │   ├── flags.rs      # BlockStateFlags bitfield
│   │   ├── properties.rs # Block properties + transition tables
│   │   └── shapes.rs     # VoxelShape collision shape table
│   ├── registry/
│   │   ├── core.rs       # Compiled registries (build.rs output)
│   │   ├── data_driven.rs # Runtime JSON-loaded registries
│   │   ├── access.rs     # RegistryAccess<T> trait
│   │   └── tags.rs       # Tag resolution + FrozenSet
│   ├── item/
│   │   ├── registry.rs
│   │   ├── stack.rs
│   │   └── components.rs
│   ├── chunk/
│   │   ├── palette.rs    # PalettedContainer
│   │   ├── section.rs    # ChunkSection (per-section RwLock)
│   │   ├── column.rs     # ChunkColumn (Arc-wrapped)
│   │   ├── map.rs        # ChunkMap (DashMap)
│   │   ├── ticket.rs     # Ticket system + lifecycle
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
│       ├── density.rs
│       └── scheduler.rs  # Rayon worldgen scheduler
│
├── oxidized-game/src/
│   ├── lib.rs
│   ├── level/
│   │   ├── server_level.rs
│   │   └── chunk_map.rs
│   ├── entity/
│   │   ├── components.rs    # All component definitions (core, living, mob, player)
│   │   ├── systems.rs       # System registration + phase ordering
│   │   ├── bundles.rs       # Spawn templates per entity type
│   │   ├── synced_data.rs   # SynchedEntityData dirty tracking
│   │   └── types/           # Per-type marker components + behavior systems
│   │       ├── zombie.rs
│   │       ├── skeleton.rs
│   │       ├── creeper.rs
│   │       ├── villager.rs
│   │       └── ...
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
    ├── config.rs         # oxidized.toml
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

// oxidized-macros/src/lib.rs
// No internal deps — proc-macro crate; depends only on syn/quote/proc-macro2

// oxidized-protocol/src/lib.rs
// Depends on oxidized-nbt + oxidized-macros — enforced by Cargo.toml

// oxidized-world/src/lib.rs
// Only depends on oxidized-nbt — enforced by Cargo.toml
```

The CI `cargo deny` check also validates the dependency tree does not accidentally
introduce disallowed transitive dependencies.

---

## Related ADRs

| ADR | Topic | Relevance |
|-----|-------|-----------|
| [ADR-003](../adr/adr-003-crate-architecture.md) | Crate workspace architecture | Defines the 5+1 crate split and dependency DAG |
| [ADR-006](../adr/adr-006-network-io.md) | Network I/O architecture | Per-connection reader/writer task pair in oxidized-protocol |
| [ADR-007](../adr/adr-007-packet-codec.md) | Packet codec framework | `#[derive(McPacket)]` in oxidized-macros |
| [ADR-008](../adr/adr-008-connection-state-machine.md) | Connection state machine | Typestate `Connection<S>` in oxidized-protocol |
| [ADR-010](../adr/adr-010-nbt.md) | NBT library design | Three representations in oxidized-nbt |
| [ADR-011](../adr/adr-011-registry-system.md) | Registry system | Hybrid registries in oxidized-world |
| [ADR-012](../adr/adr-012-block-state.md) | Block state representation | Flat u16 ID + dense lookup in oxidized-world |
| [ADR-013](../adr/adr-013-coordinate-types.md) | Coordinate types | Newtype wrappers in oxidized-world |
| [ADR-014](../adr/adr-014-chunk-storage.md) | Chunk storage & concurrency | DashMap + per-section RwLock in oxidized-world |
| [ADR-016](../adr/adr-016-worldgen-pipeline.md) | Worldgen pipeline | Rayon parallel generation in oxidized-world |
| [ADR-018](../adr/adr-018-entity-system.md) | Entity system | bevy_ecs in oxidized-game |
