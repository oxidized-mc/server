# Oxidized 🦀

> A high-performance Minecraft Java Edition server rewritten in Rust.
> Targets **Minecraft 26.1** (protocol `1073742124`) — the first fully unobfuscated release.

[![CI](https://github.com/dodoflix/Oxidized/actions/workflows/ci.yml/badge.svg)](https://github.com/dodoflix/Oxidized/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-1.94%2B-orange.svg)](https://www.rust-lang.org/)

---

## Why Oxidized?

| Goal | Detail |
|---|---|
| **Correctness** | Implemented against the decompiled 26.1 server source (4 789 Java files) |
| **Performance** | Async-first with Tokio; ECS-based entity system; no blocking I/O on the game thread |
| **Maintainability** | Clean Rust idioms, comprehensive tests, strict Clippy lints |
| **Compatibility** | Wire-protocol compatible with the vanilla 26.1 client |
| **Modern design** | Not a 1:1 Java port — uses data-oriented architecture, parallel tick phases, and Rust-native patterns |

---

## Status

🚧 **Pre-alpha — infrastructure and planning complete, implementation starting.**

See the [38-phase roadmap](./docs/phases/README.md) and
[32 Architecture Decision Records](./docs/adr/README.md) for the full design.

---

## Project Layout

```
Oxidized/
├── crates/
│   ├── oxidized-nbt/        # NBT read/write, SNBT, GZIP/zlib
│   ├── oxidized-macros/     # Proc-macro: #[derive(McPacket, McRead, McWrite)]
│   ├── oxidized-protocol/   # Network: TCP, packet codec, typestate connections
│   ├── oxidized-world/      # World, chunks (Anvil), blocks, items, lighting
│   ├── oxidized-game/       # ECS (bevy_ecs): entities, AI, combat, commands
│   └── oxidized-server/     # Binary — startup, tick loop, network layer
├── docs/
│   ├── adr/                 # 32 Architecture Decision Records
│   ├── architecture/        # System design documents
│   ├── phases/              # 38 implementation phase details
│   └── reference/           # Java class map, binary format specs
├── mc-server-ref/           # Decompiled vanilla server (gitignored)
├── deny.toml                # cargo-deny licence + advisory config
├── rustfmt.toml             # Formatting rules
└── rust-toolchain.toml      # Pinned to stable 1.94.0
```

---

## Quick Start

### Requirements

- Rust stable 1.85+ (see `rust-toolchain.toml` for pinned version)
- A vanilla Minecraft 26.1 client

### Build & Run

```bash
cargo build --release
./target/release/oxidized
```

### Development

```bash
cargo run                    # debug build
cargo test --workspace       # all tests
cargo fmt --check            # formatting
cargo clippy --workspace --all-targets -- -D warnings  # lints
```

---

## Documentation

| Document | Description |
|---|---|
| [Architecture Overview](./docs/architecture/overview.md) | System design, threading model, data flow |
| [Crate Layout](./docs/architecture/crate-layout.md) | 6-crate workspace, dependency rules |
| [Development Lifecycle](./docs/lifecycle/README.md) | 9-stage lifecycle, quality gates, continuous improvement |
| [Protocol](./docs/architecture/protocol.md) | Wire protocol, packet states, encryption |
| [Phases](./docs/phases/README.md) | 38-phase implementation roadmap |
| [ADRs](./docs/adr/README.md) | 32 Architecture Decision Records |
| [Memories](.github/memories.md) | Persistent learnings, patterns, gotchas |
| [Java Class Map](./docs/reference/java-class-map.md) | 110+ vanilla Java → Rust mappings |
| [Data Formats](./docs/reference/data-formats.md) | Binary format specs (VarInt, NBT, chunks) |

---

## Reference Code

The decompiled vanilla server is at `mc-server-ref/decompiled/` (gitignored).
The implementation uses the Java source as reference but rewrites everything
idiomatically in Rust — no line-by-line transliteration.

---

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md). All contributions are welcome — bugs,
docs, phases, performance.

---

## License

Licensed under either of

- **MIT License** ([LICENSE-MIT](./LICENSE-MIT))
- **Apache License, Version 2.0** ([LICENSE-APACHE](./LICENSE-APACHE))

at your option.

> **Note:** This project is not affiliated with or endorsed by Mojang or Microsoft.
> Minecraft is a trademark of Mojang AB. This server reimplementation is developed
> for educational and compatibility purposes only.
