# Architecture Decision Records

> Every significant design decision in Oxidized is captured as an **ADR**.
> ADRs are immutable once accepted — if a decision is reversed, a new ADR
> supersedes the old one rather than editing it.

## Why ADRs?

Oxidized is **not** a 1:1 clone of the vanilla Minecraft Java server. The vanilla
server was designed in 2009 with Java idioms of that era — deep OOP hierarchies,
single-threaded game loops, blocking I/O, and mutable shared state everywhere.

We keep **one thing identical**: the **wire protocol contract**. The vanilla client
dictates exactly what bytes it sends and what bytes it expects back. Everything
else — how we store data, process ticks, manage memory, schedule I/O, represent
entities — is open for modern design.

Each ADR documents **why** we chose a particular approach, what alternatives we
considered, and what trade-offs we accepted.

---

## ADR Format

We follow an enhanced [Michael Nygard](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions)
format:

```markdown
# ADR-NNN: Title

| Field      | Value                              |
|------------|------------------------------------|
| Status     | Proposed / Accepted / Superseded   |
| Date       | YYYY-MM-DD                         |
| Phases     | P01, P02, ...                      |
| Deciders   | Oxidized Core Team                 |

## Context
[Why is this decision needed?]

## Decision Drivers
- [What matters most?]

## Considered Options
1. **Option A** — ...
2. **Option B** — ...

## Decision
[What we chose and why]

## Consequences
### Positive
### Negative
### Neutral

## Compliance
[How to verify adherence]

## Related ADRs
- [ADR-NNN](link)
```

### Lifecycle

| Status | Meaning |
|--------|---------|
| **Proposed** | Under discussion, not yet binding |
| **Accepted** | Binding — all code must conform |
| **Deprecated** | No longer recommended but not yet replaced |
| **Superseded by ADR-NNN** | Replaced — see the new ADR |

---

## Guiding Principles

These principles inform every ADR:

1. **Wire protocol is sacred** — the client is our only non-negotiable constraint
2. **Data-oriented over object-oriented** — cache-friendly layouts, batch processing
3. **Composition over inheritance** — traits + components, not deep class trees
4. **Concurrency by design** — not bolted on; shared-nothing where possible
5. **Zero-cost abstractions** — newtypes, const generics, compile-time dispatch
6. **Fail fast, recover gracefully** — strong types prevent bugs; runtime errors are handled
7. **Measure, then optimize** — no premature optimization; instrument everything

---

## ADR Index

### Core Platform

| ADR | Title | Status | Phases |
|-----|-------|--------|--------|
| [001](adr-001-async-runtime.md) | Async Runtime Selection | Accepted | P01, P02, P10, P19, P26 |
| [002](adr-002-error-handling.md) | Error Handling Strategy | Accepted | All |
| [003](adr-003-crate-architecture.md) | Crate Workspace Architecture | Accepted | P01 |
| [004](adr-004-logging-observability.md) | Logging, Tracing & Observability | Accepted | P01, P38 |
| [005](adr-005-configuration.md) | Configuration Management | **Superseded by [ADR-033](adr-033-configuration-format.md)** | P01, P19 |

### Network Layer

| ADR | Title | Status | Phases |
|-----|-------|--------|--------|
| [006](adr-006-network-io.md) | Network I/O Architecture | Accepted | P02, P03, P04, P13, P38 |
| [007](adr-007-packet-codec.md) | Packet Codec Framework | Accepted | P02–P06, P12–P14 |
| [008](adr-008-connection-state-machine.md) | Connection State Machine Design | Accepted | P03, P04, P06, P12 |
| [009](adr-009-encryption-compression.md) | Encryption & Compression Pipeline | Accepted | P04 |

### Data Layer

| ADR | Title | Status | Phases |
|-----|-------|--------|--------|
| [010](adr-010-nbt.md) | NBT Library Design | Accepted | P05, P10, P20 |
| [011](adr-011-registry-system.md) | Registry & Data-Driven Content | Accepted | P06, P08, P34, P35 |
| [012](adr-012-block-state.md) | Block State Representation | Accepted | P08, P09, P22 |
| [013](adr-013-coordinate-types.md) | Type-Safe Coordinate System | Accepted | P07 |

### World & Storage

