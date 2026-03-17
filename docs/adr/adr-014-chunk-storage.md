# ADR-014: Chunk Storage & Concurrency Model

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P09, P11, P13, P14, P38 |
| Deciders | Oxidized Core Team |

## Context

Chunks are the fundamental spatial unit of Minecraft's world. A chunk column spans the full
height of the world (24 sections in the overworld, each 16×16×16 blocks) and stores block states,
biomes, block entities, heightmaps, and light data. At any given moment, a busy server may have
thousands of chunks loaded in memory, with dozens being accessed concurrently by different
subsystems: player movement checks block collisions, chunk sending serializes block data into
packets, block placement modifies block states, entity ticking reads surrounding blocks for
pathfinding, lighting reads and writes light levels, and world generation populates new chunks.

Vanilla Java handles this with a single-threaded game loop: all chunk access happens on the
"server thread," with the exception of some lighting work that is queued and processed between
ticks. This eliminates concurrency concerns but also eliminates parallelism — a vanilla server
cannot use more than one core for game logic. Server forks like Paper introduce some
parallelism (async chunk loading, parallel entity ticking) but still fundamentally serialize
most chunk access behind a single lock or the main thread.

Oxidized aims for safe concurrency from day one. Multiple threads should be able to read chunk
data simultaneously (e.g. sending chunks to different players in parallel), while writes (block
placement, lighting updates) are localized to the specific section being modified. The data
structure design must support this without sacrificing single-threaded performance — the common
case is still a single write at a time, and readers should never block other readers.

## Decision Drivers

- **Parallel reads**: Multiple threads must read chunk data concurrently without blocking each
  other. This is the common case — chunk sending, entity ticking, and collision checks all read.
- **Localized writes**: A block placement in section 5 should not block a lighting update in
  section 12 of the same chunk. Write contention must be per-section, not per-chunk.
- **O(1) chunk lookup**: Looking up a chunk by its position must be constant time. This happens
  on every block access, every tick, for every entity.
- **Memory efficiency**: Chunk data is the largest memory consumer. Empty sections (all air)
  should not allocate full block arrays.
- **Cache friendliness**: Block data within a section should be contiguous in memory for fast
  iteration (lighting passes, chunk serialization).
- **Lifecycle management**: Chunks have a defined lifecycle (loading → loaded → ticking →
  unloading) and must be unloaded when no longer needed, with dirty data flushed to disk first.

## Considered Options

### Option 1: Single RwLock Per Chunk

Each chunk column is wrapped in a `RwLock<ChunkColumn>`. Readers take a shared lock; writers
take an exclusive lock. This is simple and prevents data races, but write lock on a chunk blocks
ALL readers of that chunk — a block placement in section 5 blocks chunk sending of section 17.
Since chunk sending is on the critical path for player experience (latency-sensitive), this is
unacceptable for a high-player-count server.

### Option 2: Lock-Free Chunk Map With Copy-on-Write Sections

Use a lock-free concurrent map for chunk lookup and implement sections as immutable snapshots.
On write, clone the section, modify the clone, and atomically swap the pointer. Readers always
see a consistent snapshot. This eliminates all blocking but makes writes expensive (full section
clone = 8 KB copy + palette rebuild) and increases memory churn. For frequent writes (lighting
updates can modify hundreds of blocks in a tick), the copy overhead is prohibitive.

### Option 3: Striped Lock Array

Hash chunk positions to a fixed-size array of locks. Multiple chunks can be accessed in parallel
as long as they hash to different stripes. This reduces contention compared to a global lock but
doesn't solve the per-section granularity problem — a write to any section in a chunk still
locks the entire stripe. The hash collision rate depends on the stripe count and access patterns.

### Option 4: Per-Section RwLock

Each of the 24 sections within a chunk column has its own `RwLock<ChunkSection>`. The chunk map
uses a concurrent HashMap for O(1) lookup. Writers only lock the specific section they modify,
and readers of other sections proceed unimpeded. This provides fine-grained concurrency with
minimal contention. The cost is 24 `RwLock` instances per chunk (cheap in Rust — parking_lot
RwLock is 1 word each), but the benefits of section-level parallelism outweigh this overhead.

### Option 5: Actor Model

Each chunk is an actor that owns its data and processes messages (read requests, write requests)
sequentially. Callers send messages and receive responses via channels. This provides strong
isolation but introduces significant latency (message passing overhead) and complexity (every
chunk access becomes async). For the millions of block-state reads per tick, the message-passing
overhead is unacceptable.

## Decision

We adopt **DashMap<ChunkPos, Arc<ChunkColumn>> with per-section RwLock** for chunk storage and
concurrency.

### Chunk Map

The top-level chunk map uses `DashMap<ChunkPos, Arc<ChunkColumn>>`:

```rust
pub struct ChunkMap {
    chunks: DashMap<ChunkPos, Arc<ChunkColumn>>,
}
```

