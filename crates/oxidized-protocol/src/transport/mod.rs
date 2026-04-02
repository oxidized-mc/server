//! Transport layer: TCP connection management.
//!
//! Groups the low-level protocol concerns that operate below the packet layer:
//! - [`connection`] — Per-client TCP stream, state machine, packet framing
//! - [`channel`] — Channel types and constants for the reader/writer task pair
//! - [`handle`] — Connection handle API for the outbound channel

pub mod channel;
pub mod connection;
pub mod handle;
