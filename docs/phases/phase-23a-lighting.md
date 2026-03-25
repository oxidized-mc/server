# Phase 23a — Lighting Engine

**Status:** 📋 Planned  
**Crate:** `oxidized-game`, `oxidized-world`  
**Reward:** Chunks have correct sky and block light; placing/breaking light sources updates light in real time.

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

The following types and modules already exist and must be used:

| Type | Location | Status |
|------|----------|--------|
| `DataLayer` | `oxidized-world/src/chunk/data_layer.rs` | ✅ Complete (nibble storage) |
| `LevelChunk` | `oxidized-world/src/chunk/level_chunk.rs` | ✅ Complete (sky/block light vecs) |
| `LightUpdateQueue` | `oxidized-game/src/lighting/queue.rs` | ✅ Complete (pending update batch) |
| `LightUpdate` | `oxidized-game/src/lighting/queue.rs` | ✅ Complete (emission/opacity delta) |
| `LightEngine` | `oxidized-game/src/lighting/engine.rs` | 🚧 Stubs (`todo!()`) |
| `build_light_data()` | `oxidized-game/src/net/light_serializer.rs` | ✅ Complete (packet encoding) |

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

### 23a.1 — BFS propagation core (`oxidized-game/src/lighting/propagation.rs`)

New module containing the core BFS algorithms shared by both sky and block light.
This is the heart of the lighting engine.

```rust
use std::collections::VecDeque;
use oxidized_protocol::types::BlockPos;
use oxidized_world::chunk::DataLayer;
use oxidized_world::registry::BlockStateId;

/// Six cardinal directions for BFS neighbor iteration.
const DIRECTIONS: [(i32, i32, i32); 6] = [
    (1, 0, 0), (-1, 0, 0),
    (0, 1, 0), (0, -1, 0),
    (0, 0, 1), (0, 0, -1),
];

/// Entry in the BFS increase queue.
struct LightEntry {
    /// Packed section-local position (x, y, z each 0–15).
    x: u8,
    y: u8,
    z: u8,
    /// Light level to propagate from this position.
    level: u8,
}

/// Entry in the BFS decrease queue.
struct DecreaseEntry {
    x: u8,
    y: u8,
    z: u8,
    /// The light level that was removed.
    old_level: u8,
}

/// Increase pass: BFS from sources, propagating light outward.
///
/// For each entry, check all 6 neighbors. If the neighbor's current light
/// level < (entry level - max(1, neighbor opacity)), set neighbor to
/// (entry level - max(1, neighbor opacity)) and enqueue it.
///
/// Returns positions that crossed a section boundary (for cross-section
/// propagation).
fn propagate_increase(
    queue: &mut VecDeque<LightEntry>,
    get_light: impl Fn(u8, u8, u8) -> u8,
    set_light: impl FnMut(u8, u8, u8, u8),
    get_opacity: impl Fn(u8, u8, u8) -> u8,
) -> Vec<(i32, i32, i32, u8)> {
    // BFS loop: dequeue, check neighbors, propagate
    todo!()
}

/// Decrease pass: BFS from removed sources, clearing old light.
///
/// For each entry, check all 6 neighbors. If the neighbor's light came
/// from the removed source (level <= old_level), clear it and enqueue.
/// If the neighbor has a brighter source, enqueue it on the increase
/// queue for re-propagation.
fn propagate_decrease(
    decrease_queue: &mut VecDeque<DecreaseEntry>,
    increase_queue: &mut VecDeque<LightEntry>,
    get_light: impl Fn(u8, u8, u8) -> u8,
    set_light: impl FnMut(u8, u8, u8, u8),
    get_opacity: impl Fn(u8, u8, u8) -> u8,
) -> Vec<(i32, i32, i32, u8)> {
    // BFS loop: dequeue, clear light, re-seed increase queue for
    // neighbors that have independent sources
    todo!()
}
```

**Tests:**
- Unit: BFS increase propagates torch (emission=14) through air — verify light levels at distances 1–14
- Unit: BFS decrease removes light correctly when torch is broken
- Unit: Opacity blocks light — stone (opacity=15) stops propagation immediately
- Unit: Partial opacity — water (opacity=1) attenuates by 1 per block
- Property: For any random light source placement, re-running full BFS from scratch produces the same result as incremental increase
- Property: After decrease + increase, all light values are ≤ 15 and ≥ 0

---

### 23a.2 — Sky light initialization (`oxidized-game/src/lighting/sky.rs`)

Top-down sky light calculation for a full chunk column.

