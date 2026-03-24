# Phase R3 — ADR Compliance & Code Quality Refactoring

**Status:** ✅ Done  
**Crates:** all  
**Reward:** Every crate fully complies with its governing ADRs. Lint violations are
zero. Naming, documentation, and architectural patterns match the documented
decisions. Foundational types for worldgen, lighting, and entity systems are
scaffolded with tests. The codebase is audit-clean and ready to scale to Phase 38.

---

## Progress Summary

| Sub-task | Description | Status |
|----------|-------------|--------|
| R3.1 | Lint & Error Handling Strictness (ADR-002, ADR-004) | ✅ Done |
| R3.2 | Tick Loop Threading Model (ADR-019) | ✅ Done |
| R3.3 | Remove Unused Dependencies (ADR-029, ADR-016) | ✅ Done |
| R3.4 | Environment Variable Configuration Overrides (ADR-005/033) | ✅ Done |
| R3.5 | File Size Violations (ADR-035) | ✅ Done |
| R3.6 | Boolean Naming Convention (Project Style Rules) | ✅ Done |
| R3.7 | Documentation Gaps (Project Style Rules) | ✅ Done |
| R3.8 | Worldgen Pipeline Structural Compliance (ADR-016) | ✅ Done |
| R3.9 | Lighting Engine Structural Compliance (ADR-017) | ✅ Done |
| R3.10 | Entity System Structural Compliance (ADR-018) | ✅ Done |

**All sub-tasks complete.** The codebase is audit-clean and ready for Phase 38.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-002: Error Handling Strategy](../adr/adr-002-error-handling.md) —
  `thiserror` in libraries, `anyhow` in binary only, no `unwrap()`/`expect()`
- [ADR-004: Logging & Observability](../adr/adr-004-logging-observability.md) —
  `tracing` only, no `println!`/`eprintln!`
- [ADR-005: Configuration Management](../adr/adr-005-configuration.md) —
  environment variable overrides
- [ADR-019: Server Tick Loop](../adr/adr-019-tick-loop.md) —
  dedicated OS thread
- [ADR-029: Memory Management](../adr/adr-029-memory-management.md) —
  bumpalo arena per tick
- [ADR-033: Configuration Format](../adr/adr-033-configuration-format.md) —
  `OXIDIZED_*` environment variable overrides
- [ADR-035: Module Structure](../adr/adr-035-module-structure.md) —
  800 LOC hard limit
- [ADR-016: World Generation Pipeline](../adr/adr-016-worldgen-pipeline.md) —
  Rayon thread pool, chunk status state machine, dependency-aware scheduler
- [ADR-017: Lighting Engine](../adr/adr-017-lighting.md) —
  Batched BFS, parallel section processing, NibbleArray storage
- [ADR-018: Entity System Architecture](../adr/adr-018-entity-system.md) —
  bevy_ecs components, systems, marker-based composition

---

## Goal

Bring the codebase into full compliance with all 39 accepted ADRs. This phase
fixes every deviation discovered during the comprehensive audit performed after
Phase R2. It is a **pure compliance refactoring** — no new features, no protocol
changes.

The audit found **10 categories** of ADR violations:

1. Lint & error handling strictness (ADR-002, ADR-004)
2. Tick loop architecture (ADR-019)
3. Unused declared dependencies (ADR-029, ADR-016)
4. Missing environment variable overrides (ADR-005/033)
5. File size violations (ADR-035)
6. Boolean naming convention violations (project style rules)
7. Missing documentation (project style rules)
8. Worldgen pipeline missing scaffolding (ADR-016)
9. Lighting engine missing scaffolding (ADR-017)
10. Entity system missing ECS foundation (ADR-018)

---

## Motivation

Phases R1 and R2 addressed structural and codec issues. However, a full audit
reveals that several ADRs are not reflected in the code. Some deviations are
minor (lint levels), but others are architectural (tick loop threading model).
Fixing these now prevents compounding drift as Phases 19–38 add ~100K LOC.

---

## Non-Goals

- **No new features** — zero user-visible behavior changes
- **No protocol changes** — packet wire format stays identical
- **No new ADRs** — this phase enforces existing decisions
- **No performance optimization** — structural compliance only
- **No reader/writer task split (ADR-006)** — this is a significant architecture
  change that should be its own phase; the current single-task model works
  correctly. Consider superseding ADR-006 or scheduling a dedicated network
  refactoring phase instead.
- **No full algorithm implementation (ADR-016/017/018)** — R3.8–R3.10 add
  foundational types, module structure, and compliance tests for worldgen,
  lighting, and entity systems. Full algorithm implementation (noise generation,
  BFS light propagation, ECS migration of existing entity logic) remains in
  their respective feature phases (P23/P26/P36, P13/P19, P15/P24/P25/P27).
- **No ECS migration of existing code (ADR-018)** — the current
  `Arc<RwLock<ServerPlayer>>` model is not replaced. R3.10 defines the ECS
  component types and bundles alongside the existing code so that the feature
  phase can incrementally migrate.

---

