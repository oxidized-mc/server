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
- **CI failure on this commit was expected.** Phase 2 added types used by Phase 3; the commit
  compiled but CI ran clippy which flagged unused code. Fixed in Phase 3 commit.

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
- **ADR-009 referenced `cfb8` crate but we couldn't use it.** ADRs should note implementation
  caveats when the chosen approach doesn't work. Updated ADR-009 with actual implementation.
- **URL-encode all external API parameters by default.** The auth URL injection was subtle —
  make encoding the default pattern for any URL construction.

### Technical debt acknowledged
- **No real client testing yet.** The login flow is tested with unit/integration tests but not
  against a real Minecraft 26.1-pre-3 client. The server transitions to Configuration state
  but Configuration packets are not implemented — client will hang.
- **`reqwest` is a heavy dependency.** Consider whether a lighter HTTP client would suffice
  for the single Mojang auth endpoint.

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
  chunk loading. Deferred to Phase 10 when chunk loading actually needs it.
- **Borrowed/zero-copy NBT deferred.** `BorrowedNbtCompound<'a>` for lazy parsing also
  deferred to Phase 10.
- **No benchmark suite yet.** ADR-010 calls for criterion benchmarks. Will add when we have
  real chunk data to benchmark against (Phase 9–10).

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
- **Never use `as` for narrowing casts in data loading.** Always use `u16::try_from()` / `u8::try_from()` with proper error propagation. This applies to all registry/data loading code.
- **Review→Fix→Re-review loop is essential.** Each review pass caught a different class of issue. The loop terminated after 3 passes with zero findings.
- **Error types should distinguish failure modes:** `InvalidStateId(u64)`, `MissingStateId(String)`, `InvalidItemProperty(String, &'static str, u64)` each tell you exactly what went wrong.

#### Gotchas
- **Git-tracked binary data:** `.json.gz` files in `src/data/` must be explicitly `git add`ed — they're not matched by default patterns. First CI run failed because they weren't committed.
- **`as` casts compile silently** even when they truncate. Clippy's `cast_possible_truncation` lint would catch these, but it's not enabled by default. Consider enabling it workspace-wide.

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

#### Other Fixes Applied

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

1. **Missing header entry sanitization (critical):** Java's `RegionFile` constructor validates
   all 1024 offset entries during header read and zeros out invalid ones: `sector_number < 2`
   (overlaps header), `sector_count == 0`, or `end_sector > file_sectors`. The Rust code stored
   raw entries without validation. A `sector_count == 0` entry with `sector_number != 0` would
   pass `is_present()` and attempt to read from an arbitrary offset, potentially returning
   garbage data. Fixed by sanitizing entries during `read_header()`.

2. **Missing payload-vs-sector bounds check (medium):** After reading the 4-byte `payload_len`,
   Java validates it doesn't exceed `numSectors * SECTOR_BYTES`. A corrupted `payload_len` (but
   under 16 MiB) would cause reads past the chunk's allocated sectors into adjacent chunks' data.
   Fixed by checking `payload_len + 4 <= sector_count * SECTOR_BYTES`.

3. **Error variant misuse (low):** `AnvilError::Decompression` was abused for mutex poisoning
   and `JoinError` in `AsyncChunkLoader`. Added `AnvilError::Internal(String)`.

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
   definitions have TODO comments. Must resolve before Phase 13 (chunk sending).

2. **#[non_exhaustive] added to all 31 public error enums:** Prevents breaking changes when
   adding error variants. Affects: oxidized-nbt (1), oxidized-protocol (23), oxidized-world (6),
   oxidized-server (1).

3. **Typestate NOT implemented (ADR-008):** Connection uses runtime enum, not compile-time
   `Connection<State>`. Known deviation — acceptable for current phase count but should be
   addressed before Play state packets proliferate.

4. **Zero-copy NBT (ADR-010 partial):** Only the owned tree is implemented. Arena and borrowed
   reader are deferred until chunk sending at scale (Phase 13+).

5. **DashMap chunk storage (ADR-014):** Not yet needed — only data structures exist. Required
   at Phase 11 (Server Level).

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

1. **BlockFlags too narrow (critical):** Java's `Block.java` defines 11 flag constants with
   values up to 512 (bit 9). The Rust `BlockFlags` used `u8` (max 255), which cannot represent
   `UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS` (256) or `UPDATE_SKIP_ON_PLACE` (512). Widened to `u16`.
   Missing flags: UPDATE_IMMEDIATE, UPDATE_MOVE_BY_PISTON, UPDATE_SKIP_SHAPE_UPDATE_ON_WIRE,
   UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS, UPDATE_SKIP_ON_PLACE. Missing composites: UPDATE_NONE(260),
   UPDATE_ALL(3), UPDATE_ALL_IMMEDIATE(11), UPDATE_SKIP_ALL_SIDEEFFECTS(816). Missing UPDATE_LIMIT(512).

2. **End dimension wrong values (medium):** Vanilla 26.1 generated data shows End has
   `has_skylight: true` and `ambient_light: 0.25` — the Rust code had `false` and `0.0`.

3. **Overworld logical_height wrong (medium):** Vanilla data shows 384, not 320 as the phase
   doc specified. The phase doc was wrong — always verify against generated data.

4. **DimensionType missing fields (medium):** Java 26.1 DimensionType record has
   `has_fixed_time`, `has_ender_dragon_fight`, `coordinate_scale` fields not present in Rust.

5. **LRU cache O(n) performance (medium):** Hand-rolled VecDeque-based LRU used `retain()`
   (O(n)) on every get(). Replaced with `lru` crate for O(1) operations.

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
- PlayerInventory is a stub (Phase 22)
- No ECS component integration yet (Phase 14+ per ADR-020)
- Minimal PLAY read loop only handles teleport confirmations — full PLAY handling is Phase 14+

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
- PLAY read loop is minimal (teleport confirmations only) — full handling Phase 14+
- No player removal from PlayerList on disconnect (cleanup is best-effort log + remove)
- PlayerConnection bridge channels (ADR-020) not yet implemented

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
- Chunks are empty air (no worldgen/disk loading) — real chunks in later phases
- No per-tick chunk throttling — all chunks sent in one batch during login
- Block entities always VarInt(0) — no block entity support yet

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
  to extract state logic for unit-level testing
- **Fuzz tests** — need `cargo-fuzz` infrastructure setup
- **Benchmarks** — need `criterion` setup in `benches/` dirs
- **View distance capping** — server uses client's view_distance (16) uncapped instead of
  config max (10), sending 1089 chunks instead of 441

#### Heightmap CLIENT_TYPES Fix
Phase 13 was missing `MotionBlockingNoLeaves` (type_id=5) in CLIENT_TYPES.
Java sends 3 client types: WORLD_SURFACE(1), MOTION_BLOCKING(4), MOTION_BLOCKING_NO_LEAVES(5).
Fixed in commit 478d145.

#### LEVEL_CHUNKS_LOAD_START Fix
Vanilla sends `GameEvent(13, 0.0)` after initial chunk batch — signals client to exit
"Loading Terrain" screen. We were missing this packet entirely. Fixed in commit 8315483.

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
