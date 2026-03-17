# ADR-032: Performance & Scalability Architecture

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P38 |
| Deciders | Oxidized Core Team |

## Context

The primary motivation for rewriting a Minecraft server in Rust is performance. Vanilla Java
Edition struggles to maintain 20 TPS (ticks per second) with more than 50 concurrent players,
even on high-end hardware. The bottlenecks are well-documented: the tick loop is single-
threaded (one core does all entity AI, block ticking, redstone, and world generation), the
JVM garbage collector introduces periodic pauses (10-100ms spikes during major collections),
entity tracking uses O(n^2) distance checks for visibility updates, and block/fluid ticking
schedules are processed sequentially. Server operators resort to performance mods (Paper,
Purpur, Folia), reduced view distances, entity limiting plugins, and aggressive GC tuning —
all of which compromise gameplay fidelity.

Our design opportunity is architectural: we can choose data structures, threading models, and
memory layouts that the Java codebase cannot adopt without a complete rewrite. The ECS
(Entity Component System) already gives us data-oriented entity storage. The async runtime
(tokio) already separates I/O from game logic. The question is how to compose these pieces
into a system that scales linearly with hardware — more cores should mean more capacity, not
just faster single-threaded execution.

The target is concrete: 100 concurrent players at 20 TPS with sub-50ms p99 tick time on a
modern 8-core server, using less than 4GB of RAM with default settings. This is approximately
2x the player capacity of vanilla on equivalent hardware, achieved not through gameplay
compromises but through better engineering. Every architectural decision in this ADR must
contribute measurably toward this target.

## Decision Drivers

- **20 TPS at 100 players**: The headline metric. If we can't maintain 20 TPS, every other
  optimization is irrelevant. TPS is the one number operators care about.
- **Predictable tick timing**: p99 tick time matters more than average. A server averaging
  25ms/tick but spiking to 200ms every 30 seconds feels worse than one averaging 40ms/tick
  with 50ms p99. Consistency over raw speed.
- **Linear core scaling**: Adding CPU cores should increase capacity. A 16-core server should
  handle more players than an 8-core server, not be bottlenecked by a single thread.
- **Memory efficiency**: 4GB should be sufficient for default settings (view distance 10,
  100 players). Operators shouldn't need 16GB+ for a moderately populated server.
- **Measurability**: Every optimization must be measurable. If we can't benchmark it, we
  can't verify it helps. Profiling and metrics must be first-class.
- **No gameplay compromise**: We don't reduce mob caps, skip tick processing, or alter game
  mechanics for performance. The server must be indistinguishable from vanilla to players.

## Considered Options

### Option 1: Optimize Vanilla Architecture (Faster Single Thread)

Keep the single-threaded tick loop but optimize each subsystem: faster pathfinding, better
data structures for block ticking, SIMD for collision detection.

**Pros**: Simplest architecture (no concurrency concerns). Matches vanilla's execution model
exactly. Easy to verify correctness.

**Cons**: Fundamentally limited by single-core speed. Modern CPUs improve IPC slowly — the
ceiling is perhaps 30-40% faster than vanilla on the same hardware. Cannot scale with core
count. Not sufficient for the 100-player target.

### Option 2: Multi-Threaded ECS

Leverage the ECS architecture to run systems (AI, physics, block ticking) in parallel across
cores. Systems that don't access the same components run simultaneously.

**Pros**: Near-linear scaling with cores for independent systems. The ECS already enables this
by separating data (components) from logic (systems). Rayon provides work-stealing
parallelism. Batch processing of similar entities (all zombies, all creepers) is cache-
friendly.

**Cons**: Some systems have inherent dependencies (entity movement must complete before
collision detection). Shared mutable state (world blocks, chunk data) requires careful
synchronization. Debugging concurrent systems is harder. Determinism (same input → same
output) requires careful ordering.

### Option 3: Distributed Server (Multiple Processes)

Split the world across multiple server processes, each handling a region. Players are
transparently handed off between processes as they move.

**Pros**: Scales beyond a single machine. Each process is simpler (handles fewer entities).

**Cons**: Enormous complexity (cross-process entity interaction, player handoff, shared
state). Network latency between processes adds tick time. Not necessary for 100 players.
Folia explores this approach for Paper and it's years from production quality. Overkill.

### Option 4: Region-Based Parallelism

Divide the world into independent regions (e.g., 32×32 chunk areas). Tick each region in
parallel. Interactions at region boundaries are serialized.

**Pros**: Conceptually clean — most gameplay is local. Scales well when players are spread
out. Folia (Paper fork) validates this approach.

**Cons**: Boundary interactions are common (redstone, entity movement, explosions). Handling
boundaries correctly is extremely complex. If all 100 players are in the same region,
parallelism is zero. Not a general solution.

