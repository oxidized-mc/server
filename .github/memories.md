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

### 2026-03-17 — rustsec/audit-check@v2 deprecated (Node.js 20)
**Context:** Security audit workflow failing on every push since project inception.
**Learning:** `rustsec/audit-check@v2` uses Node.js 20 which GitHub is deprecating
(forced Node.js 24 from June 2026). The action was consistently failing. Replaced with
direct `cargo install cargo-audit && cargo audit` which is more reliable and doesn't
depend on action maintenance. Always prefer running tools directly over wrapper actions
when the tool itself is simple to invoke.
**Applies to:** All CI workflows — prefer direct tool invocation over third-party actions.

### 2026-03-17 — CI pipeline status must be verified after every push
**Context:** Multiple phases were committed without verifying security audit was passing.
**Learning:** The lifecycle lacked an explicit CI verification loop. Historical failures
accumulated on `main` unnoticed because we only checked CI locally. Now Stage 8 (Integrate)
requires waiting for **all** CI jobs to complete and verifying green, including security
audit. Added CI Repair loop to the lifecycle.
**Applies to:** Every integration — wait for all workflows, not just the main CI.

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
- ✅ **Unknown key preservation** added retroactively. This was identified as an ADR-005 gap and
  fixed during the retrospective. Keys not recognized by the parser are now stored in a
  `BTreeMap` and written back on save. *(Resolved in Phase 1 retrospective.)*
- ✅ **Structured logging** retrofitted. All log calls in `main.rs` now use `key=value` fields
  per ADR-004. *(Resolved in Phase 1 retrospective.)*

### Metrics
- **Tests:** 26 → 48 (+84.6% increase)
- **Config key parse coverage:** 6/41 → 41/41 (100%)
- **Roundtrip field coverage:** 8/56 → 56/56 (100%)
- **Boundary validation tests:** 0 → 13
- **Format edge case tests:** 0 → 6
- **ADR gaps fixed:** 2 (structured logging, unknown key preservation)
- **Bugs fixed:** 2 (world CLI override, trim_start spec compliance)

### 2026-03-17 — Retrospective: question format choices during implementation
**Context:** Phase 1 v3 retrospective — config format evolution.
**Learning:** ADR-005 chose Java `.properties` for vanilla compatibility, but this was a
Java-specific format choice copied without questioning. ADR-033 superseded it with TOML after
applying the "Architectural Questioning" principle: the MC client never reads config files,
so there's no compatibility requirement. Always ask "would a Rust developer choose this?"
before implementing any decision copied from the Java reference.
**Applies to:** Any decision based on "vanilla does it this way."

### 2026-03-17 — Retrospective: serde derives eliminate 90% of parser code
**Context:** Phase 1 v3 — replacing hand-rolled Properties parser with TOML + serde.
**Learning:** The original config.rs was ~1800 lines with a hand-rolled parser, helpers,
and serialization macros. The TOML + serde version is ~500 lines — all type-safe, with
automatic (de)serialization. Prefer serde derives over hand-rolled parsers for any
structured data format.
**Applies to:** Any future parser (NBT reader/writer could also benefit from serde).

### 2026-03-17 — Phase 2: TCP listener works, MC client sends real packets
**Context:** Phase 2 — TCP Listener + Raw Framing.
**Learning:** A real MC 26.1-pre-3 client connected and we could see handshake (0x00, 18 bytes)
and status request (0x00, 0 bytes) packets in the debug log. The client retries 4+ times when
no status response comes back — important for Phase 3 to handle quickly.
**Applies to:** Phase 3 (Handshake + Status) — must respond before client timeout.

### 2026-03-17 — VarInt encoding matches vanilla exactly
**Context:** Phase 2 — VarInt codec implementation.
**Learning:** The encode/decode for known test vectors (0, 127, 128, 300, 25565, -1, i32::MAX,
i32::MIN) matches the vanilla Java implementation byte-for-byte. Proptest with all i32 values
confirms roundtrip correctness. The `varint_size()` helper is useful for pre-calculating buffer sizes.
**Applies to:** All future packet codec work.

---

## Phase 2 Retrospective (2026-03-17)

### What went well
- **VarInt/VarLong codec is rock-solid.** Proptest across all i32 values confirmed 100% roundtrip
  correctness. Test naming and coverage followed TDD properly.
- **Connection struct design is extensible.** Adding cipher and compression fields later (Phase 4)
  was straightforward because the struct was well-factored from the start.
- **Frame codec correctly handles edge cases.** Maximum packet length validation, zero-length
  frames, and multi-byte VarInt lengths all tested.

### What surprised us
- **MC client retries aggressively.** 4+ connection attempts when no status response — important
  for Phase 3 to handle connections quickly or the client gives up.
- ✅ **CI failure on this commit was expected.** Phase 2 added types used by Phase 3; the commit
  compiled but CI ran clippy which flagged unused code. *(Resolved in Phase 3 commit.)*

### Metrics
- **Tests:** 73 total (48 Phase 1 + 25 new: VarInt/VarLong, frame codec, connection)
- **Crates touched:** 2 (oxidized-protocol new, oxidized-server updated)
- **ADR compliance:** ADR-006 (network I/O), ADR-007 (packet codec) — both followed

---

## Phase 3 Retrospective (2026-03-17)

### What went well
- **Server list ping works end-to-end.** Real MC 26.1-pre-3 client shows the server in the
  multiplayer list with correct MOTD, player count (0/20), and version string.
- **Protocol dispatch pattern is clean.** The `handle_handshake()` → `handle_status()` dispatch
  pattern with match on packet IDs is simple and extensible for future states.
- **Wire type helpers are reusable.** The `read_string()`, `write_string()`, `read_u16()`, etc.
  helpers in `codec/types.rs` are used across all subsequent phases.

### What surprised us
- **Status response JSON must match vanilla exactly.** The `version.protocol` field must be the
  integer protocol version (1073742124), not a string. The `players` object must include
  `max` and `online` even when empty. Client is strict about JSON structure.
- **Ping/pong timing matters.** The client sends a `PingRequestPacket` with a timestamp and
  expects the same timestamp echoed back. This is used for latency display.

### What should change going forward
- **Test against real client earlier.** Phase 2 had no real client testing; Phase 3 caught
  issues that a real client test would have revealed (JSON format strictness).

### Metrics
- **Tests:** 98 total (73 prior + 25 new: packet codec, status JSON, dispatch, integration)
- **Integration tests:** 3 (full status exchange, protocol mismatch, graceful shutdown)
- **ADR compliance:** ADR-006, ADR-007, ADR-008 — all followed

---

## Phase 4 Retrospective (2026-03-17)

### What went well
- **Full login flow works.** Online and offline mode authentication, encryption, and compression
  all functional. Connection transitions cleanly from Login → Configuration state.
- **Manual CFB-8 implementation is correct.** Despite the `cfb8` crate being broken
  (incompatible with cipher 0.5), our manual implementation passes all vanilla test vectors
  including the tricky "simon" hash (many online sources have the wrong value).
- **Code review caught a real security bug.** URL injection vulnerability in `auth.rs` where
  username/server_hash were interpolated directly into the session server URL. Fixed with
  `urlencoding::encode()` before merge.
- **Encrypted+compressed pipeline is transparent.** The `read_raw_packet()` and `send_raw()`
  methods handle encryption and compression internally — callers don't need to know.

### What surprised us
- **`cfb-mode` 0.8 is CFB-128, NOT CFB-8.** Minecraft needs CFB-8 (1-byte feedback). The
  naming is misleading. Had to implement CFB-8 manually using AES-128 block cipher directly.
- **RSA + rand version incompatibility.** RSA 0.9 depends on rand_core 0.6, but our rand 0.10
  uses rand_core 0.9. Solution: use `rsa::rand_core::OsRng` for RSA, `rand::rng()` elsewhere.
- **Java's `UUID.nameUUIDFromBytes()` is non-standard.** It hashes raw input with MD5 (no
  namespace prefix), unlike Rust's `Uuid::new_v3()` which prepends 16 nil bytes. Had to use
  raw MD5 + manual version/variant bit setting.
- **wiki.vg "simon" test vector is wrong in many sources.** Correct value:
  `"88e16a1019277b15d58faf0541e11910eb756f6"` (no leading minus, starts with 88e).
- **Encryption is stream-level, compression is frame-level.** Encryption operates on raw TCP
  bytes INCLUDING frame length prefixes (cipher state advances per byte). Compression is
  per-packet and independent. This distinction is critical for correct implementation.

### What should change going forward
- ✅ **ADR-009 referenced `cfb8` crate but we couldn't use it.** ADRs should note implementation
  caveats when the chosen approach doesn't work. *(Resolved — ADR-009 updated with actual implementation.)*
- ✅ **URL-encode all external API parameters by default.** The auth URL injection was subtle —
  make encoding the default pattern for any URL construction. *(Resolved — `urlencoding::encode()`
  applied in auth.rs.)*

### Technical debt acknowledged
- ✅ **No real client testing yet.** The login flow is tested with unit/integration tests but not
  against a real Minecraft 26.1-pre-3 client. *(Superseded — real client testing done in Phase 6+.
  Configuration, Play states, and chunk rendering all verified against vanilla 26.1-pre-3 client.)*
- **`reqwest` is a heavy dependency.** Consider whether a lighter HTTP client would suffice
  for the single Mojang auth endpoint. *(Still open — reqwest 0.13 remains in use.)*

### Metrics
- **Tests:** 158 total (98 prior + 60 new: crypto 17, compression 10, auth 4, login packets 11,
  codec types 8, connection 5, server integration 5)
- **Security bugs found in review:** 1 (URL injection in auth.rs)
- **Crate incompatibilities worked around:** 2 (cfb8, RSA+rand)
- **ADR compliance:** ADR-006, ADR-007, ADR-008, ADR-009 — all followed (ADR-009 updated)

---

## Phase 5 Retrospective (2026-03-17)

### What went well
- **NBT crate is fully self-contained.** Zero internal deps, clean public API. The crate can
  be used independently of the rest of the project.
- **Comprehensive test coverage.** 160 unit tests + 3 doc tests covering all 13 tag types,
  roundtrip binary codec, Modified UTF-8 edge cases, SNBT parsing, serde integration.
- **Code review caught integer overflow vulnerability.** Array size accounting used unchecked
  multiplication (`4 * len`) which could wrap on 32-bit platforms, bypassing NbtAccounter
  memory limits. Fixed with checked arithmetic before merge.
- **Serde integration is ergonomic.** `to_compound`/`from_compound` provide type-safe struct
  access to NBT data, eliminating manual tag-by-tag extraction for typed use cases.

### What surprised us
- **ADR-010 had wrong quota values.** ADR says 64 MiB limit; vanilla actually uses 2 MB
  (network) / 100 MB (disk). Must always cross-reference ADRs against Java source.
- **Modified UTF-8 supplementary character handling is non-trivial.** Supplementary Unicode
  chars (> U+FFFF) use surrogate pair encoding in CESU-8 format, not standard UTF-8 4-byte
  sequences. Required careful encoding/decoding logic.
- **Agent left stray test binaries.** The general-purpose agent ran `cargo test` with
  `--test-threads=1` which left compiled test binaries in the repo root. Added cleanup step.

### What should change going forward
- **Always check for stray files after agent work.** Run `git status` and clean up any
  untracked files before committing.
- **Cross-reference ADR values against Java source.** ADR-010's quota values were incorrect.
  Any numeric constants in ADRs should be verified against the decompiled reference.

### Technical debt acknowledged
- **Arena-allocated NBT deferred.** ADR-010 specifies a `BumpNbt` arena variant for hot-path
  chunk loading. Deferred to when chunk loading at scale needs it. *(Still open — Phase 10
  completed without it; revisit when profiling shows NBT allocation as a bottleneck.)*
- **Borrowed/zero-copy NBT deferred.** `BorrowedNbtCompound<'a>` for lazy parsing also
  deferred. *(Still open — same rationale as arena NBT.)*
- **No benchmark suite yet.** ADR-010 calls for criterion benchmarks. *(Still open —
  `[profile.bench]` configured in Cargo.toml but no criterion benchmarks or `benches/` dirs
  created yet.)*

### Metrics
- **Tests:** 166 total (163 unit + 3 doc-tests)
- **Lines of code:** ~4,650 (12 source files)
- **Modules:** error, mutf8, tag, compound, list, accounter, reader, writer, io, snbt, serde
- **Security bugs found in review:** 4 across 4 iterations
  1. Integer overflow in reader size accounting (checked arithmetic fix)
  2. SNBT parser unbounded recursion → stack overflow DoS (depth parameter fix)
  3. Writer unbounded recursion → stack overflow DoS (depth check fix)
  4. Writer `len() as i32` silent truncation (i32::try_from fix)
- **Review iterations:** 4 (R1: overflow found → R2: 3 new issues → R3: depth leak on error paths → R4: clean)
- **ADR compliance:** ADR-010 followed (quota values corrected from ADR)

