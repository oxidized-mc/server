# ADR-038: Packet Trait & Unified Codec Error

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-19 |
| Phases | R2 (Refactoring), All future network phases |
| Deciders | Oxidized Core Team |

## Context

ADR-007 (Packet Codec Framework) specified a `Packet` trait, `McRead`/`McWrite` traits, and
`#[derive(McPacket)]` for automatic codec generation. This was accepted before Phase 2 but
**never implemented** — all 59 packets use manual inherent methods:

```rust
impl SomePacket {
    pub const PACKET_ID: i32 = 0x42;
    pub fn decode(mut data: Bytes) -> Result<Self, SomeError> { ... }
    pub fn encode(&self) -> BytesMut { ... }
}
```

The derive macros in `oxidized-macros` (`McPacket`, `McRead`, `McWrite`) are stubs returning
empty `TokenStream`. This was a pragmatic choice — manual implementations were faster to write
during early phases when the protocol was still being understood.

Now at 59 packets (Phase 18 complete), four structural problems have emerged:

1. **No generic send/receive**: Every packet send requires 3 manual lines
   (`encode()` → `send_raw(PACKET_ID, &body)` → `flush()`). Every receive requires a 5-line
   `map_err` block converting the packet-specific error to `ConnectionError`. There are 16
   identical `map_err` conversions across the server crate.

2. **15 nearly-identical error types**: `HelloError`, `PingError`, `KeyError`,
   `DisconnectError`, etc. — most are single-variant wrappers around `TypeError`. Each
   packet file defines its own error enum even when the error structure is identical.

3. **No trait-based dispatch**: The server dispatches packets via manual `match pkt.id`
   statements. Play state already has 14 match arms. By Phase 38 this will be ~58 serverbound
   play packet arms alone.

4. **No generic roundtrip testing**: Each packet's roundtrip test is hand-written. A trait
   would enable property-based testing across all packets simultaneously.

With Phases 19–38 adding approximately 126 more packets, these problems will compound
to the point where a retrofit becomes prohibitively expensive. The time to act is now,
between the completion of Phase R1 (structural refactoring) and the start of Phase 19
(world ticking).

## Decision Drivers

- **Backward compatibility**: Existing code must continue working during migration — no
  big-bang rewrite
- **Zero runtime overhead**: The trait must compile to the same code as current inherent
  methods (monomorphization, no vtable dispatch in the hot path)
- **Incremental adoption**: Packets can be migrated one-by-one; unmigrated packets retain
  inherent methods
- **Unified errors**: A single decode error type eliminates per-packet error boilerplate
- **Generic programming**: Enable `send_packet<P>()`, `decode_packet<P>()`, and generic
  roundtrip tests
- **Derive macro readiness**: The trait must be the target for future `McPacket` derive
  macro implementation (ADR-007)

## Considered Options

### Option 1: Implement ADR-007 as written (traits + derive macros + registry)

Implement the full `McRead`/`McWrite` trait system, `Packet` trait, and `PacketRegistry`
with `Box<dyn Packet>` dispatch as specified in ADR-007. This is comprehensive but very
large — it requires implementing the derive macros, changing every packet's type signature,
and introducing dynamic dispatch.

### Option 2: Minimal Packet trait with unified error (incremental)

Add a `Packet` trait and `PacketDecodeError` to `oxidized-protocol`. Implement the trait
for all existing packets alongside their current inherent methods. Add generic `send_packet`
and `decode_packet` to `Connection`. Defer derive macros and registry to a later phase.

### Option 3: Just unify errors, no trait

Replace the 15 per-packet error types with a single `PacketDecodeError` but don't add a
`Packet` trait. This solves the error duplication but doesn't enable generic programming.

## Decision

**We adopt Option 2: Minimal Packet trait with unified error, implemented incrementally.**
This captures 80% of the benefit (generic send/receive, unified errors, generic testing)
with 20% of the effort of the full ADR-007 vision. The derive macros and packet registry
are deferred — they can be added later as a pure enhancement.

### Unified Error Type

```rust
/// Errors that can occur when decoding any packet from wire bytes.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PacketDecodeError {
    /// A wire type could not be read from the buffer.
    #[error(transparent)]
    Type(#[from] TypeError),

    /// A VarInt/VarLong exceeded its maximum size.
    #[error(transparent)]
    VarInt(#[from] VarIntError),

    /// An I/O error occurred during decode.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// A resource location could not be parsed.
    #[error(transparent)]
    ResourceLocation(#[from] ResourceLocationError),

    /// An NBT value could not be decoded.
    #[error(transparent)]
    Nbt(#[from] NbtError),

    /// Packet-specific decode failure with a descriptive message.
    #[error("{0}")]
    InvalidData(String),
}
```

This replaces: `HelloError`, `PingError`, `KeyError`, `LoginFinishedError`,
`LoginCompressionError`, `DisconnectError`, `IntentionError`, `RegistryDataError`,
`KnownPacksError`, `UpdateEnabledFeaturesError`, `UpdateTagsError`,
`ServerboundKnownPacksError`, `ClientInformationError`, `PlayPacketError`.