## Decision

**Multi-layered scalability strategy.** Rather than a single parallel execution model, we
apply different optimization strategies at each level of the system. Each layer is independent
and provides measurable improvement.

### Level 1: Data-Oriented Design

The foundation of performance is data layout. Cache misses dominate modern CPU performance —
a single L3 cache miss costs ~40ns, equivalent to ~100 simple operations.

**ECS component storage**: Components of the same type are stored in contiguous arrays
(SoA — Structure of Arrays). When a system iterates over all entities with `Position` and
`Velocity`, it reads two contiguous arrays sequentially — maximum cache line utilization.

**Chunk block storage**: `PalettedContainer` uses indirect indexing through a palette. Single-
value sections (e.g., all air, all stone) use zero storage for the data array — just the
single palette entry. Sections with 2-16 unique states use 4-bit indices. This typically
compresses 16×16×16 = 4096 block states from 16KB (raw) to 2-4KB.

**SIMD opportunities**:
- Collision detection: AABB intersection tests can use `f32x4` SIMD for 4-wide parallel
  testing.
- Light propagation: BFS flood fill with SIMD-accelerated queue operations.
- Chunk serialization: Palette index packing uses bit manipulation that maps to SIMD.

### Level 2: Parallel Tick Execution

The tick pipeline is decomposed into systems with explicit data dependencies. Independent
systems run in parallel on the Rayon thread pool.

```
Tick Pipeline (systems with dependencies):
┌─────────────────────────────────────────────────────────┐
│ Phase 1: Network Input (tokio → game thread)            │
│   Read all pending packets, queue as events             │
├─────────────────────────────────────────────────────────┤
│ Phase 2: World Tick (parallelizable)                    │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│   │ Block Tick    │  │ Fluid Tick   │  │ Raid Tick    │ │
│   │ (per chunk)   │  │ (per chunk)  │  │             │ │
│   └──────────────┘  └──────────────┘  └──────────────┘ │
├─────────────────────────────────────────────────────────┤
│ Phase 3: Entity Tick (parallelizable by independence)   │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│   │ AI/Goals     │  │ Movement     │  │ Block Entity │ │
│   │ (read world) │  │ (r/w pos)    │  │ Tick         │ │
│   └──────┬───────┘  └──────┬───────┘  └──────────────┘ │
│          └────────┬────────┘                            │
│                   ▼                                     │
│          ┌──────────────┐                               │
│          │ Collision    │ (depends on Movement)          │
│          │ Resolution   │                               │
│          └──────────────┘                               │
├─────────────────────────────────────────────────────────┤
│ Phase 4: Player Tick (per-player, parallelizable)       │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│   │ Inventory    │  │ Advancement  │  │ Recipe Book  │ │
│   │ Updates      │  │ Checks       │  │ Updates      │ │
│   └──────────────┘  └──────────────┘  └──────────────┘ │
├─────────────────────────────────────────────────────────┤
│ Phase 5: Network Output (game thread → tokio)           │
│   Serialize and queue all outgoing packets              │
└─────────────────────────────────────────────────────────┘
```

Within Phase 3, entity AI is parallelized using Rayon's `par_iter()` over entity batches.
Entities that don't interact (e.g., zombies in different chunks) are processed in parallel.
Entities within interaction range are batched together and processed sequentially within
the batch.

**World generation** runs entirely on the Rayon thread pool, separate from the tick pipeline.
Chunk generation tasks are queued with priorities (player proximity = higher priority) and
executed asynchronously. Completed chunks are integrated into the world at the next tick
boundary.

### Level 3: Smart Scheduling

Not all work is equally important. Smart scheduling prioritizes visible, impactful work:

**Entity tick culling**:
- Entities in chunks with no nearby players (beyond `entity-activation-range`) tick at
  reduced frequency (1/4 rate for passive mobs, 1/2 for hostile mobs).
- Entities beyond simulation distance are not ticked at all (vanilla behavior).
- This is configurable and matches vanilla's behavior — it's not a gameplay compromise.

**View distance adaptation**:
- If TPS drops below 18, automatically reduce effective view distance by 1 chunk (down to
  minimum of 4). Restore when TPS recovers above 19 for 60 seconds.
- This is optional and off by default (`adaptive-view-distance=false`). When enabled, it
  provides graceful degradation under load.

**Chunk priority queue**:
- Chunk operations (loading, generation, saving) are prioritized:
  1. Chunks containing players (immediate).
  2. Chunks within player view distance (high).
  3. Chunks queued for generation by exploration (medium).
  4. Autosave chunks (low, spread across ticks).
- Priority queue ensures player-facing operations complete first.

### Level 4: Network Optimization