### Review↔Fix Loop Learnings
- **Mutable depth state is error-prone.** Review #3 caught that `push_depth`/`pop_depth` on
  a mutable field leaks depth on early `?` returns. Passing depth as an immutable parameter
  through recursive calls makes leaks impossible by construction. Prefer parameter-passing
  over mutable state for recursion depth tracking.
- **Review iteration #1 missed the writer.** The first review focused on the reader and found
  the overflow. But the same class of bug (unchecked arithmetic, unbounded recursion) existed
  in the writer and SNBT formatter. Lesson: when fixing a bug class, grep for ALL instances
  across the entire crate, not just the file where it was found.
- **The loop works.** 4 iterations caught 4 distinct security issues that would have shipped
  without the Review↔Fix loop enforcement.

---

### 2026-03-18 — Phase 6: Configuration State

**Context:** Implementing the Configuration protocol state (LOGIN → CONFIGURATION → PLAY).

#### Key Decisions
- **Registry data embedding:** Bundled 28 synchronized registries as a single `registries.json`
  (382 entries, ~254 KB) via `tools/bundle_registries.py`. Included at compile time with
  `include_str!`. Runtime JSON→NBT conversion on first access via `LazyLock`. Startup cost
  is negligible (~ms) and avoids complex build.rs dependencies on oxidized-nbt.
- **Tags deferred:** Sent empty `UpdateTagsPacket`. Full tag support requires block/item
  registries (Phase 8+) since tags reference entries by integer ID.
- **Known pack negotiation simplified:** Always send full registry data regardless of client
  response. Known-pack optimization deferred — marginal benefit until data packs are supported.

#### Gotchas
- **NBT type ambiguity from JSON:** JSON loses int/float distinction. Heuristic: no fractional
  part → `Int`; fractional → `Float`. The vanilla client uses DynamicOps which is type-flexible,
  so `Int` vs `Long` and `Float` vs `Double` both work if the value fits.
- **Registry order matters:** The 28 registries must be sent in the order defined by
  `RegistryDataLoader.SYNCHRONIZED_REGISTRIES`. This order is preserved in `SYNCHRONIZED_REGISTRIES`
  constant.
- **handle_login() return bug:** The Login arm in `handle_connection()` returned `Ok(())`
  after `handle_login()`, closing the connection before configuration could run. The fix is
  to call `handle_configuration()` immediately after login succeeds.
- **Version string for KnownPack:** 26.1-pre-3 maps to version "1.21.6" for the vanilla core
  pack in `SelectKnownPacks`.

#### Metrics
- **Files:** 16 changed, 1871 insertions
- **Tests:** 381 total (180 protocol, 163 NBT, 35 server, 3 doc-tests)
- **Review iterations:** 1 (clean pass)

#### Phase 6.6 Completion — ServerboundClientInformationPacket (2026-03-18)
- **What went well:** Existing enum patterns (GameType, Difficulty) made new enum types
  trivial — copy the structure, change the variants. TDD cycle was smooth; all 35 new tests
  passed on first implementation.
- **Gotcha — client info arrives before SelectKnownPacks response:** The vanilla client
  sends `ServerboundClientInformationPacket` (0x00) *before* responding to `SelectKnownPacks`
  (0x02). A rigid "read next packet, expect X" approach would reject valid clients. The fix:
  read packets in a loop, accepting 0x00 at any point during configuration and breaking
  when the expected packet arrives.
- **Metrics:** 7 files changed, 952 insertions, 35 new tests (20 enum + 15 packet), 1 review
  iteration (clean pass). Total test count: 447 protocol, 35 server.

### Phase 7 — Core Data Types

#### What Went Well
- **Tier 1/Tier 2 decomposition worked perfectly.** Implementing independent types first (Direction, Vec3i, Vec3, Vec2, GameType, Difficulty) then dependent types (BlockPos, ChunkPos, SectionPos, Aabb) avoided any circular dependency issues.
- **Code review caught real bugs:** integer overflow in `dist_manhattan()`, `dist_chessboard()`, `offset()`, `multiply()`, `cross()`, `relative_steps()`, and Add/Sub traits. All fixed before merge.
- **Java reference guided bit-packing exactly right** — BlockPos 26/26/12 layout and SectionPos 22/22/20 layout match vanilla perfectly, with sign extension via arithmetic right shift.

#### Patterns Established
- **Distance calculations widen to i64** to avoid overflow: `i64::from(self.x) - i64::from(other.x)`. This is safe because i32 differences always fit in i64.
- **Spatial arithmetic uses wrapping** (`wrapping_add`, `wrapping_mul`) to match Java's default overflow behavior. In practice, Minecraft world coordinates are bounded by ±30M so overflow can't occur with valid game data.
- **Wire format helpers pattern**: `read_f32`/`write_f32`/`read_f64`/`write_f64` added to `codec/types.rs` following existing `read_i32`/`write_i32` pattern.
- **Newtype coordinate wrappers** (BlockPos, ChunkPos, SectionPos) enforce compile-time safety per ADR-013.

#### Gotchas
- **Unicode box-drawing characters in Rust source**: section separator comments like `// ── Distances ──` are safe inside comments, but edit operations can accidentally place them outside comments, causing "unknown start of token" compile errors. Always verify edits near decorated comments.
- **BlockPos sign extension trick**: Rust's `>>` on `i64` is arithmetic (preserves sign), same as Java. The pattern `((packed << N) >> M) as i32` correctly sign-extends packed fields.
- **Aabb auto-correction**: Constructor must swap min/max if inverted — matches `AABB.java`'s behavior.

#### Metrics
- **Files:** 12 changed, 4234 insertions
- **Tests:** 412 protocol tests (up from 381), 613 total workspace
- **Review iterations:** 2 (overflow fix required re-review)
- **Types added:** Direction, Axis, AxisDirection, Vec3i, Vec3, Vec2, BlockPos, ChunkPos, SectionPos, Aabb, GameType, Difficulty

---

### Phase 8 — Block & Item Registry (2025-07)

#### What Went Well
- **Vanilla data extraction worked perfectly:** `java -DbundlerMainClass=net.minecraft.data.Main -jar server.jar --reports` generated accurate `blocks.json` (1168 blocks, 29873 states) and items data.
- **Embedded compressed data approach is clean:** `include_bytes!` on `.json.gz` files keeps the binary small and avoids runtime file I/O for registry initialization.
- **Tests caught real correctness issues:** AIR=0, STONE=1 verified against vanilla; block counts, state counts, and property parsing all validated.

#### What Surprised Us
- **Code review found 5 silent truncation bugs** across 3 review iterations. All used `as u16`/`as u8` casts that silently truncate — a pattern that's easy to write but dangerous. Every `as` cast should be questioned.
- **One bug was a silent data drop** — out-of-bounds state IDs were silently skipped instead of returning an error. This would have been extremely hard to debug in production.
- **Item registry had inconsistent error handling** compared to block registry — clamping to `MAX` instead of erroring. Consistency matters.

#### Patterns Established
- ✅ **Never use `as` for narrowing casts in data loading.** Always use `u16::try_from()` / `u8::try_from()` with proper error propagation. This applies to all registry/data loading code. *(Convention established and followed.)*
- ✅ **Review→Fix→Re-review loop is essential.** Each review pass caught a different class of issue. The loop terminated after 3 passes with zero findings. *(Process adopted in lifecycle.)*
- ✅ **Error types should distinguish failure modes:** `InvalidStateId(u64)`, `MissingStateId(String)`, `InvalidItemProperty(String, &'static str, u64)` each tell you exactly what went wrong. *(Pattern established.)*

#### Gotchas
- **Git-tracked binary data:** `.json.gz` files in `src/data/` must be explicitly `git add`ed — they're not matched by default patterns. First CI run failed because they weren't committed.
- **`as` casts compile silently** even when they truncate. Clippy's `cast_possible_truncation` lint would catch these, but it's not enabled by default. *(Decision made: lint kept as `allow` in workspace `Cargo.toml` — team uses `try_from()` by convention instead of relying on the lint. See Cargo.toml line 132.)*

#### Metrics
- **Files:** 10 changed, 676 insertions (+ 35 fix insertions)
- **Tests:** 19 registry tests, 632 total workspace
- **Review iterations:** 3 (truncation bugs → out-of-bounds drop → item clamping)
- **Blocks:** 1168, **States:** 29873, **Items:** 1506

---

### 2026-07-14 — Phase 9 Review: Global palette bit width is registry-derived

**Context:** Lifecycle re-run of Phase 9 (Chunk Data Structures). Full Java-vs-Rust comparison
of `Strategy.java`, `Configuration.java`, `PalettedContainer.java`, `SimpleBitStorage.java`.

#### Critical Discovery — Global Palette Bit Width

The most dangerous bug found: the Rust `upgrade_and_set` was using `bits_for_count(distinct_values)`
(e.g. 9 bits for 257 distinct block states) for Global palette BitStorage creation. Vanilla uses
`globalPaletteBitsInMemory = ceillog2(registry.size())` — 15 bits for 29,873 block states, 7 bits
for 65 biomes. This caused a **wire format mismatch**: the Rust server would write 586 longs
(9-bit packing) but the vanilla client expects 1024 longs (15-bit packing), causing a crash.

**Key insight:** Java `Configuration.java` has two distinct bit values:
- `bitsInMemory` — used for BitStorage allocation (the number of bits per entry in the long array)
- `bitsInStorage` — written as the wire format byte (palette type discriminator)

For Global palette: `bitsInStorage` can be anything ≥ threshold (client ignores the exact value,
only uses it to determine palette TYPE). But `bitsInMemory` **must** be `ceillog2(registry_size)`.

**Rule:** Always read `Configuration.java` alongside `Strategy.java` — the Strategy creates
Configurations, but the Configuration fields are what actually control wire format.

#### Biome vs Block Palette Differences

- Block states: SingleValue (0) → Linear 4-bit (1–4) → HashMap (5–8) → Global 15-bit (9+)
- Biomes: SingleValue (0) → Linear 1/2/3-bit (1–3) → Global 7-bit (4+). **No HashMap palette!**
- Biome registry has 65 entries (vanilla 26.1-pre-3), needing 7 bits not 6

#### Other Fixes Applied (all ✅ resolved)

- Added `get_and_set()` to `BitStorage` and `PalettedContainer` (vanilla uses for atomic get+set)
- Added `ticking_block_count` / `ticking_fluid_count` to `LevelChunkSection` (in-memory only)
- Added `WorldSurfaceWg` and `OceanFloorWg` heightmap types
- Improved `PalettedContainerError` with `InsufficientData` and `MalformedVarInt` variants
- Optimized `upgrade_and_set` distinct counting with `HashSet` instead of clone+sort+dedup
- Added `bits_per_entry()` accessor

#### Lessons

- **Global palette bits are NOT the same as the palette threshold** — they are `ceillog2(registry_size)`
- **Always verify wire format against vanilla client expectations**, not just server encoding logic
- **Biome count matters:** 65 biomes need 7 bits. If data packs add biomes, this must be dynamic.
  Consider making `global_palette_bits` runtime-configurable in a future phase.
- **Code review catches real bugs** — the biome bits issue (6 vs 7) was caught by the review agent

#### Metrics

- **Tests:** 83 → 87 (4 new: global roundtrip, get_and_set ×2, bits_per_entry)
- **Files changed:** 4 (`bit_storage.rs`, `paletted_container.rs`, `section.rs`, `heightmap.rs`)
- **Review iterations:** 2 (initial review found biome bits issue → fixed → clean)

---

### Phase 10 — Anvil World Loading (2025-07)

#### What Went Well

- Straightforward implementation — the Anvil format is well-documented
- `thiserror` error handling kept all error cases typed and clear
- Reusing existing `PalettedContainer`/`BitStorage`/`DataLayer` types worked cleanly
- All 120 tests pass (added ~30 new tests for anvil + storage modules)

#### Key Design Decisions

- **`PrimaryLevelData` uses raw `i32`/`i8` for game_type/difficulty** — `oxidized-world` cannot
  depend on `oxidized-protocol` (lower-layer rule). Conversion to `GameType`/`Difficulty` enums
  happens at the server layer.
- **`PalettedContainer::from_nbt_data()`** — new constructor for disk palette format (NBT
  palette + i64 LongArray) vs wire format (VarInt palette + network bytes). Disk format
  uses variable-length palettes stored as NBT compounds.
- **External `.mcc` chunks are logged and skipped** — extremely rare edge case, not worth
  implementing in this phase.
- **Region file I/O is synchronous** — called from `tokio::task::spawn_blocking` via
  `AsyncChunkLoader`.

#### Gotchas

- `oxidized_nbt` re-exports `read_bytes`, `read_file`, `write_file` at crate root — don't
  use `oxidized_nbt::io::*` (the `io` module is private)
- NBT `NbtCompound` getters return `Option<T>`, not `Result` — need `.ok_or_else()` wrapping
- `NbtList` has `compounds()` iterator but no `strings()` — use `iter()` + `NbtTag::as_str()`
- Clippy denies `expect()` in production code — use `match`/`let-else` instead
- Disk palette for biomes uses `List<String>` (resource IDs), blocks use `List<Compound>`
  (Name + Properties) — different deserialization paths needed
