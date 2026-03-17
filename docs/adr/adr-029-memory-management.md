# ADR-029: Memory Management & Allocation

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P38 |
| Deciders | Oxidized Core Team |

## Context

A Minecraft server managing 100 concurrent players with 100,000 loaded chunks is a long-
running, memory-intensive application. Vanilla Java Edition relies entirely on the JVM garbage
collector, which introduces periodic pauses that manifest as server lag spikes — particularly
during world saving when large numbers of objects become unreachable simultaneously. The G1
and ZGC collectors mitigate this but never eliminate it. Server operators routinely tune JVM
flags (`-Xms`, `-Xmx`, `-XX:+UseZGC`, etc.) to manage GC behavior, and GC tuning is one of
the most common topics in Minecraft server administration.

Rust gives us deterministic deallocation — objects are freed exactly when they go out of scope,
with no stop-the-world pauses. However, this doesn't mean memory management is free. The
default system allocator (glibc malloc on Linux) can exhibit fragmentation over time as chunks
are loaded and unloaded, entities spawn and despawn, and packet buffers are allocated and freed
in varying sizes. A server running for days or weeks can see RSS grow well beyond its logical
working set due to fragmentation. Additionally, the allocation pattern matters: frequent small
allocations (e.g., one `Vec<u8>` per packet) put pressure on the allocator's fast path, and
thread contention on the global allocator can become a bottleneck in multi-threaded workloads.

The memory budget for a Minecraft server is well-understood. Each chunk section (16×16×16) uses
a PalettedContainer for block states (~4-12KB depending on palette complexity), another for
biomes (~64 bytes for single-biome sections), plus light data (~2KB per section for block +
sky light). A full 24-section chunk is roughly 28KB for block data + light + heightmaps +
metadata. With 100,000 loaded chunks, that's ~2.8GB for world data alone. Entities, player
data, network buffers, and the tick pipeline add another 500MB–1GB. We need to be intentional
about where memory goes and how it's managed.

## Decision Drivers

- **No GC pauses**: The primary advantage over vanilla — we must not reintroduce pause-like
  behavior through allocator contention or excessive memory pressure.
- **Low fragmentation**: A server running for weeks must not see unbounded RSS growth. The
  allocator must handle the load/unload pattern (chunks, entities) gracefully.
- **Multi-threaded performance**: The ECS tick pipeline, world generation, and network I/O
  all run on multiple threads. Allocator contention must be minimal.
- **Allocation efficiency**: Hot paths (packet serialization, loot evaluation, pathfinding)
  should minimize allocator calls. Per-tick temporary data should avoid hitting the global
  allocator entirely.
- **Memory visibility**: Operators should be able to monitor memory usage (total RSS, per-
  subsystem breakdown, allocation rates) to diagnose issues.
- **Simplicity**: Exotic allocation strategies add complexity. Use the simplest approach that
  meets performance requirements.

## Considered Options

### Option 1: System Allocator (glibc malloc / musl malloc)

Use Rust's default global allocator, which delegates to the platform's malloc.

**Pros**: Zero configuration, no additional dependencies, well-tested, debugger-friendly
(valgrind, AddressSanitizer work out of the box).

**Cons**: glibc malloc has known fragmentation issues with long-running processes. Thread
contention is moderate — it uses per-thread arenas but can still contend under high load.
musl malloc is worse for multi-threaded workloads. No built-in profiling.

### Option 2: jemalloc

Facebook's allocator, designed for long-running multi-threaded applications. Used by Firefox
and many database systems.

**Pros**: Excellent fragmentation resistance (slab-based size classes). Good multi-threaded
scaling (thread-local caches). Built-in profiling (`malloc_stats_print`, heap profiling).
Mature and battle-tested.

**Cons**: ~200KB binary size increase. Slightly slower than mimalloc for allocation-heavy
workloads. Configuration is complex (many tuning knobs). Can over-retain memory (configurable
via `dirty_decay_ms` and `muzzy_decay_ms`).

### Option 3: mimalloc

Microsoft's allocator, designed for high-performance multi-threaded applications. Used by the
.NET runtime, Zig, and several game engines.

**Pros**: Fastest general-purpose allocator in most benchmarks (especially for multi-threaded
workloads). Very low fragmentation via segment-based memory management. Minimal configuration
needed. Small binary footprint. Free page retirement reduces RSS when memory usage decreases.

