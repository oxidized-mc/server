//! Shared utility functions for network packet handling.
//!
//! Provides the generic [`decode_packet`] helper that eliminates repeated
//! decode-match-log boilerplate, and the [`disconnect`] / [`disconnect_err`]
//! utilities for cleanly terminating a connection.

use oxidized_protocol::connection::{Connection, ConnectionError};
use oxidized_protocol::packets::login::ClientboundDisconnectPacket;
use tracing::debug;

/// Decodes a packet from the result of a `Packet::decode(data)` call,
/// logging failures with connection context.
///
/// Converts any `Display`-implementing error into a [`ConnectionError::Io`]
/// with an `InvalidData` error kind.
///
/// # Usage
///
/// ```ignore
/// let ka = decode_packet(
///     ServerboundKeepAlivePacket::decode(pkt.data),
///     addr, player_name, "KeepAlive",
/// )?;
/// ```
pub fn decode_packet<T, E: std::fmt::Display>(
    result: Result<T, E>,
    addr: std::net::SocketAddr,
    player_name: &str,
    packet_name: &str,
) -> Result<T, ConnectionError> {
    result.map_err(|e| {
        debug!(
            peer = %addr,
            name = %player_name,
            error = %e,
            "Failed to decode {packet_name}",
        );
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Failed to decode {packet_name}: {e}"),
        ))
    })
}

/// Sends a disconnect packet to the client and returns a corresponding
/// [`ConnectionError`].
pub async fn disconnect(conn: &mut Connection, reason: &str) -> Result<(), ConnectionError> {
    let pkt = ClientboundDisconnectPacket {
        reason: reason.to_string(),
    };
    let body = pkt.encode();
    let _ = conn
        .send_raw(ClientboundDisconnectPacket::PACKET_ID, &body)
        .await;
    let _ = conn.flush().await;
    Err(ConnectionError::Io(std::io::Error::new(
        std::io::ErrorKind::ConnectionAborted,
        reason.to_string(),
    )))
}

/// Sends a disconnect packet and returns the [`ConnectionError`] directly
/// (for use in expressions where the caller builds its own `Err`).
pub async fn disconnect_err(conn: &mut Connection, reason: &str) -> ConnectionError {
    let pkt = ClientboundDisconnectPacket {
        reason: reason.to_string(),
    };
    let body = pkt.encode();
    let _ = conn
        .send_raw(ClientboundDisconnectPacket::PACKET_ID, &body)
        .await;
    let _ = conn.flush().await;
    ConnectionError::Io(std::io::Error::new(
        std::io::ErrorKind::ConnectionAborted,
        reason.to_string(),
    ))
}
