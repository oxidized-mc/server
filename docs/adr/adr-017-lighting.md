# ADR-017: Lighting Engine

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P13, P19, P23a |
| Deciders | Oxidized Core Team |

## Context

Lighting is a critical system in Minecraft that affects both gameplay (mob spawning depends on
light levels) and client rendering (the server must send correct light data before a chunk can be
displayed). Every block position in the world has two light values: sky light (0–15, from the
sun/moon, attenuated by blocks) and block light (0–15, from light-emitting blocks like torches,
glowstone, lava). These are stored as 4-bit nibbles — two blocks per byte — in per-section
arrays of 2048 bytes each (16×16×16 blocks ÷ 2 = 2048).

Light propagation follows a breadth-first search (BFS) from light sources. Sky light starts at
level 15 above the heightmap and propagates downward through transparent blocks, decreasing by 1
per block (or more for blocks with high opacity). Block light starts at the emitter's level
(e.g. torch = 14, glowstone = 15) and propagates outward, also decreasing by 1 per block.
When a block changes (placed, broken, state changed), the lighting engine must recalculate the
affected region — removing old light, propagating new light, and updating all impacted positions.

Lighting is on the critical path for chunk sending. A chunk cannot be sent to a client until its
light data is fully calculated. During world generation, the Light status is one of the final
steps before a chunk reaches Full status. During gameplay, a block change triggers an incremental
light update that must complete before the next tick's chunk section updates are sent to clients.
This means the lighting engine must be both fast for initial full-chunk calculations (worldgen)
and low-latency for incremental updates (gameplay).

Vanilla's `ThreadedLevelLightEngine` queues light updates and processes them on a dedicated
thread (or the main thread in older versions). Paper's StarLight engine is a significant
optimization that processes light in batched passes with reduced algorithmic complexity. We take
inspiration from StarLight's approach of batching and section-level parallelism while adapting
the design to Rust's concurrency model.

## Decision Drivers

- **Correctness**: Light values must exactly match vanilla's calculation for every block position.
  Clients use light data for rendering, and mob spawning depends on light levels. Any
  discrepancy is visible or gameplay-affecting.
- **Chunk sending latency**: Light calculation must complete fast enough that newly generated
  chunks can be sent to the client without visible delay. Target: < 1 ms for a single chunk's
  full lighting pass.
- **Incremental update latency**: When a player places or breaks a block, the light update must
  complete within the same tick (50 ms budget) to be included in the next section update packet.
- **Parallelism**: Full-chunk lighting (worldgen) should leverage multiple cores. Incremental
  updates are typically small enough for a single thread but should not block other work.
- **Memory efficiency**: Light data (2048 bytes per section, 24 sections per chunk) is a
  significant memory cost. Empty sections above the heightmap should use a shared "full bright"
  constant rather than allocating individual arrays.

## Considered Options

### Option 1: Single-Threaded BFS Per Chunk Like Vanilla

Process light updates sequentially, one chunk at a time. A queue holds pending light updates, and
a single thread drains the queue between ticks. This is simple and matches vanilla's basic
approach. However, full-chunk lighting during worldgen is slow (each chunk processes independently,
no parallelism), and the single thread can become a bottleneck when many chunks are generated
simultaneously or many blocks change in one tick (e.g. TNT explosion).

### Option 2: Parallel BFS With Lock-Free Queues

Each section's light data is behind a lock, and multiple threads process BFS expansions in
parallel. Light propagation that crosses a section boundary enqueues work for the neighboring
section. Lock-free queues (crossbeam-deque) pass work between threads. This achieves high
parallelism but the synchronization overhead for fine-grained cross-section propagation (light
crosses section boundaries frequently) may negate the benefits. Correctness is hard to verify
because the order of BFS expansion affects intermediate states.

### Option 3: Deferred Lighting

Send chunks to clients without light data, then compute lighting asynchronously and send light
update packets. This decouples chunk sending from lighting, reducing latency for initial chunk
display. However, the client renders the chunk in complete darkness until light arrives, causing
visible flicker. This is a poor player experience and is not how vanilla works. Modern clients
expect light data in the initial chunk packet.

### Option 4: StarLight-Style Batched Propagation

