//! Procedural macros for the Oxidized Minecraft server.
//!
//! # ⚠️ Stub Crate — Not Yet Implemented
//!
//! **All macros in this crate are placeholder stubs.** They accept any input and
//! return an empty [`TokenStream`], generating no code. Applying these derives to a
//! struct will compile successfully but will **not** produce any trait implementations.
//!
//! Currently, all 59+ packets in `oxidized-protocol` are manually implemented using
//! inherent `decode()`/`encode()` methods and the [`Packet`] trait from
//! [ADR-038](../../../docs/adr/adr-038-packet-trait-unified-codec.md). The derive
//! macros below will be implemented in a future phase to replace that boilerplate.
//!
//! # Planned Functionality
//!
//! When implemented, this crate will provide three derive macros:
//!
//! - **`#[derive(McPacket)]`** — generates an `impl Packet` block with packet ID,
//!   `decode()`, and `encode()` methods, replacing per-packet manual implementations.
//! - **`#[derive(McRead)]`** — generates field-by-field deserialization from the
//!   Minecraft binary wire format.
//! - **`#[derive(McWrite)]`** — generates field-by-field serialization to the
//!   Minecraft binary wire format.
//!
//! # Design References
//!
//! - [ADR-007](../../../docs/adr/adr-007-packet-codec.md) — original derive-macro
//!   design specifying `#[packet(...)]` attributes and supported wire types.
//! - [ADR-038](../../../docs/adr/adr-038-packet-trait-unified-codec.md) — the
//!   `Packet` trait and unified `PacketDecodeError` that these macros will target.
//!
//! # Intended Usage (Future)
//!
//! ```ignore
//! use oxidized_macros::{McPacket, McRead, McWrite};
//!
//! #[derive(Debug, Clone, McPacket, McRead, McWrite)]
//! #[packet(id = 0x00, state = Handshaking, direction = Serverbound)]
//! pub struct HandshakePacket {
//!     pub protocol_version: VarInt,
//!     pub server_address: McString<255>,
//!     pub server_port: u16,
//!     pub next_state: VarInt,
//! }
//! ```

use proc_macro::TokenStream;

/// Derives the `Packet` trait, providing packet ID, decode, and encode methods.
///
/// # ⚠️ Stub — returns an empty `TokenStream` (no-op)
///
/// This macro is **not yet implemented**. Applying it to a struct will compile but
/// will not generate any code. All packets currently use manual `impl Packet` blocks
/// in `oxidized-protocol`.
///
/// # Planned Attributes
///
/// - `#[packet(id = 0x00, state = Handshaking, direction = Serverbound)]`
///   — Specifies the numeric packet ID, protocol state (`Handshaking`, `Status`,
///   `Login`, `Configuration`, `Play`), and direction (`Clientbound`/`Serverbound`).
///
/// # Planned Behaviour
///
/// When implemented, this macro will generate:
/// - `impl Packet for T` with `const PACKET_ID: i32`
/// - `fn decode(data: Bytes) -> Result<Self, PacketDecodeError>` — field-by-field
///   deserialization in declaration order
/// - `fn encode(&self) -> BytesMut` — field-by-field serialization in declaration order
///
/// # Example (Future)
///
/// ```ignore
/// #[derive(Debug, Clone, McPacket)]
/// #[packet(id = 0x00, state = Handshaking, direction = Serverbound)]
/// pub struct HandshakePacket {
///     pub protocol_version: VarInt,
///     pub server_address: McString<255>,
///     pub server_port: u16,
///     pub next_state: VarInt,
/// }
///
/// // Will generate (conceptually):
/// // impl Packet for HandshakePacket {
/// //     const PACKET_ID: i32 = 0x00;
/// //     fn decode(data: Bytes) -> Result<Self, PacketDecodeError> { ... }
/// //     fn encode(&self) -> BytesMut { ... }
/// // }
/// ```
///
/// See [ADR-007] and [ADR-038] for design details.
///
/// [ADR-007]: ../../../docs/adr/adr-007-packet-codec.md
/// [ADR-038]: ../../../docs/adr/adr-038-packet-trait-unified-codec.md
#[proc_macro_derive(McPacket, attributes(packet))]
pub fn derive_mc_packet(_input: TokenStream) -> TokenStream {
    // STUB: returns empty TokenStream — no code is generated.
    // See ADR-007 / ADR-038 for the planned implementation.
    TokenStream::new()
}