Per-packet error types that have variants beyond `TypeError` wrapping (e.g.,
`IntentionError::UnknownIntent`) are mapped to `PacketDecodeError::InvalidData`.

### Packet Trait

```rust
/// A Minecraft protocol packet that can be encoded and decoded.
///
/// All packets in the protocol implement this trait, providing their wire
/// packet ID and encode/decode methods with a unified error type.
pub trait Packet: Sized + std::fmt::Debug {
    /// The packet ID on the wire (state-dependent).
    const PACKET_ID: i32;

    /// Decodes the packet from raw body bytes (after the packet ID has been
    /// stripped by the framing layer).
    fn decode(data: Bytes) -> Result<Self, PacketDecodeError>;

    /// Encodes the packet body to bytes (without the packet ID or framing).
    fn encode(&self) -> BytesMut;
}
```

Key design choices:
- `Sized` bound — no `Box<dyn Packet>` dispatch; everything is monomorphized
- `PACKET_ID` is an associated const — no runtime lookup, inlined by the compiler
- Error type is `PacketDecodeError` — unified across all packets
- `encode` returns `BytesMut` — consistent with existing pattern

### Generic Connection Methods

```rust
impl Connection {
    /// Sends a typed packet (encodes, frames, and flushes).
    pub async fn send_packet<P: Packet>(
        &mut self,
        pkt: &P,
    ) -> Result<(), ConnectionError> {
        let body = pkt.encode();
        self.send_raw(P::PACKET_ID, &body).await?;
        self.flush().await
    }

    /// Decodes a raw packet into a typed packet.
    pub fn decode_packet<P: Packet>(
        &self,
        raw: &RawPacket,
    ) -> Result<P, PacketDecodeError> {
        P::decode(raw.data.clone())
    }
}
```

### Migration Strategy

Each packet is migrated in 3 steps (can be done one packet at a time):

1. **Add `impl Packet for SomePacket`** — delegates to existing `decode`/`encode` methods,
   converting the per-packet error to `PacketDecodeError`
2. **Update server handler** — replace `SomePacket::decode(data).map_err(...)` with
   `conn.decode_packet::<SomePacket>(&raw)?` or `P::decode(data)?`
3. **Remove per-packet error type** — once no code references it

The inherent `PACKET_ID`, `decode`, and `encode` methods remain during migration. Once all
callers use the trait, they can be removed (or kept as aliases).

### Relationship to ADR-007

This ADR implements the **trait layer** from ADR-007 (`Packet` trait, unified errors) but
defers:
- `McRead`/`McWrite` traits (fine-grained field-level codec traits)
- `#[derive(McPacket)]` implementation (proc-macro code generation)
- `PacketRegistry` with dynamic dispatch (`Box<dyn Packet>`)

These can be added incrementally on top of this foundation. When the derive macros are
eventually implemented, they will generate `impl Packet for X` — the same trait.

## Consequences

### Positive

- **Generic programming**: `conn.send_packet(&pkt)` replaces 3-line manual encode+send+flush
- **Unified errors**: 15 error types collapse to 1; 16 `map_err` conversions eliminated
- **Testability**: Generic `assert_roundtrip::<P>()` tests all packets uniformly
- **Future-proof**: Foundation for derive macros, packet registry, and protocol versioning
- **Incremental**: No big-bang migration — each packet can be converted independently
- **Zero overhead**: `Packet` trait compiles away via monomorphization

### Negative

- **Dual API during migration**: Both inherent methods and trait methods exist temporarily
- **Loss of per-packet error specificity**: `IntentionError::UnknownIntent(i32)` becomes
  `PacketDecodeError::InvalidData("unknown intent: 3")` — the string is less matchable
- **Trait coherence**: External crates cannot implement `Packet` for their own types
  (intentional — this is our protocol)

### Neutral

- Existing proptest roundtrip tests continue to work — they can optionally be migrated to
  use the trait-based generic test helper
- The `Packet` trait does not include `state()` or `direction()` — these are known
  statically at the handler level and don't need runtime dispatch

## Compliance

- **All new packets** (P19+) must implement `Packet` trait — inherent-method-only packets
  are no longer accepted
- **Migration tracking**: All 59 existing packets must be migrated during Phase R2
- **Error type**: No new per-packet error types — use `PacketDecodeError`
- **Generic send**: Server handlers must use `conn.send_packet()` or `P::encode()` via the
  trait, not `SomePacket::encode()` + `conn.send_raw()`
- **Round-trip test**: Every `Packet` impl must have a round-trip test

## Related ADRs

- [ADR-007: Packet Codec Framework](adr-007-packet-codec.md) — the original vision; this
  ADR implements the trait layer and defers the derive macro layer
- [ADR-002: Error Handling Strategy](adr-002-error-handling.md) — `PacketDecodeError` uses
  `thiserror`, consistent with library error strategy
- [ADR-036: Packet Handler Architecture](adr-036-packet-handler-architecture.md) — the
  `decode_packet<T>()` helper will be updated to use the `Packet` trait bound
- [ADR-008: Connection State Machine](adr-008-connection-state-machine.md) — dispatch
  remains module-level per ADR-036; the `Packet` trait does not encode state
