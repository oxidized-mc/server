# Architecture Overview

## What is Oxidized?

Oxidized is a full reimplementation of the Minecraft Java Edition **26.1** server in Rust.
It is wire-protocol compatible with the vanilla client — any 26.1 client can connect to it.
The internals are completely rewritten using idiomatic Rust: async Tokio for I/O, `bevy_ecs`
for entities, and a dedicated tick thread with parallel phase execution.

---

## Guiding Principles

| Principle | Meaning |
|---|---|
| **Correctness first** | The reference is the decompiled vanilla Java source. When in doubt, match vanilla behaviour exactly. |
| **Async I/O everywhere** | All network and disk I/O uses Tokio. The game loop never blocks. |
| **No unsafe (by default)** | `#![deny(unsafe_code)]` on all library crates unless documented with a `SAFETY:` comment. |
| **No `unwrap` in production** | All error paths use `?` or typed errors. `unwrap()` is a compile-time warning. |
| **Data-oriented where it matters** | Chunk storage, entity tracking, and block states use compact, cache-friendly layouts. |
| **Reference before implementation** | Every module has a `## Java Reference` section in its phase doc pointing to the source class. |

---

## Crate Architecture

Six crates in a strict dependency DAG
([ADR-003](../adr/adr-003-crate-architecture.md)):

```
oxidized-nbt         (leaf — no internal deps)
oxidized-macros      (proc-macro leaf — no internal deps)
    ↑           ↑
oxidized-protocol   oxidized-world
    ↑           ↑       ↑
    oxidized-game ──────┘
         ↑
    oxidized-server    (binary entry point)
```

| Crate | Responsibility |
|---|---|
| `oxidized-nbt` | NBT binary format codec. Zero Minecraft-specific knowledge. |
| `oxidized-macros` | Proc-macro crate: `#[derive(McPacket, McRead, McWrite)]` for compile-time codegen. |
| `oxidized-protocol` | ~300 packet structs, 5 protocol states, wire types (`VarInt`, `Position`, etc.). Depends on `nbt`. |
| `oxidized-world` | Anvil I/O, chunk storage, block state registry, lighting, world generation. Depends on `nbt`. |
| `oxidized-game` | Game simulation: ECS tick phases, entity logic, commands, crafting, loot. Depends on `protocol` + `world`. |
| `oxidized-server` | Binary entry point: CLI, config, TCP listener, RCON/Query, shutdown. Depends on all crates. |

Boundaries are compiler-enforced — `oxidized-nbt` physically cannot import networking
types. Each crate is independently testable with `cargo test -p <crate>`.

---

## Threading Model

| Work type | Runs on | Rationale |
|---|---|---|
| Network I/O (read/write) | Tokio I/O thread pool | Async epoll/io_uring via Tokio ([ADR-001](../adr/adr-001-async-runtime.md)) |
| Game tick loop (6 phases) | **Dedicated OS thread** | Avoids Tokio scheduler interference ([ADR-019](../adr/adr-019-tick-loop.md)) |
| ECS parallel systems | Tick thread + rayon worker pool | `bevy_ecs` schedules non-conflicting systems across cores |
| Chunk generation (CPU) | **Rayon** thread pool | CPU-bound work stays off Tokio's executor |
| Disk I/O (load/save) | `tokio::task::spawn_blocking` | Blocking FS calls must not starve async tasks |
| RCON / Query server | Separate Tokio tasks | Lightweight; shares the async runtime |

The tick thread **never calls Tokio spawn or await** — it communicates with the Tokio
runtime exclusively through `mpsc` channels. This keeps the 50 ms budget predictable.

---

## Runtime Architecture