- LZ4 on disk uses block mode (`lz4_flex::decompress_size_prepended`), not framed mode

#### Metrics

- **Tests:** 87 → 120 (33 new tests across anvil and storage modules)
- **New files:** 9 (anvil: 5, storage: 4, including mod.rs files)
- **Modified files:** 6 (lib.rs, section.rs, paletted_container.rs, 2× Cargo.toml, Cargo.lock)
- **Lines added:** ~1,738

---

### 2026-03-18 — Phase 10 Re-run: Header Validation Bugs

**Context:** Full lifecycle re-run of Phase 10 comparing Java `RegionFile.java` constructor
against the Rust `read_header()` implementation.

#### Bugs Found

1. ✅ **Missing header entry sanitization (critical):** Java's `RegionFile` constructor validates
   all 1024 offset entries during header read and zeros out invalid ones: `sector_number < 2`
   (overlaps header), `sector_count == 0`, or `end_sector > file_sectors`. The Rust code stored
   raw entries without validation. *(Resolved — sanitization added to `read_header()`.)*

2. ✅ **Missing payload-vs-sector bounds check (medium):** After reading the 4-byte `payload_len`,
   Java validates it doesn't exceed `numSectors * SECTOR_BYTES`. *(Resolved — bounds check
   added: `payload_len + 4 <= sector_count * SECTOR_BYTES`.)*

3. ✅ **Error variant misuse (low):** `AnvilError::Decompression` was abused for mutex poisoning
   and `JoinError` in `AsyncChunkLoader`. *(Resolved — `AnvilError::Internal(String)` added.)*

#### Lessons

- **Always validate untrusted data at parse time**, not at use time. Java sanitizes during
  header read; deferring validation to `read_chunk_data` missed the `sector_count == 0` case.
- **Error types are semantic contracts.** Using `Decompression` for internal errors confuses
  callers who might retry decompression failures but should not retry mutex poisoning.
- **Our validation is intentionally stricter than Java's** for the EOF edge case: Java checks
  `sectorStart > fileSize` while Rust checks `sectorEnd > fileSize`. Both reject clearly invalid
  entries; Rust additionally rejects sectors that start at EOF (which would fail to read anyway).

#### Metrics

- **Tests:** 120 → 123 (3 new: sector_count_zero, header_overlap, payload_overflow)
- **Review iterations:** 2 (unused import + comment accuracy → fixed → clean)

---

### Architectural Audit — Phases 1–10 (Session 2)

**Date:** 2026-03-18
**Scope:** Full architectural review of all code through Phase 10

#### Lifecycle Process Improvement

Promoted "Architectural Questioning" from a soft sub-step (Stage 6.0 — during implementation)
to a **hard gate** (Stage 2.5 — between Research and Decide). This ensures ADRs are validated
*before* planning and test writing, preventing wasted work when an ADR needs superseding.

Updated: `docs/lifecycle/README.md` and `.github/copilot-instructions.md`.

#### Key Findings

1. **ChunkPos duplication (CRITICAL, deferred):** Defined in both `oxidized-protocol` and
   `oxidized-world`. Cannot fix without a shared `oxidized-types` crate (needs ADR). Both
   definitions have TODO comments. *(Still open — workaround in place. Separate definitions
   coexist; no data is shared across the crate boundary yet.)*

2. ✅ **#[non_exhaustive] added to all 31 public error enums:** Prevents breaking changes when
   adding error variants. Affects: oxidized-nbt (1), oxidized-protocol (23), oxidized-world (6),
   oxidized-server (1).

3. **Typestate NOT implemented (ADR-008):** Connection uses runtime enum, not compile-time
   `Connection<State>`. Known deviation — acceptable for current phase count but should be
   addressed before Play state packets proliferate. *(Still open — acceptable technical debt.)*

4. **Zero-copy NBT (ADR-010 partial):** Only the owned tree is implemented. Arena and borrowed
   reader are deferred until chunk sending at scale (Phase 13+). *(Still open — not yet needed.)*

5. **DashMap chunk storage (ADR-014):** Not yet needed — only data structures exist. Required
   at Phase 11 (Server Level). *(Still open.)*

#### Patterns

- The crate layering rules (`oxidized-world ← oxidized-nbt` only) prevent sharing coordinate
  types between protocol and world. A shared `oxidized-types` crate is the right solution.
- All 31 error enums were missing `#[non_exhaustive]` — add it to every new public enum.
- Stage 2.5 (Architectural Review Gate) should be followed for every phase going forward.

---

### 2026-03-18 — Phase 11 Re-run: Vanilla Data Verification

**Context:** Full lifecycle re-run of Phase 11 (Server Level + Block Access) comparing
implementation against Java Block.java, DimensionType.java, and vanilla generated data.

#### Bugs Found

1. ✅ **BlockFlags too narrow (critical):** Java's `Block.java` defines 11 flag constants with
   values up to 512 (bit 9). The Rust `BlockFlags` used `u8` (max 255), which cannot represent
   `UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS` (256) or `UPDATE_SKIP_ON_PLACE` (512). Widened to `u16`.
   *(Resolved — `flags.rs` now uses `u16` backing type with all 11 flags.)*

2. ✅ **End dimension wrong values (medium):** Vanilla 26.1 generated data shows End has
   `has_skylight: true` and `ambient_light: 0.25` — the Rust code had `false` and `0.0`.
   *(Resolved — `dimension.rs` now has correct End values.)*

3. ✅ **Overworld logical_height wrong (medium):** Vanilla data shows 384, not 320 as the phase
   doc specified. The phase doc was wrong — always verify against generated data.
   *(Resolved — `dimension.rs` now has `logical_height: 384`.)*

4. ✅ **DimensionType missing fields (medium):** Java 26.1 DimensionType record has
   `has_fixed_time`, `has_ender_dragon_fight`, `coordinate_scale` fields not present in Rust.
   *(Resolved — all three fields now present in `dimension.rs`.)*

5. ✅ **LRU cache O(n) performance (medium):** Hand-rolled VecDeque-based LRU used `retain()`
   (O(n)) on every get(). Replaced with `lru` crate for O(1) operations.
   *(Resolved — `lru` crate v0.16 integrated as workspace dependency.)*

#### Lessons

- **Always verify dimension values against vanilla generated data** (`mc-server-ref/generated/data/
  minecraft/dimension_type/*.json`), not against the phase doc or Java source alone. The generated
  data is the ground truth.
- **Check Java constant ranges before choosing a Rust backing type.** BlockFlags needed `u16` not
  `u8` — a quick scan of Block.java would have caught this during initial implementation.
- **The `lru` crate is O(1) and well-maintained** — prefer it over hand-rolled LRU implementations
  using VecDeque+HashMap.

#### Metrics

- **Tests:** 31 → 42 (11 new)
- **Files changed:** 7
- **Review iterations:** 2 (doc comment fixes → clean)

---

### Phase 12 — Player Join + World Entry

**Date:** 2026-03-18

#### What Went Well
- Vec3, BlockPos, ResourceLocation types from oxidized-protocol integrated cleanly into oxidized-game
- GameProfile constructor pattern (new/with_properties) works well for both test and auth code paths
- parking_lot::RwLock + Arc pattern for thread-safe player storage is ergonomic
- NBT load/save with graceful defaults (no panics on missing fields) matches vanilla behavior

#### What Surprised Us
- GameProfile.uuid() returns Option<Uuid> because the internal storage is a hex string — callers must handle this
- Phase doc pseudocode references types/APIs that don't exist exactly as shown (e.g., profile.uuid vs profile.uuid())
- NbtList::push() returns Result — need explicit `let _ =` to suppress warnings
- ResourceLocation uses from_string() not try_parse()

#### What Should Change
- **Lifecycle compliance is non-negotiable.** First attempt skipped TDD, code review, arch review gate, and retrospective. Must follow Identify → Research → Arch Review → Plan → Test First → Implement → Review → Integrate → Retrospect every time.
- Phase doc pseudocode should be treated as aspirational, not literal — always verify actual API signatures

#### Patterns Established
- `ServerPlayer::new(entity_id, profile, dimension, game_mode)` — entity ID from PlayerList, not global static
- `PlayerList::next_entity_id()` — atomic counter owned by the list, not a global
- `GameMode::from_id(i32) -> Self` — defaults to Survival for unknown IDs (matches vanilla)
- `PlayerList::add()` returns `Arc<RwLock<ServerPlayer>>` for immediate use by caller
- Test helpers: `make_test_player(id, name)` and `make_player_with_id(list, name)` patterns

#### Technical Debt
- PlayerInventory is a stub (Phase 22) *(Still open — by design.)*
- No ECS component integration yet (Phase 14+ per ADR-020) *(Still open — by design.)*
- ✅ Minimal PLAY read loop only handles teleport confirmations — full PLAY handling is Phase 14+
  *(Superseded — Phase 14 implemented full movement, input, and player command handling.)*

#### Metrics
- **Tests:** 75 game + 471 protocol = 546 total (all pass, 0 warnings)
- **Files created:** 16 new files (6 game, 10 protocol)
- **Files modified:** 4 (lib.rs, packets/mod.rs, auth.rs, primary_level_data.rs)
- **Review iterations:** 1 (clean pass)

### Phase 12 — Server Integration (2025-07)

**Date:** 2025-07

#### What Went Well
- Wiring ServerContext through the connection handler was clean — only 2 files needed changes
- build_login_sequence() from oxidized-game integrated directly — all 8 packets sent in order
- authenticate_online() refactored to return GameProfile directly (cleaner than decomposed tuple)
- Code review caught real bugs: ghost player on send failure, dead `unreachable!()` code

#### What Surprised Us
- Two distinct ProfileProperty types exist: `auth::ProfileProperty` (private fields, Deserialize) vs `packets::login::ProfileProperty` (public fields) — must convert between them
- `disconnect()` always returns `Err` — using `disconnect()?; unreachable!()` is dead code; use `return Err(disconnect_err(...).await)` instead
- PrimaryLevelData has no Default impl — must use `from_nbt(&NbtCompound::new())` for defaults

#### Patterns Established
- `return Err(disconnect_err(conn, msg).await)` — consistent disconnect pattern (no unreachable)
- Add player to PlayerList AFTER sending login packets — prevents ghost entries on send failure
- `ServerContext` struct: shared server state (PlayerList, PrimaryLevelData, dimensions) wrapped in Arc
- `map_err(|e| anyhow::anyhow!("context: {e}"))?` for infallible-in-practice calls in main.rs (no expect)

#### Technical Debt
- ✅ PLAY read loop is minimal (teleport confirmations only) — full handling Phase 14+
  *(Superseded — Phase 14 added movement, input, player commands, chunk tracking.)*
- No player removal from PlayerList on disconnect (cleanup is best-effort log + remove)
  *(Still open.)*
- PlayerConnection bridge channels (ADR-020) not yet implemented *(Still open.)*

---

### 2025-07-25 — Phase 13: Chunk Sending

**Context:** Implementing the full chunk sending pipeline so vanilla clients render chunks.

#### Key Discovery: Heightmap Wire Format Changed in 26.1-pre-3
The phase doc describes heightmaps as NBT-encoded, but **26.1-pre-3 uses a binary map format**:
`VarInt(map_size) [VarInt(type_id) VarInt(longs_count) i64[]]...`

This was discovered by tracing `ClientboundLevelChunkWithLightPacket` → `ChunkData` →
`ByteBufCodecs.map()` in the decompiled Java reference. Always verify wire formats against
the actual Java source, not just phase docs.

#### Heightmap Type IDs (Java enum ordinals)
- `WORLD_SURFACE_WG` = 0, `WORLD_SURFACE` = 1, `OCEAN_FLOOR_WG` = 2
- `OCEAN_FLOOR` = 3, `MOTION_BLOCKING` = 4, `MOTION_BLOCKING_NO_LEAVES` = 5
- Client receives only `WORLD_SURFACE`(1) and `MOTION_BLOCKING`(4)

#### Chunk Batch Protocol
- Server sends: `BatchStart` → N × `LevelChunkWithLight` → `BatchFinished(count)`
- Client responds: `ChunkBatchReceived(desired_chunks_per_tick: f32)`
- **Validate client rate** — clamp to (0.1, 100.0) and reject NaN/infinity

#### LevelChunkSection Wire Format
Each section writes: `i16(non_empty_block_count)` + `i16(fluid_count)` +
`PalettedContainer(block_states)` + `PalettedContainer(biomes)`.
The fluid count was added in 26.1-pre-3 — older protocol docs may not show it.

#### Patterns Established
- `send_initial_chunks()` — sends empty air chunks in spiral order for initial join
- `build_chunk_packet()` in `oxidized-game::net::chunk_serializer` — bridge between world and protocol
- `build_light_data()` — converts DataLayer arrays to BitSet masks + 2048-byte arrays
- `spiral_chunks()` — closest-first iteration for chunk sending order

