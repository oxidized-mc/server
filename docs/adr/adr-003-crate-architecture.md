# ADR-003: Crate Workspace Architecture

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P01 |
| Deciders | Oxidized Core Team |

## Context

The vanilla Minecraft Java server is a monolith — all code lives in a single deobfuscated JAR with interleaved concerns. NBT parsing references packet classes, networking code reaches into world storage, and game logic is tangled with serialization. This makes it nearly impossible to test components in isolation, reuse subsystems independently, or enforce architectural boundaries.

Rust's crate system provides first-class support for modular compilation units with explicit dependency declarations. A Cargo workspace allows multiple crates to share a single `Cargo.lock` and build cache while maintaining strict dependency boundaries enforced at compile time. If crate A doesn't declare a dependency on crate B, it physically cannot import B's types — the compiler prevents it.

We need to decompose the server into crates with a clear dependency DAG (directed acyclic graph). Each crate should own a single domain, have a well-defined public API, and be independently testable. The dependency graph must prevent circular references and minimize coupling between domains.

## Decision Drivers

- **Strict compile-time boundaries**: crates enforce that NBT knows nothing about networking, protocol knows nothing about game logic, etc.
- **Independent testability**: each crate can be tested in isolation with `cargo test -p oxidized-nbt` without building the entire server
- **Incremental compilation**: changing a leaf crate (nbt) doesn't recompile unrelated crates; changing a mid-level crate only recompiles its dependents
- **Reusability**: `oxidized-nbt` and `oxidized-protocol` could be published as standalone libraries for third-party tools
- **Minimal dependency depth**: the longest path in the DAG should be short to limit cascade rebuilds
- **Clear ownership**: every type, function, and concept has exactly one crate that owns it

## Considered Options

### Option 1: Single crate with modules

Put everything in one crate, organized by `mod nbt;`, `mod protocol;`, `mod world;`, etc. This is simple to set up and avoids cross-crate friction (no `pub` visibility planning). However, it provides zero compile-time enforcement of boundaries — any module can import any other module. Incremental compilation is coarse-grained (the whole crate rebuilds). It also prevents publishing subsystems as independent libraries.

### Option 2: 5-crate workspace (nbt, protocol, world, game, server)

Five crates with a clear DAG: `nbt` is a leaf with no internal dependencies, `protocol` depends on `nbt`, `world` depends on `nbt`, `game` depends on `protocol` + `world`, and `server` is the binary that depends on all of them. Each crate owns a single domain. Boundaries are compiler-enforced. Incremental compilation works well — changing NBT recompiles protocol and world, but not the other way around.

### Option 3: Fine-grained 10+ crates

Split further: separate crates for each protocol state (handshake, status, login, config, play), separate crates for chunk storage, entity management, block state registry, etc. This maximizes boundary enforcement but introduces significant friction — cross-crate type sharing requires careful `pub` API design, and the dependency graph becomes complex. Small changes may require coordinating across many crate boundaries.

### Option 4: Dynamic plugin system

Use `libloading` or `abi_stable` for dynamically loaded plugins. Each subsystem is a shared library loaded at runtime. This enables hot-reloading but adds massive complexity (ABI stability, FFI safety, version management) and loses Rust's compile-time guarantees across plugin boundaries. Premature for an initial implementation.

## Decision

**We adopt a 5-crate Cargo workspace.** The crates and their responsibilities are:

```
oxidized-nbt         (leaf — no internal deps)
    ↑           ↑
oxidized-protocol   oxidized-world
    ↑           ↑       ↑
    oxidized-game ──────┘
         ↑
    oxidized-server    (binary entry point)
```

### Crate Responsibilities

**`oxidized-nbt`** — NBT (Named Binary Tag) serialization library
- Encodes and decodes the NBT binary format used throughout Minecraft
- Provides `NbtCompound`, `NbtList`, `NbtTag` types and a `Value` enum
- Serde integration: `#[derive(Serialize, Deserialize)]` for NBT
- Zero Minecraft-specific knowledge — this is a pure data format library
- Could be published to crates.io independently