Network I/O is often the bottleneck for high player counts — not bandwidth, but packet
processing overhead.

**Packet batching**:
- Outgoing packets are accumulated in a per-player buffer during the tick.
- At the end of the tick (Phase 5), all buffers are flushed in one syscall per player
  (`writev` / vectored I/O).
- This reduces syscall overhead from ~50 syscalls/player/tick to 1 syscall/player/tick.

**Entity update culling**:
- Entity metadata updates are only sent to players who can see the entity (within tracking
  distance).
- If an entity's metadata hasn't changed since the last update, no packet is sent.
- Position updates use delta encoding: `ClientboundMoveEntityPosPacket` sends only the
  position delta (3 shorts) instead of absolute position (3 doubles).

**Chunk data compression**:
- Chunk data packets use zlib compression. We use `zlib-rs` (Rust-native zlib) with
  compression level 6 (good ratio, acceptable speed).
- Pre-compute chunk packet data when a chunk is modified, not when it's sent. Multiple
  players receiving the same chunk get the same pre-compressed bytes.

**View distance-based throttling**:
- Entities at the edge of tracking distance receive lower-frequency updates (every 3 ticks
  instead of every tick for position, every 20 ticks instead of every 5 for metadata).
- Players at the edge of view distance receive chunk updates at lower priority.

### Level 5: Memory Optimization

See ADR-029 for full details. Summary relevant to scalability:

- **mimalloc** as global allocator (low fragmentation, multi-threaded scaling).
- **Arena allocation** for per-tick temporaries (zero deallocation cost).
- **Buffer pooling** for network packets (eliminates per-packet allocation).
- **Chunk LRU eviction** with configurable cap: when loaded chunk count exceeds the cap,
  evict least-recently-accessed chunks that have no nearby players. Default cap: 150,000
  chunks.

### Performance Metrics and Observability

The server exposes metrics via the `/tps` command, the management API (ADR-031), and optional
Prometheus-compatible metrics endpoint.

**Core metrics**:

| Metric | Description | Target |
|--------|-------------|--------|
| TPS | Ticks per second (20 = perfect) | ≥ 20.0 |
| MSPT | Milliseconds per tick (average) | < 30ms |
| MSPT p99 | 99th percentile tick duration | < 50ms |
| Entity count | Total loaded entities | Monitoring |
| Chunk count | Total loaded chunks | Monitoring |
| Player count | Online players | Monitoring |
| Network throughput | Bytes/sec in + out | Monitoring |
| Memory RSS | Process resident set size | < 4GB default |
| GC pauses | N/A (Rust, no GC) | 0ms always |

**`/tps` command output format**:

```
TPS from last 1m, 5m, 15m: §a20.0, §a20.0, §a20.0
MSPT from last 1m: §a12.3ms §7(p50: 10.1ms, p95: 18.4ms, p99: 23.7ms)
```

Color coding: §a green (< 40ms), §e yellow (40-50ms), §c red (> 50ms).

**Profiling integration**:

- **tracing spans**: Every major system (entity AI, block tick, world gen, network flush) is
  wrapped in a `tracing::instrument` span. Spans are zero-cost when no subscriber is active.
- **Flamegraph generation**: Connect a `tracing-flame` subscriber via a debug command or
  config flag to generate flamegraphs from live server operation.
- **Criterion benchmarks**: Hot paths (pathfinding, loot evaluation, chunk serialization,
  packet encoding) have criterion benchmarks in `benches/`. CI runs benchmarks on every PR
  and alerts on regressions exceeding 5%.

### Benchmarking Framework

```rust
// benches/pathfinding.rs
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_pathfind_short(c: &mut Criterion) {
    c.bench_function("pathfind_20_blocks_flat", |b| {
        let world = test_world::flat();
        b.iter(|| {
            pathfinder::find_path(&world, BlockPos::new(0, 64, 0), BlockPos::new(20, 64, 0))
        })
    });
}

fn bench_pathfind_complex(c: &mut Criterion) {
    c.bench_function("pathfind_50_blocks_terrain", |b| {
        let world = test_world::hilly();
        b.iter(|| {
            pathfinder::find_path(&world, BlockPos::new(0, 64, 0), BlockPos::new(50, 80, 50))
        })
    });
}

criterion_group!(pathfinding, bench_pathfind_short, bench_pathfind_complex);
criterion_main!(pathfinding);
```

**Performance regression CI**:
- Criterion benchmarks run on every PR against the `main` branch baseline.
- Results are compared using `critcmp`. Regressions > 5% block the PR with a comment
  explaining which benchmark regressed and by how much.
- Baseline is updated on every merge to `main`.

### Target Performance Budget

Per-tick time budget at 20 TPS (50ms total available):