#### Technical Debt
- Chunks are empty air (no worldgen/disk loading) — real chunks in later phases *(Still open.)*
- No per-tick chunk throttling — all chunks sent in one batch during login *(Still open.)*
- Block entities always VarInt(0) — no block entity support yet *(Still open.)*

### 2026-03-19 — Testing Infrastructure: ADR-034 Compliance

**Context:** Expanded from unit-tests-only to 6 of 8 ADR-034 test types.

#### Test Infrastructure Summary
| Type | Count | Framework | Status |
|------|-------|-----------|--------|
| Unit | 908 | `#[test]` | ✅ Pre-existing |
| Integration | 40 | `tests/` dirs | ✅ Added |
| Property-based | 25 | `proptest` | ✅ Added |
| Compliance | 5 | custom | ✅ Added |
| Doc tests | 37 | `///` examples | ✅ Added |
| Snapshot | 27 | `insta` | ✅ Added |
| Fuzz | 0 | `cargo-fuzz` | ❌ Future |
| Benchmark | 0 | `criterion` | ❌ Future |

#### Key Decisions
- **Integration tests use public API only** — no `pub(crate)` access. Files in `crates/*/tests/`.
- **Proptest added to 4 crates** (nbt, protocol, world, game) — covers all codecs/parsers per ADR-034.
- **Insta snapshot tests for error Display** — prevents accidental error message changes.
  Snapshots are `.snap` files next to source in `snapshots/` dirs.
- **Compliance tests** in `crates/oxidized-protocol/tests/compliance.rs` — VarInt/VarLong
  wiki.vg test vectors + handshake packet byte-for-byte verification.

#### Test Conventions Established
- Integration test files: `crates/<crate>/tests/<descriptive_name>.rs`
- Every test file starts with `#[allow(clippy::unwrap_used, clippy::expect_used)]`
- Proptest functions named `proptest_<thing>_<invariant>`
- Snapshot test functions named `test_<error_type>_display_snapshots`
- Doc examples must be self-contained (no external state)

#### What's Missing (Still Needed)
- **Connection state tests** — `Connection::new()` requires real TcpStream; needs refactoring
  to extract state logic for unit-level testing *(Still open.)*
- **Fuzz tests** — need `cargo-fuzz` infrastructure setup *(Still open.)*
- **Benchmarks** — need `criterion` setup in `benches/` dirs *(Still open.)*
- ✅ **View distance capping** — *(Resolved — server now caps client view_distance to config
  max via `i32::from(client_info.view_distance).min(server_ctx.max_view_distance)` in network.rs.)*

#### ✅ Heightmap CLIENT_TYPES Fix
Phase 13 was missing `MotionBlockingNoLeaves` (type_id=5) in CLIENT_TYPES.
Java sends 3 client types: WORLD_SURFACE(1), MOTION_BLOCKING(4), MOTION_BLOCKING_NO_LEAVES(5).
*(Resolved in commit 478d145.)*

#### ✅ LEVEL_CHUNKS_LOAD_START Fix
Vanilla sends `GameEvent(13, 0.0)` after initial chunk batch — signals client to exit
"Loading Terrain" screen. We were missing this packet entirely.
*(Resolved in commit 8315483.)*

### Phase 14 — Player Movement (2025-07)

#### What Went Well
- Parallel agent dispatch (3 agents: serverbound, clientbound, game logic) worked perfectly
  — all three compiled together on first try with no conflicts
- Packet ID verification method (counting addPacket() calls in GameProtocols.java) confirmed
  reliable — all 15 pre-existing IDs matched, all 8 new IDs verified correct

#### Phase Doc Errors Discovered
- **Sneak handling is WRONG in phase doc**: Phase doc describes `PressShiftKey`/`ReleaseShiftKey`
  actions in `ServerboundPlayerCommandPacket`. In 26.1-pre-3, sneak is handled via
  `ServerboundPlayerInputPacket` (Input.java) with bit flags, NOT PlayerCommand.
- **PlayerCommandAction enum is WRONG**: Phase doc shows 9 actions starting from PressShiftKey=0.
  Actual 26.1-pre-3 enum has 7 actions: StopSleeping=0, StartSprinting=1, StopSprinting=2,
  StartRidingJump=3, StopRidingJump=4, OpenInventory=5, StartFallFlying=6.
- **Input.java format**: Single byte with 7 bit flags: forward(0x01), backward(0x02),
  left(0x04), right(0x08), jump(0x10), shift/sneak(0x20), sprint(0x40).

#### Key Patterns
- Movement validation: `validate_movement()` in `oxidized_game::player::movement` —
  MAX_MOVEMENT_PER_TICK=100.0, coordinate clamp ±3.0e7, pitch clamp ±90°
- Delta encoding: scale factor 4096.0 (1 block = 4096 units as i16), max delta ~7.999 blocks
- `PlayerChunkTracker` wraps `chunks_to_load()`/`chunks_to_unload()` with persistent HashSet
- Lock guards must be dropped before `.await` points in network.rs (use block scoping)

#### What to Remember
- Always verify phase doc claims against Java reference — phase docs were written before
  detailed 26.1-pre-3 analysis and may contain pre-26.1 information
- Sprint state is redundantly sent in BOTH PlayerCommand and PlayerInput packets —
  server handles both to stay in sync

### Phase 15 — Entity Framework + Tracking

#### What Went Well
- TDD cycle worked smoothly — 20 new tests including property-based tests for AABB, 
  tracker, serializer types, and all 3 entity packets
- Java source verification prevented 3 major implementation errors from the phase doc
- Entity module structure is clean: id, synched_data, data_slots, aabb, tracker, mod.rs

#### What Surprised Us
1. **43 serializers, not 31**: Phase doc listed 31 `EntityDataSerializers` (IDs 0–30). 
   Java `EntityDataSerializers.java` static block registers 43 (IDs 0–42). New 26.1 types 
   include CatSoundVariant, CowVariant, PigVariant, ChickenVariant, ZombieNautilusVariant, 
   CopperGolemState, WeatheringCopperState, HumanoidArm, etc. Order diverges from phase 
   doc at ID 13 (OptionalLivingEntityReference, not OptUuid).
2. **LpVec3 velocity encoding**: Phase doc said `i16 * 8000`. Actually uses 
   `net.minecraft.network.LpVec3` — a complex bit-packed format with 15-bit quantization, 
   shared scale factor, and optional VarInt continuation. Zero vectors = single byte 0x00.
3. **Tracking ranges in chunks**: Java's `EntityType.clientTrackingRange()` returns chunk 
   counts (×16 for blocks). Default = 5 chunks = 80 blocks. Player = 32 chunks = 512 blocks.
4. **SetEntityData decode limitation**: Without a codec registry, full decode of multi-entry 
   packets is impossible — each serializer type has different byte-length values. Current 
   decode handles single-entry packets correctly; multi-entry needs registry-aware decoder.

#### Technical Decisions
- `DataSerializerType` uses `#[repr(u32)]` with exhaustive `match` for `from_id()` — no 
  unsafe transmute since `#![deny(unsafe_code)]` is enforced
- `SynchedEntityData` uses `Box<dyn Any + Send + Sync>` for type-erased storage — allows 
  any Rust type to be stored while maintaining dirty tracking
- Entity struct is monolithic (not ECS Components yet) — will decompose when bevy_ecs 
  World/systems are introduced in later phases (per ADR-018)
- `ClientboundRemoveEntitiesPacket::decode()` validates negative VarInt counts to prevent 
  DoS via massive allocation

#### Verified Packet IDs (26.1-pre-3)
- ClientboundAddEntityPacket = 0x01
- ClientboundRemoveEntitiesPacket = 0x4D (77)
- ClientboundSetEntityDataPacket = 0x63 (99)

#### What to Remember
- Phase doc serializer lists are WRONG for 26.1-pre-3 — always count IDs from 
  EntityDataSerializers.java static block
- LpVec3 is the velocity encoding, NOT i16*8000 — see net.minecraft.network.LpVec3
- Always validate VarInt counts before allocating (negative VarInt → huge usize on cast)
- Also validate count against `data.remaining()` — prevents allocation DoS even with positive counts
- Test entity packets with proptest for encode/decode roundtrips

### Phase 15 — Verification Pass (Re-run)

#### Findings
- **Tracking range constants were wrong**: Player was 10 chunks (should be 32), Animal was
  8 chunks (should be 10), Misc was 5 chunks (should be 6). Corrected in tracker.rs.
- **Missing allocation upper-bound**: `ClientboundRemoveEntitiesPacket::decode` only checked
  for negative counts but not inflated positive counts. Added `count > data.remaining()`
  bounds check to prevent DoS via buffer over-allocation.
- **Missing debug_assert on flag bit index**: `Entity::get_flag`/`set_flag` accepted any
  `u8` but only bits 0-7 are valid. Added `debug_assert!(bit < 8)`.
- **Integration test hardcoded old constant value**: Test used `80.0`/`80.01` instead of
  deriving from the `TRACKING_RANGE_MISC` constant. Fixed to use `range` variable.

#### Key Learning
- Java `EntityType.clientTrackingRange()` returns CHUNKS, not blocks. Player = 32 chunks
  (512 blocks), not 10 chunks. Always multiply by 16 to get block distance.
- Bounds-checking Vec allocation against remaining buffer bytes is essential safety hardening
  beyond just checking for negative counts.

### Phase 16 — Basic Physics (2026-03)

#### What Went Well
- Java reference verification prevented 5 major discrepancies from phase doc
- TDD cycle smooth — 40 new tests (unit + integration), all passing on first fix cycle
- Code review passed with zero significant issues

#### Phase Doc Errors Discovered
1. **Axis order WRONG**: Phase doc uses X→Y→Z. Java uses Y first, then X/Z by movement 
   magnitude (`Direction.axisStepOrder()`): |dx|>=|dz| → Y→X→Z, else Y→Z→X.
2. **Gravity timing WRONG**: Phase doc applies gravity first. Java applies AFTER friction/input.
3. **Velocity packet WRONG**: Phase doc says i16*8000. Java uses LpVec3 (same as AddEntity).
4. **Powder snow speed WRONG**: Phase doc says 0.3. Java uses `makeStuckInBlock(Vec3(0.9, 1.5, 0.9))`.
5. **VoxelShape::translated bug**: Phase doc has `bx` instead of `bz` for max_z.
6. **Blue ice missing**: Phase doc doesn't mention BLUE_ICE (friction=0.989 vs 0.98 for other ice).

#### Key Patterns
- `physics_tick()` in `oxidized_game::physics::tick` — full per-tick physics update
- `collide_with_shapes()` — movement-dependent axis ordering (Y first)
- `clip_x/y/z()` — per-axis AABB sweep collision
- `collect_obstacles()` — gather block shapes in swept volume
- `apply_jump()` in `oxidized_game::physics::jump` — jump with boost/sprint
- `BlockShapeProvider` trait + `FullCubeShapeProvider` — block collision shape lookup
- `VoxelShape::translated()` — convert block-local shapes to world-space Aabb

#### Block Friction Values (from Blocks.java)
- Default: 0.6
- ICE/PACKED_ICE/FROSTED_ICE: 0.98
- BLUE_ICE: 0.989
- SLIME_BLOCK: 0.8

#### Technical Debt (Resolved)
- ~~Block friction/speed lookups are stubbed~~ → **RESOLVED**: `PhysicsBlockProperties` dense lookup table wired to `BlockRegistry` (commit 0901789)
- ~~Slime block bounce not implemented~~ → **RESOLVED**: Negates vy on landing when on slime block (commit 0901789)

#### Technical Debt (Remaining)
- No step-up algorithm yet — Entity default is 0.0, LivingEntity uses STEP_HEIGHT attribute (0.6)
- No entity-entity collision (boats, minecarts, mob pushing)
- Honey block sticky sliding not implemented
- Cobweb/sweet berry/bubble column velocity modifiers not implemented

#### Pattern: PhysicsBlockProperties
- Dense `Vec<f64>` arrays indexed by block state ID for O(1) friction/speed/jump lookups
- Built from `BlockRegistry` at startup via `PhysicsBlockProperties::from_registry()`
- `PhysicsBlockProperties::defaults()` returns empty vecs (all lookups return defaults) — use in tests that don't care about block-specific physics
- Located in `crates/oxidized-game/src/physics/block_properties.rs`
- Add new block overrides to `PHYSICS_OVERRIDES` const array

### Phase 17 — Chat System (2025-07)

#### Key Architecture Decisions
- **Component lives in `oxidized-protocol/src/chat/`**, not `oxidized-game` — protocol packets
  reference Component directly, and game depends on protocol (not vice versa)
- **Component wire format is NBT** on the play-state wire, NOT JSON strings. The phase doc
  was wrong — vanilla uses `ComponentSerialization.TRUSTED_STREAM_CODEC` which is NBT-based.
  JSON is only used for status response (server list ping)
- **Chat broadcast uses `tokio::sync::broadcast` channel** stored in `ServerContext` —
  each player's play loop subscribes via `tokio::select!` to receive broadcasts
