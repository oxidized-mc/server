//! Per-client connection for the Minecraft protocol.
//!
//! Each accepted TCP connection is represented by a [`Connection`] that
//! tracks the remote address, protocol state, and provides methods to
//! read/write raw packet frames.

use std::fmt;
use std::io;
use std::net::SocketAddr;

use bytes::{Bytes, BytesMut};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

use crate::codec::frame::{self, FrameError, MAX_PACKET_SIZE};
use crate::codec::varint::{self, VarIntError};

// ---------------------------------------------------------------------------
// ConnectionState
// ---------------------------------------------------------------------------

/// Protocol state of a Minecraft connection.
///
/// Connections start in [`Handshaking`](ConnectionState::Handshaking) and
/// transition based on the client's intention packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Initial state — waiting for the handshake packet.
    Handshaking,
    /// Server list ping / status query.
    Status,
    /// Authentication / login flow.
    Login,
    /// Configuration state (1.20.2+).
    Configuration,
    /// Main gameplay state.
    Play,
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handshaking => write!(f, "Handshaking"),
            Self::Status => write!(f, "Status"),
            Self::Login => write!(f, "Login"),
            Self::Configuration => write!(f, "Configuration"),
            Self::Play => write!(f, "Play"),
        }
    }
}

// ---------------------------------------------------------------------------
// RawPacket
// ---------------------------------------------------------------------------

/// A raw (undecoded) packet: just the numeric ID and the body bytes.
#[derive(Debug, Clone)]
pub struct RawPacket {
    /// Packet ID (VarInt on the wire, decoded to i32).
    pub id: i32,
    /// Packet body bytes (everything after the packet ID).
    pub data: Bytes,
}

// ---------------------------------------------------------------------------
// ConnectionError
// ---------------------------------------------------------------------------

/// Errors that can occur on a [`Connection`].
#[derive(Debug, Error)]
pub enum ConnectionError {
    /// A frame-level error (bad length prefix, oversized packet, etc.).
    #[error("frame error: {0}")]
    Frame(#[from] FrameError),

    /// A VarInt decoding error in the packet ID.
    #[error("packet ID decode error: {0}")]
    VarInt(#[from] VarIntError),

    /// An I/O error on the underlying TCP stream.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

// ---------------------------------------------------------------------------
// Connection
// ---------------------------------------------------------------------------

/// A single client connection.
///
/// Owns the split TCP stream halves and tracks protocol state.
pub struct Connection {
    reader: OwnedReadHalf,
    writer: OwnedWriteHalf,
    addr: SocketAddr,

    /// Current protocol state.
    pub state: ConnectionState,
    /// Protocol version reported by the client in the handshake.
    pub protocol_version: i32,
}

impl Connection {
    /// Creates a new connection from an accepted [`TcpStream`].
    ///
    /// Sets `TCP_NODELAY` for low-latency writes (per ADR-006) and splits
    /// the stream into independent read/write halves.
    pub fn new(stream: TcpStream, addr: SocketAddr) -> io::Result<Self> {
        stream.set_nodelay(true)?;
        let (reader, writer) = stream.into_split();
        Ok(Self {
            reader,
            writer,
            addr,
            state: ConnectionState::Handshaking,
            protocol_version: 0,
        })
    }

    /// Returns the remote socket address.
    pub fn remote_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Reads one raw packet from the connection.
    ///
    /// Reads a VarInt-framed payload, extracts the packet ID (VarInt),
    /// and returns the remaining bytes as the packet body.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectionError`] on I/O failure, malformed framing,
    /// or oversized packets.
    pub async fn read_raw_packet(&mut self) -> Result<RawPacket, ConnectionError> {
        let frame = frame::read_frame(&mut self.reader, MAX_PACKET_SIZE).await?;
        let mut buf = frame;
        let id = varint::read_varint_buf(&mut buf)?;
        Ok(RawPacket { id, data: buf })
    }

    /// Sends a raw packet (ID + body) as a single frame.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectionError`] on I/O failure.
    pub async fn send_raw(&mut self, id: i32, data: &[u8]) -> Result<(), ConnectionError> {
        let mut payload = BytesMut::new();
        varint::write_varint_buf(id, &mut payload);
        payload.extend_from_slice(data);
        frame::write_frame(&mut self.writer, &payload).await?;
        Ok(())
    }

    /// Flushes the write buffer, ensuring all data reaches the OS send buffer.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectionError`] on I/O failure.
    pub async fn flush(&mut self) -> Result<(), ConnectionError> {
        self.writer.flush().await?;
        Ok(())
    }

    /// Shuts down the write half of the connection.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectionError`] on I/O failure.
    pub async fn shutdown(&mut self) -> Result<(), ConnectionError> {
        self.writer.shutdown().await?;
        Ok(())
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection")
            .field("addr", &self.addr)
            .field("state", &self.state)
            .field("protocol_version", &self.protocol_version)
            .finish()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    /// Helper: creates a connected pair using a loopback listener.
    async fn loopback_pair() -> (Connection, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let client_handle = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });
        let (server_stream, peer_addr) = listener.accept().await.unwrap();
        let client_stream = client_handle.await.unwrap();

        let conn = Connection::new(server_stream, peer_addr).unwrap();
        (conn, client_stream)
    }

    #[tokio::test]
    async fn test_connection_initial_state() {
        let (conn, _client) = loopback_pair().await;
        assert_eq!(conn.state, ConnectionState::Handshaking);
        assert_eq!(conn.protocol_version, 0);
    }

    #[tokio::test]
    async fn test_raw_packet_roundtrip() {
        let (mut server, mut client) = loopback_pair().await;

        // Client sends a framed packet: VarInt(len) + VarInt(packet_id) + body
        let packet_id: i32 = 0x00;
        let body = b"hello";

        // Build the inner payload (packet_id + body)
        let mut inner = BytesMut::new();
        varint::write_varint_buf(packet_id, &mut inner);
        inner.extend_from_slice(body);

        // Write as a frame from the client side
        use tokio::io::AsyncWriteExt;
        frame::write_frame(&mut client, &inner).await.unwrap();
        client.flush().await.unwrap();

        // Server reads the raw packet
        let pkt = server.read_raw_packet().await.unwrap();
        assert_eq!(pkt.id, 0x00);
        assert_eq!(&pkt.data[..], body);
    }

    #[tokio::test]
    async fn test_send_raw_and_read_back() {
        let (mut server, client) = loopback_pair().await;

        // Server sends a packet
        server.send_raw(0x01, b"pong").await.unwrap();
        server.flush().await.unwrap();

        // Read it back from the client side using frame codec
        let mut client_read = tokio::io::BufReader::new(client);
        let frame = frame::read_frame(&mut client_read, MAX_PACKET_SIZE)
            .await
            .unwrap();
        let mut buf = frame;
        let id = varint::read_varint_buf(&mut buf).unwrap();
        assert_eq!(id, 0x01);
        assert_eq!(&buf[..], b"pong");
    }

    #[tokio::test]
    async fn test_connection_state_display() {
        assert_eq!(ConnectionState::Handshaking.to_string(), "Handshaking");
        assert_eq!(ConnectionState::Status.to_string(), "Status");
        assert_eq!(ConnectionState::Login.to_string(), "Login");
        assert_eq!(ConnectionState::Configuration.to_string(), "Configuration");
        assert_eq!(ConnectionState::Play.to_string(), "Play");
    }
}