/// Derives `McRead` for automatic deserialization from the Minecraft binary protocol.
///
/// # ⚠️ Stub — returns an empty `TokenStream` (no-op)
///
/// This macro is **not yet implemented**. Applying it to a struct will compile but
/// will not generate any deserialization code.
///
/// # Planned Attributes
///
/// - `#[mc(with = "read_fn")]` — use a custom read function for a field
/// - `#[mc(length_prefix)]` — read a `VarInt` count followed by that many elements
/// - `#[mc(if = "self.some_flag")]` — conditionally read a field based on a prior value
///
/// # Planned Behaviour
///
/// When implemented, this macro will generate a `read` method that deserializes each
/// struct field sequentially from a `bytes::Buf`, using the Minecraft wire-format
/// encoding for each type:
///
/// | Rust type | Wire encoding |
/// |-----------|---------------|
/// | `VarInt` | Variable-length i32 (1–5 bytes) |
/// | `VarLong` | Variable-length i64 (1–10 bytes) |
/// | `McString<N>` | VarInt length + UTF-8 bytes (max N chars) |
/// | `bool` | Single byte (0x00 / 0x01) |
/// | `u8`, `i8` | 1 byte |
/// | `u16`, `i16` | 2 bytes big-endian |
/// | `i32`, `i64` | 4/8 bytes big-endian |
/// | `f32`, `f64` | 4/8 bytes big-endian IEEE 754 |
/// | `u128` | 16 bytes big-endian (UUID) |
///
/// # Example (Future)
///
/// ```ignore
/// #[derive(McRead)]
/// pub struct StatusRequest;  // zero fields — just reads nothing
///
/// #[derive(McRead)]
/// pub struct LoginStart {
///     pub username: McString<16>,
///     pub uuid: u128,
/// }
/// ```
///
/// See [ADR-007] for the full wire-type mapping.
///
/// [ADR-007]: ../../../docs/adr/adr-007-packet-codec.md
#[proc_macro_derive(McRead, attributes(mc))]
pub fn derive_mc_read(_input: TokenStream) -> TokenStream {
    // STUB: returns empty TokenStream — no code is generated.
    // See ADR-007 for the planned implementation.
    TokenStream::new()
}

/// Derives `McWrite` for automatic serialization to the Minecraft binary protocol.
///
/// # ⚠️ Stub — returns an empty `TokenStream` (no-op)
///
/// This macro is **not yet implemented**. Applying it to a struct will compile but
/// will not generate any serialization code.
///
/// # Planned Attributes
///
/// - `#[mc(with = "write_fn")]` — use a custom write function for a field
/// - `#[mc(length_prefix)]` — write a `VarInt` count before the collection elements
/// - `#[mc(if = "self.some_flag")]` — conditionally write a field
///
/// # Planned Behaviour
///
/// When implemented, this macro will generate a `write` method that serializes each
/// struct field sequentially into a `bytes::BufMut`, using the Minecraft wire-format
/// encoding (same type table as [`McRead`](derive.McRead.html)).
///
/// # Example (Future)
///
/// ```ignore
/// #[derive(McWrite)]
/// pub struct StatusResponse {
///     pub json_response: McString<32767>,
/// }
///
/// #[derive(McWrite)]
/// pub struct KeepAlive {
///     pub id: i64,
/// }
/// ```
///
/// See [ADR-007] for the full wire-type mapping.
///
/// [ADR-007]: ../../../docs/adr/adr-007-packet-codec.md
#[proc_macro_derive(McWrite, attributes(mc))]
pub fn derive_mc_write(_input: TokenStream) -> TokenStream {
    // STUB: returns empty TokenStream — no code is generated.
    // See ADR-007 for the planned implementation.
    TokenStream::new()
}