- **ADR-028 mandates manual serde** (not derive) for Component JSON — the JSON format varies
  by content type (text/translate/selector have different tag keys)

#### Verified Packet IDs (26.1-pre-3)
- ServerboundChatPacket = 0x09
- ServerboundChatCommandPacket = 0x07
- ServerboundChatAckPacket = 0x06
- ClientboundPlayerChatPacket = 0x40
- ClientboundSystemChatPacket = 0x79
- ClientboundDisguisedChatPacket = 0x20
- ClientboundDeleteChatPacket = 0x1E

#### Gotchas
- `NbtCompound::put_string()` returns `Option<NbtTag>` (previous value), not `()`
- `NbtList::push()` returns `Result` (type validation) — use `let _ =` to suppress
- Raw string literals containing `#` need `r##"..."##` syntax
- Component `to_nbt()` returns `NbtTag`, not `NbtCompound` — match on tag variant
- Phase doc packet IDs were wrong — always verify against `GameProtocols.java`

#### Technical Debt
- `/say` and `/me` are simple string matching — full command dispatcher comes in Phase 18
- Rate limiter sends warning but doesn't disconnect persistent spammers
- `read_component_nbt` uses unlimited NbtAccounter — fine for server-created clientbound
  packets but should be bounded if ever used for untrusted input

---

### Phase 17 Re-verification (post-audit)

#### Issues Found & Fixed
1. **LastSeenMessagesUpdate missing checksum byte** — Java `LastSeenMessages.Update` has
   `(VarInt offset, FixedBitSet(20) acknowledged, byte checksum)`. The trailing checksum
   byte was missing, which would misalign parsing of `ServerboundChatPacket`. Fixed.
2. **ClientboundPlayerChatPacket missing globalIndex** — Java has `globalIndex` (VarInt) as
   the first field; it's a per-connection counter. Was missing from the Rust struct. Fixed.
3. **/say used plain text SystemChatPacket** — Should use `ClientboundDisguisedChatPacket`
   with `SAY_COMMAND` chat type (registry id 1). Client-side decoration handles
   `[%s] %s` formatting. Fixed.
4. **/me used plain text SystemChatPacket** — Should use `ClientboundDisguisedChatPacket`
   with `EMOTE_COMMAND` chat type (registry id 6). Client-side decoration handles
   `* %s %s` formatting. Fixed.

#### Verified Correct
- All 7 chat packet IDs match GameProtocols.java (bundle delimiter offsets CB IDs by 1)
- Component system (component.rs, style.rs, formatting.rs) — no changes needed
- Chat type registry: 0=chat, 1=say_command, 2=msg_command_incoming, 3=msg_command_outgoing,
  4=team_msg_command_incoming, 5=team_msg_command_outgoing, 6=emote_command
- ChatType.Bound encoding: Holder VarInt (id+1) + NBT sender name + optional NBT target
- Regular chat broadcast as SystemChatPacket with `chat.type.text` translatable is acceptable
  for unsigned mode (PlayerChatPacket is for signed chat flow)

#### Lessons Learned
- **Always verify sub-fields of composite types** against the Java reference. The
  `LastSeenMessagesUpdate` looked complete with offset + bitset, but the checksum byte
  at the end was invisible unless reading the actual Java `Update` record definition.
- **Vanilla commands send message content only** — the chat type decoration (defined in
  the registry) handles formatting. Don't construct formatted strings server-side for
  commands that have a registered chat type.

### Keepalive & Color System (Ad-hoc)

#### What was implemented
- **Keepalive packets**: CB 0x16, SB 0x1C — both are single `i64` field (read/writeLong).
  Located in `common` package in vanilla (shared between game/config states).
- **Keepalive timer**: 15s interval via `tokio::time::interval`, challenge = `SystemTime`
  millis since epoch. Disconnect after 30s with no valid response.
- **Unified color parsing**: `Component::from_legacy_with_char(s, char)` accepts both `§`
  and a custom prefix character. Applied to MOTD, chat messages, /say, /me commands.
- **Config**: `[chat]` section with `color_char` field (default `"&"`). Validated: single
  non-alphanumeric ASCII char or empty string (disabled).

#### Gotchas
- **`Instant::now().elapsed()` is always ~0** — Don't use it for challenge generation.
  Use `SystemTime::now().duration_since(UNIX_EPOCH).as_millis() as i64` instead.
- **Keepalive packet IDs determined by counting**: CB 0x16 and SB 0x1C were found by
  counting `addPacket()` calls in GameProtocols.java. If client doesn't respond,
  verify these IDs against wiki.vg or actual packet captures.

---

### Phase 18 — Commands (Brigadier)

**Date:** 2025-07-17  
**Scope:** `oxidized-game/src/commands/`, `oxidized-protocol/src/packets/play/`, `oxidized-server/src/network.rs`

#### Patterns & Best Practices

- **`#[derive(Clone)]` on generic structs adds `S: Clone` bound** — Even if `S` only
  appears inside `Arc<dyn Fn(&S)>` or `PhantomData<S>`, Rust's derive macro adds
  `S: Clone` to the impl. Fix: write manual `Clone` impls without the bound.
- **ServerHandle trait for cross-crate interaction** — `oxidized-game` cannot depend on
  `oxidized-server` (crate dependency rules). Solution: define a `ServerHandle` trait in
  `oxidized-game::commands::source`, implement it on `ServerContext` in `oxidized-server`.
- **BFS serialization for command tree** — The Brigadier wire format uses a flat array of
  nodes with child indices. BFS traversal produces correct index ordering. Permission
  filtering must happen during serialization (skip nodes the player can't see).
- **CommandNode as Clone-able enum** — Root/Literal/Argument variants. Children stored in
  `BTreeMap` for deterministic wire format ordering.
- **Feedback via broadcast channel** — Commands can't own the connection, so feedback
  messages go through `chat_tx: broadcast::Sender`. Server-side message display.
- **Builder DSL** — `literal("name").then(argument("arg", type).executes(fn))` mirrors
  Java Brigadier. Functions are `Arc<dyn Fn(...)>` for Clone + Send + Sync.

#### Gotchas

- **Argument type IDs** — 57 entries (0-56), NOT 56. The count includes `uuid` at ID 56.
  Order comes from `ArgumentTypeInfos.java` bootstrap() method. Getting these wrong causes
  client-side tab-completion to silently fail or crash.
- **Time argument overflow** — Vanilla allows `999999d` which when multiplied by 24000
  overflows `i32`. Use `checked_mul()` for all time multipliers.
- **QuotablePhrase escape sequences** — StringReader for quoted strings must handle `\"`
  and `\\` escape sequences character-by-character. A simple `find('"')` misses escaped
  quotes and produces wrong parse results.
- **Tooltip encoding in CommandSuggestions** — Uses JSON string encoding (not NBT),
  unlike most play-state Component encoding. The vanilla protocol uses
  `ComponentSerialization.TRUSTED_STREAM_CODEC` which in this context serializes as JSON.
- **Entity selector parsing stubbed** — Full `@a`, `@e`, `@p`, `@s`, `@r` parsing with
  filters (`[distance=..10,type=zombie]`) is complex. Phase 18 reads it as a raw string.
  Full parsing comes in later phases when entity queries are available.
- **Permission levels hardcoded** — All connected players get permission level 4 (op).
  Real permission reading from player data comes in a later phase.
- **`reader.remaining()` borrows from reader** — Can't call `reader.remaining()` (returns
  `&str`) and then mutate the reader. Fix: `.to_string()` to own the data before parsing.

#### Architecture Notes

- **56 vanilla argument types** mapped in `ArgumentType` enum with `registry_id()` and
  `write_properties()`. Properties vary per type (e.g., Float has min/max flags + values,
  Entity has flags byte, String has enum 0/1/2).
- **Wire format node flags**: bits 0-1 = type (root/literal/argument), bit 2 = executable,
  bit 3 = has redirect, bit 4 = has custom suggestions.
- **16 core commands implemented**: stop, tp, gamemode, give, kill, time, weather, say, me,
  help, list, kick, difficulty, seed, setblock, effect, gamerule.
- **Commands packet sent during play entry** — after cache radius, before chunk loading.
  Ensures client has the command tree for tab-completion before the player is fully in-game.

#### Test Coverage

- 32 unit tests: 20 dispatcher (parse, execute, permissions, completions, serialization),
  6 arguments (registry IDs, property encoding), 6 context (escapes, time overflow).
- All pass alongside existing 1409 workspace tests (1441 total).

### 2026-07-XX — Phase 18b: Command System Improvements

**Context:** Post-phase-18 improvements to the Brigadier command framework.

#### Key Learnings

- **Vanilla translation keys for commands**: All command feedback should use
  `Component::translatable(key, args)` with vanilla keys like `"commands.time.query"`,
  `"commands.difficulty.success"`, etc. The vanilla arg order is verified in the Java
  decompiled source — never guess, always check `mc-server-ref/decompiled`.
- **Effect give translation args order**: `[effect_name, target_name, duration_seconds]` — 
  verified from `EffectCommands.java`.
- **Kick failure key**: Use `"argument.entity.notfound.player"` when the target player isn't
  found, NOT the success key.
- **`display_name` is a field, not a method** on `CommandSourceStack` — easy to confuse
  with accessor methods. Access as `ctx.source.display_name.clone()`.
- **RootCommandNode.children is a direct field** (`BTreeMap`), not accessed via `.children()`
  method. The `.children()` method exists only on the `CommandNode` enum, not the inner
  struct types.

#### Patterns Established

- **Command descriptions**: `literal("cmd").description("...")` stores description in
  `Option<String>` on the node. `ServerHandle::command_descriptions()` returns
  `Vec<(String, Option<String>)>` for dynamic help enumeration.
- **Pagination pattern**: `PaginatedMessage::new(title, cmd_prefix).per_page(7)` then
  `.add_line()` and `.render_page(n)`. Navigation uses `ClickEvent::RunCommand` for
  `«« Previous` / `Next »»` buttons.
- **Interactive help entries**: Each command in `/help` uses `ClickEvent::SuggestCommand`
  (pre-fills the chat) + `HoverEvent::ShowText` (shows description tooltip).
- **Username autocomplete**: `get_completions` on the dispatcher takes `player_names: &[String]`
  parameter. Entity and GameProfile argument types use these to suggest online player names.
- **GameType::translation_key()**: Returns `"gameMode.survival"` etc. for use in translatable
  components where vanilla expects a translatable game mode name.

#### Architecture Decisions

- `command_descriptions()` lives on `ServerHandle` (returns data) rather than being a method
  on `Commands` — avoids needing `Commands` reference inside command execution closures.
- `Commands::dispatcher()` exposes `&CommandDispatcher` for `ServerContext` to enumerate the
  command tree when implementing `command_descriptions()`.

#### Test Coverage

- 13 new tests added: 8 pagination (single/multi-page, navigation, empty, boundary, min-per-page),
  3 description field (literal, argument, none), 2 username autocomplete (suggest + filter).
- Workspace total: 1454 tests (up from 1441).

### 2026-07-XX — Phase 18c: Autocomplete & Suggestion Fixes

#### Bugs Found & Fixed

1. **Suggestion packet `start`/`length` wrong** — `ClientboundCommandSuggestionsPacket` had
   `start: 0, length: input.len()` which told the client to replace the ENTIRE command text.
   Fix: use `StringRange` from suggestions + add `prefix_len` (1 for `/` prefix).

2. **Entity/GameProfile args missing `ask_server` flag** — Without `FLAG_SUGGESTIONS` (bit 4)
   and `suggestions_type: "minecraft:ask_server"`, the Minecraft client NEVER sends
   `ServerboundCommandSuggestionPacket` for entity args — it handles them client-side showing
   only `@a/@e/@s/@p/@r` selectors. Fix: auto-detect Entity/GameProfile args in serializer.rs.

3. **`collect_child_suggestions` had wrong offsets** — All `StringRange`s were `(0, word.len())`
   instead of being relative to the full input string. Fix: add `offset` parameter that
   accumulates through recursion: starts at `command_name.len() + 1`, each level adds
   `current_word.len() + 1`.

#### Key Learnings

- **Client only asks server for suggestions when `FLAG_SUGGESTIONS` is set** in the serialized
  command tree node. For Entity/GameProfile args, vanilla sets `suggestions_type` to
  `"minecraft:ask_server"`. Without this, autocomplete silently falls back to client-side only.
- **`start` field in suggestion packet is relative to the RAW command string the client sent**
  (including the `/` prefix). So if client sends `/give Al`, and suggestions replace "Al",
  then `start = 6` (position of 'A' in `/give Al`), `length = 2`.
