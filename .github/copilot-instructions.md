# Copilot Instructions — Oxidized

> Authoritative. Follow every rule. If any rule is outdated or the codebase has drifted,
> update this file as part of the task.

---

## Before Any Task

1. Read the **[key ADRs](#key-adrs)** + any ADRs linked from the phase doc or touching your crate
2. Read the **relevant phase doc** (`docs/phases/phase-NN-*.md`) if task belongs to a phase
3. Read **[lifecycle docs](../docs/lifecycle/README.md)** when process questions arise

---

## Project Overview

**Oxidized** — Minecraft Java Edition server in Rust.

- **Protocol:** MC 26.1 (version `1073742124`, world `4782`)
- **Reference:** `mc-server-ref/decompiled/` — decompiled vanilla 26.1-pre-3 JAR (gitignored)
- **Philosophy:** Wire-compatible with vanilla clients, idiomatic Rust internals

---

## Workspace & Crate Dependencies

```
oxidized-types     ← no internal deps (shared coordinate types)
oxidized-nbt       ← no internal deps
oxidized-macros    ← no internal deps (proc-macro)
oxidized-protocol  ← types, nbt, macros
oxidized-world     ← types, nbt
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
- **Memories:** Update [memories.md](memories.md) after phases or when discovering gotchas.
- **Improvement:** Outdated ADRs → supersede. Better patterns → record + refactor. Missing tests → add now. Tech debt → TODO + memories.md.
- **Retrospective** after every phase → check memories.md for learnings, update it with new findings.

---

## Workflow

### Plan first when:

- New crate, module, or public trait
- Task touches >3 files
- Ambiguous request or multiple valid approaches
- Change affects a public trait

**Steps:** Explore Java ref + Rust code → plan + SQL todos → confirm with user.
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

#### Built-in Agents

| Agent | Use for |
|---|---|
| `explore` | Quick codebase search, file discovery, answering structural questions |
| `task` | Build & test: `cargo test -p <crate>`, `cargo check --workspace` |
| `code-review` | General code review when custom agents aren't available |

#### Custom Agents (`.github/agents/`)

Prefer these over built-in agents — they have project-specific knowledge.

| Agent | File | Use for |
|---|---|---|
| `@rust-engineer` | `rust-engineer.md` | Rust implementation — features, bug fixes, refactoring across the workspace |
| `@java-reference` | `java-reference.md` | Analyze vanilla Java source in `mc-server-ref/decompiled/`, explain algorithms and protocol details |
| `@reviewer` | `reviewer.md` | Code review — ADR compliance, correctness, vanilla compatibility, performance |
| `@tester` | `tester.md` | Write tests — unit, integration, property-based, compliance, snapshots (ADR-034) |
| `@docs-writer` | `docs-writer.md` | Write ADRs, phase docs, code documentation, update memories.md |

#### Dispatch Rules

- Parallelise independent `explore` and `@java-reference` calls.
- **Review↔Fix loop:** `@reviewer` flags issues → fix them → `@reviewer` re-reviews → repeat until clean pass.
- Use `@java-reference` before implementing any game logic to understand vanilla behavior first.
- Use `@tester` for test strategy and test writing.
- Use `@docs-writer` for ADRs, phase docs, and documentation updates.
- **Do not delegate implementation to sub-agents** — use `@rust-engineer` for guidance, implement yourself.

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

## Key ADRs

Framework-level decisions that affect all code. All ADRs in `docs/adr/`.

| ADR | Decision |
|-----|----------|
| [002] | `thiserror` in libraries, `anyhow` in binary |
| [018] | ECS with `bevy_ecs` for all entity/game logic |
| [019] | Dedicated tick thread with parallel phases |

Read the phase doc's "Architecture Decisions" section for domain-specific ADRs.
**New ADR when:** new crate/public trait, choosing between approaches, expensive-to-reverse decision.
**Lifecycle:** Proposed → Accepted → Superseded. Never edit accepted — create a superseding one.

---

## Versioning

This repo uses [Conventional Commits](https://www.conventionalcommits.org/) for automated versioning.

Format: `<type>(<scope>): <description>`
**Types:** `feat` (minor), `fix` (patch), `perf` (patch), `refactor`, `test`, `docs`, `chore`, `ci`
**Scopes:** `types`, `nbt`, `macros`, `protocol`, `world`, `game`, `server`, `ci`, `deps`
**Breaking:** `feat!:` + `BREAKING CHANGE:` in body. No `Co-authored-by:` trailers.

---

<!-- ADR link references -->
[002]: ../docs/adr/adr-002-error-handling.md
[018]: ../docs/adr/adr-018-entity-system.md
[019]: ../docs/adr/adr-019-tick-loop.md
[ADR-006]: ../docs/adr/adr-006-network-io.md
[ADR-012]: ../docs/adr/adr-012-block-state.md
[ADR-016]: ../docs/adr/adr-016-worldgen-pipeline.md
[ADR-019]: ../docs/adr/adr-019-tick-loop.md
[ADR-020]: ../docs/adr/adr-020-player-session.md
[ADR-029]: ../docs/adr/adr-029-memory-management.md
[ADR-034]: ../docs/adr/adr-034-testing-strategy.md