```
Tokio Runtime (multi-threaded, work-stealing)
│
├── TcpListener::accept() loop
│     │
│     └── per-connection task pair (ADR-006) ──────────────────────────┐
│           ├── Reader Task: socket → decrypt → decompress → decode    │
│           │       → inbound_tx (mpsc, bounded 128)                   │
│           └── Writer Task: outbound_rx (mpsc, bounded 512)           │
│                   → encode → compress → encrypt → socket             │
│                                                                      │
├── RCON / Query / WebSocket (separate Tokio tasks)                    │
│                                                                      │
│          inbound_tx ──────────┐       ┌────────── outbound_rx        │
│          (per player)         │       │          (per player)        │
│                               ▼       ▼                              │
│  ┌─────────────────────────────────────────────────────────────┐     │
│  │           Dedicated Tick Thread (ADR-019)                   │     │
│  │                                                             │     │
│  │  ┌───────────────────────────────────────────────────────┐  │     │
│  │  │  Phase 1: NETWORK_RECEIVE                             │  │     │
│  │  │    Drain inbound channels → ECS events/commands       │  │     │
│  │  ├── ─── barrier ─── ────────────────────────────────────┤  │     │
│  │  │  Phase 2: WORLD_TICK                                  │  │     │
│  │  │    Daylight, weather, scheduled ticks, raid spawning  │  │     │
│  │  ├── ─── barrier ─── ────────────────────────────────────┤  │     │
│  │  │  Phase 3: ENTITY_TICK                                 │  │     │
│  │  │    Physics, AI, entity behavior, status effects       │  │     │
│  │  │    (bevy_ecs parallel systems across rayon threads)   │  │     │
│  │  ├── ─── barrier ─── ────────────────────────────────────┤  │     │
│  │  │  Phase 4: BLOCK_TICK                                  │  │     │
│  │  │    Random ticks, scheduled ticks, block entities      │  │     │
│  │  ├── ─── barrier ─── ────────────────────────────────────┤  │     │
│  │  │  Phase 5: NETWORK_SEND                                │  │     │
│  │  │    Dirty entity data → outbound channels              │  │     │
│  │  ├── ─── barrier ─── ────────────────────────────────────┤  │     │
│  │  │  Phase 6: HOUSEKEEPING                                │  │     │
│  │  │    Auto-save, TPS measurement, chunk ticket expiry    │  │     │
│  │  └───────────────────────────────────────────────────────┘  │     │
│  │                                                             │     │
│  │  Tick arena reset (bumpalo) ← O(1) pointer reset           │     │
│  └─────────────────────────────────────────────────────────────┘     │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

Phase barriers ensure all work in phase N completes before phase N+1 begins.
Within a phase, `bevy_ecs` schedules non-conflicting systems in parallel.
This gives **determinism** (fixed phase order) with **parallelism** (multi-core
within phases). See [ADR-019](../adr/adr-019-tick-loop.md) for tick timing,
catch-up logic, `/tick` command support, and the watchdog thread.

---

## Entity Component System (ECS)

Oxidized uses [`bevy_ecs`](https://docs.rs/bevy_ecs) as a standalone library for all
entity representation and processing
([ADR-018](../adr/adr-018-entity-system.md)). This is the largest architectural
divergence from vanilla.

### Core Concepts

| Concept | Meaning |
|---|---|
| **Entity** | An opaque integer ID (no data, no methods). |
| **Component** | A plain Rust struct holding data. `#[derive(Component)]` |
| **System** | A function that queries components. Declares read/write access so `bevy_ecs` can parallelize. |
| **Resource** | Singleton data shared across systems (e.g., `DayTime`, `TickRateManager`). |
| **Commands** | Deferred entity spawn/despawn/component insert, applied at the next phase barrier. |

### Why ECS, Not OOP

Vanilla's deep inheritance hierarchy (`Entity` → `LivingEntity` → `Mob` → `Monster` →
`Zombie`) causes poor cache locality, prevents safe parallelism, and makes composition
rigid. In Oxidized:

- **Cache-friendly iteration** — components of the same type are stored contiguously
  (archetype storage). Iterating all `(Position, Velocity)` pairs is a linear memory scan.
- **Automatic parallelism** — `bevy_ecs` runs non-conflicting systems on different cores.
  The gravity system (writes `Velocity`) runs alongside the AI target system (writes
  `AiTarget`) with zero locks.
- **Composition via marker components** — making any entity "burnable" means inserting a
  `Burning` component. No hierarchy changes.

### Example