- **Interactive chat (ClickEvent/HoverEvent) IS fully implemented** at the protocol level —
  `Component::to_nbt()` and `to_json()` both serialize click/hover events. The `feedback_sender`
  closure sends `ClientboundSystemChatPacket` which preserves the full component tree.
  **IMPORTANT:** Vanilla uses snake_case field names (`click_event`, `hover_event`) in both
  JSON and NBT serialization, NOT camelCase (`clickEvent`, `hoverEvent`). The data fixer
  `TextComponentHoverAndClickEventFix` renamed these fields. HoverEvent uses a flattened
  dispatch codec — ShowText uses `"value"` (not `"contents"`), ShowEntity uses `"id"` for
  entity type and `"uuid"` for UUID (not nested under `"contents"`), ShowItem fields are
  flattened directly into the hover event compound.

#### Test Coverage

- 7 new tests: 5 range correctness tests (single arg, multi arg, partial match, no match,
  trailing space), 1 serializer flag test (entity arg has ask_server), 1 updated existing.
- Workspace total: 1460 tests (up from 1454).

### 2026-07-XX — Phase 18d: Critical Bugs & Feature Completeness

#### Bugs Found & Fixed

1. **Help pagination click not working** — Client sends `ServerboundChatCommandSignedPacket`
   (0x08) when clicking RunCommand in chat, but server only handled unsigned variant (0x07).
   Fix: decode signed packet (extract command, skip signature fields), dispatch to same handler.

2. **Command feedback broadcasting to all players** — `feedback_sender` used `chat_tx` broadcast
   channel. Fix: use `std::sync::mpsc::channel` per command execution, drain after dispatch,
   send only to the executing player's connection.

3. **MOTD showing 0 players** — `ServerStatus` was created once at startup with `online: 0`.
   Fix: build status response dynamically in `handle_status()` by querying `player_list`.
   Important: scope `RwLockReadGuard` in a block to avoid holding it across `.await`.

4. **Tab list not updating for existing players** — No `PlayerInfoUpdate` broadcast on join,
   no `PlayerInfoRemove` broadcast on leave. Fix: broadcast via `chat_tx` after add/before
   remove. Note: joining player already gets existing players via `build_login_sequence`.

#### Key Learnings

- **`parking_lot::RwLockReadGuard` is NOT `Send`** — cannot hold across `.await` in Tokio.
  Solution: scope the guard in a `{ }` block.
- **Signed vs unsigned command packets** — Both dispatch to same system. Client uses signed
  (0x08) for chat clicks, unsigned (0x07) for direct typing. Decode: extract command string,
  skip timestamp/salt/signatures/last-seen/checksum.
- **`std::sync::mpsc::Sender` IS `Send`** — safe to use in async-to-sync bridge for command
  feedback. Create channel before dispatch, pass sender to source, drain receiver after.
- **Status sample capped at 12** in vanilla (random subset of online players).

#### Entity Selectors

- Basic implementation: parse `@a/@e/@p/@r/@s/@n`, resolve from `ServerHandle` online player
  list. `@s` returns executing player (error from console). `@r` is deterministic (TODO: rand).
- Filter syntax `[key=value]` accepted by parser (bracket detected) but not processed.
- `get_entities()`/`get_entity()` context getters handle selectors OR plain player names.
- Tab-completion suggests selectors when input starts with `@` or is empty.

#### Console Commands

- Tokio task reads stdin via `BufReader::lines()`, builds `CommandSourceStack` with Console
  source, dispatches through `server_ctx.commands`. Feedback printed to stdout.
- `/stop` detection: exact word match on first whitespace-delimited token (NOT prefix match).
- Shutdown signal via `broadcast::channel` — `tokio::select!` on Ctrl+C OR shutdown_rx.

#### Vanilla Command Stubs

- 78 total commands registered (16 implemented + 62 stubs). Stubs return "not yet implemented".
- Aliases registered separately: experience→xp, msg→tell/w, teammsg→tm.
- All stubs visible in client tab-completion.

#### Test Coverage

- 9 new selector tests, workspace total: 1476 tests.

---

### Post-Phase-18 Architectural Review (2026-03-19)

#### God Files Identified

- **`network.rs` (2079 LOC)** — 6 responsibilities in one file: listener, 4 state handlers,
  auth, chat, commands, helpers. `handle_play_entry()` is 715 lines. 433-line if-else chain
  for packet dispatch. 19 repeated decode+match+log blocks.
- **`component.rs` (1439 LOC)** — 6 responsibilities: structs, builders, display, JSON serde,
  NBT serde, legacy. Same 6 content variants matched in 11 separate blocks across 3 formats.
- **`context.rs` (642 LOC)** — 155-line `parse_argument()` match. 13 near-identical typed
  getter functions. Numeric validation copy-pasted 4 times.

#### ADR-008 Deviation

- ADR-008 specified typestate `Connection<S: State>` but implementation uses runtime
  `ConnectionState` enum. Pragmatic choice — typestate adds async signature complexity.
  Module-level safety (per ADR-036) achieves the same goal. Do NOT retroactively implement
  typestate.

#### Refactoring ADRs Created

- **ADR-035** — Module Structure & File Size Policy (soft ~500 LOC guideline, hard rules
  on responsibility count and match arm count)
- **ADR-036** — Packet Handler Architecture (match dispatch + handler functions +
  decode_packet helper + PlayContext struct)
- **ADR-037** — Coordinate & Vector Type Macros (impl_vector_ops!, impl_directional!,
  impl_axis_accessor!, impl_wire_primitive!)

#### Type Boilerplate

- ~200 lines of duplicated patterns across Vec3/Vec3i/BlockPos/SectionPos: operator
  overloading, directional accessors, axis accessors, wire format read/write.
- VarInt/VarLong encode/decode are near-identical functions (~67 lines duplication).
- `codec/types.rs` has 11 read/write primitive pairs as copy-paste boilerplate.

#### Well-Architected Areas (Don't Touch)

- **World crate** — bit_storage.rs, region.rs, level_chunk.rs are clean, focused, well-tested.
- **Physics** — tick.rs is clean (84-line main function), minimal branching, good helpers.
- **snbt.rs/serde.rs** — well-organized with clear sections. Duplication is localized (macro
  candidates but not urgent). No splitting needed until files exceed ~1500 LOC.

#### Refactoring Phase

- Phase R1 doc at `docs/phases/phase-r1-refactoring.md`. Should be done between P18 and P19.
- 6 sub-phases (R1–R6), 27 work items. Critical path: ADRs → network.rs split → component split.

### Phase R2 Retrospective — network.rs → network/ Module Split

**Completed:** Split 2079-LOC `network.rs` into 11 files in `network/` module tree.

**What went well:**
- `mod network;` in main.rs automatically picks up `network/mod.rs` — zero changes to main.rs
- The `decode_packet()` helper pattern (log + return `Result`) works cleanly with both
  `?` propagation (hard errors) and `if let Ok()` (soft errors / log-and-continue)
- PlayContext struct bundles connection state cleanly; keepalive state stays as locals in
  the select! loop since it's modified across branches

**Gotchas discovered:**
- Borrow splitting with PlayContext in select!: handler functions that need `&mut Connection`
  separately from other PlayContext fields must take individual params, not `&mut PlayContext`
  (e.g., `handle_chat_command` takes `conn, command, name, uuid, ...` separately)
- Zero-field packets (StatusRequest, LoginAcknowledged) — `let _ack = Packet::decode(data)`
  is intentional, not a missed error. decode_packet is overkill for empty payloads.
- Movement has 4 decode variants (pos/pos_rot/rot/status_only) requiring a match before
  decode_packet — the helper still works, just pass the match result in

**File sizes (LOC):**
- `mod.rs`: 508 (soft limit, acceptable — contains tests)
- `play/mod.rs`: 582 (soft limit, acceptable — dispatch + small handlers)
- All others: 50–280 LOC, well under limits

**Status:** R2 complete. Next: R3 (ECS component split) per phase doc.

### Phase R4 Retrospective — Command Context Argument Dispatch Refactor

**Completed:** Refactored `context.rs` (641 LOC → 789 LOC; 587 non-test + 202 test).

**What went well:**
- `validate_range<T: PartialOrd + Display>()` cleanly replaces 4 copy-pasted min/max blocks
  with identical behavior (same error messages). Generic over i32/i64/f32/f64.
- `get_arg_result()` + `get_typed()` helper pair consolidates 13 getters. `get_typed` works
  for all Copy-type getters; `get_string` (borrows `&'a str`) and `get_time` (matches 2
  variants) use `get_arg_result` directly — correct design choice.
- Per-type parser functions (14 total) make `parse_argument()` a clean dispatch table where
  each match arm is a single function call.
- Coordinate parsing moved to `commands/coordinates.rs` module. Functions: `parse_single_coordinate`,
  `parse_coordinates3`, `parse_int_coordinates3`, `parse_coordinates2`. Types: `WorldCoordinate`,
  `CoordinateKind`, `Coordinates`, `EntityAnchorKind`. Support absolute, `~` relative, and `^` local
  coordinate forms. Returns `Coordinates` when any component is relative.
- `NamedColor` removed — use `ChatFormatting` from `oxidized-protocol::chat::formatting` (same 16 colors).
  Filter with `.is_color()` to reject bold/italic/etc modifiers.
- Generic `parse_range<T: FromStr + Copy>()` replaces 3 duplicate range parsers (int, float, double).
- `/tp` consolidated from 6 near-identical exec functions into `exec_tp_targets_to_location`.
- `/list` consolidated into `execute_list_impl(ctx, include_uuids: bool)`.
- `StringReader::advance(n)` added for cross-module cursor advancement (field is private).

**Patterns to reuse:**
- `get_typed()` pattern: `extract(get_arg_result(ctx, name)?).ok_or_else(...)` — clean
  for any match-and-extract-from-enum pattern. Reusable if more argument types are added.
- `validate_range<T>()` — reusable anywhere numeric bounds are checked.
- `parse_word_as_string()` — fallback parser for unimplemented argument types reduces
  repeated `Ok(ArgumentResult::String(reader.read_word().to_string()))`.

**Gotchas discovered:**
- `get_string` returns `&'a str` borrowing from `CommandContext`, so it can't use `get_typed`
  (which returns owned T). This is correct — don't force the pattern where lifetimes differ.
- File went from 641 → 789 LOC total, but non-test LOC stayed ~587 (below 800 hard limit).
  The increase is purely from 16 new tests (53 → 202 test lines). This is healthy growth.

**Status:** R4 complete. R3 (component.rs split) and R5 (type macros) are independent next steps.

### Phase R5 Retrospective — Protocol Type Macros

**Completed:** Eliminated ~144 lines of duplicated boilerplate across types + codec.

**What went well:**
- `impl_vector_ops!` with `no_neg` variant cleanly handles Vec3 (Add+Sub+Neg) vs Vec3i
  (Add+Sub only). The nesting pattern (`$type` variant calls `$type, no_neg` then adds Neg)
  avoids macro code duplication.
- `impl_wire_primitive!` replaced 7 read/write pairs without `paste` crate — passing both
  `$read_fn` and `$write_fn` names avoids a new dependency. Doc comments passed via
  `$(#[$meta])*` pattern.
- VarEncoding trait + generic `encode_var`/`decode_var` cleanly unifies VarInt/VarLong.
  Trait is module-private; public API preserved as thin wrappers.
- AABB `expand_axis`/`contract_axis` helpers are mathematically distinct (expand: `min+d` /
  `max+d`; contract: `min-d` / `max-d`) — the phase doc's suggestion that contract=expand(-d)
  was mathematically incorrect. Caught during implementation.

**Patterns to reuse:**
- `#[macro_use] mod type_macros;` as first module in `mod.rs` — ensures macros available
  to all sibling modules. Must come before any module that invokes the macros.
- `impl_vector_ops!` requires `add_vec(self, rhs) -> Self`, `subtract_vec(self, rhs) -> Self`,
  `negate(self) -> Self` methods. Vec3i's `add_vec` delegates to `offset(other.x, other.y, other.z)`.
- Avoiding `paste` crate: pass both names to macros instead of concatenating identifiers.

**Gotchas discovered:**
- `contract()` ≠ `expand_towards(-d)`. They affect opposite sides of the AABB. Separate
  helpers needed — do not try to unify them.
- Vec3 has both `subtract(dx, dy, dz)` (3 args) and `subtract_vec(rhs: Vec3)` (1 arg).
  The macro uses `subtract_vec` to avoid ambiguity with the inherent 3-arg method.
- `std::ops::{Add, Sub, Neg}` imports can be removed from files using the macro since the
  macro uses fully-qualified `std::ops::Add`. Operator `+`/`-` work without trait in scope.
- `impl_directional!` generates `pub const fn` methods — requires `offset()` to also be
  `const fn`, which it already is on both Vec3i and BlockPos.

**Status:** R5 complete. R3 (component.rs split), R6 (module docs), R7 (doc comments) remain.

### R6 — Small Wins (Localized Duplication)

**What went well:**
- All 5 items completed in one pass with zero test failures.
- `deserialize_prim!` macro must be defined **outside** the `impl` block for serde
  `Deserializer` — defining inside compiles but the generated methods aren't recognized
  as trait implementations. Use `$ty_name:literal` (not `expr`) for `concat!` compatibility.
