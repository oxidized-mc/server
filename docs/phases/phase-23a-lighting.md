# Phase 23a — Lighting Engine

**Status:** ✅ Complete  
**Crate:** `oxidized-game`, `oxidized-world`, `oxidized-protocol`, `oxidized-server`  
**Reward:** Chunks have correct sky and block light; placing/breaking light sources updates light in real time; light propagates correctly across chunk boundaries and through non-full blocks.

---

## Implementation Summary

### What was built

| Module | File | Description |
|--------|------|-------------|
| BFS propagation | `oxidized-game/src/lighting/propagation.rs` | Core BFS increase/decrease algorithms with direction bitmask tracking |
| Sky light init | `oxidized-game/src/lighting/sky.rs` | Top-down sky light calculation with horizontal BFS and bulk-fill |
| Block light init | `oxidized-game/src/lighting/block_light.rs` | Emitter scan + BFS for block light sources |
| Cross-chunk | `oxidized-game/src/lighting/cross_chunk.rs` | Light propagation across chunk boundaries |
| Parallel | `oxidized-game/src/lighting/parallel.rs` | Sequential wrapper for full-chunk lighting (parallel deferred) |
| Engine | `oxidized-game/src/lighting/engine.rs` | `LightEngine` orchestrator — `process_updates()` returns `LightResult` |
| Face occlusion | `oxidized-game/src/lighting/occlusion.rs` | `shape_occludes()` and `get_light_block_into()` for non-full blocks |
| World lighting | `oxidized-game/src/lighting/world_lighting.rs` | Persistent `WorldLighting` state across ticks with boundary queuing |
| Light queue | `oxidized-game/src/lighting/queue.rs` | `LightUpdateQueue` and `LightUpdate` types |
| Light packet | `oxidized-protocol/src/packets/play/clientbound_light_update.rs` | `ClientboundLightUpdatePacket` (ID 0x30) |
| Sky light sources | `oxidized-world/src/chunk/sky_light_sources.rs` | `ChunkSkyLightSources` per-column heightmap for sky light seeding |
| Chunk accessors | `oxidized-world/src/chunk/level_chunk.rs` | `get/set_sky_light_at()`, `get/set_block_light_at()` |

### Integration hooks

| Hook | Location | Description |
|------|----------|-------------|
| Worldgen | `oxidized-game/src/worldgen/flat/generator.rs` | `initialize_sky_light()` then `initialize_block_light()` after template cloning |
| Block changes | `oxidized-server/src/network/play/block_interaction.rs` | `set_block()` queues `LightUpdate` via `has_different_light_properties()` (emission, opacity, shape occlusion) |
| Tick processing | `oxidized-server/src/tick.rs` | `process_light_updates()` at tick end — drains queue, runs BFS, propagates cross-chunk, broadcasts |
| Packet delivery | `oxidized-server/src/tick.rs` | `ClientboundLightUpdatePacket` broadcast for primary + neighbor chunks |
| World lighting | `oxidized-server/src/network/mod.rs` | `WorldContext.lighting: Mutex<WorldLighting>` — persistent engine + pending updates + boundary entries |

### Algorithm (ADR-017)

1. Block changes push `LightUpdate` to `WorldContext.lighting` (via `WorldLighting.queue_update()`)
2. At tick end, `process_light_updates()` drains the queue and groups updates by chunk
3. Per chunk: creates a `LightEngine`, pushes updates, calls `process_updates(&mut chunk)`
4. Inside `process_updates`: emission changes seed decrease/increase queues, opacity changes seed sky/block queues
5. Decrease passes run first to clear stale light (re-checking emitters at each visited position), then opacity-decrease re-seeding (including sky column re-seeding), then increase passes
6. Changed sections are serialized via `build_light_data_filtered()` (only changed sections + neighbors) and broadcast as `ClientboundLightUpdatePacket`

### Test coverage

- 69 unit tests across all lighting modules (propagation, sky, block_light, cross_chunk, parallel, engine, queue, occlusion, world_lighting)
- 2 roundtrip tests for `ClientboundLightUpdatePacket` (inline in packet module)
- 13 tests for `LightUpdateData` wire format and roundtrips (in `light_compliance.rs`)
- 29 unit tests for `DataLayer` lazy allocation (in `data_layer.rs`)
- 14 unit tests for `ChunkSkyLightSources` (in `sky_light_sources.rs`)
- All tests pass with the full workspace suite

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-017: Lighting Engine](../adr/adr-017-lighting.md) — batched BFS with parallel section processing
- [ADR-012: Block State](../adr/adr-012-block-state.md) — `BlockStateId::light_emission()`, `light_opacity()`
- [ADR-016: Worldgen Pipeline](../adr/adr-016-worldgen-pipeline.md) — Light status in worldgen pipeline
- [ADR-014: Chunk Storage](../adr/adr-014-chunk-storage.md) — section-level concurrency model


## Goal

Implement the full lighting engine described in ADR-017. The engine computes
sky light (top-down from the sun, attenuated by opacity) and block light
(outward from emitting blocks) using breadth-first search propagation. It
supports two modes: **full-chunk lighting** for newly generated chunks (worldgen)
and **incremental updates** when blocks change during gameplay. Light data is
stored per-section as 4-bit nibble arrays (`DataLayer`) and sent to clients via
the existing `LightUpdateData` serializer.

Phase 23 (flat worldgen) generates chunks without light. This phase adds correct
light calculation so chunks are lit when sent to clients. Phase 22 (block
interaction) changes blocks without triggering light updates. This phase hooks
the lighting engine into the block change pipeline so light propagates correctly.

---

## Existing Scaffolding (from R3.9)

The following types and modules were used:

| Type | Location | Status |
|------|----------|--------|
| `DataLayer` | `oxidized-world/src/chunk/data_layer.rs` | ✅ Complete (lazy nibble storage) |
| `LevelChunk` | `oxidized-world/src/chunk/level_chunk.rs` | ✅ Complete (sky/block light vecs + per-block accessors) |
| `LightUpdateQueue` | `oxidized-game/src/lighting/queue.rs` | ✅ Complete (pending update batch) |
| `LightUpdate` | `oxidized-game/src/lighting/queue.rs` | ✅ Complete (emission/opacity delta) |
| `LightEngine` | `oxidized-game/src/lighting/engine.rs` | ✅ Complete (BFS orchestrator) |
| `build_light_data()` | `oxidized-game/src/net/light_serializer.rs` | ✅ Complete (packet encoding; also `build_light_data_filtered()` for incremental) |
| `ClientboundLightUpdatePacket` | `oxidized-protocol/src/packets/play/` | ✅ New (incremental light updates) |

