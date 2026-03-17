# ADR-016: World Generation Pipeline

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P23, P26, P36 |
| Deciders | Oxidized Core Team |

## Context

World generation is the most CPU-intensive operation a Minecraft server performs. When a player
explores new territory, the server must generate chunks on the fly — computing terrain shape
from density functions, placing biomes, carving caves, generating structures (villages, temples,
strongholds), decorating with features (trees, ores, vegetation), and calculating initial
lighting. Vanilla processes chunks through a status pipeline where each status represents a
generation phase, and some phases require neighboring chunks to be at a certain status before
they can proceed (e.g. feature placement needs 8 neighbors at noise status to avoid gaps at
chunk boundaries).

Vanilla's world generation runs primarily on the main server thread, with limited parallelism
via `CompletableFuture` for some phases. This means a player exploring at high speed can
outpace generation, causing visible chunk loading delays. Server forks like Paper have improved
this with async chunk generation, but the fundamental architecture — a single CompletableFuture
chain per chunk with neighbor dependency tracking — remains complex and underutilizes modern
multi-core CPUs. The noise generation phase is particularly expensive, evaluating 3D density
functions at thousands of points per chunk with trilinear interpolation.

Oxidized has the opportunity to design worldgen for parallelism from the ground up. The noise
pipeline is embarrassingly parallel across chunks (each chunk's noise is independent given its
coordinates and the world seed). Feature placement has neighbor dependencies but can still be
parallelized across independent chunk groups. By using Rayon's work-stealing thread pool for
CPU-bound computation and a dependency-aware scheduler for ordering, we can achieve generation
throughput that scales with available CPU cores.

## Decision Drivers

- **Throughput**: The server must generate chunks faster than players can explore. At maximum
  sprint speed (~8 blocks/second, ~0.5 chunks/second per player), a 100-player server needs to
  sustain ~50 chunks/second in the worst case (all exploring new territory simultaneously).
- **Parallelism**: Modern servers have 8–32 cores. Worldgen should scale with core count for
  CPU-bound phases (noise, surface, features).
- **Dependency correctness**: Feature placement (trees, structures) can extend into neighboring
  chunks. The generation pipeline must ensure neighbors are at the correct status before
  proceeding, or features will be cut off at chunk boundaries.
- **Reproducibility**: Given the same world seed and chunk coordinates, generation must produce
  identical results regardless of generation order or thread count. This is critical for
  deterministic worlds.
- **Cancellation**: If a player teleports away while chunks are generating, in-progress
  generation for no-longer-needed chunks should be cancelled to free CPU for chunks that are
  actually needed.
- **Memory budget**: Concurrent generation of many chunks consumes memory (partial chunk data,
  noise caches). The system must limit how many chunks are in-flight simultaneously.

## Considered Options

### Option 1: Single-Threaded Like Vanilla

Process chunks sequentially on the main game thread. Simple and correct, but wastes all but one
CPU core. A server with 16 cores would use ~6% of available CPU for the most expensive
operation. Player experience suffers when exploring, with visible chunk pop-in. Rejected because
parallelism is a core goal of Oxidized.

### Option 2: Parallel Chunk Generation With Rayon Thread Pool

Use Rayon's work-stealing thread pool to process chunks in parallel. Rayon automatically
distributes work across available cores and handles load balancing. For phases without neighbor
dependencies (noise generation), chunks are processed in a `par_iter`. For phases with
dependencies (features), a dependency-aware scheduler ensures ordering. Rayon is well-suited for
CPU-bound computation with no I/O — exactly the profile of worldgen.

### Option 3: Async Pipeline With tokio::task::spawn_blocking

Use tokio's blocking thread pool to run each generation phase as an async task. Chain phases with
`.await` for dependency management. This integrates worldgen with the existing tokio runtime but
has drawbacks: `spawn_blocking` is designed for I/O, not CPU-bound work, and the default thread
pool is shared with disk I/O (ADR-015). Rayon's work-stealing is more efficient for compute
workloads than tokio's blocking pool. Additionally, async overhead (Future state machines, waker
registration) adds unnecessary cost for pure computation.

