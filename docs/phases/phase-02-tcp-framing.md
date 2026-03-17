# Phase 2 — TCP Listener + Raw Framing

**Crate:** `oxidized-protocol`  
**Reward:** Server accepts TCP connections; raw packet bytes are logged in debug mode.

---

## Goal

Establish the TCP listener, per-connection task architecture, and VarInt-framed
packet I/O. No packet decoding yet — just prove bytes flow.

---

## Java Reference

| Concept | Java class |
|---------|-----------|
| Server network | `net.minecraft.server.network.ServerConnectionListener` |
| Connection | `net.minecraft.network.Connection` |
| Frame decoder | `net.minecraft.network.Varint21FrameDecoder` |
| Frame encoder | `net.minecraft.network.Varint21LengthFieldPrepender` |
| VarInt | `net.minecraft.network.VarInt` |
| VarLong | `net.minecraft.network.VarLong` |

---

## VarInt Encoding

```
Value fits in 7 bits:  [0VVVVVVV]
Value needs more:      [1VVVVVVV] [next byte ...]

Max 5 bytes for VarInt (32-bit), 10 bytes for VarLong (64-bit).

Encode:
  loop:
    byte = value & 0x7F
    value >>= 7
    if value != 0: byte |= 0x80
    write byte
    if value == 0: break

Decode:
  result = 0, shift = 0
  loop:
    byte = read_u8()
    result |= (byte & 0x7F) << shift
    if byte & 0x80 == 0: return result
    shift += 7
    if shift >= 32: error "VarInt too big"
```

Java reference: `net.minecraft.network.VarInt.write()` / `.read()`

---

## Tasks

### 2.1 — VarInt (`oxidized-protocol/src/codec/varint.rs`)

```rust
/// Encode a VarInt into a buffer. Returns number of bytes written (1–5).
pub fn encode_varint(value: i32, buf: &mut [u8; 5]) -> usize;

/// Decode a VarInt from bytes. Returns (value, bytes_consumed).
pub fn decode_varint(buf: &[u8]) -> Result<(i32, usize), VarIntError>;

/// Encode a VarLong (1–10 bytes).
pub fn encode_varlong(value: i64, buf: &mut [u8; 10]) -> usize;

/// Decode a VarLong.
pub fn decode_varlong(buf: &[u8]) -> Result<(i64, usize), VarIntError>;

/// Async read a VarInt from a tokio AsyncRead.
pub async fn read_varint(reader: &mut (impl AsyncRead + Unpin)) -> Result<i32, IoError>;

/// Async write a VarInt to a tokio AsyncWrite.
pub async fn write_varint(writer: &mut (impl AsyncWrite + Unpin), value: i32) -> Result<(), IoError>;
```

### 2.2 — Frame codec (`oxidized-protocol/src/codec/frame.rs`)

```rust
/// Read one packet frame: read length VarInt, then read exactly that many bytes.
/// Returns raw packet bytes (not including the length prefix).
pub async fn read_frame(
    reader: &mut (impl AsyncRead + Unpin),
    max_packet_size: usize,
) -> Result<Bytes, FrameError>;

/// Write one packet frame: write length VarInt, then the bytes.
pub async fn write_frame(
    writer: &mut (impl AsyncWrite + Unpin),
    data: &[u8],
) -> Result<(), IoError>;
```

### 2.3 — Connection struct (`oxidized-protocol/src/connection.rs`)

```rust
pub struct Connection {
    // Network
    read: OwnedReadHalf,
    write: OwnedWriteHalf,
    addr: SocketAddr,

    // Outbound queue
    send_tx: mpsc::Sender<Bytes>,

    // State
    pub state: ConnectionState,
    pub protocol_version: i32,
    pub compression_threshold: Option<i32>,  // None = disabled
    pub encrypted: bool,
}

pub enum ConnectionState {
    Handshaking,
    Status,
    Login,
    Configuration,
    Play,
}

impl Connection {
    pub async fn new(stream: TcpStream, addr: SocketAddr) -> Self;
    pub async fn read_raw_packet(&mut self) -> Result<RawPacket, ConnectionError>;
    pub async fn send_raw(&self, data: Bytes) -> Result<(), ConnectionError>;
    pub fn remote_addr(&self) -> SocketAddr;
}

pub struct RawPacket {
    pub id: i32,
    pub data: Bytes,   // packet body without ID
}
```

### 2.4 — TCP Listener (`oxidized-server/src/network.rs`)

```rust
pub async fn listen(
    addr: SocketAddr,
    shutdown: broadcast::Receiver<()>,
) -> Result<(), IoError> {
    let listener = TcpListener::bind(addr).await?;
    info!("Listening on {}", addr);
    loop {
        tokio::select! {
            Ok((stream, addr)) = listener.accept() => {
                info!("New connection from {}", addr);
                tokio::spawn(handle_connection(stream, addr));
            }
            _ = shutdown.recv() => break,
        }
    }
    Ok(())
}

async fn handle_connection(stream: TcpStream, addr: SocketAddr) {
    let mut conn = Connection::new(stream, addr).await;
    loop {
        match conn.read_raw_packet().await {
            Ok(pkt) => {
                debug!("[{}] Packet 0x{:02X} ({} bytes)", addr, pkt.id, pkt.data.len());
                // TODO: dispatch (Phase 3)
            }
            Err(e) => { debug!("Connection {} closed: {}", addr, e); break; }
        }
    }
}
```

### 2.5 — Graceful shutdown

```rust
// In main.rs
let (shutdown_tx, _) = broadcast::channel::<()>(1);

// Spawn Ctrl+C handler
let tx = shutdown_tx.clone();
tokio::spawn(async move {
    tokio::signal::ctrl_c().await.ok();
    info!("Shutdown signal received");
    let _ = tx.send(());
});

// Start listener
listen(addr, shutdown_tx.subscribe()).await?;
```

---

## Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum VarIntError {
    #[error("VarInt is too large (> 5 bytes)")]
    TooLarge,
    #[error("Unexpected end of stream")]
    UnexpectedEof,
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("Packet too large: {size} > {max}")]
    PacketTooLarge { size: usize, max: usize },
    #[error("VarInt error: {0}")]
    VarInt(#[from] VarIntError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
```

---

## Tests

```rust
#[test]
fn test_varint_encode_small()    { assert_eq!(encode_varint_bytes(0), [0x00]); }
#[test]
fn test_varint_encode_127()      { assert_eq!(encode_varint_bytes(127), [0x7F]); }
#[test]
fn test_varint_encode_128()      { assert_eq!(encode_varint_bytes(128), [0x80, 0x01]); }
#[test]
fn test_varint_encode_300()      { assert_eq!(encode_varint_bytes(300), [0xAC, 0x02]); }
#[test]
fn test_varint_encode_negative() { assert_eq!(encode_varint_bytes(-1), [0xFF,0xFF,0xFF,0xFF,0x0F]); }
#[test]
fn test_varint_roundtrip()       { /* encode then decode all edge cases */ }
#[tokio::test]
async fn test_frame_roundtrip()  { /* write frame, read it back with cursor */ }
#[tokio::test]
async fn test_frame_too_large()  { /* packet > max_size returns error */ }
```

---

## Files Created / Modified

```
crates/oxidized-protocol/src/
├── lib.rs          ← add mod declarations
├── codec/
│   ├── mod.rs
│   ├── varint.rs
│   └── frame.rs
└── connection.rs

crates/oxidized-server/src/
├── main.rs         ← wire up listener
└── network.rs
```
