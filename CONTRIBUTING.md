# Contributing to Oxidized

Thank you for your interest in contributing! This document explains the process.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Development Lifecycle](#development-lifecycle)
- [How to Contribute](#how-to-contribute)
- [Development Setup](#development-setup)
- [Architecture](#architecture)
- [Design Decisions (ADRs)](#design-decisions-adrs)
- [Commit Style](#commit-style)
- [Pull Request Process](#pull-request-process)
- [Testing](#testing)
- [Continuous Improvement](#continuous-improvement)

---

## Code of Conduct

This project follows the [Contributor Covenant](./CODE_OF_CONDUCT.md).
Be respectful and constructive.

---

## Development Lifecycle

Every change in Oxidized follows a structured
[Development Lifecycle](./docs/lifecycle/README.md) with 9 stages:

```
Identify → Research → Decide (ADR) → Plan → Test First → Implement → Review → Integrate → Retrospect
```

Each stage has explicit [Quality Gates](./docs/lifecycle/quality-gates.md) that must
be satisfied before advancing. The lifecycle ensures that we:

- Understand the problem before writing code
- Record significant decisions as ADRs
- Write tests before implementation (TDD)
- Actively identify improvements during review
- Capture learnings in [persistent memories](.github/memories.md)

For trivial changes (typo fixes, dependency bumps), an abbreviated lifecycle applies —
see [Lifecycle Variants](./docs/lifecycle/README.md#lifecycle-variants).

---

## How to Contribute

| Type | How |
|---|---|
| 🐛 Bug report | Open a [bug report issue](.github/ISSUE_TEMPLATE/bug_report.yml) |
| 💡 Feature request | Open a [feature request issue](.github/ISSUE_TEMPLATE/feature_request.yml) |
| 📖 Docs | Edit any `.md` file and open a PR |
| 🧩 Implementation | Pick a phase from the [roadmap](./docs/phases/README.md) and open a PR |
| 🔍 Review | Review open PRs and leave constructive feedback |
| 💡 Improvement | Found a better approach? Propose a new ADR or update [memories](.github/memories.md) |

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

For detailed architecture docs, see [`docs/architecture/`](./docs/architecture/).

---

## Design Decisions (ADRs)

All significant design decisions are documented as
[Architecture Decision Records](./docs/adr/README.md). Before implementing any
phase, read the ADRs linked in that phase's doc file.

Key principle: **the wire protocol is sacred** (we can't change what the client
sends or expects), but everything server-side uses modern, data-oriented Rust
design rather than cloning vanilla Java patterns.

### ADR Evolution

ADRs are living knowledge. When you discover a better approach:
1. Create a new ADR that supersedes the old one
2. Mark the old ADR as "Superseded by ADR-NNN"
3. Plan the migration of existing code

See [Continuous Improvement](./docs/lifecycle/continuous-improvement.md#adr-evolution)
for the full process.

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
2. Make your changes, following the [Development Lifecycle](./docs/lifecycle/README.md)
3. Ensure all [Quality Gates](./docs/lifecycle/quality-gates.md) pass
4. Commit with conventional commits
5. Open a PR targeting `main`; fill in the PR template completely
6. At least one approving review is required before merge
7. Squash-merge preferred for feature branches; merge commit for phase completions

### PR Review Standards

Reviews are not just about catching bugs — they actively seek improvements:
- Does the code follow relevant ADRs?
- Are any ADRs outdated given this change?
- Could any existing pattern be improved?
- Are there learnings to record in [memories.md](.github/memories.md)?

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

---

## Continuous Improvement

We believe the codebase should always be getting better. See the full
[Continuous Improvement](./docs/lifecycle/continuous-improvement.md) process for details.

**After every phase:**
- Conduct a retrospective
- Update [persistent memories](.github/memories.md) with learnings
- Review ADRs for accuracy
- Record any technical debt incurred

**During every PR review:**
- Check for ADR compliance and identify outdated decisions
- Look for patterns that should be extracted or formalized
- Suggest improvements (not just catch bugs)

**When you find something better:**
- Don't just note it — act on it
- Create a new ADR if needed
- Plan and execute the refactoring
- Time is never a reason to skip improvement
