# ADR-034: Comprehensive Testing Strategy

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | All phases |
| Deciders | Oxidized Core Team |

## Context

Phase 1 retrospective revealed that unit tests alone provided only 18% coverage of critical scenarios. A Minecraft server has complex interactions: protocol parsing, world generation, entity systems, tick loops, and network I/O. Unit tests catch function-level bugs but miss integration issues, edge cases in binary parsing, performance regressions, and protocol compliance.

We need a testing strategy that provides confidence the server works correctly, handles malformed input safely, performs acceptably, and maintains protocol compatibility with vanilla clients.

## Decision Drivers

- **Reliability**: every code path must be tested before merging
- **Safety**: parsers and network code must handle malformed input without panicking
- **Performance**: regressions must be detected before they reach main
- **Protocol compliance**: server must be indistinguishable from vanilla to clients
- **Developer experience**: tests must be fast to write and run locally
- **CI integration**: all test types must run in CI automatically

## Test Categories

### 1. Unit Tests (built-in `#[test]`)

**Purpose**: Test individual functions and methods in isolation.
**Framework**: Rust's built-in test framework.
**Location**: Inline `#[cfg(test)] mod tests` at the bottom of each source file.
**Coverage target**: Every public function, every error path, every branch.
**Naming**: `test_<thing>_<outcome>_when_<condition>`
**Requirements**:
- Every PR must include unit tests for new/changed logic
- Boundary value tests for all validated ranges
- Error path tests for every `Result`-returning function
- No `unwrap()` or `expect()` in test code without `#[allow]` annotation at module level

### 2. Integration Tests (`tests/` directories)

**Purpose**: Test modules working together (e.g., config load → validate → apply → save).
**Framework**: Rust's built-in integration test support (`tests/` directory at crate root).
**Location**: `crates/<crate>/tests/*.rs`
**Coverage target**: Every cross-module workflow, every end-to-end scenario.
**Requirements**:
- At least one integration test per crate that exercises the primary workflow
- Integration tests use the public API only — no `pub(crate)` or internal access
- File I/O tests use `tempfile` crate for cleanup

### 3. Property-Based Tests (proptest)

**Purpose**: Automatically generate thousands of random inputs and verify invariants hold.
**Framework**: `proptest` crate.
**Location**: Inside `#[cfg(test)]` modules or in `tests/` directory.
**Ideal for**:
- Parsers (config, NBT, protocol packets): "any valid input round-trips correctly"
- Codecs (VarInt, VarLong): "encode → decode = identity"
- Coordinate systems: "any valid coordinate converts correctly"
- Serialization: "serialize → deserialize = identity"
**Requirements**:
- All parsers and codecs MUST have property-based tests
- Use `proptest::arbitrary` derives where possible
- Regression files committed to repo (`.proptest-regressions/`)

### 4. Fuzz Testing (cargo-fuzz)

**Purpose**: Find crashes, panics, and undefined behavior with random byte sequences.
**Framework**: `cargo-fuzz` (libFuzzer).
**Location**: `fuzz/` directory at workspace root.
**Ideal for**:
- Protocol packet parsing (byte streams from untrusted clients)
- NBT deserialization (complex binary format)
- Config file parsing (user-edited text)
- Any function that takes `&[u8]` or `&str` as input
**Requirements**:
- Every parser that handles untrusted input MUST have a fuzz target
- Fuzz targets run in CI nightly (not on every PR — too slow)
- Crash artifacts committed to `fuzz/artifacts/` for regression testing
- `#[cfg(fuzzing)]` attribute for fuzz-specific code paths

### 5. Snapshot/Golden Tests (insta)

**Purpose**: Compare output against known-good reference files. Detect unintended format changes.
**Framework**: `insta` crate.
**Location**: Inside `#[cfg(test)]` modules. Snapshots in `snapshots/` directory.
**Ideal for**:
- Config file generation (default `oxidized.toml` output)
- Protocol packet serialization (binary format snapshots)
- Error message formatting
- CLI help text
**Requirements**:
- Generated output that users see (config files, error messages) MUST have snapshot tests
- Snapshots committed to repo and reviewed in PRs
- Use `insta::assert_snapshot!` for text, `insta::assert_debug_snapshot!` for debug output

### 6. Benchmark Tests (criterion)

