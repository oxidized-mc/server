# Phase R2 — Packet Trait & Unified Codec Refactoring

**Status:** ✅ Complete  
**Crates:** `oxidized-protocol`, `oxidized-server`
**Reward:** All packets implement a common `Packet` trait with unified error handling,
enabling generic send/receive, eliminating 15 duplicate error types and 16 identical
`map_err` conversions, and providing the foundation for derive macros (ADR-007).

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-038: Packet Trait & Unified Codec Error](../adr/adr-038-packet-trait.md) —
  trait design, error unification, migration strategy
- [ADR-007: Packet Codec Framework](../adr/adr-007-packet-codec.md) —
  the original vision (trait layer implemented here; derive macros deferred)

Also relevant:
- [ADR-002: Error Handling Strategy](../adr/adr-002-error-handling.md) —
  `thiserror` for library errors
- [ADR-036: Packet Handler Architecture](../adr/adr-036-packet-handler-architecture.md) —
  `decode_packet<T>()` helper will gain a `Packet` trait bound

---

## Goal

Implement the `Packet` trait and `PacketDecodeError` from ADR-038 across the entire
protocol crate. This is a **pure structural refactoring** — no new features, no behavior
changes, no protocol additions. Every test that passes before must pass after.

The refactoring eliminates 3 classes of boilerplate simultaneously:

1. **15 per-packet error types** → 1 unified `PacketDecodeError`
2. **16 identical `map_err` conversions** in server handlers → generic `decode_packet<P>()`
3. **3-line manual send pattern** → `conn.send_packet(&pkt)`

---

## Motivation

The codebase has 59 packets today. Phases 19–38 will add approximately 126 more, bringing
the total to ~185. Without a trait:

