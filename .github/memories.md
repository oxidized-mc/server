# Persistent Memories

> This file captures institutional knowledge — patterns, gotchas, decisions, and learnings
> that should persist across sessions, contributors, and time. Every retrospective (Stage 9
> of the [Development Lifecycle](../docs/lifecycle/README.md)) should update this file.
>
> **Rules:**
> - Entries are append-only (never delete — mark as superseded if outdated)
> - Every entry has a date and context
> - Entries are categorized for quick scanning
> - This file is committed to the repo — it is the project's institutional memory

---

## Table of Contents

- [Patterns & Best Practices](#patterns--best-practices)
- [Gotchas & Pitfalls](#gotchas--pitfalls)
- [Performance Insights](#performance-insights)
- [CI/CD Learnings](#cicd-learnings)
- [Codebase Conventions](#codebase-conventions)
- [ADR Evolution Notes](#adr-evolution-notes)
- [Tool & Dependency Notes](#tool--dependency-notes)

---

## Patterns & Best Practices

### 2026-03-17 — Java Properties parser: hand-roll instead of serde
**Context:** Phase 1, server.properties config parser.
**Learning:** Java Properties format (`key=value` with `#` comments, `!` comments, `\`
line continuations, and both `=` and `:` as separators) does not map cleanly to any Rust
serde format. A hand-rolled line-by-line parser is simpler, more correct, and easier to
maintain than trying to wrangle serde deserialization. The parser also needs to write files
back in the exact Java Properties format for vanilla compatibility.
**Applies to:** Any future Java-format config parsing (e.g., RCON properties).

### 2026-03-17 — Java Properties: trim_start only, not trim
**Context:** Phase 1 code review finding.
**Learning:** The Java Properties spec only trims *leading* whitespace from values, preserving
trailing whitespace. Using `.trim()` instead of `.trim_start()` silently corrupts values that
intentionally have trailing spaces (e.g., `motd` formatting). Always match the source format's
exact semantics.
**Applies to:** Any parser that reads a format defined by another system.

### 2026-03-17 — CLI argument override: verify field names carefully
**Context:** Phase 1 code review caught `--world` CLI flag setting `server_ip` instead of
`level_name` — a copy-paste bug in `main.rs`.
**Learning:** When wiring CLI overrides to config struct fields, review each field name
individually. Copy-paste between override blocks is a known source of wrong-field bugs.
Consider a macro or builder pattern for CLI→config mapping if the number of overrides grows.
**Applies to:** Any CLI-to-config wiring.

---

## Gotchas & Pitfalls

### 2026-03-17 — cargo-deny deprecated keys silently
**Context:** CI pipeline failing on cargo-deny check.
**Learning:** cargo-deny `deny = []` keys under `[advisories]` and `[licenses]` were
deprecated in PR #611 without a clear migration guide. The error message was cryptic
("unknown field `deny`"). Always check the cargo-deny CHANGELOG when CI starts failing
after a dependency update. The fix: remove `deny = []` entirely (deny is now the default).
**Applies to:** Any tool with config files — check changelogs before assuming config is valid.

### 2026-03-17 — Workspace-inherited deps show as wildcards to cargo-deny
**Context:** CI pipeline failing with "wildcard dependency" errors.
**Learning:** When crates use `version.workspace = true`, cargo-deny sees the resolved
dependency as having a wildcard version specifier. The fix: set `wildcards = "allow"` in
`[bans]` section. This is a known cargo-deny limitation with workspace-inherited deps.
**Applies to:** Any workspace using `[workspace.dependencies]` + cargo-deny.

### 2026-03-17 — Windows CI cargo fmt CRLF failures
**Context:** Windows CI job failing on `cargo fmt --check`.
**Learning:** Git on Windows defaults to `core.autocrlf = true`, converting LF to CRLF on
checkout. `cargo fmt` expects LF, so `cargo fmt --check` fails. Fix: add `.gitattributes`
with `* text=auto eol=lf` to force LF line endings everywhere.
**Applies to:** Any Rust project with multi-platform CI.

### 2026-03-17 — Retrospective: test coverage illusion with roundtrip tests
**Context:** Phase 1 retrospective audit found roundtrip test only covered 8 of 56 fields.
**Learning:** A "roundtrip test" that only checks a handful of fields creates a false sense
of security. Write explicit assertions for EVERY field. If the struct grows, the compiler
won't warn you that the roundtrip test is stale — so use a full-field test from day one.
**Applies to:** Any config/serialization roundtrip test.

### 2026-03-17 — Retrospective: unknown keys must be preserved in config files
**Context:** Phase 1 retrospective found ADR-005 compliance gap — unknown keys were silently
discarded by the Properties parser. Server admins or future MC versions may add keys we
don't recognize yet.
**Learning:** Config parsers that write files back must preserve keys they don't understand.
Use a `BTreeMap<String, String>` for deterministic ordering. Write unknown keys in a separate
section at the end of the file.
**Applies to:** Any config format that supports forward-compatibility.

### 2026-03-17 — rustsec/audit-check@v2 needs explicit permissions
**Context:** Security audit workflow failing silently.
**Learning:** `rustsec/audit-check@v2` creates GitHub check runs, which requires
`permissions: checks: write, contents: read` in the workflow. Without it, the job
succeeds but creates no visible check results.
**Applies to:** Any GitHub Action that creates check runs.

---

## Performance Insights

*(No entries yet — will be populated as implementation progresses)*

---

## CI/CD Learnings

### 2026-03-17 — actions/checkout and actions/cache versions
**Context:** CI setup.
**Learning:** Use `actions/checkout@v6` and `actions/cache@v5` (latest as of 2026-03).
Always check for latest versions when setting up CI — outdated action versions may have
security issues or miss features.
**Applies to:** All workflow files.

### 2026-03-17 — MSRV check runs cargo check, not cargo build
**Context:** CI MSRV job.
**Learning:** MSRV check only needs `cargo check --workspace --all-features` — not a full
build. This is faster and sufficient to verify compatibility. Use `dtolnay/rust-toolchain`
action to install the specific MSRV version.
**Applies to:** MSRV CI configuration.

### 2026-03-17 — Dependabot PRs auto-closed when superseded
**Context:** 7 Dependabot PRs were force-closed.
**Learning:** When you manually update dependencies that Dependabot has open PRs for,
GitHub auto-closes the Dependabot PRs. This is expected behavior — not an error.
**Applies to:** Dependency management workflow.

---

## Codebase Conventions

### 2026-03-17 — Config struct field names: snake_case in Rust, kebab-case in file
**Context:** Phase 1 server.properties parser.
**Learning:** Java Properties uses `server-port` (kebab-case), Rust struct uses `server_port`
(snake_case). The parser handles this mapping. All 56 vanilla config keys use kebab-case
in the properties file.
**Convention:** When parsing external formats, always translate to Rust naming conventions
in the struct definition. Keep the file format exactly as the source system expects it.

### 2026-03-17 — Test module annotations
**Context:** Clippy warnings in test code.
**Learning:** Test modules should have `#[allow(clippy::unwrap_used, clippy::expect_used)]`
since tests legitimately use `unwrap()` and `expect()` for assertion-like behavior. Add this
at the module level, not on individual tests.
**Convention:** Every `#[cfg(test)] mod tests` block gets these allows.

### 2026-03-17 — Enum variant naming: suppress clippy when semantically correct
**Context:** `ConfigError::InvalidPort`, `ConfigError::InvalidViewDistance`, etc.
**Learning:** When all enum variants legitimately share a prefix because they represent
the same category (all validation errors), use `#[allow(clippy::enum_variant_names)]`
rather than removing the semantically meaningful prefix.
**Convention:** Prefer semantic clarity over clippy style suggestions.

---

## ADR Evolution Notes

*(Track when ADRs should be reconsidered)*

### 2026-03-17 — ADR-003 updated from 5 to 6 crates
**Context:** `oxidized-macros` crate was added for proc-macros but ADR-003 still said 5 crates.
**Action taken:** Updated ADR-003 inline (pre-implementation, no code depending on old ADR).
**Note:** In future, once ADRs have code depending on them, create a new superseding ADR
rather than editing in place.

---

## Tool & Dependency Notes

### 2026-03-17 — rsa crate RUSTSEC-2023-0071 (Marvin Attack)
**Context:** Security audit.
**Learning:** rsa v0.9.x has a known timing side-channel (Marvin Attack, RUSTSEC-2023-0071).
Only rsa 0.10-rc (release candidate) patches it. We ignore this advisory in `deny.toml`
until a stable 0.10 release. Monitor: https://github.com/RustCrypto/RSA/issues
**Action required:** When rsa 0.10 stable releases, remove the advisory ignore and upgrade.

### 2026-03-17 — License allowlist for cargo-deny
**Context:** CI cargo-deny configuration.
**Learning:** Our dependency tree requires these licenses in the allowlist:
`MIT`, `Apache-2.0`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`,
`CDLA-Permissive-2.0` (webpki-root-certs), `Unicode-3.0` (unicode-ident).
The `MPL-2.0` and `Unicode-DFS-2016` entries are not needed.
**Applies to:** Any time a new dependency is added — check its license.

---

## Phase 1 Retrospective (2026-03-17)

### What went well
- **Core architecture is solid.** The crate layout, ServerConfig struct, and Properties parser all
  passed ADR compliance audit (ADR-001 through ADR-005, ADR-030 all compliant for Phase 1 scope).
- **CI pipeline caught real issues.** Cross-platform builds exposed no OS-specific bugs.
- **Code review found two genuine bugs** before any user encountered them (world→level_name
  copy-paste, trim→trim_start spec violation).

### What surprised us
- **85% of config keys were untested.** The initial test suite only covered 6 of 41+ keys via
  parsing — a dangerous blind spot. The all-keys parsing test is now the single highest-value
  test in the module.
- **Roundtrip test only verified 8 of 56+ fields.** Even with a "roundtrip test" in place, most
  fields were never actually roundtrip-verified.
- **Format strings in logging seemed fine until ADR-004 audit.** Structured `key=value` fields
  were mandated by ADR-004 but overlooked during initial implementation.

### What should change going forward
- **Test-first is non-negotiable.** Phase 1 wrote tests after implementation, which missed the
  85% coverage gap. Future phases must write tests before code (TDD cycle enforced by lifecycle).
- **"One test per feature" is insufficient.** Use the pattern: one test per *behavior* —
  parsing, validation, roundtrip, and edge cases are separate concerns requiring separate tests.
- **ADR compliance must be checked during implementation, not after.** The lifecycle now mandates
  this at the Review stage (Stage 7).

### Technical debt acknowledged
- **Unknown key preservation** added retroactively. This was identified as an ADR-005 gap and
  fixed during the retrospective. Keys not recognized by the parser are now stored in a
  `BTreeMap` and written back on save.
- **Structured logging** retrofitted. All log calls in `main.rs` now use `key=value` fields
  per ADR-004.

### Metrics
- **Tests:** 26 → 48 (+84.6% increase)
- **Config key parse coverage:** 6/41 → 41/41 (100%)
- **Roundtrip field coverage:** 8/56 → 56/56 (100%)
- **Boundary validation tests:** 0 → 13
- **Format edge case tests:** 0 → 6
- **ADR gaps fixed:** 2 (structured logging, unknown key preservation)
- **Bugs fixed:** 2 (world CLI override, trim_start spec compliance)