**Cons**: Less mature than jemalloc (but well-tested in production at Microsoft). Profiling
support is less extensive than jemalloc's. Slightly higher memory overhead per thread cache
than system malloc.

### Option 4: Arena Allocation for Per-Tick Temporary Data

Use a bump allocator (like `bumpalo`) for allocations that live only within a single tick.
Reset the arena between ticks — deallocation of all tick-temporary data is a single pointer
reset (effectively free).

**Pros**: Zero deallocation cost for temporary data. No fragmentation (arena is contiguous).
Excellent cache locality. Enables allocation patterns that would be expensive with the global
allocator (e.g., hundreds of small `Vec`s during pathfinding).

**Cons**: Requires discipline — arena-allocated data must not escape the tick. Lifetime
management can be tricky (`'arena` lifetime parameter on allocated types). Arena size must be
pre-allocated or grown with realloc. Not suitable for all allocation patterns.

### Option 5: Object Pooling for Hot Allocations

Pool frequently allocated and freed objects (packet buffers, NBT nodes, entity component
scratch space) to avoid hitting the allocator on the hot path.

**Pros**: Eliminates allocation for pooled types. Predictable memory usage. Reduces allocator
contention.

**Cons**: Pool management adds complexity (sizing, thread safety, cleanup). Pooled objects may
hold stale data if not properly cleared. Over-sized pools waste memory. Requires profiling to
identify which objects benefit from pooling.

## Decision

**mimalloc as global allocator + arena allocation for per-tick temporaries + object pooling
for network buffers.** This is a layered strategy where each layer addresses a specific class
of allocation pattern.

### Layer 1: mimalloc Global Allocator

Set mimalloc as the global allocator via `#[global_allocator]`:

```rust
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
```

This replaces the system allocator for all heap allocations. mimalloc provides:

- **Thread-local free lists**: Each thread has its own allocation cache, minimizing contention.
  Only when the local free list is exhausted does the thread interact with the global heap.
- **Segment-based memory**: Memory is managed in 4MB segments divided into pages. Pages within
  a segment are used for allocations of similar sizes, reducing fragmentation.
- **Free page retirement**: When a page becomes fully free, it's returned to the OS (or
  retained in a local cache for reuse), keeping RSS close to the actual working set.
- **Batch deallocation**: When a thread frees memory belonging to another thread's segment,
  the free is batched and processed lazily, reducing cross-thread contention.

Configuration (via environment variables for runtime tuning):

```
MIMALLOC_ARENA_EAGER_COMMIT=1    # Pre-commit arena pages (avoid page faults)
MIMALLOC_PURGE_DELAY=5000        # Delay before returning pages to OS (5s)
MIMALLOC_RESERVE_HUGE_OS_PAGES=1 # Use huge pages if available (reduces TLB misses)
```

### Layer 2: Arena Allocation for Per-Tick Temporaries

Many subsystems allocate temporary data that lives only within a single tick:

- **Pathfinding**: Open/closed sets for A* (hundreds of nodes per mob per tick).
- **Loot evaluation**: Intermediate item lists, condition results, function scratch space.
- **Command parsing**: Parsed command tree, argument values, selector results.
- **Redstone updates**: Block update queue, power level calculations.
- **Entity collision**: Broadphase candidate lists, collision manifolds.

For these, we use `bumpalo::Bump` arenas — one per thread, reset between ticks:

```rust
pub struct TickArena {
    bump: Bump,
}

impl TickArena {
    pub fn new(initial_capacity: usize) -> Self {
        Self { bump: Bump::with_capacity(initial_capacity) }
    }

    pub fn alloc<T>(&self, val: T) -> &T {
        self.bump.alloc(val)
    }

    pub fn alloc_vec<T>(&self) -> bumpalo::collections::Vec<'_, T> {
        bumpalo::collections::Vec::new_in(&self.bump)
    }

    pub fn reset(&mut self) {
        self.bump.reset();  // single pointer reset; no destructors called
    }
}
```

Arena sizing: start at 1MB per thread, grow if needed. Typical per-tick temporary allocation
is 100KB–500KB. The arena grows but never shrinks during the process lifetime (freed on
thread exit). The `reset()` operation is O(1) — it simply resets the allocation pointer to
the start of the arena.

**Critical safety rule**: Arena-allocated data must not outlive the tick. This is enforced by
the borrow checker — arena-allocated references carry the arena's lifetime (`'arena`), so
storing them in long-lived structures is a compile error. Subsystem APIs that use tick arenas
take `&TickArena` as a parameter and return `'arena`-lifetime references.