```rust
#[derive(Component)]
struct Position(DVec3);

#[derive(Component)]
struct Velocity(DVec3);

#[derive(Component)]
struct ZombieMarker;

fn gravity_system(mut query: Query<&mut Velocity, Without<NoGravity>>) {
    for mut vel in &mut query {
        vel.0.y -= 0.08;
        vel.0.y *= 0.98;
    }
}

fn zombie_sunlight_burning_system(
    mut commands: Commands,
    query: Query<(Entity, &Position), (With<ZombieMarker>, Without<Burning>)>,
    time: Res<DayTime>,
) {
    for (entity, pos) in &query {
        if time.is_day() && pos.0.y > 64.0 {
            commands.entity(entity).insert(Burning { ticks_remaining: 80 });
        }
    }
}
```

Every field in vanilla's entity class hierarchy maps to a named component. Vanilla's
`Entity` base class becomes `Position`, `Velocity`, `Rotation`, `OnGround`,
`FallDistance`, `EntityFlags`, `BoundingBox`, etc. Entity-type-specific behaviour is
driven by marker components (`ZombieMarker`, `PlayerMarker`, `CreeperMarker`).

---

## Player Session: Split Architecture

Each player connection is split into three parts
([ADR-020](../adr/adr-020-player-session.md)):

```
  Tokio Runtime                          Dedicated Tick Thread
 ─────────────────────────              ─────────────────────────
 ┌───────────────────────┐   inbound    ┌───────────────────────┐
 │   Reader Task         │───(mpsc)───►│                       │
 │   socket → decrypt    │   128 cap    │   ECS Entity          │
 │   → decompress        │              │                       │
 │   → decode            │              │   Position            │
 └───────────────────────┘              │   Velocity            │
                                        │   Health              │
 ┌───────────────────────┐   outbound   │   Inventory           │
 │   Writer Task         │◄──(mpsc)────│   GameMode            │
 │   encode → compress   │   512 cap    │   NetworkBridge {     │
 │   → encrypt → socket  │              │     inbound_rx,       │
 │   (batch flush/tick)  │              │     outbound_tx }     │
 └───────────────────────┘              └───────────────────────┘
```

- **Reader task** (Tokio) — decodes packets from the wire, converts them to `PlayerEvent`
  enum variants, and sends them through a bounded `mpsc` channel to the game world.
- **Writer task** (Tokio) — receives `OutboundPacket` variants from the game world,
  batches them per tick, and flushes in a single `write_all` syscall with `TCP_NODELAY`.
- **ECS entity** (tick thread) — holds all game state as components. The
  `NetworkBridge` component contains the channel endpoints. During
  `NETWORK_RECEIVE` the `process_player_events` system drains inbound channels; during
  `NETWORK_SEND` systems push dirty state into outbound channels.

Bounded channels provide natural backpressure: if the game loop falls behind, the inbound
channel fills, the reader task blocks, TCP flow control kicks in, and the client sees
increased latency — no unbounded memory growth.

---

## Memory Management

A three-layer allocation strategy keeps latency predictable and fragmentation low
([ADR-029](../adr/adr-029-memory-management.md)):

### Layer 1: mimalloc Global Allocator

```rust
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
```

Replaces the system allocator for all heap allocations. Provides thread-local free lists
(minimal contention), segment-based memory management (low fragmentation), and free page
retirement (RSS stays close to working set).

### Layer 2: bumpalo Tick Arena

Per-thread `bumpalo::Bump` arenas for allocations that live only within a single tick
(pathfinding open sets, loot evaluation scratch, command parse trees, collision
candidates). Reset between ticks with a single O(1) pointer reset — no per-object
destructors. The borrow checker enforces that arena-allocated data cannot escape the tick
(`'arena` lifetime).

```rust
// In HOUSEKEEPING phase, after all other work:
tick_arena.reset(); // O(1) — resets allocation pointer
```

### Layer 3: Buffer Pooling for Network I/O

Lock-free `SegQueue`-based pool of `BytesMut` buffers (default 4 KB capacity, max 2048
pooled). Eliminates per-packet allocation for the ~100,000 buffer ops/second generated by
100 players at 50 packets/player/tick.

