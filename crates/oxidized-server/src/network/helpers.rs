//! Shared utility functions for network packet handling.
//!
//! Provides the generic [`decode_packet`] helper that eliminates repeated
//! decode-match-log boilerplate, and the [`disconnect`] / [`disconnect_err`]
//! utilities for cleanly terminating a connection.

use oxidized_protocol::codec::Packet;
use oxidized_protocol::connection::{Connection, ConnectionError};
use oxidized_protocol::packets::login::ClientboundDisconnectPacket;
use tracing::debug;

/// Decodes a typed packet from raw body bytes using the [`Packet`] trait,
/// logging failures with connection context.
///
/// Converts [`PacketDecodeError`](oxidized_protocol::codec::PacketDecodeError)
/// into a [`ConnectionError::Protocol`].
///
/// # Usage
///
/// ```ignore
/// let ka: ServerboundKeepAlivePacket = decode_packet(
///     pkt.data, addr, player_name, "KeepAlive",
/// )?;
/// ```
pub fn decode_packet<P: Packet>(
    data: bytes::Bytes,
    addr: std::net::SocketAddr,
    player_name: &str,
    packet_name: &str,
) -> Result<P, ConnectionError> {
    P::decode(data).map_err(|e| {
        debug!(
            peer = %addr,
            name = %player_name,
            error = %e,
            "Failed to decode {packet_name}",
        );
        ConnectionError::Protocol(e)
    })
}

/// Sends a disconnect packet and returns the [`ConnectionError`] directly
/// (for use in expressions where the caller builds its own `Err`).
pub async fn disconnect_err(conn: &mut Connection, reason: &str) -> ConnectionError {
    let pkt = ClientboundDisconnectPacket {
        reason: reason.to_string(),
    };
    let _ = conn.send_packet(&pkt).await;
    ConnectionError::Io(std::io::Error::new(
        std::io::ErrorKind::ConnectionAborted,
        reason.to_string(),
    ))
}

/// Sends a disconnect packet to the client and returns a corresponding
/// [`ConnectionError`].
///
/// # Errors
///
/// Always returns a [`ConnectionError`] wrapping the disconnect reason.
pub async fn disconnect(conn: &mut Connection, reason: &str) -> Result<(), ConnectionError> {
    Err(disconnect_err(conn, reason).await)
}
