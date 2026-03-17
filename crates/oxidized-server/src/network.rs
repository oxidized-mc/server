//! TCP listener and per-connection handler for the Oxidized server.
//!
//! Binds to the configured address, accepts incoming connections, and
//! spawns a Tokio task per client that reads raw packet frames and logs
//! them in debug mode. Actual packet dispatch is deferred to Phase 3.

use std::net::SocketAddr;

use oxidized_protocol::connection::{Connection, ConnectionError};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

/// Starts the TCP listener and accepts connections until a shutdown signal
/// is received.
///
/// # Errors
///
/// Returns an error if the listener fails to bind to `addr`.
pub async fn listen(
    addr: SocketAddr,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!(address = %addr, "Listening for connections");

    loop {
        tokio::select! {
            biased;

            _ = shutdown_rx.recv() => {
                info!("Shutdown signal received — stopping listener");
                break;
            }

            result = listener.accept() => {
                match result {
                    Ok((stream, peer_addr)) => {
                        info!(peer = %peer_addr, "New connection");
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, peer_addr).await {
                                debug!(peer = %peer_addr, error = %e, "Connection closed");
                            }
                        });
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to accept connection");
                    }
                }
            }
        }
    }

    Ok(())
}

/// Handles a single client connection by reading raw packets in a loop.
///
/// Logs each packet's ID and size at debug level. Returns when the client
/// disconnects or an unrecoverable error occurs.
async fn handle_connection(
    stream: tokio::net::TcpStream,
    addr: SocketAddr,
) -> Result<(), ConnectionError> {
    let mut conn = Connection::new(stream, addr)?;
    debug!(
        peer = %addr,
        state = %conn.state,
        "Connection established",
    );

    loop {
        match conn.read_raw_packet().await {
            Ok(pkt) => {
                debug!(
                    peer = %addr,
                    state = %conn.state,
                    packet_id = format_args!("0x{:02X}", pkt.id),
                    size = pkt.data.len(),
                    "Received packet",
                );
                // TODO: Phase 3 — dispatch based on conn.state
            },
            Err(ConnectionError::Io(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                debug!(peer = %addr, "Client disconnected");
                return Ok(());
            },
            Err(e) => {
                debug!(peer = %addr, error = %e, "Connection error");
                return Err(e);
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidized_protocol::codec::{frame, varint};
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpStream;

    #[tokio::test]
    async fn test_listener_accepts_connection() {
        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        // Bind to a random port
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let listener = TcpListener::bind(addr).await.unwrap();
        let bound_addr = listener.local_addr().unwrap();
        drop(listener); // Release the port so `listen` can bind it

        let shutdown_rx = shutdown_tx.subscribe();
        let server = tokio::spawn(async move {
            listen(bound_addr, shutdown_rx).await.unwrap();
        });

        // Give the server time to bind
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Connect a client and send a raw handshake-like packet
        let mut client = TcpStream::connect(bound_addr).await.unwrap();

        // Build a minimal packet: VarInt(packet_id=0x00) + body
        let mut inner = bytes::BytesMut::new();
        varint::write_varint_buf(0x00, &mut inner);
        inner.extend_from_slice(b"\x00"); // minimal body

        frame::write_frame(&mut client, &inner).await.unwrap();
        client.flush().await.unwrap();

        // Allow time for the server to process
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Shut down
        let _ = shutdown_tx.send(());
        let _ = server.await;
    }

    #[tokio::test]
    async fn test_listener_graceful_shutdown() {
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let listener = TcpListener::bind(addr).await.unwrap();
        let bound_addr = listener.local_addr().unwrap();
        drop(listener);

        let shutdown_rx = shutdown_tx.subscribe();
        let server = tokio::spawn(async move {
            listen(bound_addr, shutdown_rx).await.unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Send shutdown immediately
        let _ = shutdown_tx.send(());

        // Server should exit cleanly
        let result = tokio::time::timeout(tokio::time::Duration::from_secs(2), server).await;
        assert!(result.is_ok(), "server should shut down within 2 seconds");
    }
}
