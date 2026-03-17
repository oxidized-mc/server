# ADR-006: Network I/O Architecture

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P02, P03, P04, P13, P38 |
| Deciders | Oxidized Core Team |

## Context

The vanilla Minecraft server uses Netty for all network I/O. Each player connection is represented by a Netty `Channel` with a pipeline of handlers: `VarIntFrameDecoder` → `PacketDecoder` → `PacketHandler` → `PacketEncoder` → `VarIntFrameEncoder`. Netty's event loop threads handle I/O multiplexing, and the `Channel` pipeline processes inbound/outbound data through a chain of transformations. This model works well but is tightly coupled to Netty's abstractions and Java's NIO selector model.

In Oxidized, we need an async I/O architecture that handles thousands of concurrent player connections efficiently, integrates with our Tokio runtime (ADR-001), supports the Minecraft protocol's specific requirements (frame decoding, encryption, compression), and provides natural backpressure when clients or the server fall behind.

A critical design question is how to structure the read and write paths for each connection. Minecraft's protocol is asymmetric — the client sends packets sporadically (player actions, chat messages, keep-alive responses) while the server sends packets in bursts every tick (entity updates, chunk data, block changes). The write path must support batching multiple packets per tick and flushing them efficiently.

## Decision Drivers

- **Scalability**: must handle 1000+ concurrent connections on a modern server without excessive memory or CPU overhead
- **Natural backpressure**: if a client's network is slow, the server should detect this and eventually disconnect rather than buffering unboundedly
- **Per-tick flush batching**: all packets queued during a tick should be written in a single syscall batch, not one-by-one
- **Clean cancellation**: when a player disconnects, all associated tasks must clean up without leaked resources
- **Memory budget per connection**: each connection should have a bounded memory footprint — no unbounded buffers
- **Rate limiting**: malicious clients sending excessive packets must be detected and throttled or disconnected

## Considered Options

### Option 1: Per-connection tokio::spawn with split read/write halves

For each accepted TCP connection, split the `TcpStream` into `OwnedReadHalf` and `OwnedWriteHalf`. Spawn two tasks per connection: a reader task that decodes frames and dispatches packets to game logic via channels, and a writer task that receives packets from a channel and encodes/flushes them. The tasks communicate with the game loop through `mpsc` channels. Dropping the channel senders cleanly shuts down the writer; dropping the TCP stream cancels the reader.

### Option 2: Actor model with message passing

Model each connection as an "actor" — a struct with an `async fn run(self)` loop that processes messages from an `mpsc` receiver. The actor owns the TCP stream and handles both reading and writing in a single task using `tokio::select!`. This simplifies the model (one task per connection) but means reading and writing can't proceed concurrently — a slow write blocks packet reading and vice versa.

### Option 3: io_uring via tokio-uring

Use Linux's `io_uring` for truly asynchronous kernel-level I/O via `tokio-uring`. This offers the highest possible throughput by avoiding syscall overhead and enabling zero-copy I/O. However, `tokio-uring` requires a per-thread runtime (incompatible with Tokio's default work-stealing scheduler), only works on Linux 5.1+, and the API is less mature. The performance gains are unlikely to matter for Minecraft's relatively modest I/O requirements compared to, say, a database.

### Option 4: Manual epoll with thread-per-core

Bypass Tokio entirely and use raw `epoll` with a thread-per-core architecture (like Seastar/Glommio). This offers maximum control and minimal overhead but requires reimplementing everything Tokio provides — timers, channels, task scheduling, cancellation. The engineering cost is enormous for marginal benefit in a game server context.

## Decision

**We adopt the per-connection task pair model (Option 1).** Each accepted TCP connection spawns two Tokio tasks — a reader and a writer — that communicate with each other and the game loop through bounded `mpsc` channels.

### Architecture

```
                         ┌──────────────────────────┐
                         │       Game Loop           │
                         │  (processes player input, │
                         │   generates world state)  │
                         └─────┬──────────────┬──────┘
                               │              │
                    inbound packets    outbound packets
                         (mpsc)           (mpsc)
                               │              │
┌─────────────────────┐        │              │       ┌─────────────────────┐
│   Reader Task       │        │              │       │   Writer Task       │
│                     │────────┘              └───────│                     │
│ TcpStream (read)    │                               │ TcpStream (write)   │
│ → decrypt           │                               │ ← encrypt           │
│ → frame decode      │                               │ ← frame encode      │
│ → decompress        │                               │ ← compress          │
│ → packet decode     │                               │ ← packet encode     │
│ → dispatch          │                               │ ← batch & flush     │
└─────────────────────┘                               └─────────────────────┘
```

### Reader Task

```rust
async fn reader_loop(
    mut read_half: OwnedReadHalf,
    inbound_tx: mpsc::Sender<InboundPacket>,
    mut cipher: Option<DecryptState>,
    mut decompressor: Option<ZlibDecoder>,
) -> Result<()> {
    let mut buf = BytesMut::with_capacity(4096);
    loop {
        // Read from socket into buffer
        let n = read_half.read_buf(&mut buf).await?;
        if n == 0 { return Ok(()); } // EOF — client disconnected

        // Decrypt in-place if encryption is active
        if let Some(ref mut cipher) = cipher {
            cipher.decrypt(&mut buf[buf.len() - n..]);
        }

        // Decode frames from buffer (may yield 0, 1, or many frames)
        while let Some(frame) = decode_frame(&mut buf)? {
            let packet = decompress_and_decode(frame, &mut decompressor)?;
            inbound_tx.send(packet).await
                .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "game loop dropped"))?;
        }
    }
}
```

### Writer Task