## Detailed Refactoring Plan

### R3.1: Lint & Error Handling Strictness (ADR-002, ADR-004)

**Targets:** `Cargo.toml` (workspace), `crates/oxidized-protocol/Cargo.toml`,
`crates/oxidized-server/src/app/console.rs`

**Current problems:**

| Problem | Location | ADR |
|---------|----------|-----|
| `unwrap_used` = "warn" (should be "deny") | `Cargo.toml` workspace lints | ADR-002 |
| `expect_used` = "warn" (should be "deny") | `Cargo.toml` workspace lints | ADR-002 |
| `panic` = "warn" (should be "deny") | `Cargo.toml` workspace lints | ADR-002 |
| `anyhow` dependency in library crate | `oxidized-protocol/Cargo.toml:26` | ADR-002 |
| `println!`/`eprintln!` not linted | `Cargo.toml` workspace lints | ADR-004 |
| `println!`/`eprintln!` used in console.rs | `oxidized-server/src/app/console.rs:119,129` | ADR-004 |
| `CommandError` missing `#[non_exhaustive]` | `oxidized-game/src/commands/mod.rs:96` | ADR-002 |
| `FlatConfigError` missing `#[non_exhaustive]` | `oxidized-game/src/worldgen/flat/config.rs:16` | ADR-002 |

**Steps:**

1. **Escalate clippy lint levels** in `Cargo.toml` workspace lints:
   ```toml
   unwrap_used = "deny"
   expect_used = "deny"
   panic       = "deny"
   print_stdout = "deny"
   print_stderr = "deny"
   ```

2. **Fix resulting compile errors:**
   - `console.rs:119`: Replace `println!("{component}")` with a dedicated console
     output function that uses `write!` to stdout (exempt from clippy lint via
     targeted `#[allow]` with justification comment)
   - `console.rs:129`: Replace `eprintln!("Error: {e}")` with `tracing::error!`
     or similar targeted `#[allow]`
   - Any `unwrap()`/`expect()` calls that surface after escalation to "deny"
     must be converted to `?` + `.context()` or `.map_err()`

3. **Remove `anyhow` from `oxidized-protocol/Cargo.toml`** (line 26). Verify no
   code imports it: `grep -rn "anyhow" crates/oxidized-protocol/src/`

4. **Add `#[non_exhaustive]` to remaining error enums:**
   - `CommandError` in `crates/oxidized-game/src/commands/mod.rs:96`
   - `FlatConfigError` in `crates/oxidized-game/src/worldgen/flat/config.rs:16`

**Verification:** `cargo clippy --workspace -- -D warnings` produces zero errors.

---

### R3.2: Tick Loop Threading Model (ADR-019)

**Target:** `crates/oxidized-server/src/main.rs`,
`crates/oxidized-server/src/tick.rs`

**Current problem:**
- ADR-019 mandates: "Tick loop must run on a dedicated OS thread"
- Actual: `tokio::spawn(tick::run_tick_loop(...))` (line 247 of main.rs)
- The tick loop is an `async fn` running on a Tokio worker thread

**Impact of current approach:**
- Tick loop competes with network I/O for Tokio worker threads
- Cannot use blocking operations inside tick processing
- ECS entity systems may migrate between threads non-deterministically

**Steps:**

1. **Convert `run_tick_loop` from `async fn` to blocking function:**
   - Create a `tokio::runtime::Handle` before spawning the tick thread so it
     can still call `.block_on()` for async save operations
   - Replace `tokio::time::interval` with `std::thread::sleep` or a
     crossbeam-based timer
   - Replace `tokio::time::Instant` with `std::time::Instant`

2. **Spawn on dedicated OS thread:**
   ```rust
   let tick_thread = std::thread::Builder::new()
       .name("tick".into())
       .spawn(move || {
           run_tick_loop(tick_ctx, tick_shutdown);
       })?;
   ```

3. **Bridge async operations from tick thread:**
   - Level.dat save: use `handle.block_on(spawn_blocking(...))` or direct
     synchronous file I/O (tick thread is allowed to block)
   - Chunk saves: submit to a dedicated I/O channel, processed by a Tokio task
   - Player data: submit to I/O channel

4. **Shutdown coordination:**
   - Tick thread signals completion via `AtomicBool` or oneshot channel
   - Main thread joins tick thread with timeout during shutdown

**Verification:** `cargo test --workspace` passes. Server starts and tick loop
runs at 20 TPS. Confirm with `std::thread::current().name()` logging that tick
runs on the "tick" thread.

---

### R3.3: Remove Unused Dependencies (ADR-029, ADR-016)

**Targets:** `Cargo.toml` (workspace), `crates/oxidized-world/Cargo.toml`

**Current problems:**
- `bumpalo = "3"` is declared at workspace level but **zero** usages exist in
  any crate source
- `rayon = "1.10"` is declared in oxidized-world but **zero** `use rayon` or
  `par_iter` calls exist

**Decision point:** These dependencies represent _future_ architectural
decisions (ADR-029 arena allocation, ADR-016 worldgen parallelism). Since the
code doesn't use them yet, they should be removed now to keep Cargo.lock clean
and re-added when their respective phases are implemented.

