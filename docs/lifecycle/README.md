# Development Lifecycle

> This document defines the complete development lifecycle for Oxidized. Every change —
> from a single-line fix to a multi-phase feature — follows this lifecycle. The lifecycle
> is designed to produce flawless, production-grade code through systematic quality gates,
> continuous improvement, and institutional memory.

---

## Table of Contents

- [Philosophy](#philosophy)
- [Lifecycle Overview](#lifecycle-overview)
- [Stage 1 — Identify](#stage-1--identify)
- [Stage 2 — Research](#stage-2--research)
- [Stage 3 — Decide (ADR)](#stage-3--decide-adr)
- [Stage 4 — Plan](#stage-4--plan)
- [Stage 5 — Test First (TDD Red)](#stage-5--test-first-tdd-red)
- [Stage 6 — Implement (TDD Green)](#stage-6--implement-tdd-green)
- [Stage 7 — Review](#stage-7--review)
- [Stage 8 — Integrate](#stage-8--integrate)
- [Stage 9 — Retrospect](#stage-9--retrospect)
- [Lifecycle Variants](#lifecycle-variants)
- [Cross-References](#cross-references)

---

## Philosophy

**Perfection is not a destination; it is a discipline.** Every piece of code committed to
Oxidized must be the best we can produce at the time — and we must always be willing to
improve it when we learn more. Time is never an excuse for cutting corners.

### Core Principles

1. **Never ship knowingly imperfect work.** If you see a problem, fix it now — not later.
2. **Every decision is recorded.** Architecture Decision Records (ADRs) capture the *why*
   behind every significant choice, so future developers (and future you) can understand
   the reasoning and know when to revisit it.
3. **Memory is institutional, not personal.** Patterns, gotchas, and learnings are written
   down in [persistent memories](../../.github/memories.md) so they survive across sessions,
   contributors, and time.
4. **Improvement is continuous.** After every phase, we retrospect. If an ADR is outdated,
   we supersede it. If a pattern is suboptimal, we refactor. The codebase is a living thing
   that always moves toward better.
5. **The wire protocol is sacred; everything else is malleable.** We cannot change what
   the Minecraft client sends or expects. But every server-side design choice is ours to
   make — and we choose modern, scalable, robust approaches over 1:1 Java clones.

---

## Lifecycle Overview

Every change flows through these 9 stages. Some stages are skipped for trivial changes
(see [Lifecycle Variants](#lifecycle-variants)), but the full lifecycle always applies to
non-trivial work.

```
┌──────────────────────────────────────────────────────────────────────┐
│                                                                      │
│  ① IDENTIFY → ② RESEARCH → ③ DECIDE → ④ PLAN → ⑤ TEST FIRST       │
│                                              │                       │
│                                              ↓                       │
│  ⑨ RETROSPECT ← ⑧ INTEGRATE ← ⑦ REVIEW ← ⑥ IMPLEMENT             │
│       │                                                              │
│       └──→ (feed improvements back into ① for next cycle)           │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

---

## Stage 1 — Identify

**Purpose:** Recognize what needs to be done and why.

**Activities:**
- Read the relevant phase document (`docs/phases/phase-NN-*.md`)
- Understand the user-facing goal (what does the player/operator experience?)
- Check the Java reference (`mc-server-ref/decompiled/`) to understand vanilla behavior
- Identify which crates and modules are affected
- Check [persistent memories](../../.github/memories.md) for relevant prior learnings

**Quality Gate:** Can you articulate in one sentence what this change does and why it matters?

**Output:** A clear problem statement — written in the session plan or PR description.

---

## Stage 2 — Research

**Purpose:** Understand the problem space deeply before committing to a solution.

**Activities:**
- Read the decompiled Java source for the relevant classes (always listed in phase docs)
- Read related ADRs (every phase doc links its relevant ADRs)
- Search the codebase for existing patterns that apply
- Research Rust ecosystem best practices (crates, patterns, prior art)
- Check if any existing ADRs are outdated given new information
- Identify multiple possible approaches (at least 2 for non-trivial decisions)

**Quality Gate:** You can explain:
1. How vanilla Java implements this
2. How we plan to do it differently (and why)
3. What alternatives you considered

**Output:** Research notes — either in the session plan or as an ADR context section.

---

## Stage 3 — Decide (ADR)

**Purpose:** Record significant decisions so they can be understood and revisited.

**When Required:**
- Adding a new crate, public trait, or module boundary
- Choosing between multiple valid approaches for a non-trivial system
- Making a decision that would be expensive to reverse
- **Superseding an existing ADR** because we found a better approach

**When Skipped:**
- Bug fixes with obvious correct solutions
- Mechanical changes (dependency bumps, formatting)
- Implementation that follows an existing accepted ADR

**Activities:**
- Write a new ADR following the template in `docs/adr/`
- Document context, all options considered (minimum 3 for major decisions), decision, and consequences
- If superseding an existing ADR, add "**Status: Superseded by [ADR-NNN](adr-NNN-*.md)**"
  to the old ADR and explain what changed in the new one
- Link the ADR from the relevant phase document

**Quality Gate:**
- Does the ADR explain the decision to someone who wasn't in the room?
- Are consequences (positive, negative, neutral) all documented?
- Is the ADR linked from the relevant phase docs?

**Output:** An accepted ADR in `docs/adr/adr-NNN-*.md`.

---

## Stage 4 — Plan

**Purpose:** Break work into small, ordered, testable units before touching code.

**Activities:**
- Write a task breakdown with clear deliverables for each unit
- Identify dependencies between tasks
- Register tasks in the SQL `todos` table with dependencies
- Confirm the plan with the user before proceeding (if scope is large)
- Estimate which existing tests need updating

**Quality Gate:**
- Every task has a clear "done" definition
- Dependencies are explicit (no hidden ordering assumptions)
- Tasks are small enough to complete and verify independently

**Output:** Updated session plan + SQL todos with dependencies.

---

## Stage 5 — Test First (TDD Red)

**Purpose:** Define expected behavior before writing any implementation.

**Activities:**
- Write failing tests for the new behavior — one test per requirement
- Use parameterized tests (`#[test_case]` or custom macros) for multiple input combinations
- Use descriptive test names: `test_<thing>_<outcome>_when_<condition>`
- Run `cargo test -p <crate>` — confirm the test **fails** (not compile-error)
- Never skip this step — a test that never fails proves nothing

**Quality Gate:**
- Tests are written and fail for the right reason (not compilation error)
- Test names clearly describe what they verify
- Edge cases are covered (empty inputs, boundaries, error paths)

**Output:** Failing tests committed or staged with the implementation.

---

## Stage 6 — Implement (TDD Green)

**Purpose:** Write the minimum correct implementation.

#### 6.0 — Question Assumptions

Before writing any code, review the ADRs that govern this phase and explicitly ask:
- Is this still the best approach given what we know now?
- Are we replicating a Java/vanilla pattern that has a better Rust-native alternative?
- Has the ecosystem changed since the ADR was written?

If the answer to any question is "yes, there's a better way," pause implementation and create a superseding ADR first. This step prevents accumulating technical debt from outdated decisions.

**Activities:**
- Implement the minimum code to make failing tests pass
- Follow all coding standards (see [copilot-instructions](../../.github/copilot-instructions.md))
- Check Java reference for algorithm correctness, but write idiomatic Rust
- Run `cargo test -p <crate>` — confirm **green**
- Run `cargo clippy --workspace --all-targets -- -D warnings` — zero warnings
- Run `cargo fmt --check` — no formatting issues
- Refactor if needed, re-run tests after every refactor

**Quality Gate:**
- All tests pass
- Clippy clean (zero warnings with `-D warnings`)
- Formatted (`cargo fmt --check`)
- No `unwrap()`/`expect()` in production code
- All public items have `///` doc comments
- No hardcoded magic numbers — use named constants

**Output:** Working implementation with all tests green.

---

## Stage 7 — Review

**Purpose:** Catch issues before they enter the codebase, and identify improvement opportunities.

**This is the most important stage.** Reviews are not just about catching bugs — they are
about ensuring the codebase is always moving toward better.

### Pre-Commit Review (every change)

Run the `code-review` agent on staged changes. The review checks:

1. **Correctness:** Does the code do what it claims? Are edge cases handled?
2. **ADR Compliance:** Does the implementation follow the decisions in relevant ADRs?
3. **Pattern Consistency:** Does the code follow established patterns in the codebase?
4. **Stale References:** Are there any dangling references to renamed/moved items?
5. **Documentation:** Are public APIs documented? Are complex algorithms explained?

### Improvement Identification (every change)

During review, actively look for:

1. **Outdated ADRs:** Has this implementation revealed that an existing ADR's assumptions
   are wrong, or that a better approach exists? → Create a new superseding ADR.
2. **Pattern Improvements:** Could an existing pattern be improved based on what we've
   learned? → Record in [memories](../../.github/memories.md) and plan a refactor.
3. **Missing Tests:** Are there untested paths that should be covered?
4. **Performance Opportunities:** Are there obvious performance improvements?
5. **API Design:** Could the public API be more ergonomic or more correct?

### PR Review (for pull requests)

In addition to the above, PR reviews check:
- Conventional commit messages
- Phase document is updated (if applicable)
- CHANGELOG.md is updated (if user-visible change)
- No dependency regressions (cargo-deny passes)
- CI is green on all platforms

**Quality Gate:** Reviewer can answer "yes" to: "Would I be proud to maintain this code?"

**Output:** Approved changes (possibly with improvement items filed for follow-up).

---

## Stage 8 — Integrate

**Purpose:** Merge changes cleanly and verify nothing broke.

**Activities:**
- Commit with conventional commit message (type, scope, description)
- Push to `main` (or merge PR)
- Verify CI passes on all platforms (Ubuntu, Windows, macOS, cargo-deny, MSRV)
- If CI fails, fix immediately — never leave `main` broken

**Quality Gate:**
- CI green on all platforms
- No regressions in existing tests
- Commit message follows conventional commits

**Output:** Changes on `main`, CI green.

---

## Stage 9 — Retrospect

**Purpose:** Extract learnings and feed them back into the process.

**When:** After every phase completion (at minimum). Also after significant bugs, CI failures,
or design pivots.

**Activities:**

### Update Persistent Memories
Record in [`.github/memories.md`](../../.github/memories.md):
- **Patterns:** What worked well? (e.g., "hand-rolled parser for Java Properties format
  was better than trying to use serde — the format doesn't map cleanly")
- **Gotchas:** What tripped us up? (e.g., "cargo-deny `deny = []` keys were deprecated
  silently — always check changelogs before assuming config is valid")
- **Performance:** What performance insights did we gain?
- **CI/CD:** What CI issues did we encounter and how were they resolved?

### Review ADRs
- Are any accepted ADRs now known to be suboptimal?
- Have we discovered patterns that should be formalized as new ADRs?
- Should any ADR's status change to "Superseded" or "Deprecated"?

### Review Phase Documents
- Is the completed phase document accurate? Update it to reflect what was actually built.
- Are the next phase documents still accurate given what we learned?

### Technical Debt Inventory
- Did we accumulate any `TODO` or `FIXME` items?
- Are there known improvements we deferred for scope reasons?
- Record these in [continuous improvement](continuous-improvement.md) tracking.

**Quality Gate:** Memories are updated, phase doc is accurate, no unrecorded tech debt.

**Output:** Updated memories, updated phase doc, improvement items filed.

---

## Lifecycle Variants

Not every change needs the full 9-stage lifecycle. Here are the variants:

### Full Lifecycle (Stages 1–9)
**Use for:** Phase implementations, new features, new crates, architectural changes.

### Abbreviated Lifecycle (Stages 1, 5–8)
**Use for:** Bug fixes with clear solutions, adding tests for existing code, small
refactors within established patterns.

Skipped stages:
- **Research:** Not needed if the fix is obvious
- **Decide (ADR):** Not needed if no new decisions
- **Plan:** Not needed for single-task changes
- **Retrospect:** Only if the fix revealed something surprising

### Minimal Lifecycle (Stages 6, 8)
**Use for:** Dependency bumps, typo fixes, CI config tweaks, documentation-only changes.

Skipped stages: Everything except implement and integrate. These changes are mechanical
and don't need formal planning or testing.

### Improvement Lifecycle (Stages 2, 3, 4, 5–8, 9)
**Use for:** Refactoring triggered by retrospective findings. Starts with research
(understand what's wrong), creates a superseding ADR if needed, then follows TDD.

---

## Cross-References

| Document | Purpose |
|---|---|
| [Quality Gates](quality-gates.md) | Detailed pass/fail criteria for each stage |
| [Continuous Improvement](continuous-improvement.md) | ADR evolution, tech debt, improvement process |
| [Persistent Memories](../../.github/memories.md) | Cross-session learnings, patterns, gotchas |
| [Copilot Instructions](../../.github/copilot-instructions.md) | Coding standards and agent workflow |
| [Contributing Guide](../../CONTRIBUTING.md) | External contributor onboarding |
| [Architecture Overview](../architecture/overview.md) | System architecture |
| [ADR Index](../adr/README.md) | All architecture decision records |
| [Phase Index](../phases/README.md) | 38-phase implementation roadmap |