---

## Java Reference

| Concept | Java class |
|---------|-----------|
| Light engine base | `net.minecraft.world.level.lighting.LightEngine` |
| Sky light engine | `net.minecraft.world.level.lighting.SkyLightEngine` |
| Block light engine | `net.minecraft.world.level.lighting.BlockLightEngine` |
| Threaded engine | `net.minecraft.server.level.ThreadedLevelLightEngine` |
| Combined engine | `net.minecraft.world.level.lighting.LevelLightEngine` |
| Section storage | `net.minecraft.world.level.lighting.LayerLightSectionStorage` |
| Sky section storage | `net.minecraft.world.level.lighting.SkyLightSectionStorage` |
| Block section storage | `net.minecraft.world.level.lighting.BlockLightSectionStorage` |
| Nibble array | `net.minecraft.world.level.chunk.DataLayer` |
| Sky light sources | `net.minecraft.world.level.lighting.ChunkSkyLightSources` |
| Light event listener | `net.minecraft.world.level.lighting.LightEventListener` |
| Light update packet | `net.minecraft.network.protocol.game.ClientboundLightUpdatePacket` |
| Chunk data + light | `net.minecraft.network.protocol.game.ClientboundLevelChunkWithLightPacket` |

---

## Tasks

### 23a.1 — BFS propagation core (`oxidized-game/src/lighting/propagation.rs`) ✅

New module containing the core BFS algorithms shared by both sky and block light.
This is the heart of the lighting engine.

```rust
use std::collections::VecDeque;
use oxidized_world::chunk::LevelChunk;
use oxidized_world::registry::BlockStateId;

/// Six cardinal directions as (dx, dy, dz, Direction) offsets.
pub(crate) const DIRECTIONS: [(i32, i32, i32, Direction); 6] = [
    (1, 0, 0, Direction::East), (-1, 0, 0, Direction::West),
    (0, 1, 0, Direction::Up), (0, -1, 0, Direction::Down),
    (0, 0, 1, Direction::South), (0, 0, -1, Direction::North),
];

/// All 6 direction bits set.
pub(crate) const ALL_DIRECTIONS: u8 = 0x3F;

/// Entry in the BFS increase queue.
pub(crate) struct LightEntry {
    /// Chunk-local X (0–15).
    pub x: i32,
    /// World Y coordinate.
    pub y: i32,
    /// Chunk-local Z (0–15).
    pub z: i32,
    /// Light level to propagate from this position.
    pub level: u8,
    /// Bitmask of directions to propagate. 0x3F = all 6.
    pub directions: u8,
}

/// Entry in the BFS decrease queue.
pub(crate) struct DecreaseEntry {
    /// Chunk-local X (0–15).
    pub x: i32,
    /// World Y coordinate.
    pub y: i32,
    /// Chunk-local Z (0–15).
    pub z: i32,
    /// The light level that was at this position before removal.
    pub old_level: u8,
    /// Bitmask of directions to propagate.
    pub directions: u8,
}

/// A cross-boundary entry produced when BFS reaches the edge of a chunk.
pub struct BoundaryEntry {
    pub world_x: i32,
    pub world_y: i32,
    pub world_z: i32,
    pub level: u8,
    pub directions: u8,
}

/// BFS increase pass for block light within a single chunk.
///
/// For each entry, checks all directions in the entry's bitmask. If the
/// neighbor's current light level is less than `(entry level - effective_opacity)`,
/// where effective_opacity uses `get_light_block_into()` for face occlusion,
/// sets the neighbor to that value and enqueues it.
///
/// Returns boundary entries for positions that fall outside the chunk column.
pub(crate) fn propagate_block_light_increase(
    chunk: &mut LevelChunk,
    queue: &mut VecDeque<LightEntry>,
    chunk_base_x: i32,
    chunk_base_z: i32,
) -> Vec<BoundaryEntry> { /* BFS loop */ }

/// BFS decrease pass for block light.
///
/// Clears light from removed sources and re-seeds the increase queue
/// for neighbors that have independent sources. Re-checks emitter
/// emission at each BFS position.
pub(crate) fn propagate_block_light_decrease(
    chunk: &mut LevelChunk,
    decrease_queue: &mut VecDeque<DecreaseEntry>,
    increase_queue: &mut VecDeque<LightEntry>,
    chunk_base_x: i32,
    chunk_base_z: i32,
) -> Vec<BoundaryEntry> { /* BFS loop */ }

// Equivalent sky light variants:
// propagate_sky_light_increase(...) -> Vec<BoundaryEntry>
// propagate_sky_light_decrease(...) -> Vec<BoundaryEntry>
```

**Tests:**
- Unit: BFS increase propagates torch (emission=14) through air — verify light levels at distances 1–14
- Unit: BFS decrease removes light correctly when torch is broken
- Unit: Opacity blocks light — stone (opacity=15) stops propagation immediately
- Unit: Partial opacity — water (opacity=1) attenuates by 1 per block
- Property: For any random light source placement, re-running full BFS from scratch produces the same result as incremental increase
- Property: After decrease + increase, all light values are ≤ 15 and ≥ 0

---

### 23a.2 — Sky light initialization (`oxidized-game/src/lighting/sky.rs`) ✅

Top-down sky light calculation for a full chunk column.