**Steps:**

1. **Remove `bumpalo` from workspace `[dependencies]`** and from any crate that
   declares it as a dependency
2. **Remove `rayon` from workspace `[dependencies]`** and from oxidized-world's
   `Cargo.toml`
3. Run `cargo check --workspace` to confirm no breakage

**Verification:** `cargo deny check` passes. `cargo tree` shows no unused
dependencies.

**Note:** `bumpalo` should be re-added when Phase 38 (Performance) is
implemented. `rayon` is re-added in R3.8 below, where the worldgen scheduler
scaffolding provides actual usage.

---

### R3.4: Environment Variable Configuration Overrides (ADR-005/033)

**Target:** `crates/oxidized-server/src/config/mod.rs`

**Current problem:**
- ADR-005 mandates precedence: CLI flags > environment variables > file > defaults
- ADR-033 specifies format: `OXIDIZED_NETWORK_PORT=25566` (section + field,
  uppercase, underscores)
- **Not implemented**: No code reads `OXIDIZED_*` environment variables

**Steps:**

1. **Add `apply_env_overrides(&mut self)` method to `ServerConfig`:**
   ```rust
   pub fn apply_env_overrides(&mut self) {
       if let Ok(val) = std::env::var("OXIDIZED_NETWORK_PORT") {
           if let Ok(port) = val.parse::<u16>() {
               self.network.port = port;
           }
       }
       // ... repeat for all config fields
   }
   ```

2. **Better approach — derive-based reflection:**
   - Use a macro or loop over known field paths to reduce boilerplate
   - Pattern: `OXIDIZED_{SECTION}_{FIELD}` → `config.{section}.{field}`
   - Support types: `u16`, `u32`, `i32`, `i64`, `bool`, `String`
   - Log each override at `info!` level: `"Config override: network.port = 25566
     (from OXIDIZED_NETWORK_PORT)"`

3. **Call in the loading sequence** (in `main.rs`):
   ```rust
   let mut config = ServerConfig::load_or_create(&args.config)?;
   config.apply_env_overrides();  // ← after file load, before CLI
   if let Some(port) = args.port { config.network.port = port; }
   config.validate()?;
   ```

4. **Add unit tests** for env var parsing (use `std::env::set_var` in test,
   with `serial_test` if needed for env var isolation)

5. **Document in oxidized.toml header comment:**
   ```toml
   # Environment variables override file values.
   # Format: OXIDIZED_{SECTION}_{FIELD}=value
   # Example: OXIDIZED_NETWORK_PORT=25566
   ```

**Verification:** `cargo test -p oxidized-server` — new env override tests pass.
Manual test: `OXIDIZED_NETWORK_PORT=25566 cargo run` binds to port 25566.

---

### R3.5: File Size Violations (ADR-035)

**Targets:**
- `crates/oxidized-server/src/network/play/mod.rs` (1,369 LOC)
- `crates/oxidized-server/src/network/play/block_interaction.rs` (1,366 LOC)

**Current problems per ADR-035:**
- Files > 800 LOC (excluding tests) must be split
- `play/mod.rs` has the main play loop + keepalive + tick handling + all packet
  dispatch — at least 3 responsibilities
- `block_interaction.rs` has mining + placing + sign editing + creative slot +
  item use — at least 5 responsibilities

**Steps:**

#### 5a: Split `play/mod.rs` (1,369 LOC → ~4 files, ~300 LOC avg)

1. **Extract `play/keepalive.rs`:**
   - Keepalive timer logic, keepalive send/receive
   - `handle_keepalive()` function

2. **Extract `play/entity_tracking.rs`:**
   - Entity spawn/despawn tracking
   - Entity metadata broadcast
   - Player position broadcast to other players

3. **Extract `play/tick_integration.rs`:**
   - Tick rate packet handling (ticking_state, ticking_step)
   - Time synchronization
   - Any tick-related event processing

4. **Keep in `play/mod.rs`:**
   - `PlayContext` struct definition
   - `handle_play()` main loop with `select!`
   - Packet dispatch `match` (compact routing table)
   - Re-exports from submodules

#### 5b: Split `block_interaction.rs` (1,366 LOC → ~4 files, ~300 LOC avg)

1. **Extract `play/mining.rs`:**
   - `handle_player_action()` for block digging (start/abort/finish)
   - Mining speed calculation
   - Tool effectiveness checks
   - Drop item logic

2. **Extract `play/placement.rs`:**
   - `handle_use_item_on()` for block placement
   - Block placement validation (reach, collision, face direction)
   - Block state calculation for placed blocks

3. **Extract `play/sign_editing.rs`:**
   - `handle_sign_update()` — sign text editing
   - Sign validation (length, forbidden characters)

4. **Extract `play/creative.rs`:**
   - `handle_set_creative_mode_slot()` — creative inventory
   - Creative-mode-only validation

5. **Keep in `block_interaction.rs`:**
   - Shared constants (reach distances, mining durations)
   - Re-exports from submodules
   - Any shared helper functions

