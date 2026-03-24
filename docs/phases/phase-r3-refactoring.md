# Phase R3 — ADR Compliance & Code Quality Refactoring

**Crates:** all  
**Reward:** Every crate fully complies with its governing ADRs. Lint violations are
zero. Naming, documentation, and architectural patterns match the documented
decisions. The codebase is audit-clean and ready to scale to Phase 38.

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

---

## Goal

Bring the codebase into full compliance with all 39 accepted ADRs. This phase
fixes every deviation discovered during the comprehensive audit performed after
Phase R2. It is a **pure compliance refactoring** — no new features, no protocol
changes.

The audit found **7 categories** of ADR violations:

1. Lint & error handling strictness (ADR-002, ADR-004)
2. Tick loop architecture (ADR-019)
3. Unused declared dependencies (ADR-029, ADR-016)
4. Missing environment variable overrides (ADR-005/033)
5. File size violations (ADR-035)
6. Boolean naming convention violations (project style rules)
7. Missing documentation (project style rules)

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
- **No ECS migration (ADR-018/020)** — the `Arc<RwLock<ServerPlayer>>` model
  works for the current player count. Full ECS migration (bevy_ecs) is a Phase
  15+ concern and far too large for a compliance-only refactoring.

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

**Note:** When Phase 38 (Performance) or a worldgen phase is implemented, these
dependencies should be re-added with actual usage. Update ADR-029 and ADR-016
notes accordingly.

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

## Acceptance Criteria

- [ ] `unwrap_used`, `expect_used`, `panic` clippy lints are "deny" in workspace
- [ ] `print_stdout`, `print_stderr` clippy lints are "deny" in workspace
- [ ] `anyhow` removed from `oxidized-protocol` dependencies
- [ ] All error enums have `#[non_exhaustive]`
- [ ] Tick loop runs on dedicated OS thread (not Tokio task)
- [ ] `bumpalo` and `rayon` removed from workspace (re-add when used)
- [ ] `OXIDIZED_*` environment variable overrides work for all config fields
- [ ] No file exceeds 800 LOC (excluding tests) per ADR-035
- [ ] `play/mod.rs` split into ≥3 submodules
- [ ] `block_interaction.rs` split into ≥3 submodules
- [ ] Boolean fields use `is_`/`has_`/`can_` prefix (with serde renames)
- [x] Zero `missing_docs` warnings on cross-crate public APIs
- [x] All public `Result`-returning functions have `# Errors` doc section
- [ ] `cargo test --workspace` passes with zero failures
- [ ] `cargo clippy --workspace -- -D warnings` produces zero warnings
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
```

**Critical path:** R3.1 → R3.5 → R3.6 → R3.7

**Recommended order:**
1. R3.3 (5 min — remove unused deps)
2. R3.1 (1-2 sessions — lint strictness + fix violations)
3. R3.2 (1-2 sessions — tick thread refactor)
4. R3.4 (1 session — env var overrides)
5. R3.5 (1 session — file splits)
6. R3.6 (2-3 sessions — bool naming, largest by volume)
7. R3.7 (ongoing — documentation)

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

---

## ADR Compliance Matrix

Summary of all 39 ADRs vs. current codebase status:

| ADR | Title | Compliance | Notes |
|-----|-------|------------|-------|
| 001 | Async Runtime | ✅ Compliant | Tokio multi-threaded |
| 002 | Error Handling | ⚠️ **R3.1** | Lint levels too low; anyhow in protocol; 2 missing #[non_exhaustive] |
| 003 | Crate Architecture | ✅ Compliant | Dependency DAG correct |
| 004 | Logging | ⚠️ **R3.1** | println/eprintln not linted |
| 005 | Configuration | ⚠️ **R3.4** | No env var overrides |
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
| 016 | Worldgen Pipeline | 🟡 Future phase | rayon declared but unused |
| 017 | Lighting Engine | 🟡 Future phase | Not yet implemented |
| 018 | Entity System | 🟡 Future phase | bevy_ecs not yet integrated |
| 019 | Tick Loop | ❌ **R3.2** | Runs on Tokio task, not OS thread |
| 020 | Player Session | 🟡 Future phase | Arc<RwLock> model, not channel-based |
| 021 | Physics | ✅ Compliant | Per-axis sweep implemented |
| 022 | Command Framework | ✅ Compliant | Brigadier-compatible graph |
| 023 | AI & Pathfinding | 🟡 Future phase | Not yet implemented |
| 024 | Inventory | ✅ Compliant | Transactional model |
| 025 | Redstone | 🟡 Future phase | Not yet implemented |
| 026 | Loot Tables | 🟡 Future phase | Not yet implemented |
| 027 | Recipe System | 🟡 Future phase | Not yet implemented |
| 028 | Chat Components | ✅ Compliant | Enum tree + manual serde |
| 029 | Memory Management | ⚠️ **R3.3** | mimalloc ✅, bumpalo unused |
| 030 | Shutdown & Crash | ✅ Compliant | Graceful shutdown implemented |
| 031 | Management API | 🟡 Future phase | Not yet implemented |
| 032 | Scalability | 🟡 Future phase | Not yet implemented |
| 033 | Config Format | ⚠️ **R3.4** | TOML ✅, env vars missing |
| 034 | Testing Strategy | ✅ Compliant | 1657 tests, all categories |
| 035 | Module Structure | ⚠️ **R3.5** | 2 files > 800 LOC |
| 036 | Packet Handlers | ✅ Compliant | Module split complete (R1) |
| 037 | Type Macros | ✅ Compliant | impl_vector_ops/directional/axis_accessor |
| 038 | Packet Trait | ✅ Compliant | Unified Packet trait + error (R2) |
| 039 | Release Strategy | ✅ Compliant | release-please + git-cliff |

**Legend:**
- ✅ = Fully compliant
- ⚠️ = Violation found, fixed in this phase
- ❌ = Major violation, fixed in this phase
- 🟡 = Future phase (ADR accepted but implementation deferred to its phase)