### Memory Budget (100 players, view distance 10)

| Component | Per-Unit | Count | Total |
|---|---|---|---|
| Chunk block data | ~16 KB | 100,000 | ~1.6 GB |
| Chunk light data | ~8 KB | 100,000 | ~800 MB |
| Chunk metadata | ~4 KB | 100,000 | ~400 MB |
| Entity data (ECS) | ~2 KB | 50,000 | ~100 MB |
| Player data | ~50 KB | 100 | ~5 MB |
| Network buffer pool | 4 KB | 2,048 | ~8 MB |
| Tick arenas | 1 MB | 16 threads | ~16 MB |
| Registries + worldgen cache | — | — | ~250 MB |
| **Estimated total** | | | **~3.2 GB** |

---

## Data Flow: Player Joins

```
Client TCP connect
      │
      ▼ [Handshaking state]
ClientIntentionPacket (next_state=LOGIN)
      │
      ▼ [Login state]
ServerboundHelloPacket (name, uuid)
      │
      ├─ [online-mode=true] ──► RSA exchange ──► AES-CFB8 enabled
      │                          Mojang auth POST
      │
      ├─ compression enabled (ClientboundLoginCompressionPacket)
      │
      ▼ ClientboundLoginFinishedPacket (uuid, name, properties)
ServerboundLoginAcknowledgedPacket
      │
      ▼ [Configuration state]
Registry sync, feature flags, known packs negotiation
ClientboundFinishConfigurationPacket
ServerboundFinishConfigurationPacket
      │
      ▼ [Play state]
ClientboundLoginPacket (entity_id, dimensions, game_mode …)
ClientboundPlayerAbilitiesPacket
ClientboundSetDefaultSpawnPositionPacket
ClientboundGameRuleValuesPacket
ClientboundPlayerInfoUpdatePacket (add self to tab list)
ClientboundSetChunkCacheCenterPacket
[spiral of ClientboundLevelChunkWithLightPacket × view_distance²]
ClientboundPlayerPositionPacket (teleport to spawn)
      │
      ▼ Player is in-game
      (Reader task + Writer task on Tokio;
       ECS entity with NetworkBridge on tick thread)
```

---

## Protocol Version Numbers

```rust
pub const PROTOCOL_VERSION: i32 = 1073742124;  // 26.1-pre-3
pub const WORLD_VERSION: i32    = 4782;
pub const DEFAULT_PORT: u16     = 25565;
pub const TICKS_PER_SECOND: u32 = 20;
pub const TICK_DURATION_MS: u64 = 50;
pub const SECTION_HEIGHT: usize = 16;
pub const SECTION_WIDTH: usize  = 16;
pub const SECTION_SIZE: usize   = 4096;         // 16³
pub const SECTION_COUNT: usize  = 24;           // y = -64..=319
pub const AUTOSAVE_INTERVAL: u32 = 6000;        // ticks = 5 minutes
pub const COMPRESSION_THRESHOLD: i32 = 256;    // bytes
pub const KEEPALIVE_INTERVAL: u32 = 20;         // ticks
pub const CONNECTION_TIMEOUT_SECS: u64 = 30;
```

---

## Related ADRs

| ADR | Topic |
|---|---|
| [ADR-001](../adr/adr-001-async-runtime.md) | Tokio as async runtime (network I/O, disk I/O, timers) |
| [ADR-003](../adr/adr-003-crate-architecture.md) | 6-crate workspace with compile-time boundary enforcement |
| [ADR-006](../adr/adr-006-network-io.md) | Per-connection reader/writer task pair with bounded mpsc channels |
| [ADR-018](../adr/adr-018-entity-system.md) | `bevy_ecs` entity system (components, systems, parallel scheduling) |
| [ADR-019](../adr/adr-019-tick-loop.md) | 6-phase tick loop on dedicated OS thread with phase barriers |
| [ADR-020](../adr/adr-020-player-session.md) | Split player session: network task ↔ bridge channels ↔ ECS entity |
| [ADR-029](../adr/adr-029-memory-management.md) | mimalloc global allocator + bumpalo tick arena + buffer pooling |
