# Changelog

All notable changes to Oxidized will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- Cargo workspace with six crates: `oxidized-server`, `oxidized-protocol`,
  `oxidized-nbt`, `oxidized-world`, `oxidized-game`, `oxidized-macros`
- Repository scaffolding: README, licenses (MIT/Apache-2.0), CONTRIBUTING,
  CODE_OF_CONDUCT, SECURITY, CI workflow
- Rust tooling: `rustfmt.toml`, `deny.toml`, `rust-toolchain.toml`
- Decompiled Minecraft 26.1-pre-3 reference (`mc-server-ref/decompiled/`, 4 789 files)
- 38 detailed implementation phase documents
- 34 Architecture Decision Records
- Architecture documentation: system overview, crate layout, protocol, world format,
  entity system — aligned with all ADRs
- Reference documentation: Java class map (110+ mappings), binary format specs,
  protocol packet listing
- Server binary bootstrap: mimalloc global allocator, Tokio runtime, structured logging
  (Phase 1)
- TOML-based server configuration with full type safety and serde derives (Phase 1)
- GitHub issue templates (bug report, feature request, question)
- Pull request template
- Dependabot configuration for Cargo and GitHub Actions
- Development lifecycle with 9 stages, 5 feedback loops, and quality gates
- TCP listener with raw packet framing and VarInt/VarLong codec (Phase 2)
- Connection struct with protocol state tracking (Phase 2)
- Handshake + Status protocol — server appears in Minecraft multiplayer list with
  correct MOTD, version, and player count (Phase 3)
- Protocol dispatch for Handshaking and Status connection states (Phase 3)
- Wire type helpers: String, u16, i64 read/write for packet codec (Phase 3)
- Login authentication with Mojang session server (online mode) and offline UUID
  derivation (Phase 4)
- RSA-1024 key exchange and AES-128-CFB8 stream encryption (Phase 4)
- Zlib compression with configurable threshold (Phase 4)
- Login packet structs: Hello, Key, Compression, LoginFinished, Disconnect (Phase 4)
- Full encrypted + compressed connection pipeline with transparent I/O (Phase 4)
- Complete NBT library: all 13 tag types, binary codec, Modified UTF-8, NbtAccounter,
  GZIP/zlib I/O, SNBT parser+formatter, serde integration (Phase 5)

### Changed
- Configuration format from Java `.properties` to TOML

### Security
- URL-encode all query parameters in Mojang session authentication (Phase 4)
- Replaced deprecated `rustsec/audit-check@v2` CI action with direct `cargo-audit`

[Unreleased]: https://github.com/oxidized-mc/server/commits/main
