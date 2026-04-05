//! Network I/O compliance tests — full reader/writer task pipeline.
//!
//! Tests the reader task + writer task working together, exercising
//! the complete data path: client → reader → inbound channel →
//! game logic → outbound channel → writer → client.
//!
//! **Important:** Never use `..` when destructuring `TaskPairFixture`.
//! Rust keeps unnamed fields alive until end of scope, so a hidden
//! sender clone would prevent the writer from detecting channel close.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{Duration, Instant};

use bytes::Bytes;
use oxidized_protocol::transport::channel::{
    INBOUND_CHANNEL_CAPACITY, InboundPacket, OUTBOUND_CHANNEL_CAPACITY, OutboundPacket,
};
use oxidized_protocol::transport::connection::Connection;
use oxidized_protocol::transport::handle::ConnectionHandle;
use oxidized_server::network::reader::reader_loop;
use oxidized_server::network::writer::{DEFAULT_WRITE_TIMEOUT, writer_loop};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

/// Loopback task pair: server reader + writer tasks with a client connection.
struct TaskPairFixture {
    outbound_tx: mpsc::Sender<OutboundPacket>,
    inbound_rx: mpsc::Receiver<InboundPacket>,
    client: Connection,
    conn_handle: ConnectionHandle,
    reader_handle: tokio::task::JoinHandle<
        Result<(), oxidized_protocol::transport::connection::ConnectionError>,
    >,
    writer_handle: tokio::task::JoinHandle<
        Result<(), oxidized_protocol::transport::connection::ConnectionError>,
    >,
}

async fn setup_task_pair() -> TaskPairFixture {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let client_handle = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });
    let (server_stream, peer_addr) = listener.accept().await.unwrap();
    let client_stream = client_handle.await.unwrap();

    let server = Connection::new(server_stream, peer_addr).unwrap();
    let client = Connection::new(client_stream, "127.0.0.1:0".parse().unwrap()).unwrap();

    let (reader, writer) = server.into_split();
    let (inbound_tx, inbound_rx) = mpsc::channel(INBOUND_CHANNEL_CAPACITY);
    let (outbound_tx, outbound_rx) = mpsc::channel(OUTBOUND_CHANNEL_CAPACITY);

    let reader_handle = tokio::spawn(reader_loop(reader, inbound_tx));
    let writer_handle = tokio::spawn(writer_loop(writer, outbound_rx, DEFAULT_WRITE_TIMEOUT));

    let conn_handle = ConnectionHandle::new(outbound_tx.clone(), peer_addr);

    TaskPairFixture {
        outbound_tx,
        inbound_rx,
        client,
        conn_handle,
        reader_handle,
        writer_handle,
    }
}

async fn setup_encrypted_task_pair() -> TaskPairFixture {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let client_handle = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });
    let (server_stream, peer_addr) = listener.accept().await.unwrap();
    let client_stream = client_handle.await.unwrap();

    let mut server = Connection::new(server_stream, peer_addr).unwrap();
    let mut client = Connection::new(client_stream, "127.0.0.1:0".parse().unwrap()).unwrap();

    let secret = [0xE7u8; 16];
    server.enable_encryption(&secret);
    client.enable_encryption(&secret);

    let (reader, writer) = server.into_split();
    let (inbound_tx, inbound_rx) = mpsc::channel(INBOUND_CHANNEL_CAPACITY);
    let (outbound_tx, outbound_rx) = mpsc::channel(OUTBOUND_CHANNEL_CAPACITY);

    let reader_handle = tokio::spawn(reader_loop(reader, inbound_tx));
    let writer_handle = tokio::spawn(writer_loop(writer, outbound_rx, DEFAULT_WRITE_TIMEOUT));

    let conn_handle = ConnectionHandle::new(outbound_tx.clone(), peer_addr);

    TaskPairFixture {
        outbound_tx,
        inbound_rx,
        client,
        conn_handle,
        reader_handle,
        writer_handle,
    }
}

// ---------------------------------------------------------------------------
// Throughput: >5000 packets/sec required by the network I/O architecture
// ---------------------------------------------------------------------------

#[tokio::test]
async fn compliance_throughput_exceeds_5000_per_sec() {
    let TaskPairFixture {
        outbound_tx,
        inbound_rx,
        client: mut client_conn,
        conn_handle,
        reader_handle,
        writer_handle,
    } = setup_task_pair().await;
    drop(inbound_rx);
    drop(conn_handle);

    let packet_count: usize = 10_000;
    let start = Instant::now();

    // Sender runs concurrently — prevents deadlock when the bounded
    // channel (512) or TCP buffer fills up.
    let sender = tokio::spawn(async move {
        for i in 0..packet_count {
            outbound_tx
                .send(OutboundPacket {
                    id: (i % 64) as i32,
                    data: Bytes::from(vec![0xAA; 32]),
                })
                .await
                .unwrap();
        }
        outbound_tx
    });

    for _ in 0..packet_count {
        let pkt = client_conn.read_raw_packet().await.unwrap();
        assert_eq!(&pkt.data[..], &[0xAA; 32]);
    }

    let outbound_tx = sender.await.unwrap();
    let elapsed = start.elapsed();
    let pps = packet_count as f64 / elapsed.as_secs_f64();

    assert!(
        pps > 5000.0,
        "Throughput {pps:.0} packets/sec must exceed 5000; elapsed={elapsed:?}"
    );

    drop(outbound_tx);
    writer_handle.await.unwrap().unwrap();
    drop(client_conn);
    let _ = reader_handle.await.unwrap();
}