`DashMap` is a sharded concurrent HashMap — internally, it partitions entries across multiple
shards, each with its own lock. This means chunk lookups only contend with other lookups hitting
the same shard, and in practice, contention is negligible because chunk positions are well-
distributed. The `Arc<ChunkColumn>` allows multiple systems to hold references to the same chunk
without copying.

### Chunk Column Structure

```rust
pub struct ChunkColumn {
    pos: ChunkPos,
    sections: Box<[RwLock<ChunkSection>]>, // 24 sections for overworld
    heightmaps: RwLock<Heightmaps>,
    block_entities: RwLock<HashMap<BlockPos, BlockEntity>>,
    status: AtomicU8, // ChunkStatus enum as atomic
    dirty: AtomicBool,
    last_access: AtomicU64, // tick timestamp for LRU unloading
    ticket_level: AtomicU32,
}
```

Each section has its own `RwLock`, so a block placement in section 5 only locks section 5's
`RwLock` — readers and writers of other sections proceed concurrently.

### Chunk Section Structure

```rust
pub struct ChunkSection {
    block_states: PalettedContainer<BlockState>,
    biomes: PalettedContainer<Biome>,
    non_empty_count: u16, // cached count of non-air blocks
    ticking_count: u16,   // cached count of randomly-ticking blocks
}
```

`PalettedContainer` is a compact representation that adapts its storage based on the number of
distinct values in the section:

| Distinct values | Storage strategy | Bits per entry |
|-----------------|-----------------|----------------|
| 1 | Single value (no array) | 0 |
| 2–16 | Linear palette + packed u64 array | 1–4 |
| 17–256 | HashMap palette + packed u64 array | 5–8 |
| >256 | Direct global palette IDs | 15 (for blocks) |

This matches vanilla's `PalettedContainer` encoding, which is also used on the wire protocol
for chunk data packets. A section that is entirely air (common in the sky) stores just a single
value — no 8 KB block array is allocated.

### Memory Layout

The section's `PalettedContainer` stores block data as a packed array of `u64` values, where
each u64 contains multiple block entries packed at the current bits-per-entry. For a typical
section with 4 bits per entry, the block data is 4096 × 4 / 8 = 2048 bytes, plus the palette
entries. This is cache-friendly — a full section's block data fits in L2 cache (typically 256 KB
or more per core).

The `ChunkColumn`'s sections are stored in a contiguous `Box<[RwLock<ChunkSection>]>` — all 24
sections are allocated together, reducing allocator overhead and improving spatial locality.

### Chunk Lifecycle (Ticket System)

Chunks are kept loaded by a **ticket system** inspired by vanilla. A ticket is a reason why a
chunk must be loaded at a certain level:

```rust
pub enum TicketType {
    Player { pos: ChunkPos },       // Player's view distance
    Portal,                         // Near active portal
    ForcedChunk,                    // /forceload command
    Start,                          // Spawn chunks
    EntityLoad { entity_id: u32 },  // Entity is in this chunk
    WorldGen { source: ChunkPos },  // Needed for neighbor during worldgen
    Plugin,                         // Plugin API request
}

pub struct Ticket {
    ticket_type: TicketType,
    level: u32,
    created_tick: u64,
}
```

Ticket levels determine chunk loading status:

| Level | Status | What's active |
|-------|--------|---------------|
| 31 | ENTITY_TICKING | Entities tick, blocks tick, redstone runs |
| 32 | TICKING | Block ticking, random ticks, but no entity ticking |
| 33 | LOADED | Data loaded in memory, accessible, but not ticking |
| 33+ | UNLOADED | Scheduled for unload |

A chunk's effective level is the minimum level across all its tickets. When all tickets are
removed, the chunk transitions to UNLOADED and enters the unload queue.

### Chunk Loading States

Chunks progress through a state machine during loading:

```
EMPTY → LOADING → STRUCTURE_STARTS → BIOMES → NOISE → SURFACE → CARVERS → FEATURES → LIGHT → LOADED → TICKING → ENTITY_TICKING
```

The `status` field is an `AtomicU8` allowing lock-free status checks. State transitions are
monotonic during generation (always forward) and managed by the worldgen pipeline. Once LOADED,
the chunk can transition between LOADED, TICKING, and ENTITY_TICKING based on ticket levels.

### Unload Queue

When a chunk's ticket level drops below LOADED, it enters the unload queue:

```rust
pub struct UnloadQueue {
    queue: Mutex<VecDeque<(ChunkPos, u64)>>, // (pos, queued_at_tick)
}
```

The unload process:
1. Check if the chunk has regained tickets (cancel unload if so).
2. If the chunk is dirty, serialize and flush to disk (see ADR-015).
3. Remove from the `DashMap`.
4. Drop the `Arc<ChunkColumn>` — if no other system holds a reference, memory is freed.