**Verification:** `cargo test --workspace` — all existing tests pass unchanged.
No file exceeds 800 LOC (excluding tests). `wc -l` confirms.

---

### R3.6: Boolean Naming Convention (Project Style Rules)

**Target:** ~97 public boolean fields across all crates

**Current problem:**
- Project rules mandate `is_`/`has_`/`can_` prefix for boolean fields
- 97 public boolean fields violate this rule (identified in audit)

**Scope:** This is the largest change by volume but mechanically simple. Each
rename requires updating all references (field access, construction, pattern
matching, serde rename attributes).

**Approach:**

Split into per-crate batches. Each batch:
1. Rename fields with `is_`/`has_`/`can_` prefix
2. Add `#[serde(rename = "original_name")]` where the field is
   serialized/deserialized (NBT, TOML, JSON) to preserve wire compatibility
3. Update all call sites
4. Run `cargo test -p <crate>` after each batch

**Batches:**

| Batch | Crate | Fields | Priority |
|-------|-------|--------|----------|
| 6a | `oxidized-protocol` | ~21 fields (packets: on_ground, horizontal_collision, hardcore, etc.) | High |
| 6b | `oxidized-game` | ~28 fields (entity, player, abilities, tick_rate, etc.) | High |
| 6c | `oxidized-world` | ~16 fields (primary_level_data: hardcore, allow_commands, etc.) | Medium |
| 6d | `oxidized-server` | ~32 fields (config: hardcore, force_gamemode, etc.) | Medium |

**Critical serde considerations:**
- **Protocol packets** (network wire format): Field names don't affect binary
  encoding — safe to rename
- **NBT persistence** (level.dat, player.dat): Must add `#[serde(rename = "...")
  ]` to preserve vanilla compatibility
- **TOML config** (oxidized.toml): Must add `#[serde(rename = "...")]` to
  preserve user config files, OR update the TOML field names too (breaking
  config change)

**Decision:** For TOML config fields, rename the TOML keys too (e.g.,
`hardcore` → `is_hardcore`) since Oxidized is pre-1.0 and config format changes
are acceptable. Add a note in CHANGELOG.

For NBT/protocol: add `#[serde(rename = "...")]` to preserve wire compatibility.

**Verification:** `cargo test --workspace` — all tests pass. `grep -r` confirms
no old field names remain (except in serde rename attributes).

---

### R3.7: Documentation Gaps (Project Style Rules)

**Target:** ~50+ public functions missing `# Errors` docs, ~100+ public items
missing `///` docs

**This is a long-tail task.** Rather than listing every missing doc, establish
the CI gate and systematically fix during development.

**Steps:**

1. **Verify `missing_docs = "warn"` is working:**
   - Run `cargo check --workspace 2>&1 | grep "missing_docs" | wc -l`
   - This gives the current count of undocumented public items

2. **Add `# Errors` sections to Result-returning public functions:**
   - Priority: `oxidized-nbt` I/O functions, `oxidized-protocol` codec functions,
     `oxidized-game` command functions
   - Pattern:
     ```rust
     /// # Errors
     ///
     /// Returns [`CommandError::InvalidSyntax`] if the input cannot be parsed.
     /// Returns [`CommandError::PermissionDenied`] if the source lacks permission.
     ```

3. **Add `///` doc comments to undocumented public items:**
   - Focus on types and functions that are part of cross-crate APIs
   - Module-level `//!` docs on `mod.rs` files that lack them
   - Struct/enum level docs for all public types

4. **Consider escalating `missing_docs` to "deny" for new code:**
   - Add comment in `Cargo.toml`: `# TODO(phase-r3): Escalate to "deny" once
     existing gaps are filled`

**Verification:** `cargo doc --workspace --no-deps` builds without warnings.
Count of `missing_docs` warnings decreases from current baseline to zero.

---

### R3.8: Worldgen Pipeline Structural Compliance (ADR-016)

**Targets:** `crates/oxidized-game/src/worldgen/`,
`crates/oxidized-world/Cargo.toml`, workspace `Cargo.toml`

**Current state:**

| Existing | Location | Status |
|----------|----------|--------|
| `ChunkStatus` enum (12 statuses) | `oxidized-game/src/worldgen/mod.rs` | ✅ Present |
| `ChunkGenerator` trait (Send + Sync) | `oxidized-game/src/worldgen/mod.rs` | ✅ Present |
| Flat world generator | `oxidized-game/src/worldgen/flat/` | ✅ Present |
| `WorldgenScheduler` | — | ❌ Missing |
| `ChunkGenPriority` / priority system | — | ❌ Missing |
| `StatusRequirement` / neighbor deps | — | ❌ Missing |
| `CancellationToken` integration | — | ❌ Missing |
| `rayon` dependency | — | ❌ Removed in R3.3 |

**What R3.8 adds (scaffolding only — no algorithm implementation):**

1. **Re-add `rayon` to workspace dependencies:**
   - Was removed in R3.3 because nothing used it. Now justified by the
     scheduler skeleton below.