**Purpose**: Track performance over time, detect regressions.
**Framework**: `criterion` crate.
**Location**: `benches/` directory at crate root.
**Ideal for**:
- Packet encoding/decoding throughput
- Config parsing latency
- NBT read/write performance
- Chunk serialization/deserialization
- Tick loop iteration time (later phases)
**Requirements**:
- Every performance-critical code path MUST have a benchmark
- Benchmarks run in CI weekly (not on every PR — too slow)
- Results archived for trend analysis
- `criterion::black_box` used to prevent dead code elimination

### 7. Doc Tests (rustdoc `///` examples)

**Purpose**: Ensure documentation examples actually compile and work.
**Framework**: Built-in `cargo test --doc`.
**Location**: `///` comments on public items.
**Requirements**:
- Every public struct, enum, and function MUST have a doc example
- Doc examples must be self-contained (compilable without external state)
- `cargo test --doc` runs in CI on every PR

### 8. Contract/Compliance Tests (custom)

**Purpose**: Verify protocol compliance with vanilla Minecraft client expectations.
**Framework**: Custom test harness comparing against reference data.
**Location**: `crates/oxidized-protocol/tests/compliance/`
**Ideal for**:
- Packet format verification against vanilla captures
- Status response JSON schema validation
- Login sequence verification
- Chunk data format validation
**Requirements**:
- Reference data captured from vanilla server (stored in `mc-server-ref/`)
- Tests compare Oxidized output byte-for-byte with vanilla output where applicable
- New protocol implementations must include compliance tests before merging

## CI Integration

| Test Type | Trigger | Runner | Timeout |
|-----------|---------|--------|---------|
| Unit tests | Every push/PR | All platforms | 10 min |
| Integration tests | Every push/PR | All platforms | 10 min |
| Property-based | Every push/PR | Ubuntu only | 15 min |
| Snapshot | Every push/PR | Ubuntu only | 5 min |
| Doc tests | Every push/PR | Ubuntu only | 5 min |
| Benchmarks | Weekly schedule | Ubuntu only | 30 min |
| Fuzz | Nightly schedule | Ubuntu only | 60 min |
| Compliance | Every push/PR | Ubuntu only | 10 min |

## Decision

**We adopt a comprehensive 8-type testing strategy.** Every category has a clear framework, location convention, and CI enforcement. The minimum viable test suite for any PR includes unit tests, integration tests, and property-based tests for parsers/codecs. Snapshot, benchmark, fuzz, and compliance tests are added as the relevant code paths are implemented.

## Consequences

### Positive

- High confidence in correctness: unit + integration + property-based catches most bugs
- Crash resistance: fuzz testing finds panics before attackers do
- Performance tracking: criterion benchmarks detect regressions early
- Format stability: snapshot tests prevent accidental output changes
- Protocol compliance: contract tests ensure vanilla clients work correctly

### Negative

- More dependencies: proptest, insta, criterion, cargo-fuzz add to build time
- More CI time: full test suite takes longer than unit-only
- More maintenance: snapshot files, fuzz corpora, and benchmark baselines need updating

### Neutral

- Testing infrastructure grows with the codebase — not all 8 types needed from day one
- Phase 1 requires: unit, integration, property-based, snapshot. Others added as relevant code lands.

## Compliance

- Every PR must include tests matching the affected code's category requirements
- CI must pass all enabled test categories before merge
- Code review must verify test coverage is adequate (not just that tests exist)

## Related ADRs

- [ADR-002: Error Handling Strategy](adr-002-error-handling.md) — error paths must be tested
- [ADR-005: Configuration Management](adr-005-configuration.md) — config parsing needs property-based tests
- [ADR-007: Packet Codec Framework](adr-007-packet-codec.md) — codecs need fuzz testing
- [ADR-010: NBT Library Design](adr-010-nbt.md) — NBT needs fuzz + property tests
- [ADR-032: Performance & Scalability](adr-032-performance-scalability.md) — benchmarks track performance

## References

- [proptest — property-based testing](https://docs.rs/proptest/latest/proptest/)
- [insta — snapshot testing](https://docs.rs/insta/latest/insta/)
- [criterion — benchmarking](https://docs.rs/criterion/latest/criterion/)
- [cargo-fuzz — fuzz testing](https://rust-fuzz.github.io/book/cargo-fuzz.html)
- [Rust testing reference](https://doc.rust-lang.org/book/ch11-00-testing.html)