### Option 4: GPU Compute for Noise (wgpu)

Offload density function evaluation to the GPU via compute shaders. GPUs excel at parallel math,
and noise evaluation is massively parallel. However, the data transfer overhead (uploading
parameters, downloading results) and the complexity of translating vanilla's density function
tree into WGSL/SPIR-V compute shaders is enormous. Many server environments (cloud VMs, Docker
containers) lack GPU access. The benefit is marginal for a server that doesn't need real-time
frame rates — CPU cores are sufficient for Minecraft's generation throughput needs.

### Option 5: SIMD-Optimized Density Functions

Use SIMD (Single Instruction, Multiple Data) instructions to evaluate density functions on
batches of coordinates simultaneously. A single AVX2 instruction can compute 8 `f64` values in
parallel. This can be combined with any threading strategy (it optimizes the inner loop, not the
parallelism model). Not a complete solution on its own, but a valuable optimization for the
noise pipeline. Can be combined with Option 2.

## Decision

We adopt **Rayon thread pool for CPU-bound worldgen + dependency-aware chunk status state machine
+ SIMD-optimized noise evaluation**.

### Chunk Status Pipeline

Chunks progress through a fixed sequence of generation statuses, matching vanilla's pipeline:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ChunkStatus {
    Empty           = 0,
    StructureStarts = 1,
    Biomes          = 2,
    Noise           = 3,
    Surface         = 4,
    Carvers         = 5,
    Features        = 6,
    Light           = 7,
    Full            = 8,
}
```

Each status has:
- A **generation function** that produces the chunk data for that phase.
- A **neighbor requirement**: the minimum status that neighboring chunks must have reached
  before this status can begin. This is expressed as a radius and a minimum status.

```rust
pub struct StatusRequirement {
    pub radius: u8,          // 0 = no neighbors, 1 = 8 neighbors, 2 = 24 neighbors
    pub min_neighbor_status: ChunkStatus,
}
```

Key neighbor requirements (matching vanilla):
| Status | Neighbor Radius | Min Neighbor Status |
|--------|----------------|---------------------|
| StructureStarts | 0 | — |
| Biomes | 0 | — |
| Noise | 0 | — |
| Surface | 0 | Noise |
| Carvers | 0 | Noise |
| Features | 1 | Carvers |
| Light | 1 | Features |
| Full | 0 | Light |

Features require radius 1 (8 direct neighbors) at Carvers status because trees and structures
can extend up to 16 blocks into neighboring chunks. Light requires radius 1 at Features because
light propagates across chunk boundaries.

### Dependency-Aware Scheduler

The worldgen scheduler tracks the status of all chunks being generated and dispatches work to
Rayon when dependencies are satisfied:

```rust
pub struct WorldgenScheduler {
    pending: DashMap<ChunkPos, ChunkGenTask>,
    in_progress: DashMap<ChunkPos, ChunkStatus>, // currently being generated
    rayon_pool: rayon::ThreadPool,
    max_concurrent: usize, // memory budget limit
    semaphore: Arc<Semaphore>,
}

pub struct ChunkGenTask {
    target_status: ChunkStatus,
    current_status: ChunkStatus,
    priority: ChunkGenPriority,
    cancel_token: CancellationToken,
}
```

The scheduler runs a loop:
1. Collect all pending chunks whose dependencies are satisfied.
2. Sort by priority (player-requested > ticket-based > background fill).
3. Submit up to `max_concurrent` tasks to the Rayon pool.
4. When a task completes, update the chunk's status and check if any dependents are now ready.

```rust
fn dispatch_ready_chunks(&self) {
    let ready: Vec<_> = self.pending.iter()
        .filter(|entry| self.dependencies_satisfied(entry.key(), entry.value()))
        .map(|entry| (*entry.key(), entry.value().clone()))
        .collect();

    let mut sorted = ready;
    sorted.sort_by_key(|(_, task)| std::cmp::Reverse(task.priority));

    for (pos, task) in sorted.into_iter().take(self.max_concurrent) {
        let permit = self.semaphore.clone().try_acquire_owned();
        if permit.is_none() { break; } // at capacity

        self.in_progress.insert(pos, task.current_status.next());
        let cancel = task.cancel_token.clone();

        self.rayon_pool.spawn(move || {
            if cancel.is_cancelled() { return; }
            let result = generate_status(pos, task.current_status.next());
            // ... update chunk, notify scheduler
        });
    }
}
```

### Priority System

Chunk generation is prioritized based on urgency:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChunkGenPriority {
    Urgent = 3,       // player is waiting (within 2 chunks of player)
    High = 2,         // within player's view distance
    Normal = 1,       // background fill, spawn area
    Low = 0,          // pregeneration, exploration lookahead
}
```