Process light updates in batched passes over sections. Group pending updates by section, process
each section's updates in a single pass, then propagate to neighboring sections. Repeat until no
more cross-section propagation is needed. Sections that don't share boundaries can be processed
in parallel (e.g. alternating Y-layers). This batches BFS expansions for cache efficiency and
enables controlled parallelism without fine-grained locking.

### Option 5: GPU-Accelerated Lighting

Offload light propagation to the GPU via compute shaders. Each block position is a thread,
propagation happens in parallel waves. However, 4-bit values are awkward for GPU architectures
(no native 4-bit types), the BFS algorithm is branch-heavy (check opacity per block), and
cross-section dependencies require synchronization between shader invocations. The data transfer
overhead (upload block data, download light values) likely exceeds the computation time for
Minecraft's relatively small sections. GPUs are not universally available on servers.

## Decision

We adopt **batched BFS with parallel section processing**, inspired by Paper's StarLight engine
but designed for Rust's ownership and concurrency model.

### Architecture Overview

The lighting engine consists of:

1. **LightUpdateQueue**: Accumulates pending light changes during a tick.
2. **LightProcessor**: Processes the queue in batched passes, propagating light changes.
3. **NibbleArray**: Per-section storage for 4-bit light values.
4. **Heightmap integration**: Sky light initialization depends on the surface heightmap.

### Nibble Storage

Each section stores sky light and block light as `NibbleArray`:

```rust
pub struct NibbleArray {
    data: [u8; 2048], // 4096 nibbles, 2 per byte
}

impl NibbleArray {
    pub const FULL_BRIGHT: NibbleArray = NibbleArray { data: [0xFF; 2048] };
    pub const EMPTY: NibbleArray = NibbleArray { data: [0x00; 2048] };

    #[inline]
    pub fn get(&self, x: u8, y: u8, z: u8) -> u8 {
        let index = ((y as usize) << 8) | ((z as usize) << 4) | (x as usize);
        let byte = self.data[index >> 1];
        if index & 1 == 0 { byte & 0x0F } else { byte >> 4 }
    }

    #[inline]
    pub fn set(&mut self, x: u8, y: u8, z: u8, value: u8) {
        let index = ((y as usize) << 8) | ((z as usize) << 4) | (x as usize);
        let byte = &mut self.data[index >> 1];
        if index & 1 == 0 {
            *byte = (*byte & 0xF0) | (value & 0x0F);
        } else {
            *byte = (*byte & 0x0F) | ((value & 0x0F) << 4);
        }
    }
}
```

Sections above the highest block in a chunk column share a `FULL_BRIGHT` constant for sky light
instead of allocating individual arrays. This saves 2048 bytes × (number of empty sky sections)
per chunk — typically 10–15 sections, saving 20–30 KB per chunk.

### Sky Light Initialization

For initial chunk lighting (worldgen), sky light is calculated top-down:

```
1. Start at the top of the world (section 23 in overworld).
2. For each column (x, z), sky light is 15 above the heightmap.
3. Below the heightmap, light decreases by the block's opacity.
4. Propagate horizontally through transparent blocks via BFS.
```

```rust
fn initialize_sky_light(chunk: &ChunkColumn) {
    let heightmap = chunk.heightmaps.read().motion_blocking();

    // Phase 1: Vertical propagation (per-column, no cross-chunk deps)
    for x in 0..16u8 {
        for z in 0..16u8 {
            let surface_y = heightmap.get(x, z);
            let mut light = 15u8;

            // Above heightmap: full bright
            for y in (surface_y..world_height).rev() {
                set_sky_light(chunk, x, y, z, 15);
            }

            // Below heightmap: attenuate by opacity
            for y in (min_y..surface_y).rev() {
                let opacity = block_state_at(chunk, x, y, z).light_opacity();
                light = light.saturating_sub(opacity.max(1));
                set_sky_light(chunk, x, y, z, light);
                if light == 0 { break; }
            }
        }
    }

    // Phase 2: Horizontal BFS propagation
    propagate_sky_light_bfs(chunk);
}
```

### Block Light Initialization

Block light is simpler — BFS from each light-emitting block:

