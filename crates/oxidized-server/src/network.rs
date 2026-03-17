//! TCP listener and per-connection handler for the Oxidized server.
//!
//! Binds to the configured address, accepts incoming connections, and
//! spawns a Tokio task per client. Handles the HANDSHAKING and STATUS
//! protocol states so the server appears in Minecraft's multiplayer list.

use std::net::SocketAddr;
use std::sync::Arc;

use oxidized_protocol::connection::{Connection, ConnectionError, ConnectionState, RawPacket};
use oxidized_protocol::packets::handshake::{ClientIntent, ClientIntentionPacket};
use oxidized_protocol::packets::status::{
    ClientboundPongResponsePacket, ClientboundStatusResponsePacket, ServerboundPingRequestPacket,
    ServerboundStatusRequestPacket,
};
use oxidized_protocol::status::ServerStatus;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

/// Starts the TCP listener and accepts connections until a shutdown signal
/// is received.
///
/// `server_status` is pre-built from config and shared across all connections.
///
/// # Errors
///
/// Returns an error if the listener fails to bind to `addr`.
pub async fn listen(
    addr: SocketAddr,
    server_status: Arc<ServerStatus>,
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
                        let status = Arc::clone(&server_status);
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, peer_addr, &status).await {
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

/// Handles a single client connection through the protocol state machine.
///
/// Dispatches packets based on the current [`ConnectionState`]:
/// - **Handshaking** → parse [`ClientIntentionPacket`], transition state
/// - **Status** → respond with server status JSON and pong
/// - **Login** / other → not yet implemented (Phase 4+)
async fn handle_connection(
    stream: tokio::net::TcpStream,
    addr: SocketAddr,
    server_status: &ServerStatus,
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

                match conn.state {
                    ConnectionState::Handshaking => {
                        handle_handshake(&mut conn, pkt).await?;
                    },
                    ConnectionState::Status => {
                        let done = handle_status(&mut conn, pkt, server_status).await?;
                        if done {
                            debug!(peer = %addr, "Status exchange complete");
                            return Ok(());
                        }
                    },
                    ConnectionState::Login => {
                        // Phase 4 will handle login
                        debug!(peer = %addr, "Login not yet implemented");
                        return Ok(());
                    },
                    _ => {
                        debug!(peer = %addr, state = %conn.state, "Unhandled state");
                        return Ok(());
                    },
                }
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

/// Processes the handshake packet and transitions to the requested state.
async fn handle_handshake(conn: &mut Connection, pkt: RawPacket) -> Result<(), ConnectionError> {
    if pkt.id != ClientIntentionPacket::PACKET_ID {
        warn!(
            peer = %conn.remote_addr(),
            packet_id = pkt.id,
            "Expected handshake packet (0x00)",
        );
        return Ok(());
    }

    let intention = ClientIntentionPacket::decode(pkt.data).map_err(|e| {
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })?;

    conn.protocol_version = intention.protocol_version;

    debug!(
        peer = %conn.remote_addr(),
        protocol_version = intention.protocol_version,
        server_address = %intention.server_address,
        server_port = intention.server_port,
        intent = ?intention.next_state,
        "Handshake received",
    );

    conn.state = match intention.next_state {
        ClientIntent::Status => ConnectionState::Status,
        ClientIntent::Login | ClientIntent::Transfer => ConnectionState::Login,
    };

    Ok(())
}

/// Processes STATUS state packets. Returns `true` when the exchange is
/// complete (after sending the pong), signaling the connection should close.
async fn handle_status(
    conn: &mut Connection,
    pkt: RawPacket,
    server_status: &ServerStatus,
) -> Result<bool, ConnectionError> {
    match pkt.id {
        ServerboundStatusRequestPacket::PACKET_ID => {
            let _request = ServerboundStatusRequestPacket::decode(pkt.data);

            let response = ClientboundStatusResponsePacket {
                status_json: server_status
                    .to_json()
                    .map_err(|e| ConnectionError::Io(std::io::Error::other(e.to_string())))?,
            };
            let body = response.encode();
            conn.send_raw(ClientboundStatusResponsePacket::PACKET_ID, &body)
                .await?;
            conn.flush().await?;

            debug!(
                peer = %conn.remote_addr(),
                json_len = response.status_json.len(),
                "Sent status response",
            );
            Ok(false)
        },
        ServerboundPingRequestPacket::PACKET_ID => {
            let ping = ServerboundPingRequestPacket::decode(pkt.data).map_err(|e| {
                ConnectionError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.to_string(),
                ))
            })?;

            let pong = ClientboundPongResponsePacket { time: ping.time };
            let body = pong.encode();
            conn.send_raw(ClientboundPongResponsePacket::PACKET_ID, &body)
                .await?;
            conn.flush().await?;

            debug!(
                peer = %conn.remote_addr(),
                time = ping.time,
                "Sent pong response",
            );
            Ok(true) // Close after pong (vanilla behavior)
        },
        unknown => {
            warn!(
                peer = %conn.remote_addr(),
                packet_id = unknown,
                "Unknown status packet",
            );
            Ok(false)
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use bytes::BytesMut;
    use oxidized_protocol::codec::{frame, varint};
    use oxidized_protocol::constants;
    use oxidized_protocol::status::{Component, StatusPlayers, StatusVersion};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    fn test_server_status() -> ServerStatus {
        ServerStatus {
            version: StatusVersion {
                name: constants::VERSION_NAME.to_string(),
                protocol: constants::PROTOCOL_VERSION,
            },
            players: StatusPlayers {
                max: 20,
                online: 0,
                sample: Vec::new(),
            },
            description: Component::text("Test Server"),
            favicon: None,
            enforces_secure_chat: false,
        }
    }

    /// Sends a framed packet (VarInt length + VarInt packet_id + body) over a raw stream.
    async fn send_packet(stream: &mut TcpStream, packet_id: i32, body: &[u8]) {
        let mut inner = BytesMut::new();
        varint::write_varint_buf(packet_id, &mut inner);
        inner.extend_from_slice(body);
        frame::write_frame(stream, &inner).await.unwrap();
        stream.flush().await.unwrap();
    }

    /// Reads one framed packet and returns (packet_id, body).
    async fn read_packet(stream: &mut TcpStream) -> (i32, bytes::Bytes) {
        let frame_data =
            frame::read_frame(stream, oxidized_protocol::codec::frame::MAX_PACKET_SIZE)
                .await
                .unwrap();
        let mut buf = frame_data;
        let id = varint::read_varint_buf(&mut buf).unwrap();
        (id, buf)
    }

    #[tokio::test]
    async fn test_full_status_exchange() {
        let status = Arc::new(test_server_status());
        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        // Bind to a random port
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let tcp_listener = TcpListener::bind(addr).await.unwrap();
        let bound_addr = tcp_listener.local_addr().unwrap();
        drop(tcp_listener);

        let shutdown_rx = shutdown_tx.subscribe();
        let status_clone = Arc::clone(&status);
        let server = tokio::spawn(async move {
            listen(bound_addr, status_clone, shutdown_rx).await.unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let mut client = TcpStream::connect(bound_addr).await.unwrap();

        // 1. Send handshake (intent = Status)
        let handshake = ClientIntentionPacket {
            protocol_version: constants::PROTOCOL_VERSION,
            server_address: "localhost".to_string(),
            server_port: 25565,
            next_state: ClientIntent::Status,
        };
        let handshake_body = handshake.encode();
        send_packet(
            &mut client,
            ClientIntentionPacket::PACKET_ID,
            &handshake_body,
        )
        .await;

        // 2. Send status request (empty body)
        send_packet(&mut client, ServerboundStatusRequestPacket::PACKET_ID, &[]).await;

        // 3. Read status response
        let (resp_id, resp_body) = read_packet(&mut client).await;
        assert_eq!(resp_id, ClientboundStatusResponsePacket::PACKET_ID);

        // Parse the response JSON string
        use oxidized_protocol::codec::types;
        let mut resp_data = resp_body;
        let json_str = types::read_string(&mut resp_data, 32767).unwrap();
        let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(json["version"]["protocol"], constants::PROTOCOL_VERSION);
        assert_eq!(json["description"]["text"], "Test Server");
        assert_eq!(json["players"]["max"], 20);

        // 4. Send ping request
        let ping_time: i64 = 1_719_000_000_000;
        let mut ping_body = BytesMut::new();
        oxidized_protocol::codec::types::write_i64(&mut ping_body, ping_time);
        send_packet(
            &mut client,
            ServerboundPingRequestPacket::PACKET_ID,
            &ping_body,
        )
        .await;

        // 5. Read pong response
        let (pong_id, pong_body) = read_packet(&mut client).await;
        assert_eq!(pong_id, ClientboundPongResponsePacket::PACKET_ID);
        let mut pong_data = pong_body;
        let echoed_time = types::read_i64(&mut pong_data).unwrap();
        assert_eq!(echoed_time, ping_time);

        // 6. Server should have closed our connection after pong
        // Reading should return EOF
        let mut eof_buf = [0u8; 1];
        let read_result = client.read(&mut eof_buf).await.unwrap();
        assert_eq!(read_result, 0, "expected EOF after pong");

        // Clean up
        let _ = shutdown_tx.send(());
        let _ = server.await;
    }

    #[tokio::test]
    async fn test_protocol_mismatch_still_responds() {
        let status = Arc::new(test_server_status());
        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let tcp_listener = TcpListener::bind(addr).await.unwrap();
        let bound_addr = tcp_listener.local_addr().unwrap();
        drop(tcp_listener);

        let shutdown_rx = shutdown_tx.subscribe();
        let status_clone = Arc::clone(&status);
        let server = tokio::spawn(async move {
            listen(bound_addr, status_clone, shutdown_rx).await.unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let mut client = TcpStream::connect(bound_addr).await.unwrap();

        // Send handshake with WRONG protocol version
        let handshake = ClientIntentionPacket {
            protocol_version: 999,
            server_address: "localhost".to_string(),
            server_port: 25565,
            next_state: ClientIntent::Status,
        };
        let handshake_body = handshake.encode();
        send_packet(
            &mut client,
            ClientIntentionPacket::PACKET_ID,
            &handshake_body,
        )
        .await;

        // Send status request
        send_packet(&mut client, ServerboundStatusRequestPacket::PACKET_ID, &[]).await;

        // Should still get a valid response (vanilla behavior)
        let (resp_id, resp_body) = read_packet(&mut client).await;
        assert_eq!(resp_id, ClientboundStatusResponsePacket::PACKET_ID);

        use oxidized_protocol::codec::types;
        let mut resp_data = resp_body;
        let json_str = types::read_string(&mut resp_data, 32767).unwrap();
        let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(json["version"]["protocol"], constants::PROTOCOL_VERSION);

        let _ = shutdown_tx.send(());
        let _ = server.await;
    }

    #[tokio::test]
    async fn test_listener_graceful_shutdown() {
        let status = Arc::new(test_server_status());
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let tcp_listener = TcpListener::bind(addr).await.unwrap();
        let bound_addr = tcp_listener.local_addr().unwrap();
        drop(tcp_listener);

        let shutdown_rx = shutdown_tx.subscribe();
        let server = tokio::spawn(async move {
            listen(bound_addr, status, shutdown_rx).await.unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let _ = shutdown_tx.send(());

        let result = tokio::time::timeout(tokio::time::Duration::from_secs(2), server).await;
        assert!(result.is_ok(), "server should shut down within 2 seconds");
    }
}