```rust
use oxidized_world::chunk::LevelChunk;

/// Initializes sky light for a newly generated chunk.
///
/// Phase 1: Vertical pass — for each (x, z) column, set sky light to 15
/// above the heightmap, then attenuate downward by each block's opacity.
///
/// Phase 2: Horizontal BFS — propagate sky light sideways through
/// transparent blocks below the heightmap (caves, overhangs).
pub fn initialize_sky_light(chunk: &mut LevelChunk) {
    let section_count = chunk.section_count();

    // Phase 1: Vertical propagation per column
    for x in 0..16u8 {
        for z in 0..16u8 {
            let surface_y = chunk.heightmap_motion_blocking(x, z);
            let mut level = 15u8;

            // Above heightmap: full brightness
            // Below heightmap: attenuate by opacity, stop at 0
            for y in (chunk.min_build_y()..=chunk.max_build_y()).rev() {
                let section_idx = chunk.section_index_for_y(y);
                let local_y = (y - chunk.min_build_y()) as u8 & 0xF;
                if y > surface_y {
                    chunk.set_sky_light_at(section_idx, x, local_y, z, 15);
                } else {
                    let opacity = chunk.block_state_at(x, y, z).light_opacity();
                    level = level.saturating_sub(opacity.max(1));
                    chunk.set_sky_light_at(section_idx, x, local_y, z, level);
                    if level == 0 { break; }
                }
            }
        }
    }

    // Phase 2: Horizontal BFS for sky light bleeding into caves
    propagate_sky_light_horizontal(chunk);
}
```

**Tests:**
- Unit: Flat world (4 layers) — sky light is 15 at y=-60 and above, 0 inside solid layers
- Unit: Single column hole through solid — sky light propagates down the shaft
- Unit: Overhang — sky light spreads horizontally under a 1-block overhang
- Property: Sky light is always ≤ 15 and monotonically non-increasing downward in opaque columns
- Compliance: Compare flat world sky light values against vanilla snapshot

---

### 23a.3 — Block light initialization (`oxidized-game/src/lighting/block_light.rs`)

BFS from all light-emitting blocks in a chunk.

```rust
use std::collections::VecDeque;
use oxidized_world::chunk::LevelChunk;

/// Initializes block light for a newly generated chunk.
///
/// Scans all sections for blocks with `light_emission > 0`, seeds the BFS
/// queue, and propagates outward. Uses the core BFS from `propagation.rs`.
pub fn initialize_block_light(chunk: &mut LevelChunk) {
    let mut queue = VecDeque::new();

    // Scan for emitters
    for section_y in 0..chunk.section_count() {
        for index in 0..4096u16 {
            let state = chunk.block_state_in_section(section_y, index);
            let emission = state.light_emission();
            if emission > 0 {
                let x = (index & 0xF) as u8;
                let y = ((index >> 8) & 0xF) as u8;
                let z = ((index >> 4) & 0xF) as u8;
                chunk.set_block_light_at(section_y, x, y, z, emission);
                queue.push_back(/* LightEntry for BFS */);
            }
        }
    }

    // BFS propagation from all emitters
    propagate_block_light(chunk, &mut queue);
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

### 23a.4 — Cross-chunk light propagation (`oxidized-game/src/lighting/cross_chunk.rs`)

Handle light that crosses chunk boundaries (x=0/15 or z=0/15 in a section).

```rust
use oxidized_world::chunk::LevelChunk;

/// Propagates light across chunk boundaries.
///
/// When BFS reaches x=0, x=15, z=0, or z=15 within a section, the light
/// must continue into the neighboring chunk's adjacent section. This
/// requires access to the neighbor chunk's light data.
///
/// Called after initial per-chunk lighting and after incremental updates
/// that affect boundary blocks.
pub fn propagate_cross_chunk(
    center: &mut LevelChunk,
    neighbors: &mut ChunkNeighbors,
    changed_boundaries: &[(SectionPos, Face)],
) {
    // For each boundary face that changed:
    // 1. Read light values at the boundary of the center chunk
    // 2. Compare with adjacent face of neighbor chunk
    // 3. If center's boundary light > neighbor's adjacent light + 1,
    //    seed BFS into neighbor
    // 4. Repeat until no more cross-boundary propagation is needed
    todo!()
}

/// Accessor for the 4 horizontal neighbors of a chunk.
pub struct ChunkNeighbors<'a> {
    pub north: Option<&'a mut LevelChunk>, // -Z
    pub south: Option<&'a mut LevelChunk>, // +Z
    pub east: Option<&'a mut LevelChunk>,  // +X
    pub west: Option<&'a mut LevelChunk>,  // -X
}
```

**Tests:**
- Unit: Torch at (15, 64, 8) in chunk (0,0) — light propagates into chunk (1,0) at (0, 64, 8)
- Unit: Torch at chunk corner — light propagates into both diagonal neighbors via intermediate
- Unit: Missing neighbor chunk — propagation stops at boundary (no panic)
- Integration: Two adjacent generated chunks — cross-chunk light is seamless

---

### 23a.5 — Incremental light updates (`oxidized-game/src/lighting/engine.rs`)

Replace the `todo!()` stubs in `LightEngine` with the real BFS implementation.

```rust
impl LightEngine {
    /// Processes all pending light updates for this tick.
    ///
    /// Algorithm (ADR-017):
    /// 1. Drain the queue.
    /// 2. Group updates by section.
    /// 3. For each section:
    ///    a. Decrease pass: BFS-remove old light from changed positions.
    ///    b. Increase pass: BFS-propagate new light from changed positions.
    /// 4. Collect cross-section boundary overflows and propagate.
    /// 5. Return list of sections whose light data changed.
    pub fn process_updates(
        &mut self,
        chunk_map: &mut ChunkMap,
    ) -> Result<Vec<SectionPos>, LightingError> {
        let updates = self.queue.drain();
        if updates.is_empty() {
            return Ok(Vec::new());
        }

        let mut changed_sections = Vec::new();
        let mut by_section = HashMap::new();

        for update in &updates {
            let section_pos = SectionPos::from_block_pos(update.pos);
            by_section.entry(section_pos).or_insert_with(Vec::new).push(update);
        }

        for (section_pos, section_updates) in &by_section {
            // Decrease pass for removed/reduced light
            // Increase pass for added/increased light
            // Track cross-section overflows
            changed_sections.push(*section_pos);
        }

        Ok(changed_sections)
    }