```rust
use oxidized_world::chunk::LevelChunk;
use oxidized_world::chunk::heightmap::HeightmapType;

/// Initializes sky light for a newly generated chunk.
///
/// Phase 1: Vertical pass — for each (x, z) column, set sky light to 15
/// above the heightmap, then attenuate downward by each block's opacity.
/// Empty sections above all columns are bulk-filled with sky light 15
/// using lazy `DataLayer::filled(15)`.
///
/// Phase 2: Horizontal BFS — propagate sky light sideways through
/// transparent blocks below the heightmap (caves, overhangs).
///
/// Stores `ChunkSkyLightSources` in the chunk for future incremental
/// updates. Returns boundary entries for cross-chunk propagation.
pub fn initialize_sky_light(chunk: &mut LevelChunk) -> Vec<BoundaryEntry> {
    let min_y = chunk.min_y();
    let max_y = chunk.max_y();

    // Phase 1: Vertical pass — top-down per (x, z) column.
    let mut bfs_seeds = VecDeque::new();

    for x in 0..16i32 {
        for z in 0..16i32 {
            let surface_y = get_surface_y(chunk, x as usize, z as usize, min_y);
            let mut level: u8 = 15;

            for y in (min_y..max_y).rev() {
                if y >= surface_y {
                    chunk.set_sky_light_at(x, y, z, 15);
                } else {
                    let state_id = chunk.get_block_state(x & 15, y, z & 15).unwrap_or(0);
                    let opacity = BlockStateId(state_id as u16).light_opacity();
                    level = level.saturating_sub(opacity.max(1));
                    chunk.set_sky_light_at(x, y, z, level);
                    if level == 0 { break; }
                }
            }

            // Seed horizontal BFS from blocks below surface with sky light > 1.
            // ...
        }
    }

    // Phase 2: Horizontal BFS for sky light bleeding into caves.
    propagate_sky_light_increase(chunk, &mut bfs_seeds, 0, 0);
}
```

**Tests:**
- Unit: Flat world (4 layers) — sky light is 15 at y=-60 and above, 0 inside solid layers
- Unit: Single column hole through solid — sky light propagates down the shaft
- Unit: Overhang — sky light spreads horizontally under a 1-block overhang
- Property: Sky light is always ≤ 15 and monotonically non-increasing downward in opaque columns
- Compliance: Compare flat world sky light values against vanilla snapshot

---

### 23a.3 — Block light initialization (`oxidized-game/src/lighting/block_light.rs`) ✅

BFS from all light-emitting blocks in a chunk.

```rust
use std::collections::VecDeque;
use oxidized_world::chunk::LevelChunk;
use oxidized_world::registry::BlockStateId;

/// Initializes block light for a newly generated chunk.
///
/// Scans all sections for blocks with `light_emission > 0`, seeds the BFS
/// queue, and propagates outward. Uses the core BFS from `propagation.rs`
/// with face occlusion from `occlusion.rs`.
///
/// Returns boundary entries for cross-chunk propagation.
pub fn initialize_block_light(chunk: &mut LevelChunk) -> Vec<BoundaryEntry> {
    let min_y = chunk.min_y();
    let section_count = chunk.section_count();
    let mut queue = VecDeque::new();

    // Scan for emitters in all sections.
    for section_idx in 0..section_count {
        let section_base_y = min_y + (section_idx as i32 * 16);

        for local_y in 0..16u32 {
            for local_z in 0..16u32 {
                for local_x in 0..16u32 {
                    let state_id = chunk
                        .section(section_idx)
                        .and_then(|s| {
                            s.get_block_state(
                                local_x as usize, local_y as usize, local_z as usize,
                            ).ok()
                        })
                        .unwrap_or(0);

                    let emission = BlockStateId(state_id as u16).light_emission();
                    if emission > 0 {
                        let x = local_x as i32;
                        let y = section_base_y + local_y as i32;
                        let z = local_z as i32;

                        chunk.set_block_light_at(x, y, z, emission);
                        queue.push_back(LightEntry { x, y, z, level: emission });
                    }
                }
            }
        }
    }

    // BFS propagation from all emitters.
    if !queue.is_empty() {
        let _boundary = propagate_block_light_increase(chunk, &mut queue, 0, 0);
    }
}
```

**Tests:**
- Unit: Single torch (emission=14) in enclosed room — verify light levels at each distance
- Unit: Glowstone (emission=15) in open air — verify max range of 15 blocks
- Unit: Two torches — verify overlap takes the maximum
- Unit: Emitter behind glass (opacity=0) — light passes through
- Unit: Emitter behind stone (opacity=15) — light stops at stone face
- Property: Block light is always ≤ source emission and ≥ 0
- Property: For any emitter placement, light at distance d from nearest unblocked emitter equals max(0, emission - d)

---

### 23a.4 — Cross-chunk light propagation (`oxidized-game/src/lighting/cross_chunk.rs`) ✅

Handle light that crosses chunk boundaries (x=0/15 or z=0/15 in a section).

```rust
use oxidized_world::chunk::LevelChunk;
use super::propagation::BoundaryEntry;

/// Accessor for the 4 horizontal neighbors of a chunk.
pub struct ChunkNeighbors<'a> {
    pub north: Option<&'a mut LevelChunk>, // -Z
    pub south: Option<&'a mut LevelChunk>, // +Z
    pub east: Option<&'a mut LevelChunk>,  // +X
    pub west: Option<&'a mut LevelChunk>,  // -X
}

/// Propagates block light boundary entries into neighboring chunks.
///
/// For each boundary entry, determines which neighbor chunk owns that
/// position, converts to chunk-local coordinates, and runs BFS increase.
pub fn propagate_block_light_cross_chunk(
    neighbors: &mut ChunkNeighbors<'_>,
    boundary_entries: &[BoundaryEntry],
) { /* ... */ }

/// Propagates sky light boundary entries into neighboring chunks.
pub fn propagate_sky_light_cross_chunk(
    neighbors: &mut ChunkNeighbors<'_>,
    boundary_entries: &[BoundaryEntry],
) { /* ... */ }
```

**Tests:**
- Unit: Torch at (15, 64, 8) in chunk (0,0) — light propagates into chunk (1,0) at (0, 64, 8)
- Unit: Torch at chunk corner — light propagates into both diagonal neighbors via intermediate
- Unit: Missing neighbor chunk — propagation stops at boundary (no panic)
- Integration: Two adjacent generated chunks — cross-chunk light is seamless

---

### 23a.5 — Incremental light updates (`oxidized-game/src/lighting/engine.rs`) ✅

Replace the `todo!()` stubs in `LightEngine` with the real BFS implementation.

