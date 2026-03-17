# Contributing to Oxidized

Thank you for your interest in contributing! This document explains the process.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [How to Contribute](#how-to-contribute)
- [Development Setup](#development-setup)
- [Architecture](#architecture)
- [Commit Style](#commit-style)
- [Pull Request Process](#pull-request-process)
- [Testing](#testing)

---

## Code of Conduct

This project follows the [Contributor Covenant](./CODE_OF_CONDUCT.md).
Be respectful and constructive.

---

## How to Contribute

| Type | How |
|---|---|
| 🐛 Bug report | Open a [bug report issue](.github/ISSUE_TEMPLATE/bug_report.yml) |
| 💡 Feature request | Open a [feature request issue](.github/ISSUE_TEMPLATE/feature_request.yml) |
| 📖 Docs | Edit any `.md` file and open a PR |
| 🧩 Implementation | Pick a phase from [PHASES.md](./PHASES.md) and open a PR |
| 🔍 Review | Review open PRs and leave constructive feedback |

---

## Development Setup

```bash
# 1. Fork and clone
git clone https://github.com/your-org/oxidized
cd oxidized

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

The workspace has five crates — keep concerns separated:

| Crate | Responsibility | Must NOT depend on |
|---|---|---|
| `oxidized-nbt` | NBT binary format | all other oxidized crates |
| `oxidized-protocol` | Packet codec, connection state | `oxidized-world`, `oxidized-game` |
| `oxidized-world` | Chunks, blocks, Anvil I/O | `oxidized-protocol`, `oxidized-game` |
| `oxidized-game` | Entities, AI, commands | — |
| `oxidized-server` | Server bootstrap, tick loop | — (depends on all) |

**Reference code:** The decompiled vanilla server lives in `mc-server-ref/decompiled/`
(gitignored). When implementing something, always check the Java reference first.
**Rewrite idiomatically in Rust — do not transliterate Java line-by-line.**

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

Use the crate name as scope: `nbt`, `protocol`, `world`, `game`, `server`.
Use `ci` for workflow files.

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
2. Make your changes, commit with conventional commits
3. Ensure CI passes: `cargo test --workspace && cargo clippy --workspace -- -D warnings`
4. Open a PR targeting `main`; fill in the PR template
5. At least one approving review is required before merge
6. Squash-merge preferred for feature branches; merge commit for phase completions

---

## Testing

- **Unit tests** live next to the code in `#[cfg(test)]` modules
- **Integration tests** live in `crates/<name>/tests/`
- All public API must have at least one test
- Use `#[should_panic]` or `Result`-returning tests — no `unwrap()` in tests
- Reference the Java behaviour when writing expected values:
  ```rust
  // VarInt encoding mirrors net.minecraft.network.VarInt in the reference
  assert_eq!(encode_varint(300), &[0xAC, 0x02]);
  ```

Run with nextest for faster feedback:
```bash
cargo nextest run --workspace
```
