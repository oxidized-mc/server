# ADR-035: Module Structure & File Size Policy

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-19 |
| Phases | R1 (Refactoring), All future phases |
| Deciders | Oxidized Core Team |

## Context

After completing Phase 18, an architectural review of the entire codebase revealed a
pattern of growing "god files" — single source files accumulating multiple
responsibilities well beyond what is comfortable to navigate, test, or extend. The worst
offenders:

| File | LOC | Responsibilities | Symptom |
|------|-----|------------------|---------|
| `oxidized-server/src/network.rs` | 2079 | 6 (listener, 4 state handlers, auth, chat, commands, helpers) | 715-line function, 433-line if-else chain |
| `oxidized-protocol/src/chat/component.rs` | 1439 | 6 (structs, builders, display, JSON serde, NBT serde, legacy) | Same data serialized 3 ways with copy-pasted match blocks |
| `oxidized-game/src/commands/context.rs` | 642 | 4 (tokenizer, arg parsing, getters, types) | 155-line match, 13 copy-pasted getter functions |

These files are difficult to review in PRs (too much context required), prone to merge
conflicts (multiple features touch the same file), and intimidating for new contributors.
Additionally, the codebase has no documented policy on when to split a file, leading to
inconsistent structure across crates.

Rust's module system makes splitting straightforward — a file `foo.rs` can become
`foo/mod.rs` + `foo/bar.rs` + `foo/baz.rs` without changing any public API or import paths
for callers (they still `use crate::foo::Thing`).

## Decision Drivers

- **Cognitive load**: a developer should be able to understand a file's purpose by reading
  its first 20 lines, not by scrolling through 2000
- **PR reviewability**: changes to one concern should not require reviewing unrelated code
  in the same file
- **Merge conflict avoidance**: independent features touching different concerns should
  rarely conflict
- **Discoverability**: file names should guide developers to the right place
- **Test locality**: tests for a concern should live near that concern's implementation
- **No over-splitting**: splitting a 200-line file into 5 files of 40 lines each adds
  navigation friction without meaningful benefit

## Considered Options

### Option 1: No policy — let files grow organically

Continue the current approach. Developers add code to the file where it logically belongs,
and files grow until someone decides to split. This is simple but leads to the exact
problems we're observing — the decision to split is always deferred because "it works fine"
until it doesn't.

### Option 2: Strict LOC limit (e.g., 300 lines max)

Enforce a hard line-count limit via CI. This prevents god files but creates pressure to
split prematurely, leading to fragmented modules with artificial boundaries. A 350-line
file implementing a single cohesive parser should not be forced to split just because of a
line count.

### Option 3: Responsibility-based splitting with soft LOC guideline

Establish a soft guideline (~500 LOC excluding tests) combined with hard rules based on
responsibility count and control-flow complexity. Files exceeding the guideline trigger a
review, not an automatic split requirement. The decision to split is based on
**responsibilities** (number of distinct concerns) and **complexity** (match/if-else arm
count), not raw line count.

### Option 4: One file per type/trait

Every struct, enum, and trait gets its own file. This is common in Java/C# but unidiomatic
in Rust, where a module typically groups related types. It also creates excessive file
counts and import chains.

## Decision

**We adopt responsibility-based splitting with soft LOC guidelines (Option 3).**

### File Size Guidelines

| Metric | Guideline | Action |
|--------|-----------|--------|
| **LOC (excluding tests)** | ≤ 500 | Soft guideline — review if exceeded |
| **LOC (excluding tests)** | > 800 | Must split unless single-concern justification in PR |
| **Distinct responsibilities** | ≤ 2 | No split needed |
| **Distinct responsibilities** | ≥ 3 | Must split into submodules |
| **Match/if-else arms** | ≤ 15 in one block | Acceptable |
| **Match/if-else arms** | > 20 in one block | Extract to dispatch table, trait method, or helper module |
| **Single function LOC** | > 100 | Must refactor into smaller functions |
| **Single function LOC** | > 50 | Review — likely should split |

### Splitting Conventions

**When a file `foo.rs` needs splitting:**