| Phase | Budget | Notes |
|-------|--------|-------|
| Network input (packet processing) | 5ms | Parallelized across tokio threads |
| World tick (blocks, fluids, weather) | 8ms | Parallelized across chunks |
| Entity tick (AI, movement, collision) | 15ms | Parallelized via ECS |
| Player tick (inventory, advancements) | 5ms | Per-player, parallelized |
| Network output (serialization, flush) | 7ms | Parallelized per-player |
| Headroom (scheduling, sync, overhead) | 10ms | Buffer for spikes |
| **Total** | **50ms** | |

The 10ms headroom ensures that occasional spikes in one phase don't cause TPS drops. If
headroom is consistently consumed, the adaptive view distance system (if enabled) provides
graceful degradation.

## Consequences

### Positive

- **Measurable targets**: Concrete TPS, MSPT, and memory targets make performance a first-
  class requirement, not a vague aspiration. Every PR can be evaluated against these metrics.
- **Multi-core utilization**: The parallel tick pipeline, Rayon world gen, and tokio network
  I/O ensure all available cores contribute to server capacity.
- **Graceful degradation**: Adaptive view distance and entity tick culling provide escape
  valves when the server is overloaded, maintaining TPS at the cost of reduced visible range.
- **Observable**: tracing spans, Prometheus metrics, and `/tps` output give operators and
  developers clear visibility into server performance.
- **Regression protection**: Criterion benchmarks in CI catch performance regressions before
  they reach production.

### Negative

- **Concurrency complexity**: Parallel tick execution introduces data races, deadlocks, and
  ordering bugs that don't exist in a single-threaded model. Testing concurrent systems is
  harder.
- **Determinism challenges**: Parallel entity ticking may produce different results depending
  on thread scheduling. For vanilla parity, we may need to enforce deterministic ordering in
  some systems (at the cost of reduced parallelism).
- **Profiling overhead**: tracing spans, even when inactive, add a small amount of overhead
  (checking the subscriber). This is < 1ns per span but adds up across millions of spans per
  tick.
- **Benchmark maintenance**: Criterion benchmarks must be maintained as code changes. Stale
  benchmarks provide false confidence.

### Neutral

- The performance targets are aspirational for the initial release. The architecture supports
  reaching them, but individual subsystems will be optimized incrementally across phases.
- SIMD optimizations are platform-specific and may require feature flags for portability
  (`target_feature(enable = "avx2")`). The initial implementation uses scalar code with
  SIMD-friendly patterns that auto-vectorize where possible.

## Compliance

- [ ] 20 TPS maintained with 100 simulated players on 8-core hardware (load test).
- [ ] p99 tick time < 50ms under 100-player load.
- [ ] RSS < 4GB with default settings and 100K loaded chunks.
- [ ] Entity tick parallelism: 2+ rayon threads active during Phase 3 (verified via tracing).
- [ ] World generation fully offloaded to rayon pool (no gen work on tick thread).
- [ ] Packet batching: 1 `writev` syscall per player per tick (verified via strace).
- [ ] `/tps` command displays TPS, MSPT, and percentiles in the documented format.
- [ ] Criterion benchmarks exist for: pathfinding, loot evaluation, chunk serialization,
  packet encoding, collision detection.
- [ ] CI performance regression detection triggers on > 5% slowdown.
- [ ] Flamegraph generation works from a running server via debug command.

## Related ADRs

- **ADR-002**: Async Runtime & Threading (tokio + rayon threading model)
- **ADR-006**: Chunk Storage & Management (chunk data structures and caching)
- **ADR-009**: Entity Component System (ECS enables parallel entity processing)
- **ADR-026**: Loot Table & Predicate Engine (hot path requiring fast evaluation)
- **ADR-029**: Memory Management & Allocation (memory layer of scalability)
- **ADR-030**: Graceful Shutdown & Crash Recovery (watchdog monitors tick timing)
- **ADR-031**: Management & Remote Access APIs (TPS metrics exposed via management API)

## References

- [Data-Oriented Design (Richard Fabian)](https://www.dataorienteddesign.com/dodbook/)
- [Rayon: data parallelism in Rust](https://github.com/rayon-rs/rayon)
- [criterion.rs: statistics-driven benchmarking](https://github.com/bheisler/criterion.rs)
- [tracing: structured diagnostics for Rust](https://github.com/tokio-rs/tracing)
- [Folia: regionized multithreading for Paper](https://github.com/PaperMC/Folia)
- [Minecraft performance analysis (PaperMC)](https://paper-chan.moe/paper-optimization/)
- [SIMD in Rust (std::simd)](https://doc.rust-lang.org/std/simd/index.html)
- [Prometheus metrics format](https://prometheus.io/docs/instrumenting/exposition_formats/)
