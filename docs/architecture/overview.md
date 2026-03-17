# Architecture Overview

## What is Oxidized?

Oxidized is a full reimplementation of the Minecraft Java Edition **26.1** server in Rust.
It is wire-protocol compatible with the vanilla client — any 26.1 client can connect to it.
The internals are completely rewritten using idiomatic Rust and async Tokio.

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

## System Architecture Diagram

```
┌───────────────────────────────────────────────────────────────────────┐
│                         oxidized-server (bin)                         │
│                                                                       │
│  ┌─────────────┐  ┌──────────────────┐  ┌────────────────────────┐  │
│  │  Server     │  │   Tick Loop      │  │  Config / Properties   │  │
│  │  Bootstrap  │  │   20 TPS         │  │  server.properties     │  │
│  └──────┬──────┘  └────────┬─────────┘  └────────────────────────┘  │
│         │                  │                                          │
└─────────┼──────────────────┼──────────────────────────────────────── ┘
          │                  │
          ▼                  ▼
┌─────────────────┐  ┌────────────────────────────────────────────────┐
│oxidized-protocol│  │                oxidized-game                   │
│                 │  │                                                │
│ Connection      │  │  ServerLevel      EntityManager  CommandDisp  │
│ Packet codecs   │◄─┤  PlayerList       AI Goals       Crafting     │
│ 5 protocol      │  │  Combat           Advancements   LootTables   │
│  states         │  │                                                │
│ Crypto (AES/RSA)│  └────────────────────┬───────────────────────── ┘
│ Compression     │                       │
└────────┬────────┘                       ▼
         │                    ┌───────────────────────┐
         │                    │    oxidized-world      │
         │                    │                        │
         │                    │  LevelChunk            │
         │                    │  PalettedContainer     │
         │                    │  AnvilChunkLoader      │
         │                    │  Block/Item Registry   │
         │                    │  Lighting engine       │
         │                    │  World generation      │
         │                    └──────────┬─────────────┘
         │                               │
         ▼                               ▼
┌────────────────────────────────────────────────────────┐
│                     oxidized-nbt                       │
│                                                        │
│  CompoundTag  ListTag  ByteArrayTag  NbtIo  SNBT      │
└────────────────────────────────────────────────────────┘
```

---

## Runtime Architecture

```
Main thread (Tokio runtime)
│
├── TcpListener::accept() loop
│     └── spawn task per connection ─────────────────────────────────┐
│                                                                    │
├── Tick interval (50ms / 20 TPS)                                   │
│     ├── ServerLevel::tick() × N dimensions                        │
│     │    ├── Time/weather advance                                  │
│     │    ├── Random block ticks (3/section/tick)                  │
│     │    ├── Scheduled block tick queue drain                     │
│     │    ├── Entity tick loop                                      │
│     │    └── Chunk load/unload queue                              │
│     ├── PlayerList::tick() (keepalive, chunk tracking)            │
│     └── Auto-save check (every 6000 ticks)                        │
│                                                                    │
└── RCON / Query server (separate Tokio tasks)                      │
                                                                    │
Connection task (one per player):  ◄──────────────────────────────-─┘
    ├── read half: decode VarInt frames → packets → channel
    └── write half: receive from mpsc → encode → send
```

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
```

---

## Threading Model

| Work type | Runs on |
|---|---|
| Network I/O (read/write) | Tokio I/O thread pool |
| Game tick loop | Single Tokio task (main) |
| Chunk generation (CPU) | `tokio::task::spawn_blocking` |
| Disk I/O (load/save chunks, player data) | `tokio::task::spawn_blocking` |
| RCON server | Separate Tokio task |
| JSON-RPC WebSocket | Separate Tokio task |

The game tick task **never calls `spawn_blocking` directly** — it enqueues work and
polls results in the next tick. This keeps the 50ms budget predictable.

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