Urgent chunks (close to the player) are generated first, ensuring that the player's immediate
surroundings load quickly even when many chunks are queued.

### Cancellation

When a player moves away from an area, chunks that are no longer needed have their
`CancellationToken` cancelled. In-progress generation tasks check the token at phase boundaries
(between density function evaluations, between feature placements). If cancelled, the task
aborts early, freeing the Rayon worker for higher-priority work. Partially generated chunks are
discarded — they will be regenerated from scratch if needed later.

### Noise Pipeline With SIMD

The noise generation phase evaluates density functions at a 4×4×4 grid within each 16×16×16
section, then trilinearly interpolates to fill the full resolution. Density function evaluation
is the innermost loop and benefits greatly from SIMD.

Using `std::simd` (portable SIMD), we batch 4 or 8 coordinate evaluations per SIMD lane:

```rust
use std::simd::f64x4;

fn evaluate_noise_batch(coords: &[(f64, f64, f64); 4], seed: i64) -> f64x4 {
    let x = f64x4::from_array([coords[0].0, coords[1].0, coords[2].0, coords[3].0]);
    let y = f64x4::from_array([coords[0].1, coords[1].1, coords[2].1, coords[3].1]);
    let z = f64x4::from_array([coords[0].2, coords[1].2, coords[2].2, coords[3].2]);
    perlin_noise_simd(x, y, z, seed)
}
```

For platforms without SIMD support, a scalar fallback is provided. The SIMD path is selected at
runtime via `#[cfg(target_feature)]` or dynamic dispatch.

### Reproducibility

Deterministic generation is guaranteed by:
1. **Seed-based RNG**: All randomness derives from `world_seed + chunk_x + chunk_z` via a
   deterministic hash (not thread-local random state).
2. **Deterministic evaluation order**: Within a chunk, features are placed in a fixed order
   (sorted by position, then by feature type). The dependency scheduler ensures that a chunk's
   neighbors are always at the required status before generation proceeds, regardless of thread
   scheduling order.
3. **No floating-point non-determinism**: Density functions use deterministic evaluation order.
   SIMD operations on `f64` are associative when operand order is fixed.

### Memory Budget

Concurrent generation consumes memory for in-progress chunks (partial data, noise caches). The
`max_concurrent` limit caps the number of simultaneously in-progress chunks:

```rust
// Estimated memory per in-progress chunk: ~256 KB (noise cache + partial sections)
// With 64 concurrent chunks: ~16 MB — acceptable
const DEFAULT_MAX_CONCURRENT: usize = 64;
```

The semaphore in the scheduler enforces this limit. When the budget is exhausted, lower-priority
chunks wait until higher-priority ones complete.

### Structure Placement

Structures (villages, temples, mineshafts, strongholds) interact with worldgen in two phases:

1. **StructureStarts** (per-chunk, no neighbors needed): Determine if a structure starts in this
   chunk based on seed + position. Record the structure's bounding box and type.
2. **Features** (radius 1, neighbors at Carvers): Place the structure's blocks, potentially
   extending into neighboring chunks. The structure start data from neighbors is read to
   determine which structures extend into this chunk.

Structure placement RNG uses the structure-specific seed (`world_seed XOR structure_salt`) for
reproducibility.

