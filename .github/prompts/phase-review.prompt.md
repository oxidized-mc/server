---
description: 'Review a completed phase against its document, fix gaps, update the doc, and commit.'
---

# Phase Review & Completion

Review the implementation of a completed phase against its phase document, fix all gaps, update the document to match reality, and commit.

## Input

The phase document to review: ${{ input }}

## Instructions

1. **Read the phase document** — understand every task, code snippet, and completion criterion.
2. **Explore the codebase** — for each task in the document, verify the actual implementation exists, is correct, and is complete. Use parallel exploration agents to cover all areas efficiently.
3. **Identify discrepancies** — compare document claims against code reality:
   - Missing or incomplete implementations (structs, functions, systems, wiring)
   - Wrong names, signatures, or types in the document vs code
   - Code snippets in the doc that don't match what was actually written
   - Features listed as done but actually deferred or stubbed
   - Stale references to renamed/removed items
4. **Fix code gaps** — implement anything missing or broken. Add/update tests for every fix. Run `cargo check --workspace && cargo test --workspace` after each fix.
5. **Update the document** — correct every inaccuracy so the doc is a faithful record of what was built. For anything deferred to a future phase, note it explicitly. Update completion criteria to reflect actual state.
6. **Tag as completed** — add a completion status marker to the document header or metadata.
7. **Grep for stale references** — search `*.rs`, `*.toml`, `*.md` for old names from any renames.
8. **Commit** — stage all changes and commit with a conventional commit message covering both fixes and doc updates.

## What to Fix

- Components, bundles, systems, or wiring mentioned in the doc but missing from code
- Constructor or factory methods referenced but not implemented
- Test coverage gaps for new or changed code
- Document code blocks that diverge from actual implementation
- Completion criteria that claim something removed/added when reality differs

## What NOT to Fix

- Pre-existing issues unrelated to this phase
- Future-phase work — just note it as deferred in the doc
- Style or formatting preferences
- Architecture choices already captured in ADRs
