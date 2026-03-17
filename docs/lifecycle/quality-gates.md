# Quality Gates

> Every stage of the [Development Lifecycle](README.md) has explicit pass/fail criteria.
> No change advances to the next stage until all gates for the current stage are satisfied.
> These gates are not bureaucracy — they are the minimum standard for code we are proud to ship.

---

## Gate Summary

| Stage | Gate Name | Enforcement |
|---|---|---|
| 1 — Identify | Problem Clarity | Manual (plan review) |
| 2 — Research | Understanding Depth | Manual (can explain why) |
| 3 — Decide | ADR Completeness | Manual (review checklist) |
| 4 — Plan | Task Decomposition | Manual (SQL todos exist) |
| 5 — Test First | Red Tests | Automated (`cargo test` fails) |
| 6 — Implement | Green + Clean | Automated (CI pipeline) |
| 7 — Review | Code Review Approval | Manual + automated checks |
| 8 — Integrate | CI Green | Automated (all CI jobs pass) |
| 9 — Retrospect | Memories Updated | Manual (memories.md updated) |

---

## Detailed Gate Criteria

### Gate 1 — Problem Clarity

**Question:** Can you explain in one sentence what this change does and why it matters?

- [ ] Problem statement is written down (session plan or PR description)
- [ ] Affected crates and modules are identified
- [ ] The user-facing impact is understood (even if indirect)
- [ ] Relevant persistent memories have been checked

**Fail criteria:** You cannot articulate what you're changing or why.

---

### Gate 2 — Understanding Depth

**Question:** Do you understand the problem space well enough to choose the right solution?

- [ ] Java reference code has been read for the relevant feature
- [ ] Related ADRs have been reviewed
- [ ] At least 2 approaches have been considered for non-trivial decisions
- [ ] Existing codebase patterns have been checked for consistency
- [ ] Potential impacts on other crates/modules have been identified

**Fail criteria:** You're guessing at the solution or haven't read the Java reference.

---

### Gate 3 — ADR Completeness

**Question:** Would a new contributor understand this decision 6 months from now?

- [ ] **Context** clearly explains the situation and constraints
- [ ] **Options** lists at least 3 alternatives for major decisions (2 for minor)
- [ ] **Each option** has pros and cons
- [ ] **Decision** is stated clearly with rationale
- [ ] **Consequences** section covers positive, negative, and neutral impacts
- [ ] Related ADRs are cross-linked
- [ ] ADR is linked from the relevant phase document
- [ ] If superseding an existing ADR, the old ADR is marked "Superseded"

**Fail criteria:** Missing sections, fewer than required alternatives, or no clear rationale.

---

### Gate 4 — Task Decomposition

**Question:** Are tasks small enough to complete and verify independently?

- [ ] Each task has a clear "done" definition
- [ ] Dependencies between tasks are explicit (SQL `todo_deps`)
- [ ] No task takes more than ~200 lines of code to implement
- [ ] Tasks follow dependency order (no blocked tasks in the critical path)
- [ ] User has confirmed the plan (for large changes)

**Fail criteria:** Tasks are vague, oversized, or have hidden dependencies.

---

### Gate 5 — Red Tests

**Question:** Do failing tests clearly define the expected behavior?

- [ ] Tests exist for every requirement in the current task
- [ ] Tests follow naming convention: `test_<thing>_<outcome>_when_<condition>`
- [ ] Tests cover happy path, edge cases, and error paths
- [ ] Parameterized tests are used for multiple input combinations
- [ ] `cargo test -p <crate>` runs and tests **fail** (not compile-error)
- [ ] Tests fail for the right reason (not a missing import or syntax error)

**Fail criteria:** Tests don't exist, don't fail, or fail for the wrong reason.

---

### Gate 6 — Green + Clean

**Question:** Is the implementation correct, clean, and ready for review?

#### Correctness
- [ ] All tests pass: `cargo test --workspace`
- [ ] Implementation matches the intent described in tests

#### Code Quality
- [ ] Clippy clean: `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Formatted: `cargo fmt --check`
- [ ] No `unwrap()` or `expect()` in production code
- [ ] No hardcoded magic numbers — named constants only
- [ ] All public items have `///` doc comments