// ---------------------------------------------------------------------------
// Memory: connection memory stays under 256 KB budget
// ---------------------------------------------------------------------------

#[tokio::test]
async fn compliance_memory_under_budget_normal_load() {
    let TaskPairFixture {
        outbound_tx,
        inbound_rx,
        client: mut client_conn,
        conn_handle,
        reader_handle,
        writer_handle,
    } = setup_task_pair().await;
    drop(inbound_rx);
    drop(conn_handle);

    // 100 ticks × 20 packets × 200 bytes — interleaved send/read per tick
    // so neither channel nor TCP buffer saturates.
    for tick in 0..100 {
        for i in 0..20 {
            outbound_tx
                .send(OutboundPacket {
                    id: (tick * 20 + i) % 64,
                    data: Bytes::from(vec![0xBB; 200]),
                })
                .await
                .unwrap();
        }
        for _ in 0..20 {
            let pkt = client_conn.read_raw_packet().await.unwrap();
            assert_eq!(pkt.data.len(), 200);
        }
    }

    drop(outbound_tx);
    writer_handle.await.unwrap().unwrap();
    drop(client_conn);
    let _ = reader_handle.await.unwrap();
}

// ---------------------------------------------------------------------------
// Backpressure: slow client → disconnect, no OOM
// ---------------------------------------------------------------------------

#[tokio::test]
async fn compliance_backpressure_slow_client() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let client_handle = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });
    let (server_stream, peer_addr) = listener.accept().await.unwrap();
    let _client_stream = client_handle.await.unwrap();
    // Client never reads — simulates a slow/stalled client.

    let server = Connection::new(server_stream, peer_addr).unwrap();
    let (_reader, writer) = server.into_split();
    let (outbound_tx, outbound_rx) = mpsc::channel(OUTBOUND_CHANNEL_CAPACITY);

    let writer_handle = tokio::spawn(writer_loop(writer, outbound_rx, DEFAULT_WRITE_TIMEOUT));

    // Flood the writer from a separate task so we don't block the test
    // if the channel fills before the writer detects budget overflow.
    let sender = tokio::spawn(async move {
        let big_payload = Bytes::from(vec![0xFF; 64 * 1024]);
        for _ in 0..100 {
            if outbound_tx
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
    });

    // Writer must exit with an error (memory budget exceeded or write timeout).
    let result = tokio::time::timeout(Duration::from_secs(35), writer_handle)
        .await
        .expect("writer should exit within timeout")
        .unwrap();

    assert!(
        result.is_err(),
        "Writer must disconnect slow client (memory budget or write timeout)"
    );

    let _ = sender.await;
}

// ---------------------------------------------------------------------------
// Rate limit: 600 packets in 50ms → terminated (500 packets/tick limit)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn compliance_rate_limit_600_packets_in_50ms() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let client_handle = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });
    let (server_stream, peer_addr) = listener.accept().await.unwrap();
    let client_stream = client_handle.await.unwrap();

    let server = Connection::new(server_stream, peer_addr).unwrap();
    let mut client_conn = Connection::new(client_stream, "127.0.0.1:0".parse().unwrap()).unwrap();

    let (reader, _writer) = server.into_split();
    // Large inbound channel so backpressure doesn't prevent rate limit detection
    let (inbound_tx, _inbound_rx) = mpsc::channel(1024);

    let reader_handle = tokio::spawn(reader_loop(reader, inbound_tx));

    // Client sends 600 packets as fast as possible (> 500 limit)
    for i in 0..600u32 {
        if client_conn
            .send_raw(0x01_i32, &i.to_le_bytes())
            .await
            .is_err()
        {
            break;
        }
    }
    let _ = client_conn.flush().await;

    let result = tokio::time::timeout(Duration::from_secs(5), reader_handle)
        .await
        .expect("reader should exit within timeout")
        .unwrap();

    assert!(
        result.is_err(),
        "Reader must disconnect client exceeding rate limit"
    );
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("rate limited"),
        "Expected rate limit error, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// Clean shutdown: dropping senders causes both tasks to exit
// ---------------------------------------------------------------------------

#[tokio::test]
async fn compliance_clean_shutdown_both_tasks() {
    let TaskPairFixture {
        outbound_tx,
        inbound_rx,
        client,
        conn_handle,
        reader_handle,
        writer_handle,
    } = setup_task_pair().await;

    // Drop ALL outbound senders → writer exits cleanly
    drop(conn_handle);
    drop(outbound_tx);

    let writer_result = tokio::time::timeout(Duration::from_secs(5), writer_handle)
        .await
        .expect("writer should exit within timeout")
        .unwrap();
    assert!(
        writer_result.is_ok(),
        "Writer should exit cleanly on channel close"
    );

    // Drop client → reader gets EOF and exits
    drop(client);
    drop(inbound_rx);

    let reader_result = tokio::time::timeout(Duration::from_secs(5), reader_handle)
        .await
        .expect("reader should exit within timeout")
        .unwrap();

    // Reader may return Ok (channel close) or Err (EOF) — both acceptable
    let _ = reader_result;
}