```rust
fn initialize_block_light(chunk: &ChunkColumn) {
    let mut queue = VecDeque::new();

    // Seed queue with all emitters
    for section_y in 0..24 {
        let section = chunk.sections[section_y].read();
        for index in 0..4096 {
            let state = section.block_states.get(index);
            let emission = state.data().light_emission;
            if emission > 0 {
                let (x, y, z) = index_to_local(index);
                let world_y = section_y_to_world_y(section_y) + y as i32;
                queue.push_back((x, world_y, z, emission));
                set_block_light(chunk, x, world_y as u8, z, emission);
            }
        }
    }

    // BFS propagation
    propagate_block_light_bfs(chunk, &mut queue);
}
```

### Batched Processing

During gameplay, light updates are batched rather than processed individually:

```rust
pub struct LightUpdateQueue {
    pending: Vec<LightUpdate>,
}

pub struct LightUpdate {
    pos: BlockPos,
    old_state: BlockState,
    new_state: BlockState,
}

impl LightEngine {
    /// Called once per tick after block changes are applied.
    pub fn process_updates(&mut self, updates: &[LightUpdate], chunk_map: &ChunkMap) {
        // Group by section
        let mut by_section: HashMap<SectionPos, Vec<&LightUpdate>> = HashMap::new();
        for update in updates {
            let section = update.pos.section_pos();
            by_section.entry(section).or_default().push(update);
        }

        // Process each affected section
        for (section_pos, section_updates) in &by_section {
            self.process_section_updates(section_pos, section_updates, chunk_map);
        }
    }
}
```

For each affected section, the engine:
1. **Removes old light**: If a block's emission or opacity changed, BFS from the position to
   remove light that was propagated from the old source (decrease pass).
2. **Adds new light**: BFS from the position to propagate new light values (increase pass).
3. **Cross-section propagation**: If light changes reach a section boundary, the neighboring
   section is queued for processing.

### Parallel Section Processing

For full-chunk lighting (worldgen), sections can be processed in parallel when they don't share
boundaries. We process even-indexed Y sections first, then odd-indexed:

```rust
fn light_chunk_parallel(chunk: &ChunkColumn) {
    // Phase 1: Even sections (0, 2, 4, ...) — no shared boundaries between them
    (0..24).step_by(2).into_par_iter().for_each(|y| {
        process_section_light(chunk, y);
    });

    // Phase 2: Odd sections (1, 3, 5, ...) — propagate from even neighbors
    (1..24).step_by(2).into_par_iter().for_each(|y| {
        process_section_light(chunk, y);
    });

    // Phase 3: Cross-section boundary resolution
    resolve_boundaries(chunk);
}
```

This gives us up to 12-way parallelism per chunk for the bulk of the work. For incremental
updates during gameplay, the affected area is typically small enough that single-threaded BFS
is faster than the overhead of parallelization.

### Light Update Packets

After processing light updates, the server must notify clients. The light update packet uses a
`BitSet` to indicate which sections have changed sky light and/or block light:

```rust
pub struct LightUpdateData {
    pub sky_light_mask: BitSet,    // which sections have sky light data
    pub block_light_mask: BitSet,  // which sections have block light data
    pub empty_sky_mask: BitSet,    // which sections have empty (all-zero) sky light
    pub empty_block_mask: BitSet,  // which sections have empty block light
    pub sky_light: Vec<NibbleArray>,   // data for sections in sky_light_mask
    pub block_light: Vec<NibbleArray>, // data for sections in block_light_mask
}
```

During initial chunk sending, the full light data is included in the chunk data packet. For
incremental updates, a separate `UpdateLight` packet is sent with only the changed sections'
data. The mask system ensures that unchanged sections are not re-sent.

### Why Not GPU

GPU-based lighting was rejected for several reasons:

1. **4-bit values**: GPUs operate on 32-bit or 16-bit values natively. Packing and unpacking
   4-bit nibbles adds overhead that negates the parallelism benefit.
2. **Branch-heavy BFS**: Each BFS step checks the block's opacity, which varies per block type.
   GPU warps diverge heavily on branching, reducing throughput.
3. **Cross-section dependencies**: Light propagates across section and chunk boundaries, requiring
   synchronization between shader invocations. Multi-pass dispatch with barrier synchronization
   is complex and latency-heavy.
4. **Server environment**: Many Minecraft servers run on headless VMs or containers without GPU
   access. Requiring a GPU would limit deployment options.