1. Create `foo/mod.rs` — contains the primary types, re-exports submodule items
2. Create `foo/<concern>.rs` — one file per secondary responsibility
3. Keep tests with their concern — tests for JSON serialization go in the JSON module
4. Re-export public items from `mod.rs` so callers don't change their imports

**Module naming for serialization-heavy types:**

When a type has multiple serialization formats, each format gets its own file:

```
src/chat/
├── component.rs         # Data structures, builders, Display
├── component_json.rs    # JSON Serialize + Deserialize
├── component_nbt.rs     # NBT encoding + decoding
└── style.rs             # Style struct + event types (already exists)
```

Alternative (directory style — use when there are 4+ submodules):

```
src/network/
├── mod.rs               # Listener, shared types
├── handshake.rs         # Handshaking state handler
├── status.rs            # Status state handler
├── login.rs             # Login state handler + authentication
├── configuration.rs     # Configuration state handler
└── play/                # Play state (further split by concern)
    ├── mod.rs           # Play loop, keepalive
    ├── movement.rs      # Movement + chunk tracking
    ├── chat.rs          # Chat messages + rate limiting
    └── commands.rs      # Command dispatch + suggestions
```

### Responsibility Identification

A "responsibility" is a distinct concern that could be understood, tested, or modified
independently. Examples:

- Parsing and formatting are 2 responsibilities (even if they operate on the same type)
- JSON serialization and NBT serialization are 2 responsibilities
- TCP listening and packet handling are 2 responsibilities
- Builder methods and the struct definition are 1 responsibility (tightly coupled)
- Tests are not counted as a responsibility (they mirror the impl)

### When NOT to Split

- **Trait implementations that need private access** — a `Serialize` impl that accesses
  private fields must stay in the same module as the struct (or use `pub(crate)`)
- **Small files** — don't split a 200-line file just because it has 3 `impl` blocks
- **Tightly coupled types** — if two types are always modified together, keep them together
- **Generated code** — build.rs output or macro-generated code may be large but is not
  human-maintained

## Consequences

### Positive

- Consistent structure across the codebase — new contributors know what to expect
- PR reviews are focused — changes to chat serialization don't require reviewing the
  component builder code
- Merge conflicts are reduced — concurrent features touching different handlers work on
  different files
- Discoverability improves — `network/login.rs` is self-documenting
- Test locality — tests live near the code they test, making it easy to find and run them

### Negative

- Splitting existing files requires careful refactoring — moving code changes import paths
  for `pub(crate)` items
- More files to navigate — though IDE "go to definition" mitigates this entirely
- Initial cost of splitting existing god files — but this is a one-time investment

### Neutral

- The soft LOC guideline may cause occasional discussion in PRs about whether a split is
  warranted — this is healthy architectural discussion
- Tests can stay in the original file or move to the submodule — either is acceptable as
  long as coverage is maintained

## Compliance

- **Code review gate**: PRs adding >100 LOC to a file already >500 LOC must include a
  comment justifying why a split was not done, or include the split
- **Existing violations**: the files identified in the Context section are grandfathered
  but must be split in the R1 refactoring phase
- **New files**: all new files must follow this policy from the date of acceptance
- **Metric tracking**: periodically run `find crates -name "*.rs" -exec wc -l {} + | sort -rn`
  to monitor file size trends

## Related ADRs

- [ADR-003: Crate Workspace Architecture](adr-003-crate-architecture.md) — crate-level
  structure; this ADR covers within-crate module structure
- [ADR-036: Packet Handler Architecture](adr-036-packet-handler-architecture.md) — specific
  application of this policy to the network layer
- [ADR-034: Comprehensive Testing Strategy](adr-034-testing-strategy.md) — test placement
  follows the "tests near code" principle from this ADR

## References

- [Rust API Guidelines — Module organization](https://rust-lang.github.io/api-guidelines/)
- [Matklad — "Large Rust Workspaces"](https://matklad.github.io/2021/09/04/fast-rust-builds.html)
- [Steve Klabnik — "Structuring Rust Projects"](https://doc.rust-lang.org/book/ch07-00-managing-growing-projects-with-packages-crates-and-modules.html)