```rust
impl LightEngine {
    /// Processes all pending light updates for this tick on a single chunk.
    ///
    /// Algorithm (ADR-017):
    /// 1. Drain the queue.
    /// 2. For each update, seed decrease/increase queues for block and sky light.
    /// 3. Run decrease BFS passes first (clears stale light).
    /// 4. Handle opacity decreases (block broken) by re-seeding from neighbors.
    /// 5. Run increase BFS passes (propagates new light).
    /// 6. Return list of sections whose light data changed.
    pub fn process_updates(
        &mut self,
        chunk: &mut LevelChunk,
    ) -> Result<LightResult, LightingError> {
        let updates = self.queue.drain();
        if updates.is_empty() {
            return Ok(Vec::new());
        }

        let chunk_base_x = chunk.pos.x * 16;
        let chunk_base_z = chunk.pos.z * 16;
        let mut changed_sections: AHashMap<SectionPos, ()> = AHashMap::new();
        let mut block_decrease = VecDeque::new();
        let mut block_increase = VecDeque::new();
        let mut sky_decrease = VecDeque::new();
        let mut sky_increase = VecDeque::new();

        for update in &updates {
            let section_pos = SectionPos::of_block_pos(&update.pos);
            // Seed decrease/increase queues based on emission and opacity deltas.
            // ...
            changed_sections.insert(section_pos, ());
        }

        // Decrease passes first (per ADR-017), re-checking emitters at each position.
        let block_dec_boundary = propagate_block_light_decrease(chunk, &mut block_decrease, &mut block_increase, ...);
        let sky_dec_boundary = propagate_sky_light_decrease(chunk, &mut sky_decrease, &mut sky_increase, ...);

        // Handle opacity decreases by re-seeding from neighbor light sources
        // and sky column re-seeding (reseed_sky_column).
        // Then increase passes.
        let block_inc_boundary = propagate_block_light_increase(chunk, &mut block_increase, ...);
        let sky_inc_boundary = propagate_sky_light_increase(chunk, &mut sky_increase, ...);

        Ok(LightResult {
            changed_sections: changed_sections.into_keys().collect(),
            block_boundary: /* combined block boundaries */,
            sky_boundary: /* combined sky boundaries */,
        })
    }

    /// Computes full sky + block light for a newly generated chunk.
    /// Returns boundary entries for cross-chunk propagation.
    pub fn light_chunk(
        &mut self,
        chunk: &mut LevelChunk,
    ) -> Result<LightResult, LightingError> {
        let sky_boundary = initialize_sky_light(chunk);
        let block_boundary = initialize_block_light(chunk);
        Ok(LightResult {
            changed_sections: Vec::new(),
            block_boundary,
            sky_boundary,
        })
    }
}
```

**Tests:**
- Unit: Place torch → process_updates → verify light appears
- Unit: Break torch → process_updates → verify light removed
- Unit: Replace torch with stone → decrease + increase both run
- Unit: Place block increasing opacity → sky light recalculates below
- Unit: Break block decreasing opacity → sky light fills in
- Unit: Empty queue → process_updates returns empty vec
- Integration: Place torch, send light update packet, verify packet contents

---

### 23a.6 — Parallel section processing for worldgen (`oxidized-game/src/lighting/parallel.rs`) ✅

Sequential wrapper for full-chunk lighting used by the worldgen pipeline.
True rayon-based parallel even/odd section processing is deferred until
benchmarks justify the complexity.

```rust
use oxidized_world::chunk::LevelChunk;

/// Full-chunk lighting with parallel section processing.
///
/// Currently delegates to sequential sky + block light initialization.
/// Parallel even/odd section processing will be added when the worldgen
/// pipeline is fully operational and benchmarks justify the complexity.
///
/// Used by the worldgen pipeline at the Light status (ADR-016).
pub fn light_chunk_parallel(chunk: &mut LevelChunk) {
    // Phase 1: Sky light (vertical pass + horizontal BFS).
    initialize_sky_light(chunk);

    // Phase 2: Block light (emitter scan + BFS).
    initialize_block_light(chunk);
}
```

> **Note:** True rayon-based even/odd section parallelism (described in ADR-017) is
> deferred until the worldgen pipeline is fully operational and benchmarks justify
> the added complexity. The current implementation is a sequential wrapper.

**Tests:**
- Unit: Parallel (sequential wrapper) lighting produces identical results to calling sky + block init directly
- Deferred: Property test for byte-identical DataLayers (when true parallelism is added)
- Deferred: Benchmark for parallel vs sequential (when true parallelism is added)

---

### 23a.7 — Worldgen integration ✅

Hook `light_chunk()` / `light_chunk_parallel()` into the flat world generator
(Phase 23) so that newly generated chunks include correct light data before
being sent to clients.

```rust
// In the worldgen pipeline, after block placement and heightmap computation:
// 1. Call light_engine.light_chunk(&mut chunk)
// 2. Chunk is now ready for the Light status
// 3. Chunk proceeds to Full status and is eligible for sending
```

**Tests:**
- Integration: Generate flat chunk → verify sky light is 15 at surface + 1
- Integration: Generate flat chunk → verify block light is 0 (no emitters in default flat)
- Integration: Generate flat chunk with glowstone layer → verify block light propagation

---

### 23a.8 — Block change integration ✅

Hook `LightUpdateQueue` into the block placement/breaking pipeline (Phase 22)
so that every block change enqueues a `LightUpdate` and the engine processes
them at tick end.

```rust
// In the block change handler (placement.rs or similar):
fn on_block_change(pos: BlockPos, old_state: BlockStateId, new_state: BlockStateId) {
    let old_emission = old_state.light_emission();
    let new_emission = new_state.light_emission();
    let old_opacity = old_state.light_opacity();
    let new_opacity = new_state.light_opacity();

    if old_emission != new_emission || old_opacity != new_opacity {
        light_engine.queue_mut().push(LightUpdate {
            pos,
            old_emission,
            new_emission,
            old_opacity,
            new_opacity,
        });
    }
}

// At tick end (tick.rs):
fn end_of_tick() {
    let changed_sections = light_engine.process_updates(&mut chunk_map)?;
    for section in &changed_sections {
        send_light_update_packet(section);
    }
}
```

**Tests:**
- Integration: Place torch → tick → client receives light update packet with correct data
- Integration: Break glowstone → tick → verify light removed in update packet
- Integration: Place opaque block → tick → sky light below is recalculated
- Integration: Rapid place/break in same tick → batch processed correctly

---

### 23a.9 — Light update packets ✅

Send `ClientboundLightUpdatePacket` to watching clients when sections change
during gameplay. Full-chunk light is already included in
`ClientboundLevelChunkWithLightPacket` (Phase 13) — this task adds the
incremental update path.

