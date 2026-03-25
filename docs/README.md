# Oxidized — Documentation

Welcome to the Oxidized documentation. This folder contains the complete
design and implementation plan for the project.

---

## Structure

```
docs/
├── adr/                   # Architecture Decision Records (32 ADRs)
│   ├── README.md          # ADR index, format guide, phase cross-reference
│   ├── adr-001 … adr-009  # Core Platform + Network Layer
│   ├── adr-010 … adr-017  # Data Layer + World & Storage
│   ├── adr-018 … adr-025  # Game Architecture
│   └── adr-026 … adr-032  # Data-Driven Systems + Operations
│
├── architecture/          # How the system is designed
│   ├── overview.md        # System overview and guiding principles
│   ├── crate-layout.md    # Crate responsibilities and dependency rules
│   ├── protocol.md        # Minecraft 26.1 protocol deep-dive
│   ├── world-format.md    # Chunk, Anvil, NBT, world gen internals
│   └── entity-system.md   # Entity hierarchy, AI, synced data
│
├── lifecycle/             # Development lifecycle and processes
│   ├── README.md          # 9-stage development lifecycle
│   ├── quality-gates.md   # Pass/fail criteria for each stage
│   └── continuous-improvement.md  # ADR evolution, retrospectives, tech debt
│
├── phases/                # 38 implementation phases
│   ├── README.md          # Phase index + dependency graph
│   └── phase-01 … phase-38
│
└── reference/             # Technical reference sheets
    ├── README.md          # Reference index
    ├── java-class-map.md  # Java class → Rust module mapping
    └── data-formats.md    # NBT, Anvil region, chunk binary format
```

---

## Quick Links

| Want to... | Go to |
|---|---|
| Understand the big picture | [architecture/overview.md](architecture/overview.md) |
| Understand the development process | [lifecycle/README.md](lifecycle/README.md) |
| See quality standards | [lifecycle/quality-gates.md](lifecycle/quality-gates.md) |
| Understand crate boundaries | [architecture/crate-layout.md](architecture/crate-layout.md) |
| Understand the wire protocol | [architecture/protocol.md](architecture/protocol.md) |
| See why a design choice was made | [adr/README.md](adr/README.md) |
| Find the Java class for X | [reference/java-class-map.md](reference/java-class-map.md) |
| See all binary formats | [reference/data-formats.md](reference/data-formats.md) |
| Browse the reference index | [reference/README.md](reference/README.md) |
| See the current phase | [phases/README.md](phases/README.md) |
| Read project learnings | [../.github/memories.md](../.github/memories.md) |

---

## Minecraft Version

All documentation targets **Minecraft 26.1**:

| Field | Value |
|---|---|
| Protocol version | `775` |
| World (data) version | `4786` |
| Java version (vanilla) | 25 |
| Resource pack version | `84.0` |
| Data pack version | `101.1` |
| Build time | 2026-03-17T13:36:07Z |

The decompiled reference is at `mc-server-ref/decompiled/` (gitignored).
See [reference/README.md](reference/README.md) for instructions on regenerating it.