    /// Computes full sky + block light for a newly generated chunk.
    pub fn light_chunk(
        &mut self,
        chunk: &mut LevelChunk,
    ) -> Result<(), LightingError> {
        initialize_sky_light(chunk);
        initialize_block_light(chunk);
        Ok(())
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

### 23a.6 — Parallel section processing for worldgen (`oxidized-game/src/lighting/parallel.rs`)

Rayon-based parallel full-chunk lighting for worldgen throughput.

```rust
use rayon::prelude::*;
use oxidized_world::chunk::LevelChunk;

/// Full-chunk lighting with parallel section processing.
///
/// Per ADR-017, alternating even/odd Y-layers can be processed in parallel
/// since they don't share vertical boundaries.
///
/// Used by the worldgen pipeline at the Light status (ADR-016).
pub fn light_chunk_parallel(chunk: &mut LevelChunk) {
    // Phase 1: Sky light vertical pass (per-column, inherently parallel by column)
    initialize_sky_light_vertical(chunk);

    // Phase 2: Even sections — no shared boundaries
    // (0, 2, 4, ...).into_par_iter().for_each(|y| process_section(chunk, y))

    // Phase 3: Odd sections — propagate from even neighbors
    // (1, 3, 5, ...).into_par_iter().for_each(|y| process_section(chunk, y))

    // Phase 4: Boundary resolution
    resolve_section_boundaries(chunk);
}
```

**Tests:**
- Unit: Parallel lighting produces identical results to sequential
- Property: For any chunk content, parallel and sequential produce byte-identical DataLayers
- Benchmark: Parallel vs sequential for plains, cave, and mountain terrain (target: < 1 ms)

---

### 23a.7 — Worldgen integration

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

### 23a.8 — Block change integration

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

### 23a.9 — Light update packets

Send `ClientboundLightUpdatePacket` to watching clients when sections change
during gameplay. Full-chunk light is already included in
`ClientboundLevelChunkWithLightPacket` (Phase 13) — this task adds the
incremental update path.

```rust
/// Sends light update packets for sections that changed this tick.
///
/// Uses the existing `build_light_data()` serializer from
/// `oxidized-game/src/net/light_serializer.rs`, filtering to only the
/// changed sections' data.
fn send_light_updates(
    changed_sections: &[SectionPos],
    chunk_map: &ChunkMap,
    players: &PlayerList,
) {
    // Group by chunk column
    // For each chunk: build partial LightUpdateData with only changed sections
    // Send to all players watching that chunk
    todo!()
}
```

**Tests:**
- Unit: Changed section mask includes exactly the modified sections
- Integration: Full roundtrip — change block, process light, send packet, verify wire format
- Compliance: Packet bytes match vanilla capture for torch placement

---

## Performance Targets (from ADR-017)

| Scenario | Target |
|----------|--------|
| Full-chunk lighting (single-threaded) | < 1 ms |
| Full-chunk lighting (parallel) | < 200 µs |
| Incremental update (1–10 blocks) | < 50 µs |
| Incremental update (TNT explosion, ~100 blocks) | < 1 ms |

---

## Dependencies

- **Requires:** Phase 09 (chunk structures), Phase 13 (chunk sending), Phase 22 (block interaction), Phase 23 (flat worldgen)
- **Required by:** Phase 25 (hostile mobs — mob spawning depends on light levels), Phase 26 (noise worldgen — light status in pipeline)
- **Crate deps:** `rayon` (already in workspace for ADR-016)

---

## Completion Criteria

1. All generated chunks (flat world) have correct sky and block light
2. Placing/breaking torches, glowstone, and other emitters updates light correctly
3. Placing/breaking opaque blocks recalculates sky light below
4. Light propagates across chunk boundaries
5. Clients receive light update packets for incremental changes
6. Performance meets ADR-017 targets (benchmarked)
7. All tests pass: unit, integration, property-based, compliance
8. No `todo!()` stubs remain in `lighting/engine.rs`