- `build_palette_data_from_values()` and `build_palette_data_from_entries()` cleanly separate
  the two distinct palette-building patterns (from raw values vs from pre-read entries).
- `vec![...]` macro evaluates elements left-to-right, safe for ordered packet sequences.

**Patterns discovered:**
- `format_typed_array<T: Display>(out, prefix, arr, suffix)` — generic typed-array formatter.
- `validate_index()` / `validate_value()` / `long_bit_offset()` — standard BitStorage helpers.
- Per-packet builder functions (`build_*_packet()`) improve testability of login sequence.

**Status:** R6 complete. R3 (component.rs split), R7 (doc comments) remain.

---

## Phase R2 — Packet Trait & Unified Codec Refactoring

### Analysis Findings (Pre-Implementation)

**Pattern gap identified:** All 59 packets implement identical inherent methods
(`PACKET_ID`, `decode()`, `encode()`) but no trait unifies them. This causes:
1. **15 per-packet error types** — most are single-variant wrappers around `TypeError`
2. **16 identical `map_err` conversions** — in server handler code
3. **No generic send/receive** — 3-line manual encode+send_raw+flush pattern
4. **No generic roundtrip testing** — each test must be hand-written

**ADR-007 was never implemented:** The Packet trait, McRead/McWrite, and derive macros
were specified in ADR-007 but the macros in `oxidized-macros/src/lib.rs` are stubs
(returning empty `TokenStream`). ADR-038 captures the incremental implementation plan.

**Key design decisions (ADR-038):**
- `Packet` trait: `const PACKET_ID: i32` + `decode(Bytes)` + `encode() -> BytesMut`
- `PacketDecodeError`: unified enum wrapping TypeError, VarIntError, io::Error,
  ResourceLocationError, NbtError, plus `InvalidData(String)` catch-all
- All monomorphized (static dispatch) — no `Box<dyn Packet>`, no vtable overhead
- Incremental migration: trait impls added alongside existing inherent methods, callers
  migrated one handler at a time, old error types removed last
- Derive macros deferred to a later phase

**Migration order:** Status (4 pkt) → Handshake (1) → Login (7) → Configuration (8) → Play (39)

**Explicitly not recommended (evaluated and rejected):**
- ServerHandle sub-traits — only 2 implementors, premature
- Arc<RwLock<T>> wrapper — mixed lock types are intentional
- Plugin/dynamic registration — YAGNI
- Typestate builders — current builders are correct
- ComponentFormat trait — only 2 stable formats
- Auto-registration (inventory/linkme) — compile-time registration sufficient

**Status:** ADR-038 created, phase-r2-refactoring.md created. Implementation not started.

### Phase R2 Retrospective — Packet Trait & Unified Codec Refactoring (SP1–SP6)

**Completed:** All 6 sub-phases of the Packet Trait & Unified Codec Refactoring.

**What went well:**
- Incremental migration strategy worked perfectly — trait impls added alongside existing
  inherent methods, callers migrated one handler at a time, old error types removed last
- All 12 packet structs missing `PartialEq` could safely derive it (their fields all
  supported it), enabling uniform roundtrip testing via `assert_roundtrip<P>()`
- Generic `assert_roundtrip` helper in `tests/roundtrip.rs` exercises all protocol states
  through a single unified function — 48 deterministic + 27 proptest roundtrip tests
- Zero clippy warnings throughout; 1646 workspace tests pass

**Final metrics:**
- Per-packet error types: 15 → 0
- `map_err` conversions in server: 16 → 0
- Lines to send a packet: 3 → 1 (`conn.send_packet(&pkt)`)
- Lines to receive+decode a packet: 5–8 → 1 (`decode_packet::<P>(...)`)
- Generic roundtrip test helper: No → Yes (75 tests in `roundtrip.rs`)
- Packets implementing `Packet` trait: 0 → 64 impls across 59 packet files
- All packets now derive `PartialEq`

**Patterns to reuse:**
- `assert_roundtrip<P: Packet + PartialEq + Debug>(pkt)` — for any future packet testing
- When adding new packets: `impl Packet for` is required (no more inherent methods)
- `PacketDecodeError::InvalidData(String)` for packet-specific validation failures

**Status:** R2 complete. All acceptance criteria met.

### 2025-07-XX — Chat Component Field Name Fix

#### Bug

Interactive chat messages (click events, hover events) were not working in the client.
The /help command pagination buttons did nothing when clicked, and hover tooltips didn't
appear.

#### Root Cause

Vanilla Minecraft 26.1 uses **snake_case** field names in both NBT and JSON serialization
for Component styles, but our code was using camelCase:
- `clickEvent` → `click_event`
- `hoverEvent` → `hover_event`

Additionally, `HoverEvent` encoding was using the old nested format (wrapping values under
`"contents"`) instead of vanilla's flattened dispatch codec format:
- ShowText: uses `"value"` field directly (not `"contents"`)
- ShowEntity: uses `"id"` (entity type), `"uuid"` (entity UUID), `"name"` (display name)
  all flattened into the hover event compound — NOT nested under `"contents"` with different
  field names
- ShowItem: uses `"id"`, `"count"` flattened directly

#### Key Learnings

- **Vanilla uses DFU codecs** — the same `Codec` serializes both JSON (JsonOps) and NBT
  (NbtOps), so field names are IDENTICAL in both formats.
- **`TextComponentHoverAndClickEventFix.java`** is a data fixer that migrated old camelCase
  to snake_case — confirms this was an intentional rename.
- **HoverEvent uses `Action.CODEC.dispatch("action", ...)`** which flattens all inner codec
  fields into the same compound. No `"contents"` wrapper.
- **BundleDelimiterPacket occupies ID 0x00** — `withBundlePacket()` is called BEFORE any
  `addPacket()` calls in `GameProtocols.java`. This shifts all addPacket-registered IDs by +1.
  ALL our packet IDs were verified correct. The memories.md entry for SystemChatPacket was
  corrected from 0x78 to 0x79.
- **CommonPacketTypes are included in Play state** — GameProtocols registers both
  `GamePacketTypes` AND `CommonPacketTypes` (KeepAlive, CustomPayload, Disconnect, Ping,
  Pong, Cookie, Transfer). Must count ALL when computing packet IDs.

#### Files Changed

- `crates/oxidized-protocol/src/chat/style.rs` — All field name fixes (NBT + JSON),
  HoverEvent flattening, HoverEntity serde rename, updated tests
- `crates/oxidized-protocol/src/chat/component_json.rs` — Field names in JSON deserializer

### 2025-07-XX — ClickEvent Action-Specific Field Names (26.1)

#### Problem

After fixing snake_case field names, the client still crashed decoding
`clientbound/minecraft:system_chat` for messages with click events. The first
message (header without click/hover) displayed fine, but the second message
(with ClickEvent) caused `DecoderException: Failed to decode packet`.

#### Root Cause

Vanilla 26.1 changed ClickEvent to use **action-specific field names** instead
of the generic `"value"` field. The data fixer `TextComponentHoverAndClickEventFix.fixClickEvent()`
renames fields for old data:

| Action | Old field | New field | Type |
|--------|-----------|-----------|------|
| `open_url` | `"value"` | `"url"` | String |
| `run_command` | `"value"` | `"command"` | String |
| `suggest_command` | `"value"` | `"command"` | String |
| `copy_to_clipboard` | `"value"` | `"value"` | String (unchanged) |
| `change_page` | `"value"` (string) | `"page"` | **TAG_INT** (not string!) |
| `show_dialog` (new) | N/A | `"dialog"` | String (ResourceLocation) |
| `custom` (new) | N/A | `"value"` | String |

#### Key Learnings

- **ClickEvent.java failed to decompile** (Vineflower NullPointerException). Field
  names were inferred from: (1) `TextComponentHoverAndClickEventFix.fixClickEvent()`,
  (2) `SignBlockEntity.java` accessor pattern matching, (3) DFU codec conventions.
- **`change_page` is special** — it stores the value as TAG_INT in NBT, not a string.
  The data fixer explicitly converts `Dynamic.asString → parseInt → createInt`.
- **Why the crash only appeared after the snake_case fix**: Before, `clickEvent` (wrong
  name) was silently ignored. After fixing to `click_event`, the client found the field
  and attempted to decode it, but failed on the inner `"value"` field (expected `"command"`).
- **Refactored to `to_nbt()`/`from_nbt()` methods** (like HoverEvent) instead of the
  generic `action_value()` approach — each action type encodes with its correct field name.

#### Files Changed

- `crates/oxidized-protocol/src/chat/style.rs` — ClickEvent methods refactored, NBT
  encode/decode, JSON Serialize/Deserialize
- `crates/oxidized-protocol/src/chat/component_json.rs` — Test assertion updated
- `crates/oxidized-protocol/src/chat/component_nbt.rs` — Debug test cleaned up

---

### Phase 19 — World Ticking (2025)

#### Summary

Added the 20 TPS tick loop, day/night cycle, weather transitions, game rules,
scheduled block ticking infrastructure, and `/tick` command family. Completed the
`/time set|add`, `/weather`, and `/gamerule` commands that were previously stubs.

#### Key Learnings

- **26.1 `ClientboundSetTimePacket` changed format**: Now uses `i64 gameTime` +
  `Map<Holder<WorldClock>, ClockNetworkState>` instead of old `gameTime + dayTime`.
  Holder encoding: VarInt where 0=inline, N+1=registry ref by network ID N.
  WorldClock registry: `minecraft:overworld`=0, `minecraft:the_end`=1.
- **Packet IDs shifted by BundleDelimiter**: SetTime=0x71, TickingState=0x7F,
  TickingStep=0x80. Count from `withBundlePacket()` at ID 0x00.
- **VarLong was missing from varint module**: Had to use local helper in SetTimePacket.
  The `varint.rs` module does have `encode_varlong`/`decode_varlong` but they use
  `&[u8]` not `Bytes/BytesMut`.
- **`level_data` wrapped in `parking_lot::RwLock`**: Required for tick loop to modify
  time/weather concurrently with connection handlers reading it. All access sites
  updated to `.read()` / `.write()`.
- **`ServerHandle` trait extended with default impls**: 14 new methods for time, weather,
  game rules, and tick rate control — all with default no-op impls for backward compat.
- **Broadcast channel reuse**: `chat_tx` (ChatBroadcastMessage) carries any pre-encoded
  packet, not just chat. Time, weather, and tick state packets broadcast through it.

#### Architecture Decisions

- **GameRules in `oxidized-game::level::game_rules`** — per-level concept, 42 vanilla
  rules with typed bool/int storage and camelCase name mapping.
- **ServerTickRateManager** — freeze/step/sprint state machine, mirrors vanilla's
  `ServerTickRateManager`.
- **LevelTicks<T>** — BinaryHeap + HashSet dedup for scheduled block/fluid ticks,
  ordered by (trigger_time, priority, sub_tick).
- **Tick loop as Tokio task** — `tokio::time::interval` with `MissedTickBehavior::Skip`,
  broadcasts `ClientboundSetTimePacket` every 20 ticks.

#### Files Created

- `crates/oxidized-protocol/src/packets/play/clientbound_set_time.rs`
- `crates/oxidized-protocol/src/packets/play/clientbound_ticking_state.rs`
- `crates/oxidized-protocol/src/packets/play/clientbound_ticking_step.rs`
- `crates/oxidized-game/src/level/game_rules.rs`
- `crates/oxidized-game/src/level/tick_rate.rs`
- `crates/oxidized-game/src/level/scheduled_tick.rs`
- `crates/oxidized-game/src/commands/impls/cmd_tick.rs`
- `crates/oxidized-server/src/tick.rs`

#### Files Modified

- `crates/oxidized-protocol/src/packets/play/mod.rs` — 3 new packet modules + re-exports
- `crates/oxidized-game/src/level/mod.rs` — 3 new submodules + re-exports
- `crates/oxidized-game/src/commands/source.rs` — 14 new ServerHandle methods
- `crates/oxidized-game/src/commands/impls/mod.rs` — cmd_tick registration
- `crates/oxidized-game/src/commands/impls/stubs.rs` — removed tick stub
- `crates/oxidized-game/src/commands/impls/cmd_time.rs` — set/add now functional
- `crates/oxidized-game/src/commands/impls/cmd_weather.rs` — clear/rain/thunder functional
- `crates/oxidized-game/src/commands/impls/cmd_gamerule.rs` — get/set functional
- `crates/oxidized-server/src/main.rs` — tick loop spawn, new ServerContext fields
- `crates/oxidized-server/src/network/mod.rs` — level_data RwLock, game_rules, tick_rate_manager
- `crates/oxidized-server/src/network/play/mod.rs` — .read() for level_data access

---

### Phase 20 — World Saving (2025-07-17)

**Context:** Adding world persistence — save level.dat, chunks to region files, autosave.

#### Patterns & Best Practices

- **Anvil sector_count is u8** — The region file offset entry packs into 4 bytes: 3 bytes
  sector offset (24-bit) + 1 byte sector count (max 255 = ~1 MB per chunk). Always validate
  `sectors_needed <= 255` before writing; never silently truncate via `as u8`.