```rust
// In tick.rs — process_light_updates() handles broadcasting inline:
fn process_light_updates(ctx: &ServerContext) {
    let updates = {
        let mut lighting = ctx.world.lighting.lock();
        if !lighting.has_pending_work() { return; }
        lighting.drain_updates()
    };

    // Group by chunk position.
    let mut by_chunk: AHashMap<ChunkPos, Vec<LightUpdate>> = AHashMap::new();
    for (chunk_pos, update) in updates {
        by_chunk.entry(chunk_pos).or_default().push(update);
    }

    for (chunk_pos, chunk_updates) in &by_chunk {
        let chunk_ref = ctx.world.chunks.get(chunk_pos);
        let mut chunk = chunk_ref.write();
        let result = {
            let mut lighting = ctx.world.lighting.lock();
            let engine = lighting.engine_mut();
            for update in chunk_updates {
                engine.queue_mut().push(update.clone());
            }
            engine.process_updates(&mut chunk)?
        };

        if !result.changed_sections.is_empty() {
            let light_data = build_light_data_filtered(
                chunk.sky_light_layers(),
                chunk.block_light_layers(),
                &result.changed_sections,
                chunk.min_y(),
            );
            drop(chunk);
            broadcast_light_update(ctx, chunk_pos.x, chunk_pos.z, light_data);
        }

        // Cross-chunk propagation.
        if !result.block_boundary.is_empty() || !result.sky_boundary.is_empty() {
            propagate_cross_chunk_light(ctx, chunk_pos, &result);
        }
    }
}
```

**Tests:**
- Unit: Changed section mask includes exactly the modified sections
- Unit: Filtered light data includes only changed sections + neighbors
- Unit: Empty changed sections produces no light data
- Integration: Full roundtrip — change block, process light, send packet, verify wire format
- Compliance: Packet bytes match vanilla capture for torch placement

---

## Vanilla Compliance Audit (2026-03-25)

Line-by-line comparison against vanilla MC 26.1 (protocol 775) decompiled Java source.
Full report in session state. Summary of findings and their status:

### Fixed

| ID | Finding | Fix |
|----|---------|-----|
| **M1** | Decrease pass didn't re-check emitter emission | Re-check emission at each BFS position; re-seed if > 0 |
| **M2** | Light packets broadcast all 26 sections | `build_light_data_filtered()` sends only changed sections |
| **C1** | Cross-chunk API broken (hardcoded offsets, wrong chunk_base) | Fixed `resolve_neighbor`, chunk_base coords, public API signatures |
| **C3** | Boundary entries pre-attenuated by 1 | Pass un-attenuated source level; cross-chunk reads target opacity |
| **C5** | No sky column re-seeding on block break | `reseed_sky_column` restores sky light downward through transparent blocks |

### Remaining (tasks 23a.10–23a.15 below) — All Fixed

| ID | Finding | Effort | Task |
|----|---------|--------|------|
| **C1b** | ~~Cross-chunk boundary entries discarded in production~~ | Medium | 23a.10 ✅ |
| **M3** | ~~Fresh `LightEngine` created per tick per chunk~~ | Medium | 23a.11 ✅ |
| **C2** | ~~Missing VoxelShape face occlusion~~ | Large | 23a.12 ✅ |
| **C4** | ~~Sky light missing `ChunkSkyLightSources` algorithm~~ | Large | 23a.13 ✅ |
| **M5** | ~~No directional propagation tracking (perf)~~ | Small | 23a.14 ✅ |
| **M4** | ~~Light trigger missing shape property check~~ | Small | 23a.15 ✅ |
| **m1** | ~~`DataLayer` always allocates (no lazy)~~ | Small | 23a.16 ✅ |
| **m3** | ~~Missing empty-section sky light optimization~~ | Small | 23a.16 ✅ |

---

### 23a.10 — Cross-chunk production integration ✅

Boundary entries from BFS are now wired into the production code path. The
`LightEngine.process_updates()` method returns `LightResult` with `block_boundary`
and `sky_boundary` vectors. The tick loop's `process_light_updates()` calls
`propagate_cross_chunk_light()` which looks up neighbor chunks from the chunk map,
constructs `ChunkNeighbors`, and calls `propagate_block_light_cross_chunk` /
`propagate_sky_light_cross_chunk`. Light updates are broadcast for modified
neighbor chunks as well. `WorldLighting` stores pending boundary entries for
multi-tick propagation via `queue_boundaries()` / `drain_boundaries()`.

**Approach:**
1. In `process_light_updates()` (tick.rs), after BFS on a chunk, collect
   non-empty boundary entry vectors
2. For each boundary entry set, look up the neighbor chunk from the chunk map
   (`WorldContext.chunks`)
3. Construct `ChunkNeighbors` and call `propagate_block_light_cross_chunk` /
   `propagate_sky_light_cross_chunk`
4. Track sections changed in neighbor chunks and broadcast light updates for
   those too
5. In `engine.rs`, return boundary entries from `process_updates()` instead of
   discarding them (change return type to include boundaries)
6. In `sky.rs` and `block_light.rs`, return boundary entries from init functions
   for use by worldgen pipeline

**Vanilla reference:**
- `LightEngine.java` works in world-space coordinates — light naturally flows
  across chunk boundaries via `chunkSource.getChunkForLighting()`
- No explicit boundary/neighbor system — vanilla accesses any chunk position
  directly

```rust
// Return type for process_updates:
pub struct LightResult {
    pub changed_sections: Vec<SectionPos>,
    pub block_boundary: Vec<BoundaryEntry>,
    pub sky_boundary: Vec<BoundaryEntry>,
}

// In tick.rs — process_light_updates():
let result = engine.process_updates(&mut chunk)?;
if !result.block_boundary.is_empty() || !result.sky_boundary.is_empty() {
    propagate_cross_chunk_light(ctx, chunk_pos, &result);
}

// propagate_cross_chunk_light() acquires locks on 4 neighbors:
let mut neighbors = ChunkNeighbors {
    north: chunks.get_mut(&ChunkPos::new(cx, cz - 1)),
    south: chunks.get_mut(&ChunkPos::new(cx, cz + 1)),
    east:  chunks.get_mut(&ChunkPos::new(cx + 1, cz)),
    west:  chunks.get_mut(&ChunkPos::new(cx - 1, cz)),
};
propagate_block_light_cross_chunk(&mut neighbors, &result.block_boundary, cx, cz);
propagate_sky_light_cross_chunk(&mut neighbors, &result.sky_boundary, cx, cz);
// Broadcasts light updates for each modified neighbor.
```

**Tests:**
- Integration: Torch at chunk edge (15, 64, 8) in chunk (0,0) — verify light
  appears at (0, 64, 8) in chunk (1,0) after tick processing