// ---------------------------------------------------------------------------
// Encryption roundtrip through task pair (encryption pipeline + network I/O)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn compliance_encryption_roundtrip_task_pair() {
    let TaskPairFixture {
        outbound_tx,
        mut inbound_rx,
        mut client,
        conn_handle,
        reader_handle,
        writer_handle,
    } = setup_encrypted_task_pair().await;
    drop(conn_handle);

    // Server → Client (through writer task, encrypted)
    outbound_tx
        .send(OutboundPacket {
            id: 0x20,
            data: Bytes::from_static(b"server-to-client-encrypted"),
        })
        .await
        .unwrap();

    let pkt = client.read_raw_packet().await.unwrap();
    assert_eq!(pkt.id, 0x20);
    assert_eq!(&pkt.data[..], b"server-to-client-encrypted");

    // Client → Server (through reader task, encrypted)
    client
        .send_raw(0x0E, b"client-to-server-encrypted")
        .await
        .unwrap();
    client.flush().await.unwrap();

    let inbound = inbound_rx.recv().await.unwrap();
    assert_eq!(inbound.id, 0x0E);
    assert_eq!(&inbound.data[..], b"client-to-server-encrypted");

    // Clean shutdown
    drop(outbound_tx);
    writer_handle.await.unwrap().unwrap();
    drop(client);
    drop(inbound_rx);
    let _ = reader_handle.await.unwrap();
}

// ---------------------------------------------------------------------------
// Full lifecycle: connect → packets → disconnect → tasks exit
// ---------------------------------------------------------------------------

#[tokio::test]
async fn compliance_full_connection_lifecycle() {
    let TaskPairFixture {
        outbound_tx,
        mut inbound_rx,
        mut client,
        conn_handle,
        reader_handle,
        writer_handle,
    } = setup_task_pair().await;

    // 1. Server sends welcome packet
    conn_handle
        .send_raw(0x01, Bytes::from_static(b"welcome"))
        .await
        .unwrap();

    let pkt = client.read_raw_packet().await.unwrap();
    assert_eq!(pkt.id, 0x01);
    assert_eq!(&pkt.data[..], b"welcome");

    // 2. Client sends multiple packets
    for i in 0..5 {
        client.send_raw(0x10 + i, &[i as u8; 20]).await.unwrap();
    }
    client.flush().await.unwrap();

    for i in 0..5 {
        let inbound = inbound_rx.recv().await.unwrap();
        assert_eq!(inbound.id, 0x10 + i);
        assert_eq!(&inbound.data[..], &[i as u8; 20]);
    }

    // 3. Server sends multiple packets (simulating keepalive + chunk data)
    for i in 0..10 {
        conn_handle
            .send_raw(0x20 + i, Bytes::from(vec![0xCC; 50]))
            .await
            .unwrap();
    }

    for _ in 0..10 {
        let pkt = client.read_raw_packet().await.unwrap();
        assert_eq!(pkt.data.len(), 50);
    }

    // 4. Disconnect: drop all senders and client
    drop(outbound_tx);
    drop(conn_handle);

    let writer_result = tokio::time::timeout(Duration::from_secs(5), writer_handle)
        .await
        .expect("writer should exit")
        .unwrap();
    assert!(writer_result.is_ok(), "Writer should exit cleanly");

    drop(client);
    drop(inbound_rx);

    let reader_result = tokio::time::timeout(Duration::from_secs(5), reader_handle)
        .await
        .expect("reader should exit")
        .unwrap();
    let _ = reader_result;
}

// ---------------------------------------------------------------------------
// Batch flush: 50 packets result in correct delivery (writer batching)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn compliance_batch_flush_50_packets() {
    let TaskPairFixture {
        outbound_tx,
        inbound_rx,
        client: mut client_conn,
        conn_handle,
        reader_handle,
        writer_handle,
    } = setup_task_pair().await;
    drop(inbound_rx);
    drop(conn_handle);

    // 50 packets × 64 bytes ≈ 3.2 KB — fits in TCP buffer without deadlock
    for i in 0..50 {
        outbound_tx
            .send(OutboundPacket {
                id: i,
                data: Bytes::from(vec![i as u8; 64]),
            })
            .await
            .unwrap();
    }

    for i in 0..50 {
        let pkt = client_conn.read_raw_packet().await.unwrap();
        assert_eq!(pkt.id, i, "Batch packet {i} order mismatch");
        assert_eq!(&pkt.data[..], &[i as u8; 64]);
    }

    drop(outbound_tx);
    writer_handle.await.unwrap().unwrap();
    drop(client_conn);
    let _ = reader_handle.await.unwrap();
}