2. **Create `worldgen/scheduler.rs` — scheduler types:**
   ```rust
   pub struct WorldgenScheduler {
       pending: DashMap<ChunkPos, ChunkGenTask>,
       in_progress: DashMap<ChunkPos, ChunkStatus>,
       rayon_pool: rayon::ThreadPool,
       max_concurrent: usize,
       semaphore: Arc<Semaphore>,
   }

   pub struct ChunkGenTask {
       pub target_status: ChunkStatus,
       pub current_status: ChunkStatus,
       pub priority: ChunkGenPriority,
       pub cancel_token: CancellationToken,
   }
   ```

3. **Create `worldgen/priority.rs` — priority enum:**
   ```rust
   #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
   pub enum ChunkGenPriority {
       Low = 0,
       Normal = 1,
       High = 2,
       Urgent = 3,
   }
   ```

4. **Create `worldgen/status_requirements.rs` — neighbor requirement table:**
   ```rust
   pub struct StatusRequirement {
       pub radius: u8,
       pub min_neighbor_status: ChunkStatus,
   }
   ```
   Plus a `const fn requirements(status: ChunkStatus) -> StatusRequirement`
   function encoding the vanilla neighbor table from ADR-016.

5. **Add unit tests:**
   - `ChunkStatus` ordering matches vanilla pipeline
   - `ChunkGenPriority` ordering (Urgent > High > Normal > Low)
   - `StatusRequirement` table is consistent (no status requires neighbors at
     a higher status than itself)
   - Scheduler `dependencies_satisfied()` logic with mock chunk statuses

6. **Add property tests:**
   - `ChunkStatus::is_or_after()` is a total order
   - Priority sort is stable and deterministic

**What remains for feature phases (P23/P26/P36):**
- Noise density function evaluation (Perlin/Simplex)
- SIMD-optimized noise pipeline
- Biome placement
- Structure generation (villages, temples, strongholds)
- Feature decoration (trees, ores, vegetation)
- Cave carving
- Full scheduler dispatch loop with real chunk generation
- Cancellation integration with running tasks

**Verification:** `cargo test -p oxidized-game` — new scheduler/priority/status
tests pass. `cargo check --workspace` — rayon compiles without warnings.

---

### R3.9: Lighting Engine Structural Compliance (ADR-017)

**Targets:** `crates/oxidized-game/src/lighting/` (new module),
`crates/oxidized-world/src/chunk/data_layer.rs`

**Current state:**

| Existing | Location | Status |
|----------|----------|--------|
| `DataLayer` (NibbleArray equivalent) | `oxidized-world/src/chunk/data_layer.rs` | ✅ Present |
| `sky_light` / `block_light` in `LevelChunk` | `oxidized-world/src/chunk/level_chunk.rs` | ✅ Present |
| Light serializer for packets | `oxidized-game/src/net/light_serializer.rs` | ✅ Present |
| `LightEngine` struct | — | ❌ Missing |
| BFS propagation logic | — | ❌ Missing |
| `LightUpdateQueue` / batched processing | — | ❌ Missing |
| Sky light initialization (heightmap) | — | ❌ Missing |
| Block light propagation | — | ❌ Missing |
| Incremental update system | — | ❌ Missing |

**What R3.9 adds (scaffolding only — no BFS implementation):**

1. **Create `oxidized-game/src/lighting/mod.rs` — module skeleton:**
   ```rust
   //! Lighting engine for sky light and block light propagation.
   //!
   //! Implements batched BFS with parallel section processing per ADR-017.

   pub mod engine;
   pub mod queue;
   ```

2. **Create `lighting/queue.rs` — update queue types:**
   ```rust
   pub struct LightUpdateQueue {
       pending: Vec<LightUpdate>,
   }

   pub struct LightUpdate {
       pub pos: BlockPos,
       pub old_emission: u8,
       pub new_emission: u8,
       pub old_opacity: u8,
       pub new_opacity: u8,
   }
   ```

3. **Create `lighting/engine.rs` — engine skeleton:**
   ```rust
   pub struct LightEngine {
       queue: LightUpdateQueue,
   }

   impl LightEngine {
       /// Process all pending light updates for this tick.
       ///
       /// Groups updates by section, processes each section, and propagates
       /// cross-section changes. See ADR-017 for the batched BFS algorithm.
       ///
       /// # Errors
       ///
       /// Returns `LightingError` if a referenced chunk section is unavailable.
       pub fn process_updates(&mut self, chunk_map: &ChunkMap)
           -> Result<Vec<SectionPos>, LightingError> {
           todo!("ADR-017: BFS propagation — implemented in Phase P13")
       }

       /// Compute full sky + block light for a newly generated chunk.
       ///
       /// Called by the worldgen pipeline at the Light status (ADR-016).
       pub fn light_chunk(&mut self, chunk: &LevelChunk)
           -> Result<(), LightingError> {
           todo!("ADR-017: Full chunk lighting — implemented in Phase P13")
       }
   }
   ```

