//! Procedural macros for the Oxidized Minecraft server.
//!
//! This crate provides derive macros for automatic packet codec generation:
//!
//! - `#[derive(McPacket)]` — generates packet ID, state, and direction metadata
//! - `#[derive(McRead)]` — generates `fn read(buf: &mut impl Buf) -> Result<Self>`
//! - `#[derive(McWrite)]` — generates `fn write(&self, buf: &mut impl BufMut) -> Result<()>`
//!
//! See [ADR-007](https://github.com/dodoflix/Oxidized/blob/main/docs/adr/adr-007-packet-codec.md)
//! for design rationale.

use proc_macro::TokenStream;

/// Derives the `McPacket` trait, providing packet ID, protocol state, and direction metadata.
///
/// # Attributes
///
/// - `#[packet(id = 0x00, state = "play", direction = "clientbound")]`
///
/// # Example
///
/// ```ignore
/// #[derive(McPacket, McRead, McWrite)]
/// #[packet(id = 0x00, state = "handshaking", direction = "serverbound")]
/// pub struct HandshakePacket {
///     pub protocol_version: VarInt,
///     pub server_address: String,
///     pub server_port: u16,
///     pub next_state: VarInt,
/// }
/// ```
#[proc_macro_derive(McPacket, attributes(packet))]
pub fn derive_mc_packet(_input: TokenStream) -> TokenStream {
    // TODO: Phase 2-3 implementation
    TokenStream::new()
}

/// Derives `McRead` for automatic deserialization from the Minecraft binary protocol.
#[proc_macro_derive(McRead, attributes(mc))]
pub fn derive_mc_read(_input: TokenStream) -> TokenStream {
    // TODO: Phase 2-3 implementation
    TokenStream::new()
}

/// Derives `McWrite` for automatic serialization to the Minecraft binary protocol.
#[proc_macro_derive(McWrite, attributes(mc))]
pub fn derive_mc_write(_input: TokenStream) -> TokenStream {
    // TODO: Phase 2-3 implementation
    TokenStream::new()
}