### Layer 3: Object Pooling for Network Buffers

Network packet serialization is the highest-frequency allocation workload. Each outgoing
packet requires a `BytesMut` buffer for serialization, and each incoming packet uses a
`BytesMut` for reading. With 100 players and ~50 packets/player/tick, that's ~100,000
buffer allocations per second.

We pool `BytesMut` buffers using a lock-free stack (crossbeam-based):

```rust
pub struct BufferPool {
    pool: SegQueue<BytesMut>,
    buffer_capacity: usize,
    max_pool_size: usize,
}

impl BufferPool {
    pub fn acquire(&self) -> BytesMut {
        self.pool.pop().unwrap_or_else(|| BytesMut::with_capacity(self.buffer_capacity))
    }

    pub fn release(&self, mut buf: BytesMut) {
        buf.clear();
        if self.pool.len() < self.max_pool_size {
            buf.reserve(self.buffer_capacity.saturating_sub(buf.capacity()));
            self.pool.push(buf);
        }
        // else: drop buf, returning memory to allocator
    }
}
```

Default pool configuration:
- `buffer_capacity`: 4096 bytes (most packets fit within 4KB).
- `max_pool_size`: 2048 buffers (8MB max pool memory).
- Separate pools for different size classes (256B, 4KB, 64KB) to avoid wasting memory on
  small packets allocated from large buffers.

### Memory Budget Estimation

For a server with default settings (view distance 10, 100 players):

| Component | Per-Unit | Count | Total |
|-----------|----------|-------|-------|
| Chunk block data (PalettedContainer) | ~16KB | 100,000 | ~1.6 GB |
| Chunk light data | ~8KB | 100,000 | ~800 MB |
| Chunk metadata (heightmaps, tick lists) | ~4KB | 100,000 | ~400 MB |
| Entity data (ECS components) | ~2KB | 50,000 | ~100 MB |
| Player data (inventory, stats, advancements) | ~50KB | 100 | ~5 MB |
| Network buffers (pool) | 4KB | 2,048 | ~8 MB |
| Tick arenas (per thread) | 1MB | 16 | ~16 MB |
| Recipe/loot/tag registries | — | — | ~50 MB |
| World generation cache | — | — | ~200 MB |
| **Total estimated** | | | **~3.2 GB** |

This fits comfortably within 4GB, leaving headroom for spikes (mass mob spawning, large
explosions generating many items, etc.). The `-Xmx`-equivalent for our server would be a
configurable chunk cache cap that unloads chunks when memory exceeds a threshold.

### Ownership and Lifetime Patterns

Key ownership decisions:

- **Chunks**: Owned by the `ChunkMap` (stored in `HashMap<ChunkPos, Arc<Chunk>>`). `Arc` is
  used because chunks may be referenced by network serialization (sending chunk data to
  players) while the tick pipeline modifies the map. The `Arc` ensures a chunk being sent
  is not deallocated mid-serialization.
- **Entities**: Owned by the ECS `World`. Entity references within a tick are lifetimed to
  the tick — no dangling entity references.
- **Packet buffers**: Owned by the pool; temporarily moved to the serialization context, then
  returned to the pool after the packet is written to the socket.
- **Configuration / registries**: `Arc`-wrapped and atomically swapped on `/reload`. Old
  registries remain valid until all references are dropped.

### Clean Shutdown Drop Order

On graceful shutdown, resources are freed in a specific order to prevent use-after-free:

1. Stop accepting new connections.
2. Flush all pending outgoing packets.
3. Disconnect all players (sends disconnect packet, flushes, closes socket).
4. Save all dirty chunks to disk.
5. Save all player data.
6. Drop the ECS World (destroys all entities and their components).
7. Drop the ChunkMap (frees all chunk memory).
8. Drop registries, loot tables, recipes.
9. Drop the tick arenas.
10. Drop the buffer pools.
11. Drop the tokio runtime (joins all async tasks with a 10s timeout).
12. Process exits; mimalloc returns all segments to the OS.

### Heap Profiling Integration

For debugging memory issues, we integrate with standard profiling tools:

- **DHAT (Dynamic Heap Analysis Tool)**: Compile with `dhat` feature to enable heap profiling.
  Produces JSON output viewable in DHAT's viewer. Shows allocation hotspots, short-lived
  allocations, and peak memory usage.