| ADR | Title | Status | Phases |
|-----|-------|--------|--------|
| [014](adr-014-chunk-storage.md) | Chunk Storage & Concurrency | Accepted | P09, P11, P13, P14, P38 |
| [015](adr-015-disk-io.md) | Disk I/O & Persistence Strategy | Accepted | P10, P20 |
| [016](adr-016-worldgen-pipeline.md) | World Generation Pipeline | Accepted | P23, P26, P36 |
| [017](adr-017-lighting.md) | Lighting Engine | Accepted | P13, P19 |

### Game Architecture

| ADR | Title | Status | Phases |
|-----|-------|--------|--------|
| [018](adr-018-entity-system.md) | Entity System Architecture | Accepted | P15, P24, P25, P27 |
| [019](adr-019-tick-loop.md) | Server Tick Loop Design | Accepted | P19, P38 |
| [020](adr-020-player-session.md) | Player Session Lifecycle | Accepted | P12, P14, P17, P20 |
| [021](adr-021-physics.md) | Physics & Collision Engine | Accepted | P16 |
| [022](adr-022-command-framework.md) | Command Framework | Accepted | P18 |
| [023](adr-023-ai-pathfinding.md) | AI & Pathfinding System | Accepted | P25, P27 |
| [024](adr-024-inventory.md) | Inventory & Container Transactions | Accepted | P21, P29, P30 |
| [025](adr-025-redstone.md) | Redstone Simulation Model | Accepted | P28 |

### Data-Driven Game Systems

| ADR | Title | Status | Phases |
|-----|-------|--------|--------|
| [026](adr-026-loot-tables.md) | Loot Table & Predicate Engine | Accepted | P34 |
| [027](adr-027-recipe-system.md) | Recipe System | Accepted | P29 |
| [028](adr-028-chat-components.md) | Chat & Text Component System | Accepted | P17 |

### Operations

| ADR | Title | Status | Phases |
|-----|-------|--------|--------|
| [029](adr-029-memory-management.md) | Memory Management & Allocation | Accepted | P38 |
| [030](adr-030-shutdown-crash.md) | Graceful Shutdown & Crash Recovery | Accepted | P01, P20, P38 |
| [031](adr-031-management-api.md) | Management & Remote Access APIs | Accepted | P33, P37 |
| [032](adr-032-scalability.md) | Performance & Scalability Architecture | Accepted | P38 |

### Process & Strategy

| ADR | Title | Status | Phases |
|-----|-------|--------|--------|
| [033](adr-033-configuration-format.md) | Configuration Format Evolution | Accepted | P01 (retrofit) |
| [034](adr-034-testing-strategy.md) | Comprehensive Testing Strategy | Accepted | All |

---

## Phase → ADR Cross-Reference

| Phase | Relevant ADRs |
|-------|---------------|
| P01 Bootstrap | 001, 002, 003, 004, ~~005~~ → 033, 030, 034 |
| P02 TCP Framing | 001, 006, 007 |
| P03 Handshake/Status | 006, 007, 008 |
| P04 Login/Auth | 006, 007, 008, 009 |
| P05 NBT | 010 |
| P06 Configuration | 007, 008, 011 |
| P07 Core Types | 013 |
| P08 Block/Item Registry | 011, 012 |
| P09 Chunk Structures | 012, 014 |
| P10 Anvil Loading | 010, 015 |
| P11 Server Level | 014 |
| P12 Player Join | 008, 020 |
| P13 Chunk Sending | 006, 014, 017 |
| P14 Movement | 006, 014, 020 |
| P15 Entity Framework | 018 |
| P16 Physics | 021 |
| P17 Chat | 020, 028 |
| P18 Commands | 022 |
| P19 World Ticking | 001, 005, 019 |
| P20 World Saving | 015, 020, 030 |
| P21 Inventory | 024 |
| P22 Block Interaction | 012 |
| P23 Flat Worldgen | 016 |
| P24 Combat | 018 |
| P25 Hostile Mobs | 018, 023 |
| P26 Noise Worldgen | 001, 016 |
| P27 Animals | 018, 023 |
| P28 Redstone | 025 |
| P29 Crafting | 024, 027 |
| P30 Block Entities | 024 |
| P31 Advancements | 011 |
| P32 Scoreboard | — |
| P33 RCON/Query | 031 |
| P34 Loot Tables | 011, 026 |
| P35 Enchantments | 011 |
| P36 Structures | 016 |
| P37 JSON-RPC | 031 |
| P38 Performance | 001, 004, 006, 014, 019, 029, 030, 032 |