```rust
async fn writer_loop(
    mut write_half: OwnedWriteHalf,
    mut outbound_rx: mpsc::Receiver<OutboundPacket>,
    mut cipher: Option<EncryptState>,
    mut compressor: Option<ZlibEncoder>,
    compression_threshold: Option<usize>,
) -> Result<()> {
    let mut batch_buf = BytesMut::with_capacity(65536);
    loop {
        // Wait for at least one packet
        let packet = outbound_rx.recv().await
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "sender dropped"))?;

        // Encode first packet
        encode_packet(&packet, &mut batch_buf, &mut compressor, compression_threshold)?;

        // Drain any additional queued packets (non-blocking)
        while let Ok(packet) = outbound_rx.try_recv() {
            encode_packet(&packet, &mut batch_buf, &mut compressor, compression_threshold)?;
        }

        // Encrypt the entire batch in-place
        if let Some(ref mut cipher) = cipher {
            cipher.encrypt(&mut batch_buf);
        }

        // Single flush for the entire batch
        write_half.write_all(&batch_buf).await?;
        batch_buf.clear();
    }
}
```

### Backpressure and Bounds

- **Inbound channel**: bounded to 128 packets. If the game loop can't keep up, the reader task blocks on `send().await`, which in turn stops reading from the socket, which triggers TCP flow control. The client's send buffer fills up, and the client sees increased latency — a natural backpressure signal.
- **Outbound channel**: bounded to 512 packets (larger because the server sends more per tick). If the client's network is too slow to drain the write buffer, the channel fills, and the game loop detects this — it can log a warning and eventually disconnect the player.
- **Per-connection memory budget**: each connection allocates at most 256 KB for read/write buffers. Connections exceeding this budget are forcibly disconnected.

### TCP Configuration

```rust
let socket = TcpStream::connect(addr).await?;
socket.set_nodelay(true)?; // Disable Nagle's algorithm — we do our own batching
```

`TCP_NODELAY` is critical. Nagle's algorithm buffers small writes to coalesce them into larger segments, but our writer task already batches packets per tick. Nagle would add up to 40ms of latency on top of our flush, which is unacceptable for a 50ms tick interval.

### Rate Limiting

The reader task tracks packets received per tick interval. If a client exceeds 500 packets in a single tick window (50ms), the connection is flagged and the player is disconnected with a "Too many packets" message. This protects against packet-flood attacks.

```rust
const MAX_PACKETS_PER_TICK: u32 = 500;

// In reader loop
packet_count += 1;
if packet_count > MAX_PACKETS_PER_TICK {
    return Err(ProtocolError::RateLimited.into());
}
// Reset count each tick (signaled via a separate channel or timer)
```

## Consequences

### Positive

- Two tasks per connection enables concurrent reading and writing — a slow write doesn't block packet reception
- Bounded channels provide natural backpressure without explicit flow control logic
- Batch flushing reduces syscall overhead — one `write_all` per tick instead of one per packet
- `TCP_NODELAY` combined with application-level batching gives optimal latency
- Clean cancellation: dropping the channel sender causes the writer task to exit; dropping the `TcpStream` read half causes the reader to see EOF
- Rate limiting is a simple counter check with no additional data structures

### Negative

- Two tasks per connection means 2N Tokio tasks for N players — at 1000 players, that's 2000 tasks (acceptable for Tokio's scheduler but worth monitoring)
- Channel-based communication adds a small overhead per packet (allocation, pointer indirection) compared to direct function calls
- Encryption and compression state must be passed to tasks at creation time and updated via control messages for key changes (e.g., encryption enable after login)

### Neutral

- The two-task model naturally maps to Netty's concept of inbound and outbound pipeline directions, making it easy to reason about for developers familiar with vanilla's architecture
- Future migration to `io_uring` (if needed) would only require replacing the read/write implementations inside the tasks — the channel-based interface to the game loop remains unchanged

## Compliance

- **Benchmark**: connection throughput tests must demonstrate >5000 packets/second per connection without memory growth
- **Memory audit**: integration tests verify that per-connection memory stays below 256 KB under normal load
- **Backpressure test**: a test simulates a slow client (delayed reads) and verifies the server disconnects after the outbound channel fills, rather than growing memory unboundedly
- **Rate limit test**: a test sends 600 packets in 50ms and verifies the connection is terminated
- **TCP_NODELAY check**: integration tests verify the socket option is set on all accepted connections

## Related ADRs

- [ADR-001: Async Runtime Selection](adr-001-async-runtime.md) — tasks are spawned on Tokio's work-stealing runtime
- [ADR-007: Packet Codec Framework](adr-007-packet-codec.md) — packet encode/decode functions called within reader/writer tasks
- [ADR-008: Connection State Machine](adr-008-connection-state-machine.md) — the reader task's dispatch logic changes based on connection state
- [ADR-009: Encryption & Compression Pipeline](adr-009-encryption-compression.md) — cipher and compressor state owned by reader/writer tasks

## References

- [Tokio tutorial — I/O](https://tokio.rs/tokio/tutorial/io)
- [Tokio split — OwnedReadHalf / OwnedWriteHalf](https://docs.rs/tokio/latest/tokio/net/struct.TcpStream.html#method.into_split)
- [TCP_NODELAY explained](https://brooker.co.za/blog/2024/05/09/nagle.html)
- [Netty Channel pipeline](https://netty.io/4.1/api/io/netty/channel/ChannelPipeline.html)
- [Alice Ryhl — "Actors with Tokio"](https://ryhl.io/blog/actors-with-tokio/)
- [Backpressure in async Rust](https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html#backpressure)