- **heaptrack**: Works with mimalloc via `LD_PRELOAD`. Tracks every allocation/deallocation
  with call stacks. Useful for finding memory leaks.
- **mimalloc stats**: Enable with `MIMALLOC_SHOW_STATS=1` at shutdown. Prints segment counts,
  page utilization, and fragmentation metrics.
- **Custom metrics**: Expose allocation counters via the `/debug` command and management API:
  - Total RSS
  - mimalloc committed memory
  - Buffer pool utilization (acquired/total)
  - Per-tick arena high water mark
  - Chunk cache size (bytes and count)
  - Entity component storage size

## Consequences

### Positive

- **No GC pauses**: Deterministic deallocation means no stop-the-world events. Tick timing
  is consistent and predictable.
- **Low fragmentation**: mimalloc's segment-based approach handles the load/unload pattern
  (chunks cycling in and out) much better than glibc malloc.
- **Minimal allocator contention**: Thread-local caches in mimalloc + per-thread tick arenas
  mean threads rarely interact with global heap state.
- **Efficient hot paths**: Tick arenas eliminate allocation cost for temporary data. Buffer
  pools eliminate allocation cost for network I/O. The two hottest allocation paths are
  effectively free.
- **Predictable memory**: The memory budget table gives operators a clear formula for
  capacity planning based on view distance and player count.

### Negative

- **Additional dependency**: mimalloc is an external C library with a Rust wrapper. Build
  complexity increases slightly (needs C compiler). Updates must be tracked.
- **Arena discipline**: Developers must remember to use tick arenas for temporary data and
  not accidentally allocate on the global heap in hot paths. Code review must enforce this.
- **Pool sizing**: Buffer pool size is a tuning parameter. Too small → pool misses (falls
  through to allocator). Too large → wasted memory. Requires profiling to tune.
- **Profiling overhead**: DHAT and heaptrack add significant runtime overhead (10-50x
  slowdown). They are not suitable for production use — only development debugging.

### Neutral

- mimalloc is a drop-in replacement — existing code doesn't change, only the global allocator
  declaration. Switching to jemalloc later (if needed) is equally trivial.
- Arena allocation is an opt-in optimization. Subsystems that don't need it simply use the
  global allocator. No forced migration.

## Compliance

- [ ] mimalloc is set as `#[global_allocator]` and verified via `MIMALLOC_SHOW_STATS=1`.
- [ ] Server RSS after loading 100,000 chunks is within 20% of the memory budget estimate.
- [ ] After unloading 50,000 chunks, RSS decreases within 60 seconds (free page retirement).
- [ ] Tick arena `reset()` completes in < 1μs (no per-object destructors).
- [ ] Buffer pool hit rate is > 95% under normal load (100 players, default settings).
- [ ] No allocator contention visible in flamegraphs under 100-player load.
- [ ] DHAT and heaptrack produce valid reports when enabled.
- [ ] Shutdown completes in < 30 seconds with correct drop order (no use-after-free).
- [ ] 72-hour stress test shows RSS growth < 10% beyond initial stabilized value.

## Related ADRs

- **ADR-002**: Async Runtime & Threading (tokio runtime + rayon pool are the thread sources)
- **ADR-006**: Chunk Storage & Management (chunk memory is the largest consumer)
- **ADR-009**: Entity Component System (ECS component storage is a major allocator)
- **ADR-030**: Graceful Shutdown & Crash Recovery (shutdown drop order)
- **ADR-032**: Performance & Scalability Architecture (memory is a scalability axis)

## References

- [mimalloc: A Free and Open Source General Purpose Allocator](https://github.com/microsoft/mimalloc)
- [mimalloc benchmark results](https://github.com/microsoft/mimalloc#benchmark-results)
- [bumpalo: A fast bump allocation arena](https://github.com/fitzgen/bumpalo)
- [DHAT: Dynamic Heap Analysis Tool](https://valgrind.org/docs/manual/dh-manual.html)
- [heaptrack — heap memory profiler](https://github.com/KDE/heaptrack)
- [crossbeam SegQueue (lock-free concurrent queue)](https://docs.rs/crossbeam/latest/crossbeam/queue/struct.SegQueue.html)
- [jemalloc fragmentation analysis](https://engineering.fb.com/2011/01/03/core-data/scalable-memory-allocation-using-jemalloc/)
