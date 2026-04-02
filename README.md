# Oxidized 🦀

> A high-performance Minecraft Java Edition server rewritten in Rust.
> Targets **Minecraft 26.1** (protocol `775`).

[![CI](https://github.com/oxidized-mc/server/actions/workflows/ci.yml/badge.svg)](https://github.com/oxidized-mc/server/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)

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
├── mc-server-ref/           # Decompiled vanilla server (gitignored)
├── deny.toml                # cargo-deny licence + advisory config
├── rustfmt.toml             # Formatting rules
└── rust-toolchain.toml      # Pinned to stable (CI enforces MSRV 1.85)
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
| [Contributing](CONTRIBUTING.md) | How to contribute to the project |

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

Licensed under the [MIT License](./LICENSE).

> **Note:** This project is not affiliated with or endorsed by Mojang or Microsoft.
> Minecraft is a trademark of Mojang AB. This server reimplementation is developed
> for educational and compatibility purposes only.