#### Architecture Compliance
- [ ] Crate dependency rules are respected (no upward imports)
- [ ] Error handling follows ADR-002 (thiserror in libraries, anyhow in binary)
- [ ] Async/threading follows ADR-001 and ADR-019 patterns

#### Security
- [ ] No `unsafe` blocks (unless documented with `SAFETY:` comment and justified)
- [ ] No secrets or credentials in source
- [ ] Cryptographic code follows ADR-009

**Fail criteria:** Any automated check fails, or production code uses unwrap.

---

### Gate 7 — Code Review Approval

**Question:** Would you be proud to maintain this code?

#### Standard Checks
- [ ] Code is correct and handles edge cases
- [ ] No stale references (renamed items, moved files, changed APIs)
- [ ] Test coverage is adequate
- [ ] Documentation is clear and accurate

#### ADR Compliance
- [ ] Implementation follows relevant accepted ADRs
- [ ] No ADR is silently violated

#### Improvement Identification
- [ ] **Checked:** Are any existing ADRs outdated given this change?
- [ ] **Checked:** Could any existing pattern be improved?
- [ ] **Checked:** Are there missing tests for existing code exposed by this change?
- [ ] **Checked:** Are there performance opportunities?
- [ ] Any identified improvements are recorded (memories.md or new issue/ADR)

#### PR-Specific Checks (for pull requests)
- [ ] Conventional commit messages
- [ ] Phase document updated (if applicable)
- [ ] CHANGELOG.md updated (if user-visible change)
- [ ] cargo-deny passes (no license or advisory regressions)

**Fail criteria:** Reviewer identifies correctness issues, ADR violations, or unrecorded
improvement opportunities.

---

### Gate 8 — CI Green

**Question:** Does the change work on all target platforms?

- [ ] Ubuntu CI job passes (fmt, clippy, build, test)
- [ ] Windows CI job passes (fmt, clippy, build, test)
- [ ] macOS CI job passes (fmt, clippy, build, test)
- [ ] cargo-deny job passes (licenses, advisories)
- [ ] MSRV check passes (Rust 1.85 compatibility)
- [ ] Security audit has no new unignored advisories

**Fail criteria:** Any CI job fails. Fix immediately — never leave `main` broken.

---

### Gate 9 — Memories Updated

**Question:** Have we captured everything we learned?

- [ ] New patterns added to memories.md (if any discovered)
- [ ] New gotchas added to memories.md (if any encountered)
- [ ] Phase document updated to reflect what was actually built
- [ ] ADR evolution notes added (if any ADRs should be reconsidered)
- [ ] Technical debt items recorded (if any deferred)
- [ ] Next phase documents reviewed for accuracy given learnings

**Fail criteria:** Session ends without recording learnings that would help future work.

---

## Enforcement

### Automated (CI Pipeline)
These gates are enforced automatically and cannot be bypassed:
- `cargo fmt --check` (formatting)
- `cargo clippy --workspace --all-targets -- -D warnings` (linting)
- `cargo test --workspace` (tests)
- `cargo-deny check` (licenses + advisories)
- MSRV check (Rust 1.85 compatibility)
- `rustsec/audit-check` (security vulnerabilities)

### Manual (Review Process)
These gates require human judgment:
- Problem clarity and understanding depth
- ADR completeness and correctness
- Task decomposition quality
- Test adequacy and edge case coverage
- Code review approval
- Improvement identification
- Memory updates

### Tooling-Assisted (Code Review Agent)
The `code-review` agent assists with:
- Correctness checking
- Pattern consistency
- Stale reference detection
- ADR compliance (limited — human review still required)

---

## Escalation

If a gate cannot be satisfied, **do not bypass it.** Instead:

1. **Understand why** the gate is failing
2. **Fix the root cause** — not the symptom
3. If the gate itself is wrong, **update this document** with a better gate
4. If the fix is genuinely impossible right now, document the exception in
   [memories.md](../../.github/memories.md) with a clear plan for resolution
