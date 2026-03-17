# ADR-007: Packet Codec Framework

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P02–P06, P12–P14 |
| Deciders | Oxidized Core Team |

## Context

The Minecraft Java protocol defines approximately 300 packet types across 5 protocol states (Handshaking, Status, Login, Configuration, Play) and 2 directions (Clientbound, Serverbound). Each packet has a unique ID within its state/direction pair and a specific wire format defined by a sequence of typed fields. Vanilla Java implements each packet as a class with manual `write(FriendlyByteBuf)` and `read(FriendlyByteBuf)` methods that call `buf.readVarInt()`, `buf.writeUtf()`, etc. This is repetitive — every packet class contains boilerplate encode/decode logic that mirrors its field declarations.

The wire format uses Minecraft-specific data types that don't map directly to standard binary formats: `VarInt` (variable-length 32-bit integer, 1-5 bytes), `VarLong` (variable-length 64-bit, 1-10 bytes), `McString` (VarInt length-prefixed UTF-8 with a maximum length), `Position` (packed x/y/z into a single 64-bit integer), `Angle` (rotation as a single byte, 256 steps per revolution), and `Optional<T>` (boolean prefix followed by T if true). Nested structures like NBT compounds, item stacks, and block entities are embedded within packets.

We need a systematic approach to defining packet structures that is correct (matching vanilla's wire format exactly), maintainable (adding a new packet should be trivial), and performant (no runtime reflection, minimal allocations during encode/decode).

## Decision Drivers

- **Wire-level correctness**: encoded packets must be byte-identical to what vanilla produces — clients will reject malformed packets
- **Compile-time safety**: field type mismatches (e.g., encoding a `u32` where a `VarInt` is expected) must be caught at compile time, not runtime
- **Minimal boilerplate**: adding a new packet should require only a struct definition with annotations, not manual encode/decode implementations
- **Zero runtime reflection**: packet codec must be fully resolved at compile time — no type registries, no dynamic dispatch in the hot path
- **Auditability**: it should be easy to compare a packet definition against the protocol spec to verify correctness
- **Extensibility**: custom wire types (e.g., `BitSet`, `Identifier`, `RegistryEntry`) must be easy to add

## Considered Options

### Option 1: Manual encode/decode per packet

Define each packet as a struct and manually implement `encode` and `decode` methods, calling wire type functions directly. This is how vanilla does it. It's straightforward and requires no proc-macros, but it's extremely repetitive — each packet duplicates the encode/decode pattern, and field ordering bugs are easy to introduce. With ~300 packets, this becomes thousands of lines of near-identical boilerplate.

### Option 2: Derive macro (#[derive(McPacket)])

Define a proc-macro that generates `encode` and `decode` implementations from struct field definitions. Field types are mapped to wire operations: `VarInt` calls `read_var_int()`/`write_var_int()`, `McString<256>` calls `read_string(256)`/`write_string(256)`, etc. This is the Rust-idiomatic approach — similar to how `serde` derives `Serialize`/`Deserialize` from struct definitions. The macro generates compile-time-checked code with zero runtime overhead.

### Option 3: Schema-driven code generation from protocol spec

Use a protocol specification (e.g., from PrismarineJS's `minecraft-data` or wiki.vg) as the source of truth, and generate Rust packet structs and codecs from it. This ensures perfect alignment with the spec and enables automatic updates when the protocol changes. However, the spec formats are not always precise enough for code generation, generated code is harder to debug and customize, and we'd depend on an external data source whose format we don't control.

### Option 4: Serde with custom Serializer/Deserializer

Leverage Rust's `serde` ecosystem by implementing a custom `Serializer` and `Deserializer` for the Minecraft wire format. Packet structs would use `#[derive(Serialize, Deserialize)]` and the Minecraft-specific types would be handled by the custom serializer. This reuses serde's derive infrastructure but is a poor fit — serde assumes self-describing formats, while Minecraft's protocol is positional. Custom serde serializers for binary formats tend to be fragile and hard to debug.

## Decision

**We adopt the derive macro approach (#[derive(McPacket)]) with a trait-based wire type system.** Each packet is defined as a Rust struct with `#[derive(McPacket)]` which generates `McWrite` and `McRead` implementations. Field types determine the wire encoding automatically.

### Wire Type Traits

```rust
/// Trait for types that can be written to the Minecraft wire format.
pub trait McWrite {
    fn mc_write(&self, buf: &mut BytesMut) -> Result<(), ProtocolError>;
}

/// Trait for types that can be read from the Minecraft wire format.
pub trait McRead: Sized {
    fn mc_read(buf: &mut Bytes) -> Result<Self, ProtocolError>;
}
```

### Wire Type Implementations

| Rust Type | Wire Format | Bytes |
|-----------|-------------|-------|
| `VarInt` | Variable-length i32 (LEB128 variant) | 1–5 |
| `VarLong` | Variable-length i64 | 1–10 |
| `bool` | Single byte (0x00 or 0x01) | 1 |
| `u8` / `i8` | Unsigned/signed byte | 1 |
| `u16` / `i16` | Big-endian 16-bit | 2 |
| `u32` / `i32` | Big-endian 32-bit | 4 |
| `u64` / `i64` | Big-endian 64-bit | 8 |
| `u128` | Big-endian 128-bit (UUIDs) | 16 |
| `f32` | Big-endian IEEE 754 | 4 |
| `f64` | Big-endian IEEE 754 | 8 |
| `McString<N>` | VarInt length + UTF-8 (max N chars) | variable |
| `Position` | Packed i64 (x:26, z:26, y:12) | 8 |
| `Angle` | Single byte (256 steps = 360°) | 1 |
| `NbtCompound` | NBT binary format | variable |
| `Optional<T>` | bool prefix + T (if true) | 1 + sizeof(T) |
| `LengthPrefixed<Vec<T>>` | VarInt count + T repeated | variable |
| `RawBytes` | Remaining bytes (no length prefix) | variable |

### Example Packet Definition

```rust
#[derive(Debug, Clone, McPacket)]
#[packet(id = 0x00, state = Handshaking, direction = Serverbound)]
pub struct HandshakePacket {
    pub protocol_version: VarInt,
    pub server_address: McString<255>,
    pub server_port: u16,
    pub next_state: VarInt,
}

#[derive(Debug, Clone, McPacket)]
#[packet(id = 0x01, state = Play, direction = Clientbound)]
pub struct SpawnEntityPacket {
    pub entity_id: VarInt,
    pub entity_uuid: u128,
    pub entity_type: VarInt,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub pitch: Angle,
    pub yaw: Angle,
    pub head_yaw: Angle,
    pub data: VarInt,
    pub velocity_x: i16,
    pub velocity_y: i16,
    pub velocity_z: i16,
}
```

### Conditional Fields

Some packets have fields that are only present based on a prior field's value. These use `#[mc_if]` attributes:

```rust
#[derive(Debug, Clone, McPacket)]
#[packet(id = 0x03, state = Play, direction = Clientbound)]
pub struct PlayerInfoUpdatePacket {
    pub actions: u8,
    #[mc_length_prefix]
    pub entries: Vec<PlayerInfoEntry>,
}

// For complex conditional logic that can't be expressed with attributes,
// implement McRead/McWrite manually:
impl McRead for PlayerInfoEntry {
    fn mc_read(buf: &mut Bytes) -> Result<Self, ProtocolError> {
        // Manual decode with conditional fields
    }
}
```

### Packet Registry

A dispatch table maps `(State, Direction, PacketId)` to a decode function:

```rust
type DecodeFn = fn(&mut Bytes) -> Result<Box<dyn Packet>, ProtocolError>;

pub struct PacketRegistry {
    decoders: HashMap<(ProtocolState, Direction, i32), DecodeFn>,
}

impl PacketRegistry {
    pub fn new() -> Self {
        let mut reg = Self { decoders: HashMap::new() };

        // Generated by a build script or macro from all #[packet(...)] attributes
        reg.register::<HandshakePacket>();
        reg.register::<StatusRequestPacket>();
        reg.register::<LoginStartPacket>();
        // ... all ~300 packets
        reg
    }

    pub fn decode(
        &self,
        state: ProtocolState,
        direction: Direction,
        id: i32,
        buf: &mut Bytes,
    ) -> Result<Box<dyn Packet>, ProtocolError> {
        let decoder = self.decoders
            .get(&(state, direction, id))
            .ok_or(ProtocolError::UnknownPacket { id, state: state.as_str() })?;
        decoder(buf)
    }
}
```

### Packet Trait

```rust
pub trait Packet: std::fmt::Debug + Send + Sync {
    fn id(&self) -> i32;
    fn state(&self) -> ProtocolState;
    fn direction(&self) -> Direction;
    fn encode(&self, buf: &mut BytesMut) -> Result<(), ProtocolError>;
}
```

The `McPacket` derive macro generates a `Packet` impl for each struct, delegating `encode` to the generated `McWrite` impl and providing the `id`, `state`, and `direction` from the `#[packet(...)]` attribute.

## Consequences

### Positive

- Adding a new packet is a single struct definition with annotations — no manual encode/decode logic
- Field type mismatches are caught at compile time (e.g., using `u32` instead of `VarInt` won't compile if `u32` doesn't implement `McWrite` the right way)
- Generated code is zero-overhead — compiles to the same instructions as hand-written encode/decode
- Packet definitions serve as readable documentation of the wire format — easy to audit against the protocol spec
- The `McRead`/`McWrite` trait system is extensible — new wire types are added by implementing two trait methods

### Negative

- Proc-macros increase compile time, especially with ~300 packet structs — initial build may take 10-15 seconds longer
- Debugging generated code requires `cargo expand` to see the macro output — errors in macro-generated code can have obscure error messages
- Complex conditional fields (e.g., `PlayerInfoUpdate` with bitflag-driven field presence) may require manual `McRead`/`McWrite` impls, reducing the benefit of the derive macro for those packets

### Neutral

- The `Box<dyn Packet>` in the registry adds one heap allocation per decoded packet — acceptable given that packet processing is not the bottleneck (world simulation is)
- The registry is populated once at startup and never mutated — no runtime overhead for dispatch beyond a `HashMap` lookup

## Compliance

- **Round-trip test**: every packet type must have a test that encodes a value, decodes it, and asserts equality — generated by a test macro or build script
- **Vanilla compatibility test**: capture real vanilla packets (via proxy), decode with our codec, re-encode, and compare bytes — must be identical
- **Packet ID uniqueness**: a compile-time or startup check ensures no two packets share the same `(state, direction, id)` tuple
- **Code review**: any manual `McRead`/`McWrite` impl (bypassing the derive macro) must include a comment explaining why the derive macro is insufficient and a reference to the protocol spec
- **Spec tracking**: each packet struct must have a doc comment with a link to the corresponding wiki.vg or minecraft.wiki page

## Related ADRs

- [ADR-003: Crate Workspace Architecture](adr-003-crate-architecture.md) — packet codec lives in `oxidized-protocol`
- [ADR-006: Network I/O Architecture](adr-006-network-io.md) — reader/writer tasks call packet encode/decode
- [ADR-008: Connection State Machine](adr-008-connection-state-machine.md) — packet dispatch depends on current protocol state
- [ADR-009: Encryption & Compression Pipeline](adr-009-encryption-compression.md) — frames are decrypted/decompressed before packet decoding

## References

- [wiki.vg — Minecraft Protocol](https://wiki.vg/Protocol)
- [minecraft.wiki — Protocol](https://minecraft.wiki/w/Java_Edition_protocol)
- [bytes crate — BytesMut / Bytes](https://docs.rs/bytes/latest/bytes/)
- [Rust proc-macro workshop](https://github.com/dtolnay/proc-macro-workshop)
- [serde derive internals](https://serde.rs/derive.html) — inspiration for the derive macro approach
