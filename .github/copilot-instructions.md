# GitHub Copilot Instructions — Oxidized

> **These instructions are authoritative.** Copilot must read this file at the start of every
> session and follow every rule here. If you notice that any rule is outdated, a pattern is
> missing, or the codebase has drifted from what is documented here, **update this file as
> part of the same task** — keep it accurate and complete at all times.

---

## Required Reading — Before Any Task

**You MUST read every file in this list before writing any code, running any command, or
making any decision.** Do not skip this step — even for "simple" tasks. Context prevents
mistakes.

| # | File | Why |
|---|------|-----|
| 1 | **`.github/copilot-instructions.md`** (this file) | Project rules, coding standards, workflow |
| 2 | **[`.github/memories.md`](memories.md)** | Prior learnings, gotchas, patterns discovered in earlier sessions |
| 3 | **Relevant ADRs** (`docs/adr/adr-NNN-*.md`) | Design decisions that constrain implementation |
| 4 | **Relevant phase doc** (`docs/phases/phase-NN-*.md`) | Scope, acceptance criteria, and linked ADRs for the current phase |

### How to determine "relevant"

- **ADRs:** Read the [Key ADRs table](#architecture-decision-records-adrs) below — those
  6 ADRs apply to *every* task. Then read any additional ADRs linked from the phase doc or
  touching the crate you are modifying.
- **Phase doc:** If the task belongs to a numbered phase (p01–p38), read that phase's doc.
  If the task is ad-hoc (bug fix, refactor), read any phase doc whose scope overlaps.

### Verification

After reading, mentally confirm:
1. Do I know which crate(s) this task touches?
2. Have I checked memories.md for prior learnings about those crates?
3. Have I read every ADR that constrains my implementation?
4. Do I understand the acceptance criteria from the phase doc (if applicable)?

If the answer to any question is **no**, go back and read before proceeding.

---

## Project Overview

**Oxidized** is a high-performance Minecraft Java Edition server rewritten in Rust.

- **Target protocol:** Minecraft 26.1 (protocol version `1073742124`, world version `4782`)
- **Reference source:** `mc-server-ref/decompiled/` — 4 789 decompiled Java files from
  the vanilla 26.1-pre-3 server JAR (gitignored)
- **Philosophy:** wire-protocol compatible with vanilla clients, idiomatic Rust internals

---

## Workspace Layout

```
oxidized/
├── crates/
│   ├── oxidized-nbt/        # NBT binary format, SNBT, GZIP/zlib — NO deps on other crates
│   ├── oxidized-macros/     # Proc-macro crate: #[derive(McPacket, McRead, McWrite)]
│   ├── oxidized-protocol/   # Packet codec, connection states, typestate machine
│   ├── oxidized-world/      # Chunks, blocks, Anvil I/O, lighting, world gen
│   ├── oxidized-game/       # ECS components/systems (bevy_ecs), AI, combat, commands
│   └── oxidized-server/     # Binary: startup, tick loop, server config, network layer
├── mc-server-ref/           # (gitignored) decompiled vanilla reference
├── Cargo.toml               # Workspace manifest + shared dependency versions
├── rustfmt.toml             # Formatting rules (max_width=100)
├── deny.toml                # cargo-deny: licences + advisory config
└── rust-toolchain.toml      # Pinned to stable 1.94.0
```

### Crate dependency rules (enforce strictly)

```
oxidized-nbt       ← no internal deps
oxidized-macros    ← no internal deps (proc-macro crate)
oxidized-protocol  ← oxidized-nbt, oxidized-macros
oxidized-world     ← oxidized-nbt
oxidized-game      ← oxidized-protocol, oxidized-world, oxidized-nbt
oxidized-server    ← all crates
```

Never let a lower-layer crate import a higher-layer crate.

---

## Development Lifecycle

All work in Oxidized follows the [Development Lifecycle](../docs/lifecycle/README.md).
The 9 stages are: **Identify → Research → Decide (ADR) → Plan → Test First → Implement →
Review → Integrate → Retrospect.** The lifecycle is **not linear** — it contains 5 feedback
loops (TDD Red↔Green, Review↔Fix, CI Repair, ADR Rethink, Retrospect→Identify). See
[Quality Gates](../docs/lifecycle/quality-gates.md) for pass/fail criteria at each stage.

**CI Pipeline Rule:** After every push, **wait for all CI jobs to complete** and verify
they pass. If any fail, fix immediately and re-push — never leave `main` broken. Check
all workflows: CI (test matrix), cargo-deny, MSRV, security audit.

### Persistent Memories

**Before starting any task**, check [`.github/memories.md`](memories.md) for relevant
prior learnings. **After completing any phase or discovering something noteworthy**, update
memories.md with patterns, gotchas, or improvement notes. Memories are institutional
knowledge that persists across sessions — treat them as a first-class project artifact.

### Continuous Improvement

Every code review and phase completion must actively look for improvements:
- **Outdated ADRs?** → Create a superseding ADR and plan migration
- **Better patterns?** → Record in memories.md and schedule refactoring
- **Missing tests?** → Add them now, not later
- **Technical debt?** → Record it explicitly (TODO comment + memories.md entry)

See [Continuous Improvement](../docs/lifecycle/continuous-improvement.md) for the full process.

### Phase Retrospectives

After every phase completion, conduct a mandatory retrospective:
1. What went well? What patterns worked?
2. What surprised us? What was harder than expected?
3. What should change? Are any ADRs outdated?
4. What technical debt was incurred?
5. Update memories.md with all findings

---

## Architectural Questioning

Before implementing ANY decision from an ADR, explicitly question whether it is still the best approach:

1. **Is this the right format/tool/pattern?** — Don't assume prior ADR decisions are final. Technology evolves, requirements clarify, and better options emerge.
2. **Would a Rust developer choose this?** — If we're replicating a Java pattern just because vanilla does it, stop and evaluate Rust-native alternatives.
3. **Does the client care?** — The Minecraft client defines the contract (protocol, packet format). Everything else is implementation detail we control.
4. **What would we regret in 6 months?** — Choose the option that scales, not the one that's fastest to implement.

If questioning reveals a better approach, **create a new ADR before proceeding**. Never implement something you suspect is suboptimal just because an existing ADR says so — ADRs are living documents that can be superseded.

### Examples of Good Questioning

- "ADR-005 chose `.properties` for config — but that's a Java format. TOML is Rust-native and typed. Should we supersede?"
- "Vanilla uses NBT for everything — but is NBT the best format for disk storage, or just for network protocol?"
- "The tick loop is single-threaded in vanilla — but should it be in a Rust server with async support?"

---

## Development Workflow

### Task Size Gating — Plan Before Acting

**Always plan first when ANY of these are true:**
- New crate, new module, or new public trait is being added
- Task touches more than 3 files
- The request is ambiguous or has multiple valid approaches
- A decision affects a public trait (breaking change for all callers)

**Planning steps:**
1. Check [persistent memories](memories.md) for relevant prior learnings
2. Use `explore` agent to check the decompiled Java reference + existing Rust code
3. Write a plan to the session plan file; break into SQL `todos`
4. Confirm the plan with the user before writing any code

**Start directly (no plan needed):**
- Single-file bug fix, typo, or doc update
- Dependency bump in `Cargo.toml`

---

### Always Check the Java Reference First

Before implementing anything, read the equivalent Java class in
`mc-server-ref/decompiled/net/minecraft/`. Understand the algorithm, then
**rewrite idiomatically in Rust** — never transliterate Java line-by-line.

Key paths:
| Concern | Java path |
|---|---|
| Packets | `network/protocol/game/`, `network/protocol/login/`, etc. |
| Connection | `network/Connection.java`, `network/FriendlyByteBuf.java` |
| Chunk | `world/level/chunk/LevelChunk.java`, `LevelChunkSection.java` |
| Block states | `world/level/block/state/BlockBehaviour.java` |
| Entities | `world/entity/Entity.java`, `world/entity/LivingEntity.java` |
| Server loop | `server/MinecraftServer.java`, `server/dedicated/DedicatedServer.java` |
| NBT | `nbt/CompoundTag.java`, `nbt/NbtIo.java` |
| Commands | `commands/` (Brigadier dispatching) |

---

### Sub-Agent Dispatch

| Phase | Agent | Prompt |
|---|---|---|
| **Explore** | `explore` | "Where is X in the Java reference? Find callers of Y." |
| **Write tests (TDD)** | `general-purpose` | "Write failing tests for X in crate Y." |
| **Implement** | `general-purpose` | "Implement X in crate Y — follow Java reference at path Z." |
| **Build & test** | `task` | `cargo test -p oxidized-nbt` — full output on failure only. |
| **Code review** | `code-review` | Review staged changes — check ADR compliance + improvements. |

Parallelise independent `explore` calls. Never re-read files an agent already reported.

**Code review must check:**
1. Correctness and edge cases
2. ADR compliance
3. Pattern consistency
4. Stale references
5. **Improvement opportunities** (outdated ADRs, better patterns, missing tests)

**Review↔Fix loop (mandatory):**
If the code review finds issues → fix them → **re-run the code-review agent** on the
updated code. Never commit after fixing review findings without a clean re-review pass.
The loop terminates only when a review pass finds zero significant issues.

---

### TDD Cycle

All logic must follow TDD:

1. **Write the failing test** in `crates/<name>/src/<module>.rs` `#[cfg(test)]`
2. **Run:** `task` agent → `cargo test -p <crate>` — confirm it **fails** (not compile-errors)
3. **Implement** minimum code to pass
4. **Run again** — confirm green
5. **Refactor** + re-run
6. **Code review** + commit test + impl together

Test naming: `test_<thing>_<condition>` or `<thing>_<outcome>_when_<condition>`.

---

### Reference Consistency Check (before every commit)

After renaming, moving, or changing behaviour, grep for stale references:

```bash
grep -r "old_name" . --include="*.rs" --include="*.toml" --include="*.md"
```

Fix every stale reference in the same commit.

---

## Rust Coding Standards

### Language & Edition

- **Rust stable**, edition 2021 (pinned in `rust-toolchain.toml`)
- `#![warn(missing_docs)]` on all public library crates (enforced via workspace lints)
- `#![deny(unsafe_code)]` unless a crate explicitly needs it (document why with `SAFETY:` comment)

### Error Handling

- Use `thiserror` for library errors (typed, structured)
- Use `anyhow` for application-level errors in `oxidized-server`
- **Never** use `unwrap()` or `expect()` in non-test production code
- Use `?` propagation everywhere; add context with `.context("…")` (anyhow) or
  `.map_err(|e| MyError::Thing(e))` (thiserror)

### Naming Conventions

| Concept | Convention | Example |
|---|---|---|
| Types / Traits | `PascalCase` | `LevelChunk`, `BlockGetter` |
| Functions / methods | `snake_case` | `get_block_state`, `read_varint` |
| Constants | `SCREAMING_SNAKE_CASE` | `PROTOCOL_VERSION`, `SECTION_SIZE` |
| Modules | `snake_case` | `chunk`, `packet_codec` |
| Booleans | `is_*`, `has_*`, `can_*` | `is_empty`, `has_gravity` |
| Crate features | `kebab-case` | `serde`, `async-tokio` |

### Documentation

All public items require `///` doc comments:

```rust
/// Returns the [`BlockState`] at the given position, or [`BlockState::AIR`]
/// if the position is outside loaded chunks.
///
/// # Errors
///
/// Returns [`WorldError::OutOfBounds`] if `pos` is outside the valid world height.
pub fn get_block_state(&self, pos: BlockPos) -> Result<BlockState, WorldError> { … }
```

Private helpers may have a short `//` comment when non-obvious.

### No Magic Numbers

All protocol constants live in a `constants` module or are inline `const`:

```rust
pub const PROTOCOL_VERSION: i32 = 1073742124;
pub const WORLD_VERSION: i32 = 4782;
pub const SECTION_SIZE: usize = 16 * 16 * 16;   // 4096
pub const SECTION_COUNT: usize = 24;             // -4..=19 (overworld)
pub const DEFAULT_PORT: u16 = 25565;
pub const DEFAULT_COMPRESSION_THRESHOLD: i32 = 256;
pub const TICKS_PER_SECOND: u32 = 20;
pub const AUTOSAVE_INTERVAL_TICKS: u32 = 6000;
```

### Async & Threading Rules

- All network I/O uses `tokio::net`; all disk I/O uses `tokio::task::spawn_blocking`
- The game tick loop runs on a **dedicated OS thread** (not a Tokio task) — see [ADR-019](../docs/adr/adr-019-tick-loop.md)
- CPU-bound work (chunk generation, noise sampling) runs on a **rayon** thread pool — see [ADR-016](../docs/adr/adr-016-worldgen-pipeline.md)
- Per-connection network uses a reader task + writer task pair with bounded `mpsc` channels — see [ADR-006](../docs/adr/adr-006-network-io.md)
- Player sessions are split: network actor (Tokio) ↔ bridge channels ↔ ECS entity (tick thread) — see [ADR-020](../docs/adr/adr-020-player-session.md)
- Use `tokio::sync::{mpsc, broadcast}` for cross-thread communication
- Use `parking_lot::{RwLock, Mutex}` for non-async locks
- Use `dashmap::DashMap` for concurrent read-heavy maps (e.g., chunk storage)

### Performance

- **Global allocator:** `mimalloc` — see [ADR-029](../docs/adr/adr-029-memory-management.md)
- **Per-tick arena:** `bumpalo::Bump` reset every tick for temporaries — see [ADR-029](../docs/adr/adr-029-memory-management.md)
- Prefer `ahash::AHashMap` over `std::collections::HashMap` for hot paths
- Use `parking_lot::{RwLock, Mutex}` for low-contention non-async locks
- Avoid allocating inside the tick loop — prefer pre-allocated buffers and arena allocation
- Chunk data uses compact bit-packed representation (`PalettedContainer`)
- Block states use flat `u16` IDs with dense lookup tables — see [ADR-012](../docs/adr/adr-012-block-state.md)

---

## Protocol Reference (26.1-pre-3)

| State | Clientbound | Serverbound |
|---|---|---|
| Handshaking | 0 | 1 (`ClientIntentionPacket`) |
| Status | 2 | 2 |
| Login | 5 | 5 |
| Configuration | 6 | 6 |
| Play | 127 | 58 |

**State machine:** `Handshaking → Status` (disconnect after pong)  
**or** `Handshaking → Login → Configuration → Play`

**Encryption:** AES-128-CFB8 (symmetric). Key exchange via RSA-1024 (server pub key sent in
`ClientboundHelloPacket`).  
**Compression:** zlib/deflate threshold-based (default 256 bytes, -1 = disabled).

**Chunk format:** `ClientboundLevelChunkWithLightPacket` = 24× `LevelChunkSection` binary
(each: `non_empty_block_count: i16` + `PalettedContainer<BlockState>` + `PalettedContainer<Biome>`)
+ heightmaps CompoundTag + light BitSets.

---

## Architecture Decision Records (ADRs)

All significant design decisions are documented in `docs/adr/`. There are **32 ADRs**
covering every major system. Before implementing any phase, **read the linked ADRs**
in that phase's doc file (`docs/phases/phase-NN-*.md` → "Architecture Decisions" section).

**Key ADRs that affect ALL code:**

| ADR | Decision | Impact |
|-----|----------|--------|
| [002](../docs/adr/adr-002-error-handling.md) | `thiserror` in libraries, `anyhow` in binary | Every crate |
| [007](../docs/adr/adr-007-packet-codec.md) | `#[derive(McPacket)]` for wire format | All packets |
| [008](../docs/adr/adr-008-connection-state-machine.md) | Typestate pattern for connections | All protocol states |
| [013](../docs/adr/adr-013-coordinate-types.md) | Newtype wrappers for coordinates | All spatial code |
| [018](../docs/adr/adr-018-entity-system.md) | ECS with `bevy_ecs` | All entity/game logic |
| [019](../docs/adr/adr-019-tick-loop.md) | Parallel tick phases | Server core |

**When to create a new ADR:**
- Adding a new crate or public trait
- Choosing between multiple valid approaches for a non-trivial system
- Making a decision that would be expensive to reverse

**ADR lifecycle:** Proposed → Accepted → (Superseded by ADR-NNN). Never edit accepted ADRs — create a new one that supersedes.

---

## Implementation Roadmap (38 Phases)

Track via SQL `todos` table. Use the `id` prefix `p01-` through `p38-`.
Full descriptions in the session plan file. Summary:

| Phase | Milestone |
|---|---|
| p01–p02 | Compilable workspace, TCP + VarInt framing |
| p03 | Server appears in multiplayer list |
| p04 | Vanilla client authenticates |
| p05–p07 | NBT, configuration state, core data types |
| p08–p11 | Block registry, chunk structures, Anvil load, server level |
| p12–p14 | Player join + spawns, chunks render, movement works |
| p15–p18 | Entities, physics, chat, commands |
| p19–p22 | World ticks, saves, inventory, block interaction |
| p23–p27 | World generation (flat + noise), combat, mobs, animals |
| p28–p32 | Redstone, crafting, block entities, advancements, scoreboards |
| p33–p36 | RCON/Query, loot tables, enchants, structures |
| p37 | JSON-RPC WebSocket management server (new in 26.1) |
| p38 | Production hardening, 100+ player scale |

Before starting any phase, query ready todos:
```sql
SELECT t.id, t.title FROM todos t
WHERE t.status = 'pending'
AND NOT EXISTS (
    SELECT 1 FROM todo_deps td
    JOIN todos dep ON td.depends_on = dep.id
    WHERE td.todo_id = t.id AND dep.status != 'done'
);
```

---

## Conventional Commits

Every commit **must** follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <short description>
```

### Types

| Type | When | Version |
|---|---|---|
| `feat` | New user-visible feature | Minor |
| `fix` | Bug fix | Patch |
| `perf` | Performance improvement | Patch |
| `refactor` | Restructure, no behaviour change | None |
| `test` | Tests only | None |
| `docs` | Documentation only | None |
| `chore` | Deps, tooling, CI config | None |
| `ci` | Workflow files | None |

### Scopes

`nbt`, `macros`, `protocol`, `world`, `game`, `server`, `ci`, `deps`

### Examples

```
feat(protocol): implement VarInt/VarLong read and write
fix(world): correct PalettedContainer bit-packing for GlobalPalette
perf(game): cache entity bounding boxes across ticks
test(nbt): add round-trip fuzz tests for CompoundTag
chore(deps): bump tokio from 1.43 to 1.44
ci: add MSRV check job to CI workflow
feat!: rename BlockGetter::block_state to get_block_state

BREAKING CHANGE: method renamed for consistency with Rust naming conventions
```

**No `Co-authored-by:` trailers.** Keep commit messages clean.

---

## Key Design Decisions

- **Async-first networking:** Tokio runtime for all network I/O, per-connection task pairs
- **Dedicated tick thread:** Game loop runs on its own OS thread with 6 parallel phases
- **ECS architecture:** `bevy_ecs` for all entity/game state — entities are opaque IDs, not trait objects
- **Split player sessions:** Network actor (Tokio) ↔ bridge channels ↔ ECS entity (tick thread)
- **No unsafe in libraries** unless absolutely necessary (document with `SAFETY:` comment)
- **Memory:** `mimalloc` global allocator + `bumpalo` arena for per-tick temporaries
- **Palette compression:** `SingleValue` → `Linear` → `HashMap` → `Global` (mirrors Java)
- **Block state IDs:** flat `u16` with compile-time lookup table from vanilla `blocks.json`
- **Coordinate newtypes:** `BlockPos`, `ChunkPos`, `SectionPos` — compile-time safety
- **Chunk storage:** `DashMap<ChunkPos, Arc<ChunkColumn>>` + per-section `RwLock`
- **Chunk sections:** 24 sections covering y=−64 to y=319 (overworld); index = `(y >> 4) + 4`
- **Registries:** compiled core (blocks, items via `build.rs`) + runtime data-driven (data packs)
- **NBT:** 3 representations — owned tree (`IndexMap`), arena-allocated (`bumpalo`), borrowed (zero-copy)
- **Worldgen:** rayon thread pool for CPU-bound noise/density sampling
- **Online mode auth:** POST to `sessionserver.mojang.com/session/minecraft/hasJoined`
- **Offline mode UUID:** `UUID v3` from `"OfflinePlayer:<name>"`
- **Tick rate:** 20 TPS default (`Duration::from_millis(50)`), configurable via server config
- **Compression threshold:** 256 bytes default (send `ClientboundLoginCompressionPacket` during LOGIN)
- **JSON-RPC management:** WebSocket on a separate port (disabled by default), new in 26.1