4. **Harden `DataLayer` with property tests (ADR-034 compliance):**
   - `proptest`: `get(x,y,z)` returns the value previously `set(x,y,z)` for
     all valid coordinates (x,z ∈ 0..16, y ∈ 0..16)
   - `proptest`: adjacent nibbles are never corrupted by a `set()` call
   - `proptest`: `from_bytes(layer.as_bytes())` roundtrips perfectly
   - Snapshot tests for `DataLayer::filled(15)` byte pattern

5. **Add light packet compliance tests:**
   - Serialize `LightUpdateData` and verify wire format matches vanilla
   - Verify BitSet mask encoding for various section patterns
   - Roundtrip test: build → serialize → deserialize → compare

**What remains for feature phases (P13/P19):**
- BFS sky light initialization from heightmap
- BFS block light propagation from emitters
- Incremental update processing (block place/break → light recalc)
- Cross-chunk boundary propagation
- Parallel section processing (even/odd Y-layer passes)
- Integration with block state opacity values
- Performance: < 1 ms per chunk full lighting, < 50 µs incremental

**Verification:** `cargo test -p oxidized-game` — DataLayer property tests pass.
`cargo test -p oxidized-game` — light packet compliance tests pass.
`cargo check --workspace` — new lighting module compiles.

---

### R3.10: Entity System Structural Compliance (ADR-018)

**Targets:** `crates/oxidized-game/src/entity/`,
`crates/oxidized-game/Cargo.toml`

**Current state:**

| Existing | Location | Status |
|----------|----------|--------|
| `bevy_ecs = "0.18"` dependency | Workspace + oxidized-game | ✅ Present |
| Monolithic `Entity` struct | `oxidized-game/src/entity/mod.rs` | ✅ Present (not ECS) |
| `SynchedEntityData` dirty tracking | `oxidized-game/src/entity/synched_data.rs` | ✅ Present |
| `EntityTracker` visibility | `oxidized-game/src/entity/tracker.rs` | ✅ Present |
| Entity ID allocator | `oxidized-game/src/entity/id.rs` | ✅ Present |
| ECS `Component` types | — | ❌ Missing |
| ECS `System` functions | — | ❌ Missing |
| Marker components | — | ❌ Missing |
| Spawn template bundles | — | ❌ Missing |
| System scheduling phases | — | ❌ Missing |

**What R3.10 adds (type definitions only — no migration of existing code):**

1. **Create `entity/components.rs` — core ECS component types from ADR-018:**
   ```rust
   use bevy_ecs::prelude::*;

   // --- Entity base (vanilla Entity.java fields) ---
   #[derive(Component)]
   pub struct Position(pub DVec3);

   #[derive(Component)]
   pub struct Velocity(pub DVec3);

   #[derive(Component)]
   pub struct Rotation { pub yaw: f32, pub pitch: f32 }

   #[derive(Component)]
   pub struct OnGround(pub bool);

   #[derive(Component)]
   pub struct FallDistance(pub f32);

   #[derive(Component)]
   pub struct EntityFlags(pub u8);

   #[derive(Component)]
   pub struct NoGravity;

   #[derive(Component)]
   pub struct Silent;

   // --- LivingEntity fields ---
   #[derive(Component)]
   pub struct Health { pub current: f32, pub max: f32 }

   #[derive(Component)]
   pub struct Equipment(pub EquipmentSlots);

   #[derive(Component)]
   pub struct ArmorValue(pub f32);

   #[derive(Component)]
   pub struct AbsorptionAmount(pub f32);

   // --- Player-specific ---
   #[derive(Component)]
   pub struct PlayerMarker;

   #[derive(Component)]
   pub struct SelectedSlot(pub u8);

   #[derive(Component)]
   pub struct ExperienceData {
       pub level: i32,
       pub progress: f32,
       pub total: i32,
   }
   ```

2. **Create `entity/markers.rs` — entity-type marker components:**
   ```rust
   use bevy_ecs::prelude::*;

   #[derive(Component)] pub struct ZombieMarker;
   #[derive(Component)] pub struct SkeletonMarker;
   #[derive(Component)] pub struct CreeperMarker;
   #[derive(Component)] pub struct SpiderMarker;
   #[derive(Component)] pub struct VillagerMarker;
   #[derive(Component)] pub struct ChickenMarker;
   #[derive(Component)] pub struct CowMarker;
   #[derive(Component)] pub struct PigMarker;
   #[derive(Component)] pub struct SheepMarker;
   // ... one marker per vanilla entity type
   ```

3. **Create `entity/phases.rs` — tick phase enum (ADR-018 §System Scheduling):**
   ```rust
   #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
   pub enum TickPhase {
       PreTick,
       Physics,
       Ai,
       EntityBehavior,
       StatusEffects,
       PostTick,
       NetworkSync,
   }
   ```