5. **Small problem size**: A single chunk section is only 4096 blocks. The overhead of GPU
   dispatch (kernel launch, memory transfer) exceeds the computation time for such small inputs.

## Consequences

### Positive

- **Fast initial lighting**: Parallel section processing enables full-chunk lighting in < 1 ms,
  well within the target for chunk sending during worldgen.
- **Low-latency incremental updates**: Batched BFS processes typical block changes (1–10 blocks
  per tick) in microseconds, ensuring light data is ready for the next section update packet.
- **Memory efficiency**: Sections above the heightmap share a `FULL_BRIGHT` constant, saving
  20–30 KB per chunk. Typical memory for light data is ~24 KB per fully-lit chunk (12 sections
  with allocated nibble arrays × 2048 bytes × 2 light types).
- **Protocol compatibility**: The `BitSet` mask format matches vanilla's `UpdateLight` packet
  structure exactly, ensuring clients receive correctly formatted light data.
- **Correctness**: BFS propagation with per-block opacity checks reproduces vanilla's light
  calculation exactly. The algorithm is well-understood and testable.

### Negative

- **Cross-chunk propagation complexity**: Light propagates across chunk boundaries, requiring the
  lighting engine to access and modify neighboring chunks' section data. This interacts with the
  chunk concurrency model (ADR-014) — the engine must acquire section locks on multiple chunks.
- **Boundary resolution overhead**: The alternating even/odd parallel passes require a boundary
  resolution step that adds latency. For most chunks this is fast, but chunks with many light
  sources near section boundaries may require multiple resolution passes.
- **BFS queue memory**: The BFS queue for a large light update (e.g. removing a large light
  source in an open area) can grow to tens of thousands of entries. Using a `VecDeque` with
  capacity hints mitigates allocation churn.

### Neutral

- **StarLight divergence**: Our design is inspired by StarLight but not a direct port. We adapt
  the batching and parallelism concepts to Rust's ownership model rather than porting Java code.
  The algorithmic core (BFS propagation) is identical.
- **Sky light vs block light asymmetry**: Sky light has a top-down initialization phase that
  block light does not. This means the two light types have different code paths for initial
  calculation, though incremental updates use the same BFS logic for both.

## Compliance

- **Vanilla parity test**: Generate a set of test chunks with known block configurations (cave,
  surface, enclosed room with torches). Compare sky and block light values at every position
  against vanilla server output.
- **Incremental update test**: Place and remove light sources in various configurations. Verify
  that light values after the update match a from-scratch recalculation.
- **Cross-chunk propagation test**: Place a light source near a chunk boundary. Verify that light
  propagates correctly into the neighboring chunk.
- **Performance benchmark**: Measure full-chunk lighting time for representative terrain (plains,
  mountains, caves). Target: < 1 ms per chunk single-threaded, < 200 µs per chunk with parallel
  sections.
- **Nibble array correctness**: Property test that `get(x,y,z)` returns the value previously
  `set(x,y,z)` for all valid coordinates. Verify that adjacent nibbles are not corrupted.
- **Packet format test**: Serialize light update data and verify it matches the expected wire
  format from wiki.vg.

## Related ADRs

- **ADR-014** (Chunk Storage): Light data is stored per-section, accessed via section `RwLock`.
  The lighting engine must respect the concurrency model when updating light across sections.
- **ADR-012** (Block State Representation): Light emission and opacity values come from
  `BlockStateData`, accessed via the dense lookup table.
- **ADR-016** (Worldgen Pipeline): The Light status in worldgen invokes the full-chunk lighting
  pass. Light calculation must complete before a chunk reaches Full status.
- **ADR-013** (Coordinate Types): `BlockPos`, `SectionPos`, and `Direction` are used throughout
  the BFS propagation for neighbor iteration and section boundary detection.

## References

- [Minecraft Wiki — Light](https://minecraft.wiki/w/Light) — light mechanics
- [wiki.vg — Chunk Data](https://wiki.vg/Chunk_Format#Light) — nibble array format
- [wiki.vg — Update Light Packet](https://wiki.vg/Protocol#Update_Light) — packet format
- [Paper StarLight](https://github.com/PaperMC/Paper) — optimized lighting engine
- [Minecraft Wiki — Heightmap](https://minecraft.wiki/w/Heightmap) — surface detection for sky light