- Each new packet requires a dedicated error type (even if it's just `TypeError` wrapping)
- Each server handler duplicates the same decode+map_err+log pattern
- No generic roundtrip testing — each test must be hand-written
- The derive macros (ADR-007) have no target trait to generate

Refactoring now, after Phase R1 (structural cleanup) and before Phase 19 (world ticking),
is optimal because:
- R1 split `network.rs` into clean handler modules — ideal for systematic migration
- The test suite is comprehensive (~1500 tests) providing refactoring safety
- Phase 19 will add new packets — having the trait in place avoids immediate tech debt

---

## Non-Goals

- **No derive macro implementation** — `McPacket`, `McRead`, `McWrite` remain stubs
  (future phase)
- **No `PacketRegistry` with dynamic dispatch** — `Box<dyn Packet>` is not needed (future)
- **No new packets** — protocol stays identical
- **No handler logic changes** — packet processing behavior is unchanged
- **No dependency additions** — uses only existing workspace dependencies

---

## Detailed Refactoring Plan

### Sub-phase 1: Add `PacketDecodeError` and `Packet` trait

**Target:** `crates/oxidized-protocol/src/codec/`

**Steps:**

1. Create `crates/oxidized-protocol/src/codec/packet.rs`:
   - Define `PacketDecodeError` enum with `#[non_exhaustive]` and `thiserror::Error`
   - Variants: `Type(TypeError)`, `VarInt(VarIntError)`, `Io(io::Error)`,
     `ResourceLocation(ResourceLocationError)`, `Nbt(NbtError)`, `InvalidData(String)`
   - All variants use `#[from]` for ergonomic `?` conversion
   - Define `Packet` trait:
     ```rust
     pub trait Packet: Sized + std::fmt::Debug {
         const PACKET_ID: i32;
         fn decode(data: Bytes) -> Result<Self, PacketDecodeError>;
         fn encode(&self) -> BytesMut;
     }
     ```

2. Re-export from `crates/oxidized-protocol/src/codec/mod.rs`:
   - `pub use packet::{Packet, PacketDecodeError}`

3. Re-export from `crates/oxidized-protocol/src/lib.rs` (or via `codec` module)

4. Write unit tests for `PacketDecodeError` conversions (From impls)

**Verification:** `cargo test -p oxidized-protocol` — existing tests unchanged.

---

### Sub-phase 2: Implement `Packet` for all packets (by state)

**Target:** All 59 packet files in `crates/oxidized-protocol/src/packets/`

Migration pattern per packet:

```rust
// Before (existing — kept during migration):
impl ServerboundHelloPacket {
    pub const PACKET_ID: i32 = 0x00;
    pub fn decode(mut data: Bytes) -> Result<Self, HelloError> { ... }
    pub fn encode(&self) -> BytesMut { ... }
}

// After (added alongside existing):
impl Packet for ServerboundHelloPacket {
    const PACKET_ID: i32 = 0x00;

    fn decode(data: Bytes) -> Result<Self, PacketDecodeError> {
        // Delegates to inherent method, converting error
        Self::decode_inner(data).map_err(PacketDecodeError::from)
    }

    fn encode(&self) -> BytesMut {
        self.encode_inner()
    }
}
```

For packets whose error type is just `TypeError` wrapping (the majority), the trait
impl can directly use the existing decode body with `?` (since `TypeError` converts to
`PacketDecodeError` via `#[from]`).

**Migration order** (by state, simplest first):

| Batch | State | Packets | Complexity |
|-------|-------|---------|------------|
| 2a | Status | 4 | Trivial (2 empty, 2 single-field) |
| 2b | Handshake | 1 | Simple (has `UnknownIntent` variant → `InvalidData`) |
| 2c | Login | 7 | Moderate (key exchange, encryption) |
| 2d | Configuration | 8 | Moderate (registry data, tags) |
| 2e | Play | 39 | Mixed (simple to complex) |

For each batch:
1. Add `impl Packet for` all packets in the state
2. Rename existing inherent `decode`/`encode` to `decode_inner`/`encode_inner` (private)
3. Verify all tests still pass
4. Update inline roundtrip tests to use the trait import

**Verification after each batch:** `cargo test -p oxidized-protocol`

---

### Sub-phase 3: Add generic methods to `Connection`

**Target:** `crates/oxidized-protocol/src/connection.rs`

**Steps:**

1. Add `send_packet<P: Packet>()`:
   ```rust
   pub async fn send_packet<P: Packet>(&mut self, pkt: &P) -> Result<(), ConnectionError> {
       let body = pkt.encode();
       self.send_raw(P::PACKET_ID, &body).await?;
       self.flush().await
   }
   ```

2. Add a `ConnectionError::Protocol` variant (or use existing `Io` mapping) for
   `PacketDecodeError`:
   ```rust
   #[error("protocol error: {0}")]
   Protocol(#[from] PacketDecodeError),
   ```

3. Write tests: `send_packet` round-trips through mock connection

**Verification:** `cargo test -p oxidized-protocol`

---

### Sub-phase 4: Migrate server handlers

**Target:** `crates/oxidized-server/src/network/`

**Steps:**

1. Update `helpers.rs` — change `decode_packet<T, E>()` to use `Packet` trait bound:
   ```rust
   pub fn decode_packet<P: Packet>(
       data: Bytes,
       addr: SocketAddr,
       player_name: &str,
       packet_name: &str,
   ) -> Result<P, ConnectionError> {
       P::decode(data).map_err(|e| {
           debug!(...);
           ConnectionError::Protocol(e)
       })
   }
   ```

2. Migrate each handler file:
   - `handshake.rs` — replace `ClientIntentionPacket::decode().map_err(...)` with
     `decode_packet::<ClientIntentionPacket>(...)`
   - `status.rs` — replace manual decode+map_err for ping
   - `login.rs` — replace 3 decode+map_err blocks
   - `configuration.rs` — replace 2 decode+map_err blocks
   - `play/mod.rs` — replace inline decodes with `decode_packet::<P>()`
   - `play/chat.rs`, `play/commands.rs` — update packet sends to use `send_packet()`

3. Replace `conn.send_raw(Pkt::PACKET_ID, &pkt.encode()) + conn.flush()` with
   `conn.send_packet(&pkt)` across all handlers

**Verification:** `cargo test --workspace`

---

### Sub-phase 5: Remove per-packet error types

**Target:** All packet files in `crates/oxidized-protocol/src/packets/`

**Steps:**

1. Remove `HelloError`, `PingError`, `KeyError`, `LoginFinishedError`,
   `LoginCompressionError`, `DisconnectError`, `IntentionError`
2. Remove `RegistryDataError`, `KnownPacksError`, `UpdateEnabledFeaturesError`,
   `UpdateTagsError`, `ServerboundKnownPacksError`, `ClientInformationError`
3. Remove `PlayPacketError`
4. Remove inherent `decode`/`encode` methods (now only trait methods remain)
5. Grep for any remaining references to removed types

**Verification:** `cargo test --workspace` + `cargo clippy --workspace`

---

### Sub-phase 6: Generic roundtrip test helper

**Target:** `crates/oxidized-protocol/tests/`

**Steps:**

1. Create a generic roundtrip assertion:
   ```rust
   fn assert_roundtrip<P: Packet + PartialEq + std::fmt::Debug>(pkt: &P) {
       let encoded = pkt.encode();
       let decoded = P::decode(encoded.freeze())
           .expect("decode should succeed for a packet we just encoded");
       assert_eq!(pkt, &decoded);
   }
   ```

2. Migrate existing roundtrip tests to use the helper where appropriate

3. Add proptest strategies that test roundtrip via the `Packet` trait

**Verification:** `cargo test -p oxidized-protocol`

---

## Acceptance Criteria

- [ ] `PacketDecodeError` enum defined in `oxidized-protocol::codec::packet`
- [ ] `Packet` trait defined in `oxidized-protocol::codec::packet`
- [ ] All 59 existing packets implement `Packet` (60 impls: 59 packet files × 1 impl,
  except `clientbound_move_entity` has 3 impls and `serverbound_move_player` is exempt —
  its 4-packet-ID pattern does not fit the single `const PACKET_ID` requirement; see
  module-level doc comment)
- [ ] `Connection::send_packet<P>()` method exists and works
- [ ] All 15 per-packet error types removed
- [ ] All 16 `map_err` conversions in server handlers replaced
- [ ] Server handlers use `send_packet()` instead of manual encode+send_raw+flush
- [ ] Generic roundtrip test helper exists and is used
- [ ] `cargo test --workspace` passes with zero failures
- [ ] `cargo clippy --workspace` produces no new warnings
- [ ] No stale references to removed error types (verified by grep)

---

## Ordering & Dependencies

```
Sub-phase 1 (trait + error)
    ↓
Sub-phase 2a-2e (implement Packet for all packets)
    ↓
Sub-phase 3 (Connection generic methods)    ←── can start after 2a
    ↓
Sub-phase 4 (server handler migration)      ←── depends on 2 + 3
    ↓
Sub-phase 5 (remove old error types)        ←── depends on 4
    ↓
Sub-phase 6 (generic test helper)           ←── can start after 2
```

Sub-phases 2 and 6 can partially overlap (test helper written after first batch).

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Error type removal breaks downstream | Grep all references before removing; migrate all callers first |
| Trait method signature conflicts with inherent method | Rename inherent methods to `decode_inner`/`encode_inner` during transition |
| Complex packets (PlayerInfoUpdate, LevelChunk) don't fit the trait | `Packet::decode` returns `PacketDecodeError::InvalidData` for complex validation; manual impls are fine |
| Compile time increases from monomorphization | 59 packets is small; profile if concerned |
| Merge conflicts with concurrent Phase 19 work | Complete R2 before starting P19 |

---

## Metrics

Track before/after:

| Metric | Before | After (Target) |
|--------|--------|-----------------|
| Per-packet error types | 15 | 0 |
| `map_err` conversions in server | 16 | 0 |
| Lines to send a packet | 3 | 1 |
| Lines to receive+decode a packet | 5-8 | 1 |
| Generic roundtrip test helper | No | Yes |
| Packets implementing `Packet` trait | 0 | 59 |