- Integration: Remove torch at chunk edge — verify light decreases in neighbor
- Integration: Missing neighbor chunk — no panic, boundary entries silently
  dropped
- Unit: `process_updates` returns non-empty boundary vectors when BFS reaches
  chunk edge

---

### 23a.11 — Persistent `LightEngine` per world ✅

`WorldLighting` struct wraps a persistent `LightEngine` + pending update queue +
pending boundary entries. Stored in `WorldContext.lighting: Mutex<WorldLighting>`.
Block changes queue directly into the persistent engine via `queue_update()`.
The tick loop calls `drain_updates()` and `engine_mut().process_updates()`.
Multi-tick propagation state is preserved — boundary entries from tick N are
queued via `queue_boundaries()` and processed in tick N+1 via `drain_boundaries()`.

```rust
// In WorldContext (network/mod.rs):
pub lighting: Mutex<WorldLighting>,

// WorldLighting (lighting/world_lighting.rs):
pub struct WorldLighting {
    engine: LightEngine,
    pending_updates: Vec<(ChunkPos, LightUpdate)>,
    pending_boundaries: AHashMap<ChunkPos, PendingBoundaries>,
}
```

**Tests:**
- Unit: Engine retains state across two consecutive `process_updates()` calls
- Unit: Boundary entries queued in tick N are available in tick N+1
- Unit: `drain_updates()` returns all queued and clears
- Unit: `queue_boundaries()` merges entries for the same chunk
- Integration: Torch at chunk edge triggers cross-chunk propagation across ticks

---

### 23a.12 — VoxelShape face occlusion (C2) ✅

Directional occlusion testing for non-full blocks (stairs, slabs, fences, walls).
Per-face occlusion bitmasks are computed at compile time in `build.rs` and stored
in the static `BLOCK_STATE_DATA` array. The BFS uses `get_light_block_into()`
(in `occlusion.rs`) instead of scalar opacity, which checks `shape_occludes()`
for blocks with `use_shape_for_light_occlusion() == true`.

**Implementation:**
1. **Compile-time codegen** (`build.rs`): `compute_occlusion_faces()` generates a
   `u8` bitmask (6 bits for 6 faces) per block state. Handles slabs, stairs, snow
   layers, pistons, dirt_path, farmland, enchanting_table, lectern, stonecutter,
   and other single-box blocks. 146 block types have shape occlusion enabled.
2. **Runtime methods** (`block.rs`): `BlockStateId::occlusion_face(face_index)`,
   `use_shape_for_light_occlusion()`, `is_empty_shape()`,
   `has_different_light_properties()`
3. **Occlusion module** (`occlusion.rs`): `shape_occludes(from, to, dir)` checks
   whether exit face of source OR entry face of target fully occludes.
   `get_light_block_into(from, to, dir)` returns 16 (fully blocked) if occluded,
   else scalar opacity. Fast path when both blocks have empty shapes.
4. **BFS integration** (`propagation.rs`): `get_effective_opacity()` calls
   `get_light_block_into()` for every neighbor check in the BFS loop.

```rust
/// Returns true if light is blocked from passing from `from_state` to
/// `to_state` in the given `direction`.
///
/// Checks whether the exit face of the source block OR the entry face
/// of the target block fully occludes the face.
pub fn shape_occludes(
    from: BlockStateId,
    to: BlockStateId,
    dir: Direction,
) -> bool {
    let from_face = !from.is_empty_shape() && from.occlusion_face(dir.to_3d_data_value());
    let to_face = !to.is_empty_shape() && to.occlusion_face(dir.opposite().to_3d_data_value());
    from_face || to_face
}

/// Returns the effective light opacity when propagating from `from` to `to`
/// in the given direction. Returns 16 (fully blocked) if shape-occluded.
pub fn get_light_block_into(
    from: BlockStateId,
    to: BlockStateId,
    dir: Direction,
) -> u8 {
    let simple_opacity = to.light_opacity();
    if from.is_empty_shape() && to.is_empty_shape() {
        return simple_opacity; // Fast path
    }
    if shape_occludes(from, to, dir) { 16 } else { simple_opacity }
}
```

**Tests (16):**
- Unit: Full block → full block — uses scalar opacity
- Unit: Air → air — light passes in all 6 directions
- Unit: Bottom slab → air from above — light blocked (DOWN face full)
- Unit: Bottom slab → air from below — light passes (UP face empty)
- Unit: Bottom slab → air from side — light passes
- Unit: Top slab → air from above — light passes (DOWN face empty)
- Unit: Top slab → air from below — light blocked (UP face full)
- Unit: Double slab — uses scalar opacity (shape flag cleared)
- Unit: Bottom stairs block down and back face
- Unit: Bottom stairs pass front face
- Unit: Top stairs block up
- Unit: Slab-on-slab vertical occlusion
- Unit: Air never shape-occludes with any block
- Deferred: Sub-block precision (16×16 grid) can be added later if needed

---

### 23a.13 — `ChunkSkyLightSources` heightmap system (C4) ✅

Implement vanilla's per-column sky light source tracking for accurate sky light
in complex terrain. Currently Oxidized uses the `MOTION_BLOCKING` heightmap
with a simple top-down scan, which is correct for flat worlds but diverges from
vanilla for overhangs, caves near chunk edges, and non-full blocks.

**Vanilla reference:**
- `ChunkSkyLightSources.java` — per-column heightmap tracking the lowest Y
  where sky light enters, computed per-face with occlusion testing
- `SkyLightEngine.java:302-358` (`propagateLightSources`) — BFS seeds only
  at positions where a column's source height is lower than its neighbors'
- `SkyLightEngine.java:49-73` (`checkNode`) — calls
  `updateSourcesInColumn(x, z, lowestSourceY)` on block changes

**Approach:**
1. **`ChunkSkyLightSources` struct** in `oxidized-world/src/chunk/sky_light_sources.rs`
   - Per-column (16×16) heightmap storing the lowest Y where sky light enters
   - `update(chunk, x, y, z)` — recalculates a single column when a block changes
   - `get_lowest_source_y(x, z) -> i32`
   - `from_chunk(chunk)` / `fill_from(chunk)` for initial population
2. **Column update on block change** — when a block changes, call
   `update(chunk, x, y, z)` which returns whether the source height changed
