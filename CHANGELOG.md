# Changelog

All notable changes to Oxidized will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- Cargo workspace with five crates: `oxidized-server`, `oxidized-protocol`,
  `oxidized-nbt`, `oxidized-world`, `oxidized-game`
- Repository scaffolding: README, licenses (MIT/Apache-2.0), CONTRIBUTING,
  CODE_OF_CONDUCT, SECURITY, CI workflow
- Rust tooling: `rustfmt.toml`, `deny.toml`, `rust-toolchain.toml`
- Decompiled Minecraft 26.1-pre-3 reference (`mc-server-ref/decompiled/`, 4 789 files)
- 38 detailed implementation phase documents (`docs/phases/`)
- 32 Architecture Decision Records (`docs/adr/`)
- Architecture documentation: system overview, crate layout, protocol, world format,
  entity system (`docs/architecture/`)
- Reference documentation: Java class map (110+ mappings), binary format specs
  (`docs/reference/`)
- GitHub issue templates (bug report, feature request, question)
- Pull request template
- Dependabot configuration for Cargo and GitHub Actions

[Unreleased]: https://github.com/dodoflix/Oxidized/commits/main