Unloading is batched — up to N chunks per tick to avoid stalls. The `last_access` timestamp
enables LRU-style prioritization when memory pressure requires aggressive unloading.

### View Distance Tracking

Each player has a set of chunks within their view distance. When a player moves to a new chunk,
the server computes the delta (newly visible chunks, no longer visible chunks):

```rust
pub struct PlayerChunkTracker {
    center: ChunkPos,
    view_distance: u8,
    loaded_chunks: HashSet<ChunkPos>,
}
```

Newly visible chunks are scheduled for loading and sending. No longer visible chunks have their
player ticket removed, potentially triggering unload if no other tickets remain.

## Consequences

### Positive

- **Fine-grained concurrency**: Per-section `RwLock` allows parallel reads across sections and
  localized writes. Chunk sending and lighting can operate on different sections simultaneously.
- **O(1) chunk lookup**: `DashMap` provides constant-time chunk position lookup with minimal
  contention due to internal sharding.
- **Memory efficiency**: `PalettedContainer` compresses sparse sections (all-air sections use
  near-zero memory). The single-value optimization is critical for the vast majority of sections
  in a typical world.
- **Lifecycle correctness**: The ticket system provides a clear, deterministic model for chunk
  loading and unloading, with dirty-write-before-evict ensuring no data loss.
- **Protocol-compatible encoding**: `PalettedContainer` directly matches the chunk data wire
  format, so serialization for chunk sending is a near-zero-cost memcpy of the packed arrays.

### Negative

- **Lock granularity overhead**: 24 `RwLock` instances per chunk column, plus locks for
  heightmaps and block entities. For 10,000 loaded chunks, that is ~260,000 locks. Each
  `parking_lot::RwLock` is 1 word (8 bytes), so ~2 MB — acceptable but non-trivial.
- **Write contention on hot sections**: If many players are building in the same section, writes
  still serialize on that section's `RwLock`. This is inherent to the data model — the section is
  the minimum lockable unit. Mitigation: section-level is already quite fine-grained (16×16×16).
- **Complexity of ticket system**: The ticket and level system is complex to implement correctly.
  Bugs can cause chunks to never unload (memory leak) or unload too early (data loss). Requires
  thorough testing.

### Neutral

- **DashMap is a third-party dependency**: We rely on the `dashmap` crate for the sharded
  concurrent map. It is widely used and well-maintained. If needed, we could replace it with a
  custom implementation, but there is no current reason to do so.
- **Arc overhead**: Each chunk is reference-counted via `Arc`. The atomic reference counting adds
  a small cost on clone/drop, but this only happens during chunk load/unload, not on the hot
  read/write path.

## Compliance

- **Concurrency stress test**: Spawn N reader threads and M writer threads accessing random
  sections of the same chunk. Verify no data races (miri), no panics, and consistent reads.
- **Palette encoding tests**: Verify that `PalettedContainer` correctly transitions between
  storage strategies as blocks are added. Round-trip serialize/deserialize for all palette modes.
- **Ticket system tests**: Verify that chunk status transitions correctly based on ticket levels.
  Verify that removing all tickets eventually unloads the chunk. Verify that dirty chunks are
  saved before eviction.
- **View distance tests**: Verify delta computation when player moves between chunks. Verify edge
  cases (teleportation across large distances, view distance change).
- **Memory benchmark**: Measure memory usage for a 1000-chunk world with typical terrain.
  Verify that empty sections contribute minimal overhead.

## Related ADRs

- **ADR-012** (Block State Representation): Chunk sections store block state u16 IDs via
  `PalettedContainer`; the dense lookup table is used to interpret those IDs.
- **ADR-013** (Coordinate Types): `ChunkPos` keys the chunk map; `SectionPos` indexes into
  section arrays; `BlockPos.chunk_local()` indexes into section block data.
- **ADR-015** (Disk I/O): Dirty chunks are serialized to Anvil region files by the I/O system.
  The chunk storage layer marks chunks dirty; the I/O layer flushes them.
- **ADR-016** (Worldgen Pipeline): Chunk generation populates new `ChunkColumn` instances and
  transitions their status through the generation pipeline.
- **ADR-017** (Lighting Engine): Light data is stored per-section and accessed via section
  `RwLock`; the lighting engine reads and writes light levels in section granularity.

## References

- [DashMap crate](https://docs.rs/dashmap) — sharded concurrent HashMap
- [parking_lot crate](https://docs.rs/parking_lot) — efficient RwLock implementation
- [wiki.vg — Chunk Format](https://wiki.vg/Chunk_Format) — PalettedContainer wire format
- [Minecraft Wiki — Chunk Format](https://minecraft.wiki/w/Chunk_format) — on-disk format
- [Minecraft Wiki — Chunk Loading](https://minecraft.wiki/w/Chunk_loading) — ticket system
- [Paper — Starlight](https://github.com/PaperMC/Paper) — parallel lighting inspiration