3. **Selective BFS seeding** — only seed increase BFS at positions where a
   column's source height differs from its vertical neighbors (reduces wasteful
   BFS work from m2)
4. **Store in `LevelChunk`** — `sky_light_sources: Option<ChunkSkyLightSources>`
   field (built during initial sky light pass via take/set pattern)

```rust
/// Per-column sky light source tracking.
///
/// For each (x, z) column in a chunk, tracks the lowest Y where sky light
/// at level 15 enters the column. This is similar to a heightmap but uses
/// face occlusion testing (via C2) rather than simple opacity.
pub struct ChunkSkyLightSources {
    /// Lowest source Y per column, indexed as [x + z * 16].
    lowest_y: [i32; 256],
    min_y: i32,
}

impl ChunkSkyLightSources {
    /// Builds sky light sources by scanning all columns in a chunk.
    pub fn from_chunk(chunk: &LevelChunk) -> Self { /* ... */ }

    /// Recalculates sky light sources for all columns.
    pub fn fill_from(&mut self, chunk: &LevelChunk) { /* ... */ }

    /// Called when a block changes — returns true if source height changed.
    pub fn update(
        &mut self,
        chunk: &LevelChunk,
        x: usize,
        y: i32,
        z: usize,
    ) -> bool { /* ... */ }

    /// Returns the lowest Y in this column that has sky light 15.
    pub fn get_lowest_source_y(&self, x: usize, z: usize) -> i32 {
        self.lowest_y[x * 16 + z]
    }
}
```

**Tests:**
- Unit: Flat world — lowest_source_y matches surface + 1 for all columns
- Unit: Single column hole — lowest_source_y is bottom of hole
- Unit: Break surface block — lowest_source_y updates downward
- Unit: Place block over hole — lowest_source_y moves up
- Property: For all columns, lowest_source_y ≥ min_y
- Compliance: Sky light values in complex terrain match vanilla snapshot

**Note:** Full accuracy depends on C2 (VoxelShape occlusion), which is now
implemented. The `is_edge_occluded()` helper in `sky_light_sources.rs` uses
`occlusion_face()` for face-level checks rather than simple opacity alone.

---

### 23a.14 — Direction bitmask propagation (M5) ✅

Add directional propagation tracking to BFS entries. Currently every entry
propagates in all 6 directions; vanilla tracks which direction light came from
and skips propagating backwards.

**Vanilla reference:**
- `LightEngine.java:228-324` (`QueueEntry`) — encodes a 6-bit direction
  bitmask per entry. Light entering from east propagates to all directions
  except east (back toward source).

**Approach:**
1. Add `directions: u8` bitmask field to `LightEntry` and `DecreaseEntry`
   (bits 0–5 for ±X, ±Y, ±Z)
2. When enqueuing a neighbor, set the bitmask to all directions except the
   incoming direction
3. In the BFS loop, only iterate over directions in the bitmask

```rust
pub(crate) struct LightEntry {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub level: u8,
    /// Bitmask of directions to propagate. 0x3F = all 6.
    pub directions: u8,
}
```

**Impact:** ~17% fewer BFS iterations. No correctness change. Low priority
performance optimization.

**Tests:**
- Unit: BFS with direction tracking produces identical light values as without
- Property: For any world, direction-tracked BFS == full-BFS light values
- Benchmark: Measure improvement on torch placement + TNT explosion scenarios

---

### 23a.15 — Shape property light trigger (M4) ✅

Add `use_shape_for_light_occlusion()` change detection to the block change handler.
Implemented via `has_different_light_properties()` on `BlockStateId`, which checks
emission, opacity, and shape occlusion flag. Called in `block_interaction.rs` to
decide whether to queue a `LightUpdate`.

**Approach:**
`BlockStateId::has_different_light_properties(other)` checks:
```rust
self.light_emission() != other.light_emission()
    || self.light_opacity() != other.light_opacity()
    || self.use_shape_for_light_occlusion()
    || other.use_shape_for_light_occlusion()
```

**Tests:**
- Unit: Replacing slab with full block triggers light update
- Unit: Replacing air with air doesn't trigger (both false)

---

### 23a.16 — Minor optimizations ✅

Small optimizations identified in the vanilla compliance audit.

**m1. `DataLayer` lazy allocation:**
Vanilla uses lazy allocation (null data until first write). ~~Oxidized allocates
2048 bytes immediately.~~ Implemented `Option<Box<[u8; 2048]>>` + `default_value`
with lazy init on first `set()`. `is_empty()` is now O(1). `new()` and `filled()`
allocate zero heap memory. Added `fill()` for de-allocation, plus
`is_definitely_homogeneous()` and `is_definitely_filled_with()` introspection.
Pre-computed static arrays for all 16 fill patterns support zero-copy `as_bytes()`.

**m3. Empty-section sky light fill:**
~~Vanilla's `SkyLightEngine.propagateFromEmptySections()` fills entire empty
sections with sky light 15 at once instead of attenuating per-block at
boundaries.~~ Implemented `bulk_fill_empty_sections()` in sky light init: sections
whose min Y ≥ highest source Y across all columns get `DataLayer::filled(15)` (no
allocation). The per-block vertical pass starts just below the bulk-filled range.

**Tests:**
- ✅ Unit: `DataLayer` — `is_empty()` returns true before any `set()` call
- ✅ Unit: `DataLayer` — `get()` returns 0 before any `set()` call
- ✅ Unit: Empty air section above surface gets uniform sky light 15
- ✅ Unit: Bulk-filled sections use lazy `DataLayer::filled(15)`
- ✅ Proptest: `fill(v)` then `get()` returns v without materializing

---

### 23a.17 — Neighbor-aware sky seeding in incremental updates ⏳

**Status:** Deferred to Phase 26 (noise worldgen)

**Vanilla behavior:** `SkyLightEngine.checkNode()` calls `updateSourcesInColumn()` →
`removeSourcesBelow()` + `addSourcesAbove()` for column-aware source management. It also
calls `propagateLightSources()` with `QueueEntry.increaseSkySourceInDirections()` using
selective per-direction flags based on neighbor source heights.

**Gap:** Oxidized's incremental sky light update seeds the changed column but does not
consider neighbor column heights when computing directional flags. This can cause
under-illumination near chunk column edges when blocks are placed/broken in valleys.

**Impact:** Correctness issue visible only with complex terrain (valleys, overhangs).
Flat worlds unaffected. Low priority until noise worldgen.

