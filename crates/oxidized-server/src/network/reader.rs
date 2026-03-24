//! Reader task — reads packets from the network and dispatches to game logic.
//!
//! The reader task is one half of the [ADR-006] per-connection task pair.
//! It reads raw packets from the TCP stream (handling decryption and
//! decompression), applies rate limiting, and dispatches decoded packets
//! through the bounded inbound channel to game logic.
//!
//! [ADR-006]: ../../../docs/adr/adr-006-network-io.md

// Reader task is implemented in R4.4 but not integrated until R4.5+.
#![allow(dead_code)]

use std::time::{Duration, Instant};

use oxidized_protocol::transport::channel::{InboundPacket, MAX_PACKETS_PER_TICK};
use oxidized_protocol::transport::connection::{ConnectionError, ConnectionReader};
use tokio::sync::mpsc;
use tracing::debug;

/// Duration of one rate-limiting window (one server tick).
const TICK_WINDOW: Duration = Duration::from_millis(50);

/// Runs the reader task for a single client connection.
///
/// Reads raw packets from the [`ConnectionReader`] (which handles
/// decryption and decompression), enforces a per-tick rate limit, and
/// dispatches each packet to the game logic through the bounded inbound
/// channel.
///
/// # Rate limiting
///
/// The client may send at most [`MAX_PACKETS_PER_TICK`] (500) packets
/// per 50 ms window. Exceeding this limit causes an immediate disconnect
/// with [`ConnectionError::RateLimited`].
///
/// # Backpressure
///
/// The inbound channel has a bounded capacity (128). When the game logic
/// is slow to consume packets, `inbound_tx.send().await` blocks, which
/// stops reading from TCP, triggering TCP flow control on the client.
///
/// # Shutdown
///
/// The task exits cleanly (`Ok(())`) when the inbound channel receiver
/// is dropped (game logic disconnects the player).
///
/// # Errors
///
/// Returns [`ConnectionError`] on I/O failure, malformed packets,
/// decompression errors, or rate-limit violations.
pub async fn reader_loop(
    mut reader: ConnectionReader,
    inbound_tx: mpsc::Sender<InboundPacket>,
) -> Result<(), ConnectionError> {
    let mut packets_this_window: u32 = 0;
    let mut window_start = Instant::now();

    loop {
        let raw = reader.read_raw_packet().await?;

        // Rate limiting: reset counter each tick window
        let elapsed = window_start.elapsed();
        if elapsed >= TICK_WINDOW {
            packets_this_window = 0;
            window_start = Instant::now();
        }
        packets_this_window += 1;

        if packets_this_window > MAX_PACKETS_PER_TICK {
            debug!(
                peer = %reader.remote_addr(),
                count = packets_this_window,
                "Reader task: rate limit exceeded ({MAX_PACKETS_PER_TICK} packets/tick)"
            );
            return Err(ConnectionError::RateLimited(MAX_PACKETS_PER_TICK));
        }

        let inbound = InboundPacket {
            id: raw.id,
            data: raw.data,
        };

        // Bounded send — blocks if game logic is slow (backpressure)
        if inbound_tx.send(inbound).await.is_err() {
            debug!(
                peer = %reader.remote_addr(),
                "Reader task: inbound channel closed, shutting down"
            );
            return Ok(()); // Game loop dropped receiver — clean exit
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    use bytes::Bytes;
    use oxidized_protocol::transport::channel::{INBOUND_CHANNEL_CAPACITY, InboundPacket};
    use oxidized_protocol::transport::connection::Connection;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::mpsc;

    /// Helper: creates a connected pair, splits the server side,
    /// returns (reader, client Connection) for testing.
    async fn reader_pair() -> (ConnectionReader, Connection) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let client_handle = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });
        let (server_stream, peer_addr) = listener.accept().await.unwrap();
        let client_stream = client_handle.await.unwrap();

        let server = Connection::new(server_stream, peer_addr).unwrap();
        let client_conn = Connection::new(client_stream, "127.0.0.1:0".parse().unwrap()).unwrap();

        let (reader, _writer) = server.into_split();
        (reader, client_conn)
    }

    #[tokio::test]
    async fn test_reader_dispatches_packets() {
        let (reader, mut client) = reader_pair().await;
        let (tx, mut rx) = mpsc::channel(INBOUND_CHANNEL_CAPACITY);

        let handle = tokio::spawn(reader_loop(reader, tx));

        // Client sends a packet
        client.send_raw(0x0E, b"hello").await.unwrap();
        client.flush().await.unwrap();

        let pkt = rx.recv().await.unwrap();
        assert_eq!(pkt.id, 0x0E);
        assert_eq!(&pkt.data[..], b"hello");

        // Clean shutdown: drop client so reader gets EOF
        drop(client);
        let result = handle.await.unwrap();
        // EOF produces an I/O error — that's expected
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_reader_multiple_packets() {
        let (reader, mut client) = reader_pair().await;
        let (tx, mut rx) = mpsc::channel(INBOUND_CHANNEL_CAPACITY);

        let handle = tokio::spawn(reader_loop(reader, tx));

        for i in 0..10 {
            client
                .send_raw(i, format!("pkt_{i}").as_bytes())
                .await
                .unwrap();
        }
        client.flush().await.unwrap();

        for i in 0..10 {
            let pkt = rx.recv().await.unwrap();
            assert_eq!(pkt.id, i);
            assert_eq!(pkt.data, Bytes::from(format!("pkt_{i}")));
        }

        drop(client);
        let _ = handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_reader_clean_shutdown_on_receiver_drop() {
        let (reader, mut client) = reader_pair().await;
        let (tx, rx) = mpsc::channel(INBOUND_CHANNEL_CAPACITY);

        let handle = tokio::spawn(reader_loop(reader, tx));

        // Send a packet so the reader is active
        client.send_raw(0x01, b"data").await.unwrap();
        client.flush().await.unwrap();

        // Small delay to let reader process
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Drop receiver — reader should exit cleanly on next send attempt
        drop(rx);

        // Send another packet so reader tries to dispatch and sees closed channel
        client.send_raw(0x02, b"more").await.unwrap();
        client.flush().await.unwrap();

        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("reader task should exit within timeout")
            .unwrap();
        assert!(result.is_ok(), "Reader should return Ok on channel close");
    }

    #[tokio::test]
    async fn test_reader_rate_limit_disconnect() {
        let (reader, mut client) = reader_pair().await;
        // Large channel so backpressure doesn't interfere with rate limiting
        let (tx, _rx) = mpsc::channel(1024);

        let handle = tokio::spawn(reader_loop(reader, tx));

        // Blast 501+ packets as fast as possible within one tick window.
        // Each packet is tiny so they all fit in TCP buffers.
        for i in 0..=MAX_PACKETS_PER_TICK {
            if client.send_raw(0x01, &[i as u8]).await.is_err() {
                break;
            }
        }
        let _ = client.flush().await;

        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("reader task should exit within timeout")
            .unwrap();

        assert!(result.is_err(), "Reader should return error on rate limit");
        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("rate limited"),
            "Expected rate limit error, got: {err_str}"
        );
    }

    #[tokio::test]
    async fn test_reader_rate_limit_resets_after_window() {
        let (reader, mut client) = reader_pair().await;
        let (tx, mut rx) = mpsc::channel(1024);

        let handle = tokio::spawn(reader_loop(reader, tx));

        // Send 400 packets (under limit) in first window
        for i in 0..400u32 {
            client.send_raw(0x01, &i.to_le_bytes()).await.unwrap();
        }
        client.flush().await.unwrap();

        // Receive them all
        for _ in 0..400 {
            rx.recv().await.unwrap();
        }

        // Wait for the tick window to reset
        tokio::time::sleep(Duration::from_millis(60)).await;

        // Send another batch (should work since window reset)
        for i in 0..400u32 {
            client.send_raw(0x02, &i.to_le_bytes()).await.unwrap();
        }
        client.flush().await.unwrap();

        for _ in 0..400 {
            let pkt = rx.recv().await.unwrap();
            assert_eq!(pkt.id, 0x02);
        }

        drop(client);
        let _ = handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_reader_encrypted_packets() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let client_handle = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });
        let (server_stream, peer_addr) = listener.accept().await.unwrap();
        let client_stream = client_handle.await.unwrap();

        let mut server = Connection::new(server_stream, peer_addr).unwrap();
        let mut client_conn =
            Connection::new(client_stream, "127.0.0.1:0".parse().unwrap()).unwrap();

        let secret = [0x42u8; 16];
        server.enable_encryption(&secret);
        client_conn.enable_encryption(&secret);

        let (reader, _writer) = server.into_split();
        let (tx, mut rx) = mpsc::channel(INBOUND_CHANNEL_CAPACITY);

        let handle = tokio::spawn(reader_loop(reader, tx));

        client_conn
            .send_raw(0x05, b"encrypted payload")
            .await
            .unwrap();
        client_conn.flush().await.unwrap();

        let pkt = rx.recv().await.unwrap();
        assert_eq!(pkt.id, 0x05);
        assert_eq!(&pkt.data[..], b"encrypted payload");

        drop(client_conn);
        let _ = handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_reader_compressed_packets() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let client_handle = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });
        let (server_stream, peer_addr) = listener.accept().await.unwrap();
        let client_stream = client_handle.await.unwrap();

        let mut server = Connection::new(server_stream, peer_addr).unwrap();
        let mut client_conn =
            Connection::new(client_stream, "127.0.0.1:0".parse().unwrap()).unwrap();

        server.enable_compression(64);
        client_conn.enable_compression(64);

        let (reader, _writer) = server.into_split();
        let (tx, mut rx) = mpsc::channel(INBOUND_CHANNEL_CAPACITY);

        let handle = tokio::spawn(reader_loop(reader, tx));

        // Large payload (above threshold)
        let payload = vec![0xAB; 256];
        client_conn.send_raw(0x07, &payload).await.unwrap();
        client_conn.flush().await.unwrap();

        // Small payload (below threshold)
        client_conn.send_raw(0x08, b"tiny").await.unwrap();
        client_conn.flush().await.unwrap();

        let pkt1 = rx.recv().await.unwrap();
        assert_eq!(pkt1.id, 0x07);
        assert_eq!(&pkt1.data[..], &payload[..]);

        let pkt2 = rx.recv().await.unwrap();
        assert_eq!(pkt2.id, 0x08);
        assert_eq!(&pkt2.data[..], b"tiny");

        drop(client_conn);
        let _ = handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_reader_encrypted_and_compressed() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let client_handle = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });
        let (server_stream, peer_addr) = listener.accept().await.unwrap();
        let client_stream = client_handle.await.unwrap();

        let mut server = Connection::new(server_stream, peer_addr).unwrap();
        let mut client_conn =
            Connection::new(client_stream, "127.0.0.1:0".parse().unwrap()).unwrap();

        let secret = [0xBBu8; 16];
        server.enable_encryption(&secret);
        client_conn.enable_encryption(&secret);
        server.enable_compression(64);
        client_conn.enable_compression(64);

        let (reader, _writer) = server.into_split();
        let (tx, mut rx) = mpsc::channel(INBOUND_CHANNEL_CAPACITY);

        let handle = tokio::spawn(reader_loop(reader, tx));

        let large = vec![0xCD; 512];
        client_conn.send_raw(0x09, &large).await.unwrap();
        client_conn.flush().await.unwrap();

        client_conn.send_raw(0x0A, b"small").await.unwrap();
        client_conn.flush().await.unwrap();

        let pkt1 = rx.recv().await.unwrap();
        assert_eq!(pkt1.id, 0x09);
        assert_eq!(&pkt1.data[..], &large[..]);

        let pkt2 = rx.recv().await.unwrap();
        assert_eq!(pkt2.id, 0x0A);
        assert_eq!(&pkt2.data[..], b"small");

        drop(client_conn);
        let _ = handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_reader_backpressure_blocks() {
        let (reader, mut client) = reader_pair().await;
        // Channel capacity = 1 to easily trigger backpressure
        let (tx, rx) = mpsc::channel::<InboundPacket>(1);

        let handle = tokio::spawn(reader_loop(reader, tx));

        // Send 3 packets — channel capacity 1, so reader should block after 1
        for i in 0..3 {
            client.send_raw(i, &[i as u8]).await.unwrap();
        }
        client.flush().await.unwrap();

        // Give reader time to read and fill the channel
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Reader should be blocked on send (channel full).
        // Drop the receiver — reader should then get send error and exit.
        drop(rx);

        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("reader task should exit within timeout")
            .unwrap();
        assert!(result.is_ok(), "Reader should return Ok on channel close");
    }

    #[tokio::test]
    async fn test_reader_preserves_packet_order() {
        let (reader, mut client) = reader_pair().await;
        let (tx, mut rx) = mpsc::channel(256);

        let handle = tokio::spawn(reader_loop(reader, tx));

        // Send 100 packets with sequential IDs
        for i in 0..100 {
            client.send_raw(i, &(i as u32).to_le_bytes()).await.unwrap();
        }
        client.flush().await.unwrap();

        // Verify they arrive in order
        for i in 0..100 {
            let pkt = rx.recv().await.unwrap();
            assert_eq!(pkt.id, i, "Packet {i} arrived out of order");
        }

        drop(client);
        let _ = handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_reader_exits_on_tcp_close() {
        let (reader, client) = reader_pair().await;
        let (tx, _rx) = mpsc::channel(INBOUND_CHANNEL_CAPACITY);

        let handle = tokio::spawn(reader_loop(reader, tx));

        // Close the client side — reader should get EOF
        drop(client);

        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("reader task should exit within timeout")
            .unwrap();

        // EOF produces an I/O error
        assert!(result.is_err());
    }
}
