# Copilot Instructions — Oxidized

> Authoritative. Follow every rule. If any rule is outdated or the codebase has drifted,
> update this file as part of the task.

---

## Before Any Task

1. Read **[memories.md](memories.md)** for prior learnings about the crates you'll touch
2. Read the **[6 key ADRs](#key-adrs)** + any ADRs linked from the phase doc or touching your crate
3. Read the **relevant phase doc** (`docs/phases/phase-NN-*.md`) if task belongs to a phase
4. Read **[lifecycle docs](../docs/lifecycle/README.md)** when process questions arise

---

## Project Overview

**Oxidized** — Minecraft Java Edition server in Rust.

- **Protocol:** MC 26.1 (version `1073742124`, world `4782`)
- **Reference:** `mc-server-ref/decompiled/` — decompiled vanilla 26.1-pre-3 JAR (gitignored)
- **Philosophy:** Wire-compatible with vanilla clients, idiomatic Rust internals

---

## Workspace & Crate Dependencies

```
oxidized-nbt       ← no internal deps
oxidized-macros    ← no internal deps (proc-macro)
oxidized-protocol  ← nbt, macros
oxidized-world     ← nbt
oxidized-game      ← protocol, world, nbt
oxidized-server    ← all crates
```

**Never let a lower-layer crate import a higher-layer crate.**

Config files: `Cargo.toml` (workspace), `rustfmt.toml` (max_width=100), `deny.toml` (cargo-deny), `rust-toolchain.toml` (stable, MSRV 1.85).

---

## Lifecycle Rules

Follow the [Development Lifecycle](../docs/lifecycle/README.md): Identify → Research → **Arch Review Gate** → ADR → Plan → Test First → Implement → Review → Integrate → Retrospect.

- **Arch Review Gate (Stage 2.5):** Before planning or testing, question every constraining ADR. If outdated → create a superseding ADR first. Ask: Right pattern? Would a Rust dev choose this? Does the client care? Will we regret this in 6 months?
- **CI:** After every push, wait for all jobs to pass. Never leave `main` broken.
- **Memories:** Check before starting. Update after phases or when discovering gotchas.
- **Improvement:** Outdated ADRs → supersede. Better patterns → record + refactor. Missing tests → add now. Tech debt → TODO + memories.md.
- **Retrospective** after every phase → update memories.md.

---

## Workflow

### Plan first when:

- New crate, module, or public trait
- Task touches >3 files
- Ambiguous request or multiple valid approaches
- Change affects a public trait

**Steps:** Check memories → explore Java ref + Rust code → plan + SQL todos → confirm with user.
**Skip planning for:** single-file fixes, typos, doc edits, dep bumps.

### Java Reference

Always read the equivalent Java class in `mc-server-ref/decompiled/net/minecraft/` first. Understand the algorithm, then **rewrite idiomatically** — never transliterate.

| Concern | Java path |
|---|---|
| Packets | `network/protocol/game/`, `login/`, etc. |
| Connection | `network/Connection.java`, `FriendlyByteBuf.java` |
| Chunks | `world/level/chunk/LevelChunk.java`, `LevelChunkSection.java` |
| Block states | `world/level/block/state/BlockBehaviour.java` |
| Entities | `world/entity/Entity.java`, `LivingEntity.java` |
| Server loop | `server/MinecraftServer.java` |
| NBT | `nbt/CompoundTag.java`, `NbtIo.java` |
| Commands | `commands/` |

### Sub-Agent Dispatch

| Phase | Agent | Use for |
|---|---|---|
| Explore | `explore` | Java reference lookup, codebase search, ADR review |
| Tests (TDD) | `general-purpose` | Write failing tests |
| Implement | `general-purpose` | Feature code following Java reference |
| Build/test | `task` | `cargo test -p <crate>` |
| Code review | `code-review` | ADR compliance, correctness, patterns, improvements |

Parallelise independent `explore` calls. **Review↔Fix loop:** fix issues → re-review → repeat until clean pass.

### TDD Cycle

1. Write failing test → 2. Confirm failure (not compile-error) → 3. Implement minimum to pass → 4. Confirm green → 5. Refactor + re-run → 6. Code review + commit

**Test naming:** `test_<thing>_<condition>` or `<thing>_<outcome>_when_<condition>`

### Test Types ([ADR-034])

| Type | Location | When |
|------|----------|------|
| Unit | `#[cfg(test)] mod tests` | Every function |
| Integration | `crates/<crate>/tests/*.rs` | Cross-module, public API only |
| Property | inline or `tests/` (`proptest`) | All parsers, codecs, roundtrips |
| Compliance | `oxidized-protocol/tests/compliance.rs` | Protocol byte verification |
| Doc | `///` on public items | Every public item |
| Snapshot | `insta::assert_snapshot!` | Error messages, generated output |

**Minimum per PR:** Unit + Integration + Property-based (for parsers/codecs).
**Conventions:** `#[allow(clippy::unwrap_used, clippy::expect_used)]` in test modules. Integration = public API only. Proptest: `proptest_<thing>_<invariant>`. Doc examples: self-contained. Snapshots in `snapshots/` dirs.

### Before Every Commit

Grep for stale references after renames/moves:
```bash
grep -r "old_name" . --include="*.rs" --include="*.toml" --include="*.md"
```

---

## Rust Standards

- **Edition 2024**, stable toolchain, MSRV 1.85
- `#![warn(missing_docs)]` on library crates
- `#![deny(unsafe_code)]` unless justified with `SAFETY:` comment
- **Errors:** `thiserror` in libraries, `anyhow` in `oxidized-server`. Never `unwrap()`/`expect()` in production. Use `?` + `.context()` or `.map_err()`.
- **Naming:** Types `PascalCase`, functions `snake_case`, constants `SCREAMING_SNAKE`, modules `snake_case`, booleans `is_`/`has_`/`can_`, features `kebab-case`
- **Docs:** `///` on all public items with `# Errors` section when returning `Result`. Private helpers: `//` when non-obvious.
- **No magic numbers:** All protocol constants in a `constants` module or inline `const`.

### Async & Threading

- Network I/O: `tokio::net`. Disk I/O: `tokio::task::spawn_blocking`
- Tick loop: dedicated OS thread ([ADR-019])
- CPU-bound (worldgen): `rayon` pool ([ADR-016])
- Per-connection: reader + writer tasks with bounded `mpsc` ([ADR-006])
- Player sessions: network actor (Tokio) ↔ bridge channels ↔ ECS entity (tick thread) ([ADR-020])
- Cross-thread: `tokio::sync::{mpsc, broadcast}`. Non-async locks: `parking_lot`. Concurrent maps: `dashmap::DashMap`

### Performance

- `mimalloc` global allocator + `bumpalo::Bump` arena per tick ([ADR-029])
- `ahash::AHashMap` for hot paths
- Avoid allocations in tick loop — pre-allocated buffers + arena
- Chunk data: bit-packed `PalettedContainer`. Block states: flat `u16` IDs ([ADR-012])

---

## Protocol Quick Reference (26.1-pre-3)

| State | CB | SB |
|---|---|---|
| Handshaking | 0 | 1 |
| Status | 2 | 2 |
| Login | 5 | 5 |
| Configuration | 6 | 6 |
| Play | 127 | 58 |

**Flow:** Handshaking → Status (disconnect) **or** Handshaking → Login → Configuration → Play
**Encryption:** AES-128-CFB8 via RSA-1024. **Compression:** zlib, threshold 256 bytes.
**Chunks:** 24× sections (block count + `PalettedContainer<BlockState>` + `PalettedContainer<Biome>`) + heightmaps NBT + light BitSets.

---

## Key ADRs

All in `docs/adr/`. 32+ ADRs total. Read the phase doc's "Architecture Decisions" section before implementing.

| ADR | Decision | Impact |
|-----|----------|--------|
| [002] | `thiserror` libs / `anyhow` binary | Every crate |
| [007] | `#[derive(McPacket)]` wire format | All packets |
| [008] | Typestate connections | Protocol states |
| [013] | Coordinate newtypes | Spatial code |
| [018] | ECS with `bevy_ecs` | Entity/game logic |
| [019] | Parallel tick phases | Server core |

**New ADR when:** new crate/public trait, choosing between approaches, expensive-to-reverse decision.
**Lifecycle:** Proposed → Accepted → Superseded. Never edit accepted — create a superseding one.

---

## Roadmap (38 Phases)

Track via SQL `todos` (prefix `p01-` through `p38-`).

| Phase | Milestone |
|---|---|
| p01–p02 | Workspace, TCP + VarInt |
| p03 | Server list ping |
| p04 | Authentication |
| p05–p07 | NBT, config state, core types |
| p08–p11 | Block registry, chunks, Anvil, server level |
| p12–p14 | Player join, chunk rendering, movement |
| p15–p18 | Entities, physics, chat, commands |
| R1 | Arch refactoring (ADR-035/036/037) |
| R2 | Packet trait refactoring (ADR-007/038) |
| p19–p22 | World ticks, saves, inventory, block interaction |
| p23–p27 | Worldgen, combat, mobs, animals |
| p28–p32 | Redstone, crafting, block entities, advancements, scoreboards |
| p33–p36 | RCON, loot, enchants, structures |
| p37 | JSON-RPC management server |
| p38 | Production hardening, 100+ players |

---

## Conventional Commits

Format: `<type>(<scope>): <description>`

**Types:** `feat` (minor), `fix` (patch), `perf` (patch), `refactor`, `test`, `docs`, `chore`, `ci`
**Scopes:** `nbt`, `macros`, `protocol`, `world`, `game`, `server`, `ci`, `deps`
**Breaking:** `feat!:` + `BREAKING CHANGE:` in body. No `Co-authored-by:` trailers.

---

## Design Quick Reference

- Palette compression: `SingleValue` → `Linear` → `HashMap` → `Global`
- Chunks: `DashMap<ChunkPos, Arc<ChunkColumn>>` + per-section `RwLock`, 24 sections (y=−64..319), index = `(y >> 4) + 4`
- Registries: compiled core (`build.rs`) + runtime data-driven (data packs)
- NBT: 3 representations — owned (`IndexMap`), arena (`bumpalo`), borrowed (zero-copy)
- Auth: online → `sessionserver.mojang.com/session/minecraft/hasJoined`, offline → UUID v3 `"OfflinePlayer:<name>"`
- 20 TPS default (`50ms`), 256-byte compression threshold, JSON-RPC WebSocket management (disabled by default)

---

<!-- ADR link references -->
[002]: ../docs/adr/adr-002-error-handling.md
[007]: ../docs/adr/adr-007-packet-codec.md
[008]: ../docs/adr/adr-008-connection-state-machine.md
[013]: ../docs/adr/adr-013-coordinate-types.md
[018]: ../docs/adr/adr-018-entity-system.md
[019]: ../docs/adr/adr-019-tick-loop.md
[ADR-006]: ../docs/adr/adr-006-network-io.md
[ADR-012]: ../docs/adr/adr-012-block-state.md
[ADR-016]: ../docs/adr/adr-016-worldgen-pipeline.md
[ADR-019]: ../docs/adr/adr-019-tick-loop.md
[ADR-020]: ../docs/adr/adr-020-player-session.md
[ADR-029]: ../docs/adr/adr-029-memory-management.md
[ADR-034]: ../docs/adr/adr-034-testing-strategy.md