**Ref:** `SkyLightEngine.checkNode()` → `updateSourcesInColumn()`,
`propagateLightSources()` in `mc-server-ref/26.1/decompiled/net/minecraft/world/level/lighting/SkyLightEngine.java`

---

### 23a.18 — `propagateLightSources` for chunk load ⏳

**Status:** Deferred to Phase 26 (noise worldgen)

**Vanilla behavior:** When a chunk is first loaded, `LevelLightEngine` calls
`propagateLightSources()` on each section to seed the increase queue with all light
sources (both block emitters and sky sources). This ensures light from neighboring
already-loaded chunks flows into the new chunk.

**Gap:** Oxidized initializes sky and block light per-chunk during worldgen but does
not re-seed from neighbor chunks on chunk load. This means a chunk loaded after its
neighbors may have incorrect light at its edges if light from neighbors should flow in.

**Impact:** Only visible when chunks load out of order or when a chunk is unloaded and
reloaded. Flat worlds (all chunks generated together) are unaffected. Becomes important
with noise worldgen and dynamic chunk loading.

**Ref:** `LevelLightEngine.propagateLightSources()`,
`ServerChunkCache.onChunkReadyToSend()` in `mc-server-ref/26.1/`

---

### 23a.19 — `propagateFromEmptySections` BFS optimization ⏳

**Status:** Deferred to Phase 26 (noise worldgen)

**Vanilla behavior:** During BFS, `SkyLightEngine.propagateFromEmptySections()`
bulk-fills empty sections at chunk column edges. When BFS reaches an empty section
boundary, instead of propagating block-by-block, vanilla fills the entire column
of empty sections with sky light 15 in one operation. This dramatically reduces
BFS queue size for tall worlds with many air sections.

**Gap:** Oxidized's BFS iterates every block even in empty sections. This is
functionally correct but O(n) in the number of air blocks rather than O(1) per
section.

**Impact:** Performance only — no correctness impact. Becomes significant with
noise worldgen where chunks have many empty sections above terrain. Estimated
2-5× improvement for tall overworld chunks.

**Ref:** `SkyLightEngine.propagateFromEmptySections()` in
`mc-server-ref/26.1/decompiled/net/minecraft/world/level/lighting/SkyLightEngine.java`

---

### 23a.20 — Parallel lighting sub-phase ⏳

**Status:** Deferred — requires profiling data from Phase 26 to set priorities

Parallel lighting was deferred from the original phase to focus on correctness first.
This task covers the remaining parallelism work.

**Sub-tasks:**

1. **Parallel full-chunk lighting:** Use `rayon` to process independent chunk sections
   in parallel during worldgen. The sequential wrapper in `parallel.rs` already exists;
   replace it with actual parallel iteration. Target: < 200 µs per chunk (from ADR-017).

2. **Parallel cross-chunk propagation:** When multiple chunks have pending boundary
   entries, process non-adjacent chunks in parallel (chunks sharing no neighbors can
   be processed concurrently). Requires a chunk dependency graph or coloring scheme.

3. **Batch incremental updates:** When many blocks change in a single tick (e.g., TNT),
   batch all updates per chunk and run BFS once per chunk rather than per-block.
   The engine already batches by chunk; the parallel aspect is running multiple
   chunk BFS passes concurrently.

4. **Profile-guided tuning:** Use criterion benchmarks (`game_benchmarks.rs`) to
   measure actual parallelism gains and tune thread pool size / batch thresholds.

**Prerequisites:**
- Phase 26 (noise worldgen) for realistic chunk data to profile against
- ADR-017 performance targets validated with sequential implementation first
- Profiling to confirm lighting is actually the bottleneck (vs. network, worldgen)

---

## Performance Targets (from ADR-017)

| Scenario | Target | Status |
|----------|--------|--------|
| Full-chunk lighting (single-threaded) | < 1 ms | ✅ Implemented |
| Full-chunk lighting (parallel) | < 200 µs | ⏳ Deferred (sequential wrapper in place) |
| Incremental update (1–10 blocks) | < 50 µs | ✅ Implemented |
| Incremental update (TNT explosion, ~100 blocks) | < 1 ms | ✅ Implemented |

---

## Dependencies

- **Requires:** Phase 09 (chunk structures), Phase 13 (chunk sending), Phase 22 (block interaction), Phase 23 (flat worldgen)
- **Required by:** Phase 25 (hostile mobs — mob spawning depends on light levels), Phase 26 (noise worldgen — light status in pipeline)
- **Internal:** All internal dependencies satisfied. 23a.12 (VoxelShape occlusion) complete; 23a.13 uses face occlusion for accuracy.
- **Crate deps:** `rayon` (already in workspace for ADR-016)

---

## Completion Criteria

### Original (23a.1–23a.9)

1. ✅ All generated chunks (flat world) have correct sky and block light
2. ✅ Placing/breaking torches, glowstone, and other emitters updates light correctly
3. ✅ Placing/breaking opaque blocks recalculates sky light below
4. ✅ Light propagates across chunk boundaries
5. ✅ Clients receive light update packets for incremental changes
6. ⏳ Performance meets ADR-017 targets — sequential implementation complete; parallel optimization deferred
7. ✅ All tests pass: unit, integration, compliance (69 unit + 2 roundtrip + 13 compliance)
8. ✅ No `todo!()` stubs remain in `lighting/engine.rs`

### Vanilla Compliance (23a.10–23a.16)

9. ✅ Cross-chunk light propagation works in production — 23a.10
10. ✅ `LightEngine` persists across ticks via `WorldLighting` — 23a.11
11. ✅ Non-full blocks (slabs, stairs) correctly occlude light per-face — 23a.12
12. ✅ Sky light uses per-column source tracking for complex terrain — 23a.13
13. ✅ Direction bitmask reduces BFS work by ~17% — 23a.14
14. ✅ Shape property changes trigger light updates — 23a.15
15. ✅ `DataLayer` lazy allocation + empty-section sky fill — 23a.16

### Deferred (23a.17–23a.20)

16. ⏳ Neighbor-aware sky seeding in incremental updates — 23a.17 (Phase 26)
17. ⏳ `propagateLightSources` for chunk load — 23a.18 (Phase 26)
18. ⏳ `propagateFromEmptySections` BFS optimization — 23a.19 (Phase 26)
19. ⏳ Parallel lighting sub-phase — 23a.20 (post-Phase 26 profiling)
