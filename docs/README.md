# Oxidized — Documentation

Welcome to the Oxidized documentation. This folder contains the complete
design and implementation plan for the project.

---

## Structure

```
docs/
├── architecture/          # How the system is designed
│   ├── overview.md        # System overview and guiding principles
│   ├── crate-layout.md    # Crate responsibilities and dependency rules
│   ├── protocol.md        # Minecraft 26.1 protocol deep-dive
│   ├── world-format.md    # Chunk, Anvil, NBT, world gen internals
│   └── entity-system.md   # Entity hierarchy, AI, synced data
│
├── phases/                # 38 implementation phases
│   ├── README.md          # Phase index + dependency graph
│   ├── phase-01-bootstrap.md
│   ├── phase-02-tcp-framing.md
│   ├── phase-03-handshake-status.md
│   ├── phase-04-login-auth.md
│   ├── phase-05-nbt.md
│   ├── phase-06-configuration.md
│   ├── phase-07-core-types.md
│   ├── phase-08-block-item-registry.md
│   ├── phase-09-chunk-structures.md
│   ├── phase-10-anvil-loading.md
│   ├── phase-11-server-level.md
│   ├── phase-12-player-join.md
│   ├── phase-13-chunk-sending.md
│   ├── phase-14-player-movement.md
│   ├── phase-15-entity-framework.md
│   ├── phase-16-physics.md
│   ├── phase-17-chat.md
│   ├── phase-18-commands.md
│   ├── phase-19-world-ticking.md
│   ├── phase-20-world-saving.md
│   ├── phase-21-inventory.md
│   ├── phase-22-block-interaction.md
│   ├── phase-23-flat-worldgen.md
│   ├── phase-24-combat.md
│   ├── phase-25-hostile-mobs.md
│   ├── phase-26-noise-worldgen.md
│   ├── phase-27-animals.md
│   ├── phase-28-redstone.md
│   ├── phase-29-crafting.md
│   ├── phase-30-block-entities.md
│   ├── phase-31-advancements.md
│   ├── phase-32-scoreboard.md
│   ├── phase-33-rcon-query.md
│   ├── phase-34-loot-tables.md
│   ├── phase-35-enchantments.md
│   ├── phase-36-structures.md
│   ├── phase-37-jsonrpc.md
│   └── phase-38-performance.md
│
└── reference/             # Technical reference sheets
    ├── README.md            # Reference index
    ├── protocol-packets.md  # All 185 packets in all 5 states
    ├── java-class-map.md    # Java class → Rust module mapping
    └── data-formats.md      # NBT, Anvil region, chunk binary format
```

---

## Quick Links

| Want to... | Go to |
|---|---|
| Understand the big picture | [architecture/overview.md](architecture/overview.md) |
| Understand crate boundaries | [architecture/crate-layout.md](architecture/crate-layout.md) |
| Understand the wire protocol | [architecture/protocol.md](architecture/protocol.md) |
| See all packets | [reference/protocol-packets.md](reference/protocol-packets.md) |
| Find the Java class for X | [reference/java-class-map.md](reference/java-class-map.md) |
| See all binary formats | [reference/data-formats.md](reference/data-formats.md) |
| Browse the reference index | [reference/README.md](reference/README.md) |
| See the current phase | [phases/README.md](phases/README.md) |
| Read the NBT binary spec | [reference/data-formats.md](reference/data-formats.md) |

---

## Minecraft Version

All documentation targets **Minecraft 26.1-pre-3**:

| Field | Value |
|---|---|
| Protocol version | `1073742124` |
| World (data) version | `4782` |
| Java version (vanilla) | 25 |
| Resource pack version | `84.0` |
| Data pack version | `101.1` |
| Build time | 2026-03-17T13:36:07Z |

The decompiled reference is at `mc-server-ref/decompiled/` (gitignored).
Run `scripts/decompile.sh` to regenerate it.