## Consequences

### Positive

- **Linear scaling with cores**: Rayon's work-stealing distributes noise generation across all
  available CPU cores. An 8-core server generates ~8× faster than single-threaded.
- **Player-first prioritization**: The priority system ensures that chunks near players are
  generated first, minimizing visible pop-in even under heavy load.
- **Cancellation saves CPU**: Aborting generation for chunks the player has moved away from frees
  resources for chunks that are actually needed.
- **Deterministic worlds**: Seed-based RNG and fixed evaluation order guarantee identical worlds
  regardless of generation order or thread count.
- **SIMD acceleration**: Batch noise evaluation provides 2–4× speedup on the most expensive inner
  loop without changing the algorithm.

### Negative

- **Dependency scheduler complexity**: Tracking neighbor statuses and dispatching work when
  dependencies are satisfied is complex. Bugs can cause deadlocks (two chunks waiting for each
  other) or incorrect generation (features placed before neighbors are ready).
- **Memory pressure**: 64 concurrent in-progress chunks use ~16 MB. Under extreme load (many
  players exploring simultaneously), the memory budget may need dynamic adjustment.
- **Rayon integration**: Rayon's global thread pool must be configured at startup and cannot
  easily be resized. If worldgen and other CPU-bound tasks (lighting, pathfinding) share the
  pool, they may interfere. A dedicated Rayon pool for worldgen avoids this.

### Neutral

- **No GPU acceleration**: We chose not to use GPU compute for noise. This keeps the server
  environment-agnostic (no GPU required) at the cost of ~10× potential speedup for the noise
  phase. If profiling shows noise as the dominant bottleneck, GPU acceleration can be revisited.
- **Vanilla status pipeline**: We match vanilla's status pipeline exactly, which constrains our
  design but ensures compatibility with vanilla world data and data pack expectations.

## Compliance

- **Determinism test**: Generate the same chunk on 1 thread and 8 threads; verify block-for-block
  identity.
- **Neighbor dependency test**: Intentionally violate neighbor requirements (attempt Features
  before neighbors reach Carvers); verify the scheduler blocks until dependencies are met.
- **Cancellation test**: Start generating a chunk, cancel it, verify the task aborts within a
  bounded time and the chunk is not marked as generated.
- **Priority test**: Enqueue low-priority and urgent chunks; verify urgent chunks complete first.
- **Throughput benchmark**: Measure chunks generated per second at varying thread counts (1, 2,
  4, 8, 16). Verify near-linear scaling up to core count.
- **Compatibility test**: Generate a vanilla-seed world, compare chunk block data against vanilla
  server output for a set of test chunks.

## Related ADRs

- **ADR-014** (Chunk Storage): Generated chunks are inserted into the chunk map via `DashMap`.
  The chunk's status field tracks generation progress.
- **ADR-015** (Disk I/O): Newly generated chunks are saved to disk on the next autosave cycle.
  The worldgen scheduler may also load neighboring chunks from disk if they exist.
- **ADR-012** (Block State Representation): Generated block data uses `BlockState` IDs from the
  dense lookup table.
- **ADR-011** (Registry System): Biome registry, structure registry, and feature registry are
  read during worldgen. These must be frozen before generation starts.
- **ADR-017** (Lighting Engine): The Light status in the generation pipeline invokes the lighting
  engine to compute initial sky and block light for the generated chunk.

## References

- [Rayon crate](https://docs.rs/rayon) — data parallelism library for Rust
- [Minecraft Wiki — Chunk Format](https://minecraft.wiki/w/Chunk_format) — generation statuses
- [Minecraft Wiki — World Generation](https://minecraft.wiki/w/World_generation) — pipeline overview
- [Minecraft Wiki — Density Function](https://minecraft.wiki/w/Density_function) — noise evaluation
- [std::simd](https://doc.rust-lang.org/std/simd/index.html) — portable SIMD for Rust
- [CubiomesViewer](https://github.com/Cubitect/cubiomes) — reference for seed-based generation
