# Contributing to Oxidized

Thank you for your interest in contributing! This document explains the process.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Development Lifecycle](#development-lifecycle)
- [How to Contribute](#how-to-contribute)
- [Development Setup](#development-setup)
- [Architecture](#architecture)
- [Design Principles](#design-principles)
- [Commit Style](#commit-style)
- [Pull Request Process](#pull-request-process)
- [Testing](#testing)
- [Release Process](#release-process)
- [Continuous Improvement](#continuous-improvement)

---

## Code of Conduct

This project follows the [Contributor Covenant](./CODE_OF_CONDUCT.md).
Be respectful and constructive.

---

## Development Lifecycle

Every change in Oxidized follows a structured lifecycle:

```
Identify → Research → Decide → Plan → Test First → Implement → Review → Integrate → Retrospect
```

The lifecycle ensures that we:

- Understand the problem before writing code
- Write tests before implementation (TDD)
- Actively identify improvements during review

For trivial changes (typo fixes, dependency bumps), an abbreviated lifecycle applies.

---

## How to Contribute

| Type | How |
|---|---|
| 🐛 Bug report | Open a [bug report issue](.github/ISSUE_TEMPLATE/bug_report.yml) |
| 💡 Feature request | Open a [feature request issue](.github/ISSUE_TEMPLATE/feature_request.yml) |
| 📖 Docs | Edit any `.md` file and open a PR |
| 🧩 Implementation | Pick an open issue and open a PR |
| 🔍 Review | Review open PRs and leave constructive feedback |
| 💡 Improvement | Found a better approach? Open an issue or PR |

---

## Development Setup

```bash
# 1. Fork and clone
git clone https://github.com/dodoflix/Oxidized.git
cd Oxidized

# 2. Rust stable (toolchain pinned via rust-toolchain.toml)
rustup update stable

# 3. Build
cargo build

# 4. Run tests
cargo test --workspace

# 5. Check formatting and lints
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

### Useful tools (optional)

```bash
cargo install cargo-deny    # licence + advisory checks
cargo install cargo-nextest # faster test runner
cargo install cargo-watch   # auto-rebuild on save
```

---

## Architecture

The workspace has six crates — keep concerns separated:

| Crate | Responsibility | Must NOT depend on |
|---|---|---|
| `oxidized-nbt` | NBT binary format | all other oxidized crates |
| `oxidized-macros` | Proc-macro derives | all other oxidized crates |
| `oxidized-protocol` | Packet codec, connection state | `oxidized-world`, `oxidized-game` |
| `oxidized-world` | Chunks, blocks, Anvil I/O | `oxidized-protocol`, `oxidized-game` |
| `oxidized-game` | Entities, AI, commands | — |
| `oxidized-server` | Server bootstrap, tick loop | — (depends on all) |

**Reference code:** The decompiled vanilla server lives in `mc-server-ref/decompiled/`
(gitignored). When implementing something, always check the Java reference first.
**Rewrite idiomatically in Rust — do not transliterate Java line-by-line.**

---

## Design Principles

Key principle: **the wire protocol is sacred** (we can't change what the client
sends or expects), but everything server-side uses modern, data-oriented Rust
design rather than cloning vanilla Java patterns.

---

## Commit Style

All commits **must** follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <short description>
```

### Types

| Type | When | Version bump |
|---|---|---|
| `feat` | New user-visible feature | Minor |
| `fix` | Bug fix | Patch |
| `perf` | Performance improvement | Patch |
| `refactor` | Restructure, no behaviour change | None |
| `test` | Tests only | None |
| `docs` | Documentation only | None |
| `chore` | Dependencies, CI, tooling | None |
| `ci` | CI/CD workflow changes | None |

### Scopes

Use the crate name as scope: `nbt`, `macros`, `protocol`, `world`, `game`, `server`.
Use `ci` for workflow files, `deps` for dependency updates.

### Examples

```
feat(protocol): implement VarInt read/write
fix(world): correct PalettedContainer bit packing for edge case
perf(game): cache entity bounding boxes to avoid recomputation
test(nbt): add round-trip tests for all 13 tag types
chore(deps): bump tokio from 1.43 to 1.44
```

**Breaking changes:** add `!` after type or add `BREAKING CHANGE:` footer.

---

## Pull Request Process

1. Create a branch: `git checkout -b feat/varint-codec`
2. Make your changes following the lifecycle
3. Commit with conventional commits
4. Open a PR targeting `main`; fill in the PR template completely
5. At least one approving review is required before merge
6. Squash-merge preferred for feature branches

### PR Review Standards

Reviews are not just about catching bugs — they actively seek improvements:
- Does the code follow the project's design principles?
- Could any existing pattern be improved?
- Are there learnings to record?

---

## Testing

- **Unit tests** live next to the code in `#[cfg(test)]` modules
- **Integration tests** live in `crates/<name>/tests/`
- All public API must have at least one test
- Test modules use `#[allow(clippy::unwrap_used, clippy::expect_used)]` for assertion-like code
- Reference the Java behaviour when writing expected values:
  ```rust
  // VarInt encoding mirrors net.minecraft.network.VarInt in the reference
  assert_eq!(encode_varint(300), &[0xAC, 0x02]);
  ```

Run with nextest for faster feedback:
```bash
cargo nextest run --workspace
```

### Benchmarks

[Criterion](https://github.com/bheisler/criterion.rs) benchmarks live in
`crates/<crate>/benches/`. Run the full suite:

```bash
cargo bench --workspace
```

Run a single crate's benchmarks:

```bash
cargo bench -p oxidized-nbt
cargo bench -p oxidized-protocol
cargo bench -p oxidized-world
cargo bench -p oxidized-game
```

Benchmark results are written to `target/criterion/` with HTML reports.

### Fuzz Testing

Fuzz targets live in `crates/<crate>/fuzz/` and use
[cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) (requires nightly).

Install cargo-fuzz:

```bash
cargo install cargo-fuzz
```

List available targets for a crate:

```bash
cd crates/oxidized-nbt && cargo +nightly fuzz list
cd crates/oxidized-protocol && cargo +nightly fuzz list
cd crates/oxidized-world && cargo +nightly fuzz list
```

Run a fuzz target (runs until stopped with Ctrl-C):

```bash
cd crates/oxidized-nbt && cargo +nightly fuzz run fuzz_nbt_read
cd crates/oxidized-protocol && cargo +nightly fuzz run fuzz_varint
cd crates/oxidized-protocol && cargo +nightly fuzz run fuzz_packet_decode
cd crates/oxidized-world && cargo +nightly fuzz run fuzz_paletted_container
```

Crash artifacts are stored in `fuzz/artifacts/`. If a crash is found, add a
regression test in the corresponding crate's `tests/` directory.

---

## Release Process

Oxidized uses automated versioning and release management based on conventional commits.
See the release strategy in the commit history for the full design rationale.

### How It Works

1. **Commit with conventional prefixes** — `feat`, `fix`, `perf`, etc.
2. **release-please** automatically creates and maintains a "Release PR" on GitHub that
   accumulates changes and proposes the next version bump.
3. When a maintainer merges the Release PR, a git tag (`v0.X.Y`) and GitHub Release are
   created automatically.
4. A build pipeline then compiles cross-platform binaries and attaches them to the release.

### Development (Nightly) Releases

Every push to `main` that passes CI automatically publishes a **nightly pre-release** with
cross-platform binaries. These are tagged `nightly` and are always overwritten with the
latest build.

### Version Bump Rules

| Commit prefix | Bump |
|--------------|------|
| `feat!:` or `BREAKING CHANGE:` footer | Minor (pre-1.0) / Major (post-1.0) |
| `feat(scope):` | Minor |
| `fix(scope):`, `perf(scope):` | Patch |
| `refactor`, `test`, `docs`, `chore`, `ci` | No version bump |

### Binary Targets

| Platform | Archive |
|----------|---------|
| Linux x86_64 (glibc) | `.tar.gz` |
| Linux x86_64 (musl/static) | `.tar.gz` |
| Windows x86_64 | `.zip` |
| macOS Intel | `.tar.gz` |
| macOS Apple Silicon | `.tar.gz` |

---

## Continuous Improvement

We believe the codebase should always be getting better.

**After every milestone:**
- Conduct a retrospective
- Record learnings and identify improvements
- Record any technical debt incurred

**During every PR review:**
- Identify outdated patterns or decisions
- Look for patterns that should be extracted or formalized
- Suggest improvements (not just catch bugs)

**When you find something better:**
- Don't just note it — act on it
- Open an issue or PR
- Plan and execute the refactoring
