//! Transport layer: TCP connection management, compression, and encryption.
//!
//! Groups the low-level protocol concerns that operate below the packet layer:
//! - [`connection`] — Per-client TCP stream, state machine, packet framing
//! - [`compression`] — Zlib compression/decompression for packet payloads
//! - [`crypto`] — AES-128-CFB8 encryption, RSA-1024 key exchange, SHA-1 digest

pub mod compression;
pub mod connection;
pub mod crypto;
