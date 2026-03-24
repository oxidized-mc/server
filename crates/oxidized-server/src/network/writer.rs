//! Writer task — batches outbound packets and flushes to the network.
//!
//! The writer task is the core of the [ADR-006] performance model.
//! Instead of flushing after every packet (one syscall per packet), it
//! drains all queued outbound packets and flushes once per drain cycle.
//!
//! [ADR-006]: ../../../docs/adr/adr-006-network-io.md

// Writer task is implemented in R4.3 but not integrated until R4.5+.
#![allow(dead_code)]

use std::time::Duration;

use oxidized_protocol::transport::channel::{OutboundPacket, MAX_CONNECTION_MEMORY};
use oxidized_protocol::transport::connection::{ConnectionError, ConnectionWriter};
use tokio::sync::mpsc;
use tracing::debug;

/// Timeout for a single TCP write operation.
///
/// If the client cannot accept data within this window, it is
/// considered a slow client and the connection is terminated.
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

/// Runs the writer task for a single client connection.
///
/// Receives outbound packets from the bounded `mpsc` channel, encodes
/// them into the writer's batch buffer (compressing each frame if
/// enabled), then encrypts and flushes the entire batch in a single
/// TCP write syscall.
///
/// # Batching strategy
///
/// 1. `recv().await` — blocks until at least one packet arrives.
/// 2. `try_recv()` — drains all additional queued packets (non-blocking).
/// 3. Encode all packets into the batch buffer (compress per-frame).
/// 4. Encrypt the entire batch in-place (if encryption is enabled).
/// 5. Single `write_all` + `flush` for the batch.
/// 6. Clear the buffer and loop.
///
/// # Shutdown
///
/// The task exits cleanly (`Ok(())`) when all [`mpsc::Sender`] handles
/// for the outbound channel are dropped (game logic disconnects the
/// player).
///
/// # Errors
///
/// Returns [`ConnectionError`] on I/O failure, compression errors,
/// memory budget violations, or write timeouts.
pub async fn writer_loop(
    mut writer: ConnectionWriter,
    mut outbound_rx: mpsc::Receiver<OutboundPacket>,
) -> Result<(), ConnectionError> {
    loop {
        // Block until at least one packet arrives
        let packet = match outbound_rx.recv().await {
            Some(pkt) => pkt,
            None => {
                // All senders dropped — clean shutdown
                debug!(peer = %writer.remote_addr(), "Writer task: channel closed, shutting down");
                return Ok(());
            }
        };

        // Encode first packet into batch buffer
        writer.encode_to_batch(packet.id, &packet.data)?;

        // Drain all remaining queued packets (non-blocking)
        while let Ok(packet) = outbound_rx.try_recv() {
            writer.encode_to_batch(packet.id, &packet.data)?;

            // Memory budget check (ADR-006: 256 KB per connection)
            if writer.batch_buf_len() > MAX_CONNECTION_MEMORY {
                debug!(
                    peer = %writer.remote_addr(),
                    buf_len = writer.batch_buf_len(),
                    "Writer task: memory budget exceeded"
                );
                return Err(ConnectionError::Io(std::io::Error::new(
                    std::io::ErrorKind::OutOfMemory,
                    "per-connection memory budget exceeded",
                )));
            }
        }

        // Encrypt + write + flush the entire batch (with timeout)
        match tokio::time::timeout(WRITE_TIMEOUT, writer.flush_batch()).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                debug!(
                    peer = %writer.remote_addr(),
                    "Writer task: write timeout ({WRITE_TIMEOUT:?})"
                );
                return Err(ConnectionError::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "write timeout (slow client)",
                )));
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    use bytes::Bytes;
    use oxidized_protocol::transport::channel::OutboundPacket;
    use oxidized_protocol::transport::connection::Connection;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::mpsc;

    /// Helper: creates a connected pair, splits the server side,
    /// returns (writer, client Connection, server addr).
    async fn writer_pair() -> (ConnectionWriter, Connection) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let client_handle = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });
        let (server_stream, peer_addr) = listener.accept().await.unwrap();
        let client_stream = client_handle.await.unwrap();

        let server = Connection::new(server_stream, peer_addr).unwrap();
        let client_conn =
            Connection::new(client_stream, "127.0.0.1:0".parse().unwrap()).unwrap();

        let (_reader, writer) = server.into_split();
        (writer, client_conn)
    }

    #[tokio::test]
    async fn test_writer_single_packet() {
        let (writer, mut client) = writer_pair().await;
        let (tx, rx) = mpsc::channel(16);

        let handle = tokio::spawn(writer_loop(writer, rx));

        tx.send(OutboundPacket {
            id: 0x01,
            data: Bytes::from_static(b"hello"),
        })
        .await
        .unwrap();

        let pkt = client.read_raw_packet().await.unwrap();
        assert_eq!(pkt.id, 0x01);
        assert_eq!(&pkt.data[..], b"hello");

        // Clean shutdown
        drop(tx);
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_writer_multiple_packets() {
        let (writer, mut client) = writer_pair().await;
        let (tx, rx) = mpsc::channel(64);

        let handle = tokio::spawn(writer_loop(writer, rx));

        for i in 0..10 {
            tx.send(OutboundPacket {
                id: i,
                data: Bytes::from(format!("pkt_{i}")),
            })
            .await
            .unwrap();
        }

        for i in 0..10 {
            let pkt = client.read_raw_packet().await.unwrap();
            assert_eq!(pkt.id, i);
            assert_eq!(pkt.data, Bytes::from(format!("pkt_{i}")));
        }

        drop(tx);
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_writer_clean_shutdown_on_channel_close() {
        let (writer, _client) = writer_pair().await;
        let (tx, rx) = mpsc::channel(16);

        let handle = tokio::spawn(writer_loop(writer, rx));

        // Drop all senders — writer task should exit cleanly
        drop(tx);
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_writer_multiple_drain_cycles() {
        let (writer, mut client) = writer_pair().await;
        let (tx, rx) = mpsc::channel(16);

        let handle = tokio::spawn(writer_loop(writer, rx));

        // Cycle 1: single packet
        tx.send(OutboundPacket {
            id: 0x01,
            data: Bytes::from_static(b"first"),
        })
        .await
        .unwrap();

        let pkt = client.read_raw_packet().await.unwrap();
        assert_eq!(pkt.id, 0x01);
        assert_eq!(&pkt.data[..], b"first");

        // Cycle 2: another packet after the first was flushed
        tx.send(OutboundPacket {
            id: 0x02,
            data: Bytes::from_static(b"second"),
        })
        .await
        .unwrap();

        let pkt = client.read_raw_packet().await.unwrap();
        assert_eq!(pkt.id, 0x02);
        assert_eq!(&pkt.data[..], b"second");

        drop(tx);
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_writer_exits_on_tcp_close() {
        let (writer, client) = writer_pair().await;
        let (tx, rx) = mpsc::channel(1024);

        let handle = tokio::spawn(writer_loop(writer, rx));

        // Close the client side — the kernel may buffer the first few
        // writes, so we send many packets until the OS detects the RST.
        drop(client);

        let big_payload = Bytes::from(vec![0xFFu8; 8192]);
        for _ in 0..200 {
            if tx
                .send(OutboundPacket {
                    id: 0x01,
                    data: big_payload.clone(),
                })
                .await
                .is_err()
            {
                break;
            }
        }

        let result = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("writer task should exit within timeout")
            .unwrap();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_writer_encrypted_packets() {
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

        let (_reader, writer) = server.into_split();
        let (tx, rx) = mpsc::channel(16);

        let handle = tokio::spawn(writer_loop(writer, rx));

        tx.send(OutboundPacket {
            id: 0x05,
            data: Bytes::from_static(b"encrypted payload"),
        })
        .await
        .unwrap();

        let pkt = client_conn.read_raw_packet().await.unwrap();
        assert_eq!(pkt.id, 0x05);
        assert_eq!(&pkt.data[..], b"encrypted payload");

        drop(tx);
        handle.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_writer_compressed_packets() {
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

        let (_reader, writer) = server.into_split();
        let (tx, rx) = mpsc::channel(16);

        let handle = tokio::spawn(writer_loop(writer, rx));

        // Large payload (above threshold)
        let payload = vec![0xAB; 256];
        tx.send(OutboundPacket {
            id: 0x07,
            data: Bytes::from(payload.clone()),
        })
        .await
        .unwrap();

        // Small payload (below threshold)
        tx.send(OutboundPacket {
            id: 0x08,
            data: Bytes::from_static(b"tiny"),
        })
        .await
        .unwrap();

        let pkt1 = client_conn.read_raw_packet().await.unwrap();
        assert_eq!(pkt1.id, 0x07);
        assert_eq!(&pkt1.data[..], &payload[..]);

        let pkt2 = client_conn.read_raw_packet().await.unwrap();
        assert_eq!(pkt2.id, 0x08);
        assert_eq!(&pkt2.data[..], b"tiny");

        drop(tx);
        handle.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_writer_encrypted_and_compressed() {
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

        let (_reader, writer) = server.into_split();
        let (tx, rx) = mpsc::channel(16);

        let handle = tokio::spawn(writer_loop(writer, rx));

        let large = vec![0xCD; 512];
        tx.send(OutboundPacket {
            id: 0x09,
            data: Bytes::from(large.clone()),
        })
        .await
        .unwrap();

        tx.send(OutboundPacket {
            id: 0x0A,
            data: Bytes::from_static(b"small"),
        })
        .await
        .unwrap();

        let pkt1 = client_conn.read_raw_packet().await.unwrap();
        assert_eq!(pkt1.id, 0x09);
        assert_eq!(&pkt1.data[..], &large[..]);

        let pkt2 = client_conn.read_raw_packet().await.unwrap();
        assert_eq!(pkt2.id, 0x0A);
        assert_eq!(&pkt2.data[..], b"small");

        drop(tx);
        handle.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_writer_memory_budget_exceeded() {
        let (writer, _client) = writer_pair().await;
        let (tx, rx) = mpsc::channel(1024);

        let handle = tokio::spawn(writer_loop(writer, rx));

        // Fill the channel with huge packets that exceed 256 KB combined.
        // Each packet is 64 KB of data, so 5 should exceed the budget.
        let big_payload = Bytes::from(vec![0xFFu8; 64 * 1024]);
        for _ in 0..5 {
            // Ignore send errors (writer may have already exited)
            let _ = tx
                .send(OutboundPacket {
                    id: 0x01,
                    data: big_payload.clone(),
                })
                .await;
        }

        // Give the writer time to drain and hit the budget
        let result = handle.await.unwrap();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("memory budget"));
    }

    #[tokio::test]
    async fn test_writer_preserves_packet_order() {
        let (writer, mut client) = writer_pair().await;
        let (tx, rx) = mpsc::channel(256);

        let handle = tokio::spawn(writer_loop(writer, rx));

        // Send 100 packets with sequential IDs
        for i in 0..100 {
            tx.send(OutboundPacket {
                id: i,
                data: Bytes::from(vec![i as u8; 4]),
            })
            .await
            .unwrap();
        }

        // Verify they arrive in order
        for i in 0..100 {
            let pkt = client.read_raw_packet().await.unwrap();
            assert_eq!(pkt.id, i, "Packet {i} arrived out of order");
        }

        drop(tx);
        handle.await.unwrap().unwrap();
    }
}
