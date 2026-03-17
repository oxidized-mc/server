# Continuous Improvement

> The codebase is never "done." Every phase completion, every bug fix, every code review is
> an opportunity to make Oxidized better. This document formalizes the processes that ensure
> we always move toward perfection.

---

## Table of Contents

- [Improvement Triggers](#improvement-triggers)
- [ADR Evolution](#adr-evolution)
- [Phase Retrospectives](#phase-retrospectives)
- [Technical Debt Management](#technical-debt-management)
- [Refactoring Process](#refactoring-process)
- [Pattern Library Evolution](#pattern-library-evolution)
- [Dependency Hygiene](#dependency-hygiene)

---

## Improvement Triggers

An improvement cycle is triggered by any of these events:

### Automatic Triggers
1. **Phase completion** → Phase retrospective (mandatory)
2. **Bug fix** → Root cause analysis — was the bug caused by a design flaw?
3. **CI failure on `main`** → Immediate fix + post-mortem note in memories
4. **Security advisory** → Evaluate impact, update dependencies or add ignore with rationale
5. **Dependency major version bump** → Review if the new version enables better patterns

### Discovery Triggers
6. **Code review finds a better approach** → File improvement item
7. **Reading Rust ecosystem news reveals a better crate/pattern** → Evaluate and propose ADR
8. **Java reference study reveals we misunderstood vanilla behavior** → Fix implementation + update ADR
9. **Performance profiling reveals a bottleneck** → Create performance ADR and optimize
10. **A pattern is copy-pasted more than twice** → Extract into shared utility or macro

### External Triggers
11. **New Rust stable release** → Check if new features (e.g., new APIs, language features) improve our code
12. **Minecraft protocol update** → Evaluate compatibility impact
13. **Contributor feedback** → Review and incorporate improvements

---

## ADR Evolution

ADRs are living documents in the sense that the project's understanding evolves.
However, **accepted ADRs are never edited in place** (once code depends on them).

### ADR Lifecycle

```
Proposed → Accepted → [Active] → Superseded by ADR-NNN
                                  (or)
                               → Deprecated (no longer relevant)
```

### When to Create a Superseding ADR

Create a new ADR that supersedes an existing one when:

1. **Implementation revealed the original decision was suboptimal** — You tried the
   approach and discovered it doesn't work well in practice.
2. **The Rust ecosystem evolved** — A new crate or language feature makes a fundamentally
   better approach possible.
3. **Scale requirements changed** — What worked for Phase 5 doesn't work at Phase 30's scale.
4. **A better pattern was discovered** — Community best practices evolved, or you found a
   paper/blog describing a superior approach.

### Superseding Process

1. Write the new ADR with full context (including "this supersedes ADR-NNN because…")
2. Add status header to the old ADR: `**Status: Superseded by [ADR-NNN](adr-NNN-*.md)**`
3. Update all phase documents that reference the old ADR to also reference the new one
4. Plan the refactoring work to migrate existing code to the new approach
5. Execute the refactoring following the [Refactoring Process](#refactoring-process)

### ADR Health Check (per phase retrospective)

During every phase retrospective, review relevant ADRs:

- [ ] Are the assumptions in this ADR still valid?
- [ ] Has the implementation revealed any consequences not anticipated in the ADR?
- [ ] Are there new options we should consider that didn't exist when the ADR was written?
- [ ] Does this ADR conflict with any other accepted ADR?

Record findings in [memories.md](../../.github/memories.md) under "ADR Evolution Notes."

---

## Phase Retrospectives

After every phase completion, conduct a retrospective. This is not optional.

### Retrospective Template

Answer these questions and record findings in memories.md:

#### What Went Well?
- What patterns or approaches worked especially well?
- What tools or processes saved time?
- What decisions from ADRs proved correct?

#### What Surprised Us?
- What was harder than expected? Why?
- What was easier than expected? Why?
- What did the Java reference not tell us that we had to discover?

#### What Should Change?
- Are any ADRs wrong or outdated?
- Are any phase documents inaccurate for upcoming phases?
- Should any tools, dependencies, or CI checks be added/removed?
- Are there patterns we should formalize (new ADR or new convention)?

#### Technical Debt Incurred
- Any `TODO` or `FIXME` items added?
- Any known suboptimal implementations we accepted for scope?
- Any missing test coverage we should add?

#### Improvement Actions
List concrete next actions with owners:
- "Create ADR-NNN to supersede ADR-XXX because…"
- "Refactor X to use pattern Y (discovered during this phase)"
- "Add test coverage for edge case Z (deferred during implementation)"

---

## Technical Debt Management

Technical debt is not inherently bad — it's a conscious tradeoff. But it must be **visible,
tracked, and scheduled for repayment.**

### Recording Debt

When you incur technical debt (deferred improvements, known suboptimal code, missing tests):

1. Add a `TODO(debt):` comment in the code with a brief explanation
2. Record the debt in memories.md under a "Technical Debt" section
3. If the debt is significant (affects architecture or performance), create a GitHub issue

### Debt Categories

| Category | Priority | Examples |
|---|---|---|
| **Architecture** | High | Crate boundary violation, incorrect abstraction |
| **Performance** | Medium | Unnecessary allocation, suboptimal algorithm |
| **Testing** | Medium | Missing edge case coverage, no integration tests |
| **Documentation** | Low | Missing doc comments, outdated examples |
| **Style** | Low | Non-idiomatic patterns, naming inconsistencies |

### Debt Repayment Schedule

- **Architecture debt:** Fix before the next phase that builds on the affected code
- **Performance debt:** Fix when the affected code path becomes hot (measure first)
- **Testing debt:** Fix in the same phase (testing is never optional)
- **Documentation debt:** Fix opportunistically (when touching the file for other reasons)
- **Style debt:** Fix in bulk refactoring sessions between phases

---

## Refactoring Process

Refactoring is a first-class activity — not something done "when we have time."
Refactoring follows the [Improvement Lifecycle](README.md#lifecycle-variants).

### When to Refactor

1. **Before starting a new phase** that depends on code with known debt
2. **After a retrospective** identifies a pattern improvement
3. **When an ADR is superseded** and existing code needs to migrate
4. **When a pattern is used 3+ times** and should be extracted

### Refactoring Rules

1. **Never refactor and add features in the same commit.** Separate concerns:
   - First commit: refactor (type `refactor`)
   - Second commit: feature on top of refactored code (type `feat`)
2. **All tests must pass before, during, and after refactoring.**
   The test suite is your safety net — if tests break, the refactor is wrong.
3. **Update all references.** Renaming something means grepping the entire repo for the
   old name and updating every occurrence (code, docs, CI, copilot instructions).
4. **Update memories.md** with what was refactored and why.
5. **Create a superseding ADR** if the refactoring changes architectural decisions.

---

## Pattern Library Evolution

Patterns are formalized conventions that emerge from repeated successful use.

### Pattern Discovery

A pattern is ready to formalize when:
- The same approach has been used successfully 3+ times
- Deviating from the approach has caused bugs or confusion
- A new contributor would benefit from knowing the pattern exists

### Formalizing a Pattern

1. Add the pattern to memories.md under "Patterns & Best Practices"
2. If the pattern affects architecture, create or update an ADR
3. If the pattern affects coding style, add to copilot-instructions.md
4. If the pattern affects testing, add to the testing section of CONTRIBUTING.md
5. Consider creating a utility function, macro, or trait to enforce the pattern in code

### Pattern Retirement

When a pattern is superseded by a better approach:
1. Mark the old pattern as "Superseded" in memories.md (do not delete)
2. Document why the new approach is better
3. Plan migration of existing code

---

## Dependency Hygiene

Dependencies are a form of technical debt — every dependency is code we don't control.

### Weekly Review (via Dependabot)
- Dependabot creates PRs for dependency updates every Monday
- Review each PR: read the changelog, check for breaking changes
- Merge patch updates promptly; evaluate minor/major updates carefully

### Per-Phase Review
- Before starting a new phase, check if any dependencies have new major versions
- Evaluate if new crates in the ecosystem would serve us better than current choices
- Check advisory databases for newly disclosed vulnerabilities

### Dependency Addition Criteria
Before adding a new dependency, evaluate:
1. **Is it necessary?** Can we achieve the same with existing deps or stdlib?
2. **Is it maintained?** Last commit < 6 months, responsive to issues
3. **Is it widely used?** Downloads, stars, dependent crates
4. **Is the license compatible?** Must be in our cargo-deny allowlist
5. **Does it have known vulnerabilities?** Check RustSec advisory database
6. **What's the compile-time impact?** Check with `cargo build --timings`

Record the evaluation in the ADR or PR description for the change that adds the dependency.
