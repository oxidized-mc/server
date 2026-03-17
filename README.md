# Oxidized 🦀

> A high-performance Minecraft Java Edition server rewritten in Rust.  
> Targets **Minecraft 26.1** (protocol `1073742124`) — the first fully unobfuscated release.

[![CI](https://github.com/your-org/oxidized/actions/workflows/ci.yml/badge.svg)](https://github.com/your-org/oxidized/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org/)

---

## Why Oxidized?

| Goal | Detail |
|---|---|
| **Correctness** | Implemented against the decompiled 26.1 server source (4 789 Java files) |
| **Performance** | Async-first with Tokio; no blocking I/O on the game thread |
| **Maintainability** | Clean Rust idioms, comprehensive tests, strict Clippy lints |
| **Compatibility** | Wire-protocol compatible with the vanilla 26.1 client |

---

## Status

🚧 **Pre-alpha — Phase 1 (project bootstrap) in progress.**  
See [PHASES.md](./PHASES.md) for the full 38-phase roadmap.

| Phase | Status | Description |
|-------|--------|-------------|
| 1 | 🔄 In Progress | Project bootstrap, workspace, config |
| 2–38 | ⏳ Planned | See roadmap |

---

## Project Layout

```
oxidized/
├── crates/
│   ├── oxidized-server/     # Binary — startup, server loop, tick
│   ├── oxidized-protocol/   # Network: TCP, packet codec, all 26.1 packets
│   ├── oxidized-nbt/        # NBT read/write, SNBT, GZIP/zlib
│   ├── oxidized-world/      # World, chunks (Anvil), blocks, items, lighting
│   └── oxidized-game/       # Entities, AI, combat, commands, crafting
├── mc-server-ref/           # Decompiled vanilla server (reference only, gitignored)
├── deny.toml                # cargo-deny licence + advisory config
├── rustfmt.toml             # Formatting rules
└── .clippy.toml             # Lint rules
```

---

## Quick Start

### Requirements

- Rust stable (see `rust-toolchain.toml`)
- A vanilla Minecraft 26.1 client

### Build

```bash
cargo build --release
```

### Run

```bash
./target/release/oxidized
```

The server creates `server.properties` on first run. Edit it, then restart.

### Development build

```bash
cargo run
```

### Tests

```bash
cargo test --workspace
```

### Lints

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## Configuration (`server.properties`)

Key options (full list generated on first run):

```properties
server-ip=
server-port=25565
online-mode=true
motd=An Oxidized Server
max-players=20
view-distance=10
simulation-distance=10
difficulty=normal
gamemode=survival
hardcore=false
level-name=world
enable-rcon=false
rcon.port=25575
rcon.password=
enable-query=false
compression-threshold=256
```

---

## Reference Code

The decompiled vanilla server is at `mc-server-ref/decompiled/` (gitignored — run
`scripts/decompile.sh` to regenerate it).  The implementation follows the Java source
closely but uses idiomatic Rust at all times.

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