4. **Create `entity/bundles.rs` — spawn template bundles:**
   ```rust
   use bevy_ecs::prelude::*;

   #[derive(Bundle)]
   pub struct BaseEntityBundle {
       pub position: Position,
       pub velocity: Velocity,
       pub rotation: Rotation,
       pub on_ground: OnGround,
       pub fall_distance: FallDistance,
       pub flags: EntityFlags,
   }

   #[derive(Bundle)]
   pub struct LivingEntityBundle {
       pub base: BaseEntityBundle,
       pub health: Health,
       pub armor: ArmorValue,
       pub absorption: AbsorptionAmount,
   }

   #[derive(Bundle)]
   pub struct ZombieBundle {
       pub living: LivingEntityBundle,
       pub marker: ZombieMarker,
   }
   ```

5. **Add unit tests:**
   - Component types can be inserted into and queried from a bevy_ecs `World`
   - Marker queries return correct entity subsets
   - Bundle spawning creates entities with all expected components
   - `TickPhase` ordering matches ADR-018 phase order

6. **Add vanilla mapping stub:**
   - Create `docs/entity-mapping.md` skeleton with the table headers from
     ADR-018 (vanilla class → Oxidized components). Populated incrementally
     during feature phases.

**What remains for feature phases (P15/P24/P25/P27):**
- Migrate `Entity` struct fields to ECS components (incremental, per-system)
- Implement ECS systems for physics, AI, status effects, network sync
- Replace `Arc<RwLock<ServerPlayer>>` with ECS player entity
- Implement `SynchedEntityData` as ECS change detection
- Automatic parallel system scheduling via bevy_ecs
- Full vanilla behavior parity tests per entity type

**Verification:** `cargo test -p oxidized-game` — component/bundle/phase tests
pass. `cargo check --workspace` — bevy_ecs components compile. No existing
tests broken (monolithic Entity struct is unchanged).

---

## Acceptance Criteria

- [x] `unwrap_used`, `expect_used`, `panic` clippy lints are "deny" in workspace
- [x] `print_stdout`, `print_stderr` clippy lints are "deny" in workspace
- [x] `anyhow` removed from `oxidized-protocol` dependencies
- [x] All error enums have `#[non_exhaustive]`
- [x] Tick loop runs on dedicated OS thread (not Tokio task)
- [x] `bumpalo` removed from workspace (re-add when used)
- [x] `OXIDIZED_*` environment variable overrides work for all config fields
- [x] No file exceeds 800 LOC (excluding tests) per ADR-035
- [x] `play/mod.rs` split into ≥3 submodules
- [x] `block_interaction.rs` split into ≥3 submodules
- [x] Boolean fields use `is_`/`has_`/`can_` prefix (with serde renames)
- [x] Zero `missing_docs` warnings on cross-crate public APIs
- [x] All public `Result`-returning functions have `# Errors` doc section
- [x] `rayon` re-added to workspace with scheduler scaffolding (R3.8)
- [x] `WorldgenScheduler`, `ChunkGenPriority`, `StatusRequirement` types exist with tests
- [x] `LightEngine`, `LightUpdateQueue` module skeleton exists (R3.9)
- [x] `DataLayer` has property-based roundtrip tests
- [x] Light packet compliance tests pass
- [x] ECS component types defined with `#[derive(Component)]` (R3.10)
- [x] Marker components and spawn bundles compile and pass query tests
- [x] `TickPhase` enum matches ADR-018 phase order
- [x] `docs/entity-mapping.md` skeleton created
- [x] `cargo test --workspace` passes with zero failures
- [x] `cargo clippy --workspace -- -D warnings` produces zero warnings
- [x] `cargo doc --workspace --no-deps` builds cleanly

---

## Ordering & Dependencies

```
R3.1 (lints)  ──────────────── independent, do first (affects all code)
R3.2 (tick thread) ─────────── independent, highest architectural impact
R3.3 (unused deps) ─────────── independent, trivial
R3.4 (env var overrides) ────── independent
R3.5 (file splits) ─────────── depends on R3.1 (lint fixes may touch same files)
R3.6 (bool naming) ─────────── depends on R3.1 (serde changes may conflict)
R3.7 (docs) ────────────────── do last (after all renames/moves are settled)
R3.8 (worldgen scaffold) ───── depends on R3.3 (rayon re-added)
R3.9 (lighting scaffold) ───── independent (new module, no conflicts)
R3.10 (entity ECS scaffold) ── independent (new files alongside existing code)
```

**Critical path:** R3.1 → R3.5 → R3.6 → R3.7