- **Sector number is 24-bit** — Guard `new_sector <= 0xFF_FFFF` on writes to prevent header
  corruption in pathologically large region files (>64 GiB).
- **PalettedContainer::to_nbt_data()** — Global variant must re-palettize: collect unique values,
  build compact palette, repack into new BitStorage. Single variant returns `(vec![value], vec![])`.
- **ChunkSerializer needs BlockRegistry** — Block state IDs must be resolved back to
  `"namespace:name"` + properties for the NBT palette. Biomes use placeholder mapping
  (no biome registry yet).
- **level.dat backup pattern** — Write to `<path>_new`, rename existing to `<path>_old`,
  rename `_new` to final. This is the vanilla double-write pattern (ADR-030).
- **`do_tick` is now async** — After adding `spawn_blocking` for autosave I/O, `do_tick`
  became async and all tick tests use `#[tokio::test]`.
- **AnvilChunkLoader::deserialize_chunk is pub(crate)** — Integration tests must use
  `load_chunk()` (which opens region files internally), not `deserialize_chunk` directly.

#### Gotchas

- **RegionFile refactored from BufReader<File> to raw File** — `open()` remains read-only
  (`File::open`), `open_rw()` uses `OpenOptions::new().read(true).write(true)`, `create()`
  writes 8 KiB zero header. No BufReader/BufWriter — callers control buffering.
- **ServerContext grows — update ALL constructors** — Adding a field to `ServerContext` requires
  updating: main.rs construction, network/mod.rs test helper, tick.rs test helper. Grep for
  `ServerContext {` to find all.
- **File I/O in async context** — ADR-015 mandates `spawn_blocking` for all file I/O. Both
  autosave (tick.rs) and shutdown save (main.rs) wrap `PrimaryLevelData::save()` in
  `tokio::task::spawn_blocking`.

#### Files Created

- `crates/oxidized-world/src/storage/dirty_tracker.rs` — DirtyChunkTracker (HashSet<ChunkPos>)
- `crates/oxidized-world/src/anvil/chunk_serializer.rs` — LevelChunk → Anvil NBT format
- `crates/oxidized-world/tests/world_save.rs` — 5 integration tests (roundtrip, multi-chunk, level.dat)

#### Files Modified

- `crates/oxidized-world/src/anvil/compression.rs` — compress_zlib(), compress_zlib_level()
- `crates/oxidized-world/src/anvil/error.rs` — Compression, ChunkTooLarge variants
- `crates/oxidized-world/src/anvil/mod.rs` — chunk_serializer module + exports
- `crates/oxidized-world/src/anvil/region.rs` — read-write refactor, write_chunk_data, create
- `crates/oxidized-world/src/anvil/chunk_loader.rs` — deserialize_chunk pub(crate) visibility
- `crates/oxidized-world/src/chunk/paletted_container.rs` — to_nbt_data() (inverse of from_nbt_data)
- `crates/oxidized-world/src/storage/mod.rs` — dirty_tracker module + export
- `crates/oxidized-world/src/storage/primary_level_data.rs` — to_nbt(), save() with backup
- `crates/oxidized-server/src/main.rs` — world_dir field, shutdown save with spawn_blocking
- `crates/oxidized-server/src/network/mod.rs` — world_dir field on ServerContext
- `crates/oxidized-server/src/tick.rs` — AUTOSAVE_INTERVAL_TICKS, async autosave, do_tick async


---

### Phase 21 — Inventory & Items

#### Key Learnings

- **packets.json IDs are final wire IDs** — no +1 bundle offset needed. The stored memory about bundle offsets was incorrect for this project. All existing codebase packet IDs match packets.json directly.
- **Phase doc packet IDs were wrong** — always verify against `mc-server-ref/26.1-pre-3/generated/reports/packets.json` before implementing.
- **DataComponentPatch** cannot be skipped without a component type registry. Each added entry has variable-length type-specific data. Must reject non-empty patches (return decode error) to prevent packet desync.
- **ItemStack.to_nbt()** returns `Option<NbtCompound>` (None for empty stacks), not plain `NbtCompound`.
- **Connection::send_packet(&pkt)** is the method to send packets, not `write_packet(id, &data)`.
- **NbtList** does not support indexing (`inv[0]`). Use `.iter().collect::<Vec<_>>()` then index.
- **Login sequence is now 10 packets** (was 8). Tests using hardcoded packet indices need updating when packets are added.
- **Vanilla inventory format**: `Inventory` tag is TAG_List of compounds with `Slot` (byte), `id` (string), `count` (byte). See `ItemStackWithSlot.java`.

#### Verified Packet IDs (from packets.json)

| Packet | ID |
|---|---|
| ContainerSetContent (CB) | 0x12 |
| ContainerSetSlot (CB) | 0x14 |
| SetHeldSlot (CB) | 0x69 |
| SetPlayerInventory (CB) | 0x6C |
| SetCarriedItem (SB) | 0x35 |
| SetCreativeModeSlot (SB) | 0x38 |

#### Files Created

- `crates/oxidized-protocol/src/codec/slot.rs` — Slot wire encoding/decoding (7 tests)
- `crates/oxidized-game/src/inventory/mod.rs` — Inventory module with re-exports
- `crates/oxidized-game/src/inventory/item_stack.rs` — ItemStack, ItemId, DataComponentPatch (18 tests)
- `crates/oxidized-game/src/inventory/container.rs` — MenuType enum, ContainerStateId (3 tests)
- `crates/oxidized-protocol/src/packets/play/clientbound_container_set_content.rs` — Packet 0x12
- `crates/oxidized-protocol/src/packets/play/clientbound_container_set_slot.rs` — Packet 0x14
- `crates/oxidized-protocol/src/packets/play/clientbound_set_held_slot.rs` — Packet 0x69
- `crates/oxidized-protocol/src/packets/play/clientbound_set_player_inventory.rs` — Packet 0x6C
- `crates/oxidized-protocol/src/packets/play/serverbound_set_carried_item.rs` — Packet 0x35
- `crates/oxidized-protocol/src/packets/play/serverbound_set_creative_mode_slot.rs` — Packet 0x38
- `crates/oxidized-server/src/network/play/inventory.rs` — Handlers + helpers (5 tests)

#### Files Modified

- `crates/oxidized-game/src/player/inventory.rs` — Full 41-slot PlayerInventory (20 tests)
- `crates/oxidized-game/src/player/login.rs` — +2 packets in login sequence (ContainerSetContent, SetHeldSlot)
- `crates/oxidized-game/src/player/server_player.rs` — NBT inventory persistence (5 new tests)
- `crates/oxidized-game/tests/game_integration.rs` — Updated for 10-packet login sequence
- `crates/oxidized-protocol/src/packets/play/mod.rs` — 6 new packet module registrations
- `crates/oxidized-server/src/network/play/mod.rs` — 2 new dispatch arms + inventory module

---

### Phase 22 — Block Interaction (Retrospective)

**Date:** Phase 22 completed
**Scope:** Block breaking, block placing, block change broadcasting

#### What Went Well

- Packet ID verification against `packets.json` caught ALL wrong IDs from the phase doc
- Code review passed clean — no significant issues
- DashMap + Arc<RwLock<LevelChunk>> chunk storage pattern works well for concurrent access
- Reusing `chat_tx` broadcast channel for block updates was simple and effective

#### Key Decisions

- **Chunk storage**: `DashMap<ChunkPos, Arc<parking_lot::RwLock<LevelChunk>>>` on `ServerContext`
- Chunks registered in shared storage during `send_initial_chunks` (not at startup)
- Block breaking: creative=instant on StartDestroyBlock, survival=on StopDestroyBlock
- No tick-based mining progress tracking (simplified — all survival breaks accepted)
- `held_item_to_block_state()` uses BlockRegistry to map held item → default block state
- Sign update handler is a stub (requires block entities, not yet implemented)
- `MAX_SERVERBOUND_PLAY_ID` fixed from 0x39 to 0x44 (was wrong since initial impl)

#### Packet IDs (Verified)

| Packet | Wire ID |
|---|---|
| PlayerAction (SB) | 0x29 |
| UseItemOn (SB) | 0x42 |
| UseItem (SB) | 0x43 |
| SignUpdate (SB) | 0x3D |
| BlockChangedAck (CB) | 0x04 |
| BlockDestruction (CB) | 0x05 |
| BlockEvent (CB) | 0x07 |
| BlockUpdate (CB) | 0x08 |
| SectionBlocksUpdate (CB) | 0x54 |

#### Files Created

- `crates/oxidized-protocol/src/packets/play/serverbound_player_action.rs` — PlayerAction enum + packet
- `crates/oxidized-protocol/src/packets/play/serverbound_use_item_on.rs` — InteractionHand, BlockHitResult, packet
- `crates/oxidized-protocol/src/packets/play/serverbound_use_item.rs` — UseItem packet
- `crates/oxidized-protocol/src/packets/play/serverbound_sign_update.rs` — SignUpdate packet
- `crates/oxidized-protocol/src/packets/play/clientbound_block_changed_ack.rs` — Ack packet
- `crates/oxidized-protocol/src/packets/play/clientbound_block_destruction.rs` — Break animation packet
- `crates/oxidized-protocol/src/packets/play/clientbound_block_event.rs` — Block event packet
- `crates/oxidized-protocol/src/packets/play/clientbound_block_update.rs` — Single block update packet
- `crates/oxidized-protocol/src/packets/play/clientbound_section_blocks_update.rs` — Batch section update
- `crates/oxidized-server/src/network/play/block_interaction.rs` — Breaking/placing handlers (6 tests)

#### Files Modified

- `crates/oxidized-protocol/src/packets/play/mod.rs` — 9 new packet module registrations
- `crates/oxidized-server/Cargo.toml` — Added `dashmap` dependency
- `crates/oxidized-server/src/main.rs` — Added `chunks` to ServerContext constructor
- `crates/oxidized-server/src/network/mod.rs` — Added `chunks` field, fixed MAX_SERVERBOUND_PLAY_ID
- `crates/oxidized-server/src/network/play/helpers.rs` — Stores chunks in DashMap during initial send
- `crates/oxidized-server/src/network/play/mod.rs` — 4 new dispatch arms + block_interaction module
- `crates/oxidized-server/src/tick.rs` — Added `chunks` to test_ctx()

#### Gotchas & Future Notes

- `ServerContext` now has 4 constructor sites (main.rs, network/mod.rs test, tick.rs test, block_interaction.rs test) — grep for `ServerContext {` when adding fields
- `BlockRegistry::load()` is called per-placement (not cached on ServerContext) — may want to cache in future
- No collision detection for block placement — players can place blocks inside themselves
- No block drop/item entity creation on break — needs entity physics (future phase)
- No tool speed / mining progress validation — all survival StopDestroyBlock accepted

### Phase 22 Review Improvements (Post-Implementation)

**Changes made:**

1. **BlockRegistry cached in ServerContext** — `Arc<BlockRegistry>` loaded once at startup, eliminating gzip+JSON decompression on every block placement
2. **Renamed ChatBroadcastMessage → BroadcastMessage**, `chat_tx` → `broadcast_tx` — reflects actual general-purpose broadcast usage
3. **Broadcast sender exclusion** — `BroadcastMessage.exclude_entity: Option<i32>` skips sending to the acting player; relay loop in play/mod.rs filters by entity_id
4. **Reach/distance validation** — `MAX_REACH_DISTANCE_SQ = 7.0²` for break/place, `MAX_SIGN_EDIT_DISTANCE_SQ = 8.0²` for sign editing; uses squared distance (no sqrt)
5. **Survival mining guard** — StartDestroyBlock stores position in `spawn_pos` (temporary reuse), StopDestroyBlock validates position matches (no tick-based progress yet)
6. **Type consistency** — `get_block()` returns `Option<u32>`, internal functions use u32, i32 only at wire boundaries (`as i32` for packet fields)
7. **Dirty chunk marking** — `DashSet<ChunkPos>` on ServerContext; `set_block()` inserts chunk_pos after modification
8. **Inventory decrement** — Validates count > 0 before placement in non-Creative; decrements after successful place; clears slot via `ItemStack::empty()` when depleted
9. **Inventory validation before placement** — Checks `is_empty()` and `count > 0` before allowing block placement to prevent exploiting zero-count stacks

#### Gotchas Updated

- `ServerContext` still has 4 constructor sites — all updated with `dirty_chunks`, `block_registry`, `broadcast_tx`
- `spawn_pos` field on ServerPlayer is temporarily reused for mining position tracking — needs a proper `mining_pos: Option<BlockPos>` field added to ServerPlayer in future
- `BlockStateId` is `u16` (in oxidized-world), chunk API uses `u32` — convert with `u32::from(state.0)` at boundary
- `BroadcastMessage` is the single broadcast channel type for ALL broadcast types (chat, block updates, weather, tick state, player info)
- Block interaction handlers do NOT hold lock guards across `.await` points — always extract data into locals first, drop guard, then await
