//! Per-client connection for the Minecraft protocol.
//!
//! Each accepted TCP connection is represented by a [`Connection`] that
//! tracks the remote address, protocol state, and provides methods to
//! read/write raw packet frames.
//!
//! Supports optional encryption (AES-128-CFB8) and compression (zlib)
//! which are enabled during the login handshake.

use std::fmt;
use std::io;
use std::net::SocketAddr;

use bytes::{Bytes, BytesMut};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

use crate::codec::frame::{self, FrameError, MAX_PACKET_SIZE};
use crate::codec::varint::{self, VarIntError};
use crate::compression::{CompressionError, CompressionState};
use crate::crypto::CipherState;

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

    /// A compression/decompression error.
    #[error("compression error: {0}")]
    Compression(#[from] CompressionError),

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
/// Supports optional AES-128-CFB8 encryption and zlib compression
/// which are enabled during the login handshake.
pub struct Connection {
    reader: OwnedReadHalf,
    writer: OwnedWriteHalf,
    addr: SocketAddr,

    /// Current protocol state.
    pub state: ConnectionState,
    /// Protocol version reported by the client in the handshake.
    pub protocol_version: i32,

    /// AES-128-CFB8 cipher state (enabled after key exchange).
    cipher: Option<CipherState>,
    /// Zlib compression state (enabled after login compression packet).
    compression: Option<CompressionState>,
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
            cipher: None,
            compression: None,
        })
    }

    /// Returns the remote socket address.
    pub fn remote_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Enables AES-128-CFB8 encryption on this connection.
    ///
    /// After calling this, all subsequent reads and writes will be
    /// encrypted/decrypted using the shared secret.
    pub fn enable_encryption(&mut self, shared_secret: &[u8; 16]) {
        self.cipher = Some(CipherState::new(shared_secret));
    }

    /// Returns whether encryption is enabled.
    pub fn is_encrypted(&self) -> bool {
        self.cipher.is_some()
    }

    /// Enables zlib compression on this connection with the given threshold.
    ///
    /// After calling this, packets at or above `threshold` bytes will be
    /// zlib-compressed. The frame format changes to include a `data_length`
    /// VarInt prefix.
    pub fn enable_compression(&mut self, threshold: usize) {
        self.compression = Some(CompressionState::new(threshold));
    }

    /// Returns whether compression is enabled.
    pub fn is_compressed(&self) -> bool {
        self.compression.is_some()
    }

    // -----------------------------------------------------------------------
    // Low-level encrypted I/O
    // -----------------------------------------------------------------------

    /// Reads exactly `n` bytes from the TCP stream, decrypting if needed.
    async fn read_bytes(&mut self, n: usize) -> Result<BytesMut, io::Error> {
        let mut buf = BytesMut::zeroed(n);
        self.reader.read_exact(&mut buf).await?;
        if let Some(ref mut cipher) = self.cipher {
            cipher.decrypt(&mut buf);
        }
        Ok(buf)
    }

    /// Reads a single byte from the TCP stream, decrypting if needed.
    async fn read_byte(&mut self) -> Result<u8, io::Error> {
        let mut byte = [0u8; 1];
        self.reader.read_exact(&mut byte).await?;
        if let Some(ref mut cipher) = self.cipher {
            cipher.decrypt(&mut byte);
        }
        Ok(byte[0])
    }

    /// Writes raw bytes to the TCP stream, encrypting if needed.
    async fn write_bytes(&mut self, data: &mut [u8]) -> Result<(), io::Error> {
        if let Some(ref mut cipher) = self.cipher {
            cipher.encrypt(data);
        }
        self.writer.write_all(data).await
    }

    // -----------------------------------------------------------------------
    // Frame reading (with encryption + compression)
    // -----------------------------------------------------------------------

    /// Reads a VarInt from the (possibly encrypted) stream.
    async fn read_varint(&mut self) -> Result<i32, ConnectionError> {
        let mut result: i32 = 0;
        for i in 0..varint::VARINT_MAX_BYTES {
            let byte = self.read_byte().await?;
            result |= ((byte & 0x7F) as i32) << (7 * i);
            if byte & 0x80 == 0 {
                return Ok(result);
            }
        }
        Err(ConnectionError::VarInt(VarIntError::TooLarge {
            max_bytes: varint::VARINT_MAX_BYTES,
        }))
    }

    /// Reads one raw packet from the connection.
    ///
    /// Handles the full pipeline: decrypt → read frame → decompress →
    /// extract packet ID.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectionError`] on I/O failure, malformed framing,
    /// oversized packets, or decompression errors.
    pub async fn read_raw_packet(&mut self) -> Result<RawPacket, ConnectionError> {
        // Step 1: Read frame (encrypted bytes are decrypted transparently)
        let frame_payload = if self.cipher.is_some() {
            // Encrypted path: read VarInt + payload through decrypt layer
            let length = self.read_varint().await?;
            let length = length as usize;
            if length == 0 {
                return Err(ConnectionError::Frame(FrameError::ZeroLength));
            }
            if length > MAX_PACKET_SIZE {
                return Err(ConnectionError::Frame(FrameError::PacketTooLarge {
                    size: length,
                    max: MAX_PACKET_SIZE,
                }));
            }
            let buf = self.read_bytes(length).await?;
            buf.freeze()
        } else {
            // Unencrypted path: use existing frame reader
            frame::read_frame(&mut self.reader, MAX_PACKET_SIZE).await?
        };

        // Step 2: Handle compression (if enabled)
        let packet_data = if let Some(ref mut compression) = self.compression {
            let mut buf = frame_payload;
            let data_length = varint::read_varint_buf(&mut buf)?;
            let decompressed = compression.decompress(data_length, &buf)?;
            Bytes::from(decompressed)
        } else {
            frame_payload
        };

        // Step 3: Parse packet ID
        let mut buf = packet_data;
        let id = varint::read_varint_buf(&mut buf)?;
        Ok(RawPacket { id, data: buf })
    }

    /// Sends a raw packet (ID + body) as a single frame.
    ///
    /// Handles the full pipeline: build payload → compress → frame →
    /// encrypt → write.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectionError`] on I/O failure or compression errors.
    pub async fn send_raw(&mut self, id: i32, data: &[u8]) -> Result<(), ConnectionError> {
        // Step 1: Build inner payload (packet_id + body)
        let mut inner = BytesMut::new();
        varint::write_varint_buf(id, &mut inner);
        inner.extend_from_slice(data);

        // Step 2: Handle compression (if enabled)
        let frame_content = if let Some(ref mut compression) = self.compression {
            let (data_length, payload) = compression.compress(&inner)?;
            let mut compressed_frame = BytesMut::new();
            varint::write_varint_buf(data_length, &mut compressed_frame);
            compressed_frame.extend_from_slice(&payload);
            compressed_frame
        } else {
            inner
        };

        // Step 3: Build frame (VarInt length prefix + content)
        let mut frame = BytesMut::new();
        varint::write_varint_buf(frame_content.len() as i32, &mut frame);
        frame.extend_from_slice(&frame_content);

        // Step 4: Encrypt (if enabled) and write
        let mut frame_bytes = frame.to_vec();
        self.write_bytes(&mut frame_bytes).await?;
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
            .field("encrypted", &self.cipher.is_some())
            .field("compressed", &self.compression.is_some())
            .finish()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
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
        assert!(!conn.is_encrypted());
        assert!(!conn.is_compressed());
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

    #[tokio::test]
    async fn test_encrypted_roundtrip() {
        // Two connections: server-side and client-side both encrypt
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let client_handle = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });
        let (server_stream, peer_addr) = listener.accept().await.unwrap();
        let client_stream = client_handle.await.unwrap();

        let mut server = Connection::new(server_stream, peer_addr).unwrap();
        let mut client_conn =
            Connection::new(client_stream, "127.0.0.1:0".parse().unwrap()).unwrap();

        // Enable encryption on both sides with same shared secret
        let secret = [0x42u8; 16];
        server.enable_encryption(&secret);
        client_conn.enable_encryption(&secret);

        assert!(server.is_encrypted());
        assert!(client_conn.is_encrypted());

        // Client sends an encrypted packet
        client_conn
            .send_raw(0x05, b"encrypted payload")
            .await
            .unwrap();
        client_conn.flush().await.unwrap();

        // Server reads and decrypts
        let pkt = server.read_raw_packet().await.unwrap();
        assert_eq!(pkt.id, 0x05);
        assert_eq!(&pkt.data[..], b"encrypted payload");
    }

    #[tokio::test]
    async fn test_compressed_roundtrip() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let client_handle = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });
        let (server_stream, peer_addr) = listener.accept().await.unwrap();
        let client_stream = client_handle.await.unwrap();

        let mut server = Connection::new(server_stream, peer_addr).unwrap();
        let mut client_conn =
            Connection::new(client_stream, "127.0.0.1:0".parse().unwrap()).unwrap();

        // Enable compression (threshold=64) on both sides
        server.enable_compression(64);
        client_conn.enable_compression(64);

        // Send a large payload that will be compressed
        let payload = vec![0xAB; 256];
        client_conn.send_raw(0x07, &payload).await.unwrap();
        client_conn.flush().await.unwrap();

        let pkt = server.read_raw_packet().await.unwrap();
        assert_eq!(pkt.id, 0x07);
        assert_eq!(&pkt.data[..], &payload[..]);
    }

    #[tokio::test]
    async fn test_compressed_below_threshold() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let client_handle = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });
        let (server_stream, peer_addr) = listener.accept().await.unwrap();
        let client_stream = client_handle.await.unwrap();

        let mut server = Connection::new(server_stream, peer_addr).unwrap();
        let mut client_conn =
            Connection::new(client_stream, "127.0.0.1:0".parse().unwrap()).unwrap();

        // Enable compression (threshold=256) — small packets stay uncompressed
        server.enable_compression(256);
        client_conn.enable_compression(256);

        // Small payload stays uncompressed (data_length=0)
        client_conn.send_raw(0x01, b"tiny").await.unwrap();
        client_conn.flush().await.unwrap();

        let pkt = server.read_raw_packet().await.unwrap();
        assert_eq!(pkt.id, 0x01);
        assert_eq!(&pkt.data[..], b"tiny");
    }

    #[tokio::test]
    async fn test_encrypted_and_compressed_roundtrip() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let client_handle = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });
        let (server_stream, peer_addr) = listener.accept().await.unwrap();
        let client_stream = client_handle.await.unwrap();

        let mut server = Connection::new(server_stream, peer_addr).unwrap();
        let mut client_conn =
            Connection::new(client_stream, "127.0.0.1:0".parse().unwrap()).unwrap();

        // Enable both encryption and compression
        let secret = [0x13u8; 16];
        server.enable_encryption(&secret);
        client_conn.enable_encryption(&secret);
        server.enable_compression(64);
        client_conn.enable_compression(64);

        // Large payload: encrypted + compressed
        let payload = vec![0xCD; 512];
        client_conn.send_raw(0x0A, &payload).await.unwrap();
        client_conn.flush().await.unwrap();

        let pkt = server.read_raw_packet().await.unwrap();
        assert_eq!(pkt.id, 0x0A);
        assert_eq!(&pkt.data[..], &payload[..]);

        // Small payload: encrypted + uncompressed (below threshold)
        client_conn.send_raw(0x0B, b"small").await.unwrap();
        client_conn.flush().await.unwrap();

        let pkt2 = server.read_raw_packet().await.unwrap();
        assert_eq!(pkt2.id, 0x0B);
        assert_eq!(&pkt2.data[..], b"small");
    }
}