**`oxidized-protocol`** — Minecraft protocol codec
- Defines all ~300 packet structs across 5 protocol states (Handshaking, Status, Login, Configuration, Play)
- Provides `VarInt`, `VarLong`, `McString`, `Position`, `Angle` wire types
- Packet registry: `(State, Direction, PacketId) → decode function`
- Depends on `oxidized-nbt` for NBT fields embedded in packets
- Knows nothing about game logic — packets are pure data containers

**`oxidized-world`** — World storage, chunk management, block states
- Anvil region file format (read/write)
- Chunk data structures: sections, block states, biomes, heightmaps, light
- Block state registry: `block_id ↔ BlockState` mappings
- World generation interfaces (trait-based, implementations TBD)
- Depends on `oxidized-nbt` for chunk/entity serialization
- Knows nothing about networking or players

**`oxidized-game`** — Game logic and simulation
- Tick loop orchestration: entity ticking, block updates, weather, time
- Entity system: players, mobs, items — positions, velocities, AI
- Inventory, crafting, enchanting logic
- Game rules, difficulty, world border
- Commands and command dispatch
- Depends on `oxidized-protocol` (to construct/interpret packets) and `oxidized-world` (to read/write world state)

**`oxidized-server`** — Binary entry point and infrastructure
- `fn main()` — CLI parsing, config loading, runtime bootstrap
- Network listener: accepts connections, spawns connection tasks
- RCON, Query, and server list ping handling
- Graceful shutdown orchestration
- Depends on all other crates; this is the only binary in the workspace

### Workspace Configuration

```toml
# Root Cargo.toml
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
thiserror = "2"
anyhow = "1"
tracing = "0.1"
bytes = "1"
serde = { version = "1", features = ["derive"] }
```

All shared dependencies are declared in `[workspace.dependencies]` and referenced by crates with `dep.workspace = true`. This ensures version consistency across the workspace.

## Consequences

### Positive

- Compile-time enforcement of architectural boundaries — impossible for `oxidized-nbt` to accidentally depend on networking code
- Each crate can be tested in isolation: `cargo test -p oxidized-protocol` runs only protocol tests
- Incremental compilation is fine-grained — changing `oxidized-nbt` only rebuilds crates that depend on it
- `oxidized-nbt` and `oxidized-protocol` are candidates for open-source publication as standalone libraries
- New contributors can focus on a single crate without understanding the entire server

### Negative

- Cross-crate API changes require careful coordination — adding a field to an NBT type may require updating protocol and world crates
- `pub` visibility must be carefully designed — too restrictive blocks legitimate use, too permissive leaks implementation details
- Five crates adds overhead to CI (each crate is a compilation unit) though Cargo's caching mitigates this

### Neutral

- The 5-crate structure may evolve as the project grows — `oxidized-game` is the largest crate and may be split in later phases if it becomes unwieldy
- Workspace-level `[lints]` ensure consistent Clippy and rustc warnings across all crates

## Compliance

- **Dependency DAG check**: CI runs a script that parses `Cargo.toml` files and verifies no circular dependencies exist (redundant with Cargo's own check, but documents intent)
- **No wildcard re-exports**: code review rejects `pub use crate_name::*` at crate roots — all public APIs must be explicitly listed
- **Crate boundary test**: each crate's `lib.rs` must compile with only its declared dependencies — no `[dev-dependencies]` leaking into production code
- **Feature flag audit**: if a crate adds a feature flag, it must be documented in the crate's README and added to the workspace CI matrix

## Related ADRs

- [ADR-001: Async Runtime Selection](adr-001-async-runtime.md) — Tokio is a workspace-level dependency shared by all crates
- [ADR-002: Error Handling Strategy](adr-002-error-handling.md) — each crate defines its own error types with thiserror
- [ADR-007: Packet Codec Framework](adr-007-packet-codec.md) — lives in `oxidized-protocol`
- [ADR-008: Connection State Machine](adr-008-connection-state-machine.md) — spans `oxidized-protocol` and `oxidized-server`

## References

- [Cargo Workspaces — The Rust Book](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html)
- [Cargo workspace.dependencies](https://doc.rust-lang.org/cargo/reference/workspaces.html#the-dependencies-table)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Matklad — "Large Rust Workspaces"](https://matklad.github.io/2021/09/04/fast-rust-builds.html)