**Recommended order:**
1. R3.3 (5 min — remove unused deps)
2. R3.1 (1-2 sessions — lint strictness + fix violations)
3. R3.2 (1-2 sessions — tick thread refactor)
4. R3.4 (1 session — env var overrides)
5. R3.5 (1 session — file splits)
6. R3.6 (2-3 sessions — bool naming, largest by volume)
7. R3.8 (1 session — worldgen types + rayon re-add + tests)
8. R3.9 (1 session — lighting module skeleton + DataLayer prop tests)
9. R3.10 (1-2 sessions — ECS component types + bundles + phase enum)
10. R3.7 (ongoing — documentation, including entity-mapping.md)

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Escalating `unwrap_used` to "deny" surfaces hidden unwraps | Fix each one with `?` + context; track count before/after |
| Tick thread refactor breaks async save logic | Keep `Handle` for async bridging; comprehensive tick tests |
| Bool renames break serde deserialization | Add `#[serde(rename)]` for ALL serialized fields; roundtrip tests |
| Bool renames break config file loading | Either rename TOML keys too (pre-1.0) or add serde aliases |
| File splits break module visibility | Audit `pub(crate)` items; re-export from parent mod.rs |
| Env var override type parsing fails silently | Log warnings for malformed env vars; test all types |
| Worldgen scheduler types diverge from ADR-016 | Review ADR-016 before coding; types must match ADR signatures |
| Lighting skeleton `todo!()` panics in tests | Gate `todo!()` behind feature-phase code paths; unit tests use scaffolded types only |
| bevy_ecs version upgrade breaks components | Pin to 0.18; wrap key APIs behind project traits where needed |
| ECS components conflict with existing Entity struct | Coexistence by design — new types in separate files, no migration in R3 |

---

## ADR Compliance Matrix

Summary of all 39 ADRs vs. current codebase status:

| ADR | Title | Compliance | Notes |
|-----|-------|------------|-------|
| 001 | Async Runtime | ✅ Compliant | Tokio multi-threaded |
| 002 | Error Handling | ✅ Compliant | thiserror in libs, deny lints, #[non_exhaustive] |
| 003 | Crate Architecture | ✅ Compliant | Dependency DAG correct |
| 004 | Logging | ✅ Compliant | print_stdout/print_stderr denied |
| 005 | Configuration | ✅ Compliant | env var overrides implemented |
| 006 | Network I/O | 🟡 Deferred | Single-task model works; consider superseding ADR |
| 007 | Packet Codec | ✅ Compliant | All 87 packets have PACKET_ID + Packet trait |
| 008 | Connection State | ✅ Compliant | Pragmatic amendment in ADR-036 |
| 009 | Encryption/Compression | ✅ Compliant | AES-CFB8 + zlib pipeline |
| 010 | NBT | ✅ Compliant | Full implementation |
| 011 | Registry System | ✅ Compliant | build.rs codegen + runtime registries |
| 012 | Block State | ✅ Compliant | u16 IDs + PalettedContainer |
| 013 | Coordinate Types | ✅ Compliant | Newtype wrappers with full conversions |
| 014 | Chunk Storage | ✅ Compliant | Anvil format + atomic writes |
| 015 | Disk I/O | ✅ Compliant | spawn_blocking for all file I/O |
| 016 | Worldgen Pipeline | ⚠️ Scaffolding | ChunkStatus + trait exist; scheduler/priority types added in R3.8 |
| 017 | Lighting Engine | ⚠️ Scaffolding | DataLayer + serializer + engine/queue skeleton + proptests + compliance tests (R3.9) |
| 018 | Entity System | ⚠️ Scaffolding | bevy_ecs components, markers, bundles, and TickPhase scaffolded (R3.10) |
| 019 | Tick Loop | ✅ Compliant | Dedicated OS thread "tick" |
| 020 | Player Session | 🟡 Future phase | Arc<RwLock> model, not channel-based |
| 021 | Physics | ✅ Compliant | Per-axis sweep implemented |
| 022 | Command Framework | ✅ Compliant | Brigadier-compatible graph |
| 023 | AI & Pathfinding | 🟡 Future phase | Not yet implemented |
| 024 | Inventory | ✅ Compliant | Transactional model |
| 025 | Redstone | 🟡 Future phase | Not yet implemented |
| 026 | Loot Tables | 🟡 Future phase | Not yet implemented |
| 027 | Recipe System | 🟡 Future phase | Not yet implemented |
| 028 | Chat Components | ✅ Compliant | Enum tree + manual serde |
| 029 | Memory Management | ✅ Compliant | mimalloc ✅, bumpalo removed (re-add when used) |
| 030 | Shutdown & Crash | ✅ Compliant | Graceful shutdown implemented |
| 031 | Management API | 🟡 Future phase | Not yet implemented |
| 032 | Scalability | 🟡 Future phase | Not yet implemented |
| 033 | Config Format | ✅ Compliant | TOML ✅, OXIDIZED_* env vars ✅ |
| 034 | Testing Strategy | ✅ Compliant | 1657 tests, all categories |
| 035 | Module Structure | ✅ Compliant | All files ≤ 800 LOC (excl. tests) |
| 036 | Packet Handlers | ✅ Compliant | Module split complete (R1) |
| 037 | Type Macros | ✅ Compliant | impl_vector_ops/directional/axis_accessor |
| 038 | Packet Trait | ✅ Compliant | Unified Packet trait + error (R2) |
| 039 | Release Strategy | ✅ Compliant | release-please + git-cliff |

**Legend:**
- ✅ = Fully compliant
- ⚠️ = Partial — scaffolding added in this phase, full implementation in feature phase
- ❌ = Major violation, fixed in this phase
- 🟡 = Future phase (ADR accepted but implementation deferred to its phase)
