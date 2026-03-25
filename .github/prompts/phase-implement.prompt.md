---
description: 'Implement a phase from its document following the development lifecycle.'
---

# Phase Implementation

Implement the phase described in the given phase document, following the project's development lifecycle and TDD practices.

## Input

The phase document to implement: ${{ input }}

## Instructions

### 1. Preparation

- **Read the phase document** — understand every task, dependency, and acceptance criterion.
- **Read linked ADRs** — follow all architecture decisions referenced in the phase doc.
- **Arch Review Gate** — before coding, question every constraining ADR. If outdated, create a superseding ADR first.
- **Read the Java reference** — for each task involving game logic, read the equivalent vanilla Java class in `mc-server-ref/decompiled/net/minecraft/`. Understand the algorithm, then rewrite idiomatically in Rust — never transliterate.

### 2. Planning

- Create SQL todos for each task in the phase document, with dependencies.
- Order tasks by dependency chain — implement foundations first (types, components, structs), then systems, then wiring, then tests.
- Confirm the plan with the user before starting implementation.

### 3. Implementation (per task)

Follow the TDD cycle for each task:

1. **Write failing test** — unit, integration, or property-based as appropriate.
2. **Confirm failure** — run the test to see it fail (not a compile error).
3. **Implement minimum** to pass the test.
4. **Confirm green** — `cargo test -p <crate>`.
5. **Refactor** — clean up, then re-run tests.
6. **Check workspace** — `cargo check --workspace` after each task.

Update SQL todo status as you progress (`in_progress` → `done`).

### 4. Wiring & Integration

- Wire new modules into the crate's `mod.rs` / `lib.rs`.
- Connect systems to the tick loop or ECS schedules as specified.
- Verify cross-crate imports respect the dependency hierarchy (lower crates never import higher ones).

### 5. Verification

- Run full test suite: `cargo test --workspace`.
- Grep for stale references after any renames: `grep -r "old_name" . --include="*.rs" --include="*.toml" --include="*.md"`.
- Verify no `unwrap()`/`expect()` in production code.
- Verify `///` docs on all new public items.

### 6. Commit

- Stage and commit with conventional commit format: `feat(<scope>): <description>`.
- One commit per logical unit of work, or one commit for the whole phase if cohesive.

## Standards

- `#![warn(missing_docs)]` on library crates, `#![deny(unsafe_code)]` unless justified.
- Errors: `thiserror` in libraries, `anyhow` in binary. Use `?` + `.context()`.
- No magic numbers — constants in a `constants` module or inline `const`.
- Test naming: `test_<thing>_<condition>` or `<thing>_<outcome>_when_<condition>`.
- `#[allow(clippy::unwrap_used, clippy::expect_used)]` only in test modules.

## What NOT to Do

- Don't implement tasks from other phases — note them as deferred if encountered.
- Don't refactor unrelated code unless it blocks your implementation.
- Don't add dependencies not specified in the phase doc or ADRs without asking.
- Don't skip tests — minimum per task is unit + integration + property-based (for parsers/codecs).
