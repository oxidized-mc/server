# Phase R4 — Network I/O Refactoring (ADR-006 Compliance)

**Status:** 🟡 In Progress  
**Crates:** `oxidized-protocol`, `oxidized-server`  
**Reward:** Every player connection uses a reader/writer task pair with bounded
channels, batch flushing, per-connection memory budgets, and packet rate limiting.
The server handles 1000+ concurrent connections with natural backpressure, no
unbounded buffers, and clean cancellation. ADR-006 compliance is complete.

---

## Progress Summary

| Sub-task | Description | Status |
|----------|-------------|--------|
| R4.1 | Define channel types and connection handle API | ✅ Complete |
| R4.2 | Split Connection into reader/writer halves | ✅ Complete |
| R4.3 | Implement writer task with batch flushing | ❌ Not Started |
| R4.4 | Implement reader task with rate limiting | ❌ Not Started |
| R4.5 | Migrate pre-play states (Handshake/Status/Login/Config) | ❌ Not Started |
| R4.6 | Migrate play state — inbound path | ❌ Not Started |
| R4.7 | Migrate play state — outbound path (direct sends) | ❌ Not Started |
| R4.8 | Migrate play state — broadcast path | ❌ Not Started |
| R4.9 | Per-connection memory budget and slow-client detection | ❌ Not Started |
| R4.10 | Compliance tests and benchmarks | ❌ Not Started |

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-006: Network I/O Architecture](../adr/adr-006-network-io.md) —
  task pair model, bounded channels, batch flushing, rate limiting
- [ADR-009: Encryption & Compression Pipeline](../adr/adr-009-encryption-compression.md) —
  cipher/compressor state ownership changes in the task pair model
- [ADR-020: Player Session Lifecycle](../adr/adr-020-player-session.md) —
  inbound/outbound channel design between network and game logic
- [ADR-036: Packet Handler Architecture](../adr/adr-036-packet-handler-architecture.md) —
  play handler module structure (remains unchanged)

Also relevant:

- [ADR-001: Async Runtime Selection](../adr/adr-001-async-runtime.md) —
  Tokio work-stealing scheduler (all tasks spawned on it)
- [ADR-007: Packet Codec Framework](../adr/adr-007-packet-codec.md) —
  Packet trait encode/decode used in reader/writer tasks
- [ADR-008: Connection State Machine](../adr/adr-008-connection-state-machine.md) —
  state transitions affect which packets the reader dispatches
- [ADR-019: Server Tick Loop](../adr/adr-019-tick-loop.md) —
  tick loop consumes inbound packets and produces outbound packets

---

## Current State Analysis

### What exists today (single-task model)

Each accepted TCP connection spawns **one** Tokio task running
`handle_connection()`. This task owns a `Connection` struct containing both
`OwnedReadHalf` and `OwnedWriteHalf`. The task sequentially:

1. Reads a packet (`read_raw_packet`)
2. Decrypts/decompresses it
3. Dispatches it to the appropriate handler
4. The handler calls `conn.send_packet()` which encodes + encrypts +
   compresses + flushes **immediately** (one syscall per packet)

```
Current Architecture (single task per connection):

    ┌───────────────────────────────────────────┐
    │         handle_connection task             │
    │                                           │
    │  loop {                                   │
    │    pkt = conn.read_raw_packet().await      │  ← read blocks write
    │    match pkt.id {                         │
    │      ... => handler(&mut conn).await      │  ← handler calls
    │              conn.send_packet().await      │    send_packet inline
    │    }                                      │
    │  }                                        │
    └───────────────────────────────────────────┘
```

### Problems with current model

| Problem | Impact | ADR-006 Solution |
|---------|--------|------------------|
| Read blocks write | Can't send keepalive while waiting for client packet in a handler | Separate reader/writer tasks |
| Write blocks read | Slow client network stalls packet reception | Separate tasks + bounded channels |
| Per-packet flush | Each `send_packet()` calls `flush()` = 1 syscall per packet | Writer batches all packets, single flush |
| No backpressure | If server produces packets faster than client drains, buffers grow | Bounded outbound channel (512) |
| No packet rate limiting | Malicious client can flood packets (only chat rate-limited) | Reader enforces 500 packets/tick |
| No memory budget | Connection buffers can grow unboundedly | 256 KB cap per connection |
| Broadcast overhead | Each broadcast does `send_raw` + `flush` per player per message | Outbound channel batches broadcasts naturally |

### What works well (keep unchanged)

- `Connection::new()` sets `TCP_NODELAY` ✅
- Connection-level rate limiting in accept loop (10/IP/10s) ✅
- Protocol state machine progression ✅
- Play handler module structure (ADR-036 split) ✅
- Kick channel mechanism (keep, but change delivery) ✅
- Keepalive timing logic (keep, adapt to channel model) ✅
- Broadcast channel pattern (keep `broadcast::Sender`, change delivery) ✅

---

## Scope and Non-Goals

**In scope:**

- Split `Connection` into `ConnectionReader` and `ConnectionWriter`
- Spawn reader + writer tasks per connection
- Bounded inbound/outbound `mpsc` channels
- Writer batch flushing (drain channel, encode all, single write)
- Reader packet rate limiting (500/tick)
- Per-connection memory budget (256 KB)
- Slow-client detection and forced disconnect
- Migrate all packet send paths to use outbound channel
- Compliance tests: throughput, memory, backpressure, rate limit

**Not in scope (deferred to future phases):**

- ADR-020 player session bridge (network actor ↔ ECS entity channels) —
  requires ECS migration (P15)
- Zero-copy I/O or `io_uring` — rejected in ADR-006
- Changing the Handshake/Status/Login/Config protocol flow logic
- Modifying packet encode/decode implementations
- Changing the broadcast channel architecture

---

## Detailed Sub-task Plans

### R4.1: Define Channel Types and Connection Handle API

**Targets:** `crates/oxidized-protocol/src/transport/`

**Create `transport/channel.rs`:**

Define the types that flow through the inbound and outbound channels.

```rust
/// Decoded inbound packet from a client.
pub struct InboundPacket {
    /// Packet ID.
    pub id: i32,
    /// Packet payload (decompressed, decrypted).
    pub data: Bytes,
}

/// Outbound packet to be sent to a client.
pub struct OutboundPacket {
    /// Packet ID.
    pub id: i32,
    /// Pre-encoded packet payload (before compression/encryption).
    pub data: Bytes,
}

/// Channel capacity for inbound packets (reader → game logic).
pub const INBOUND_CHANNEL_CAPACITY: usize = 128;

/// Channel capacity for outbound packets (game logic → writer).
pub const OUTBOUND_CHANNEL_CAPACITY: usize = 512;

/// Maximum packets a client may send per tick window (50ms).
pub const MAX_PACKETS_PER_TICK: u32 = 500;

/// Maximum combined buffer memory per connection.
pub const MAX_CONNECTION_MEMORY: usize = 256 * 1024; // 256 KB
```

**Create `transport/handle.rs` — the API callers use to interact with a
connection:**

```rust
/// Handle to an active connection.
///
/// Provides a typed interface for sending packets without
/// needing direct access to the TCP stream. Sending through
/// the handle queues the packet on the outbound channel;
/// the writer task flushes it.
pub struct ConnectionHandle {
    outbound_tx: mpsc::Sender<OutboundPacket>,
    addr: SocketAddr,
}

impl ConnectionHandle {
    /// Queue a packet for sending to the client.
    ///
    /// Returns `Err` if the outbound channel is full (slow client)
    /// or the writer task has exited.
    pub async fn send_packet<P: Packet>(&self, pkt: &P) -> Result<(), ConnectionError>;

    /// Non-blocking send attempt. Returns immediately if channel is full.
    pub fn try_send_packet<P: Packet>(&self, pkt: &P) -> Result<(), ConnectionError>;

    /// Queue a pre-encoded raw packet.
    pub async fn send_raw(&self, id: i32, data: Bytes) -> Result<(), ConnectionError>;

    /// Returns the remote peer address.
    pub fn remote_addr(&self) -> SocketAddr;
}
```

**Tests:**
- `InboundPacket` and `OutboundPacket` can be sent through channels
- `ConnectionHandle::try_send_packet` returns error when channel full
- Channel capacity constants match ADR-006 specification

---

### R4.2: Split Connection into Reader/Writer Halves

**Targets:** `crates/oxidized-protocol/src/transport/connection.rs`

**Current `Connection` struct (unchanged for backwards compat in pre-play
states initially):**

The existing `Connection` struct is retained for pre-play states where
the single-task model is simpler and sufficient. For the play state, the
`Connection` is consumed and split.

**Add `Connection::into_split()` method:**

```rust
impl Connection {
    /// Consume this connection and split into reader/writer halves
    /// with their respective cipher/compression state.
    ///
    /// Call this at the transition from Configuration → Play state.
    pub fn into_split(self) -> (ConnectionReader, ConnectionWriter) {
        let (decrypt, encrypt) = match self.cipher {
            Some(cipher) => cipher.split(),
            None => (None, None),
        };
        let (decompressor, compressor) = match self.compression {
            Some(comp) => comp.split(),
            None => (None, None),
        };
        (
            ConnectionReader {
                reader: self.reader,
                addr: self.addr,
                decrypt,
                decompressor,
            },
            ConnectionWriter {
                writer: self.writer,
                addr: self.addr,
                encrypt,
                compressor,
            },
        )
    }
}
```

**New structs:**

```rust
/// Read half of a split connection. Owns decryption and decompression state.
pub struct ConnectionReader {
    reader: OwnedReadHalf,
    addr: SocketAddr,
    decrypt: Option<DecryptState>,
    decompressor: Option<DecompressState>,
}

/// Write half of a split connection. Owns encryption and compression state.
pub struct ConnectionWriter {
    writer: OwnedWriteHalf,
    addr: SocketAddr,
    encrypt: Option<EncryptState>,
    compressor: Option<CompressState>,
    batch_buf: BytesMut,
}
```

**Prerequisite:** `CipherState` must support `split()` into separate
encrypt/decrypt halves. AES-CFB8 uses independent cipher instances for
each direction (already the case — verify in `aes_cfb8.rs`).

**Tests:**
- `Connection::into_split()` produces functional reader and writer
- Reader can decode packets after split
- Writer can encode and flush after split
- Encryption/compression survive the split (roundtrip test)

---

### R4.3: Implement Writer Task with Batch Flushing

**Targets:** `crates/oxidized-server/src/network/writer.rs` (new file)

The writer task is the core performance improvement. Instead of flushing
after every packet, it drains all queued packets and flushes once.

```rust
/// Writer task: receives outbound packets from a channel, encodes them
/// into a batch buffer, and flushes once per drain cycle.
pub async fn writer_loop(
    mut writer: ConnectionWriter,
    mut outbound_rx: mpsc::Receiver<OutboundPacket>,
) -> Result<(), ConnectionError> {
    loop {
        // Block until at least one packet is available
        let packet = match outbound_rx.recv().await {
            Some(pkt) => pkt,
            None => return Ok(()),  // Channel closed — clean shutdown
        };

        // Encode first packet into batch buffer
        writer.encode_packet(&packet)?;

        // Drain all remaining queued packets (non-blocking)
        while let Ok(packet) = outbound_rx.try_recv() {
            writer.encode_packet(&packet)?;
        }

        // Single flush for the entire batch
        writer.flush().await?;
    }
}
```

**Key design points:**

- `recv().await` blocks until a packet arrives (no busy-spinning)
- `try_recv()` drains everything already queued (non-blocking)
- One `write_all` + `flush` per batch = one syscall per drain cycle
- If the game sends 20 packets/tick for one player, they go out in
  one TCP segment instead of 20
- Memory: `batch_buf` is pre-allocated (64 KB), cleared after each flush
- Clean shutdown: when all `Sender` handles drop, `recv()` returns `None`

**Tests:**
- Single packet encode + flush works
- Batch of 100 packets results in single flush
- Writer exits cleanly when channel sender drops
- Writer handles encryption correctly after batch encode
- Writer handles compression correctly
- Memory budget: batch buffer doesn't exceed limit

---

### R4.4: Implement Reader Task with Rate Limiting

**Targets:** `crates/oxidized-server/src/network/reader.rs` (new file)

```rust
/// Reader task: reads packets from the socket, decodes them, and
/// dispatches them through the inbound channel.
pub async fn reader_loop(
    mut reader: ConnectionReader,
    inbound_tx: mpsc::Sender<InboundPacket>,
) -> Result<(), ConnectionError> {
    let mut packets_this_window: u32 = 0;
    let mut window_start = Instant::now();
    let tick_duration = Duration::from_millis(50);

    loop {
        let raw = reader.read_raw_packet().await?;

        // Rate limiting: reset counter each tick window
        if window_start.elapsed() >= tick_duration {
            packets_this_window = 0;
            window_start = Instant::now();
        }
        packets_this_window += 1;
        if packets_this_window > MAX_PACKETS_PER_TICK {
            return Err(ConnectionError::RateLimited);
        }

        let inbound = InboundPacket {
            id: raw.id,
            data: raw.data,
        };

        // Bounded send — blocks if game logic is slow (backpressure)
        if inbound_tx.send(inbound).await.is_err() {
            return Ok(());  // Game loop dropped receiver — clean exit
        }
    }
}
```

**Key design points:**

- Rate limit: 500 packets per 50ms window (ADR-006)
- Backpressure: `inbound_tx.send().await` blocks when channel is full
  (128 capacity), which stops reading from TCP, which triggers TCP
  flow control on the client
- Clean shutdown: when the receiver drops, `send()` returns `Err`
- Decryption/decompression happen inside `read_raw_packet()`

**Tests:**
- Reader dispatches packets to channel
- Reader blocks when channel is full (backpressure)
- Reader disconnects after 500+ packets in 50ms window
- Reader exits cleanly when receiver drops
- Reader handles encrypted packets correctly

---

### R4.5: Migrate Pre-Play States (Handshake/Status/Login/Config)

**Targets:** `crates/oxidized-server/src/network/mod.rs`,
`handshake.rs`, `status.rs`, `login.rs`, `configuration.rs`

**Strategy: Keep the single-task model for pre-play states.**

Pre-play states are short-lived, low-throughput, and sequential. The
single-task model is simpler and sufficient:

- **Handshake**: 1 packet in, 0 out → no need for task pair
- **Status**: 1–2 packets each direction → no batching benefit
- **Login**: ~5 packets, must be sequential (encryption handshake) →
  task pair would add complexity with no benefit
- **Configuration**: ~10 packets, sequential → same reasoning

The `Connection` struct is used as-is for these states. The split
happens at Configuration → Play transition.

**Changes to `handle_connection()`:**

```rust
async fn handle_connection(stream: TcpStream, addr: SocketAddr, ctx: &LoginContext)
    -> Result<(), ConnectionError>
{
    let mut conn = Connection::new(stream, addr)?;

    // Pre-play: use Connection directly (single-task, unchanged)
    loop {
        let pkt = conn.read_raw_packet().await?;
        match conn.state {
            Handshaking => handshake::handle_handshake(&mut conn, pkt).await?,
            Status => { /* ... unchanged ... */ },
            Login => {
                let profile = login::handle_login(&mut conn, pkt, ctx).await?;
                let client_info = configuration::handle_configuration(&mut conn).await?;

                // === SPLIT POINT: single task → task pair ===
                play::handle_play_split(conn, profile, client_info, ctx).await?;
                return Ok(());
            },
            _ => return Ok(()),
        }
    }
}
```

**Tests:**
- Handshake/Status/Login/Config still work with single-task `Connection`
- Connection split happens at correct point
- Existing pre-play tests still pass (no behavioral changes)

---

### R4.6: Migrate Play State — Inbound Path

**Targets:** `crates/oxidized-server/src/network/play/mod.rs`

**New `handle_play_split()` function replacing `handle_play_entry()`:**

```rust
pub async fn handle_play_split(
    conn: Connection,
    profile: GameProfile,
    client_info: ClientInformation,
    ctx: &LoginContext,
) -> Result<(), ConnectionError> {
    // 1. Split connection
    let (reader, writer) = conn.into_split();

    // 2. Create channels
    let (inbound_tx, mut inbound_rx) = mpsc::channel(INBOUND_CHANNEL_CAPACITY);
    let (outbound_tx, outbound_rx) = mpsc::channel(OUTBOUND_CHANNEL_CAPACITY);

    // 3. Spawn reader and writer tasks
    let reader_handle = tokio::spawn(reader_loop(reader, inbound_tx));
    let writer_handle = tokio::spawn(writer_loop(writer, outbound_rx));

    // 4. Create connection handle for sending
    let conn_handle = ConnectionHandle::new(outbound_tx.clone());

    // 5. Send join sequence through outbound channel
    join::send_join_sequence(&conn_handle, &profile, &client_info, ctx).await?;

    // 6. Main play loop — reads from inbound channel instead of socket
    let result = play_loop(&mut inbound_rx, &conn_handle, &profile, ctx).await;

    // 7. Cleanup: drop outbound sender → writer task exits
    drop(outbound_tx);
    drop(conn_handle);

    // 8. Wait for tasks to finish
    let _ = writer_handle.await;
    let _ = reader_handle.await;

    result
}
```

**Main play loop change — read from channel instead of socket:**

The current `tokio::select!` has a `play_ctx.conn.read_raw_packet()`
branch. This changes to `inbound_rx.recv()`:

```rust
// BEFORE:
packet_result = play_ctx.conn.read_raw_packet() => { ... }

// AFTER:
Some(inbound) = inbound_rx.recv() => {
    match inbound.id {
        // Same dispatch logic as before
    }
}
```

**All handler functions change signature:**

```rust
// BEFORE: handlers take &mut Connection
pub async fn handle_movement(ctx: &mut PlayContext<'_>, ...) -> Result<...>

// AFTER: handlers take &ConnectionHandle for sending
pub async fn handle_movement(ctx: &mut PlayContext<'_>, ...) -> Result<...>
// where PlayContext.conn_handle: &ConnectionHandle (not &mut Connection)
```

**Tests:**
- Play loop receives packets from inbound channel
- Handler dispatch works with channel-sourced packets
- Play loop exits when reader task disconnects (channel closes)

---

### R4.7: Migrate Play State — Outbound Path (Direct Sends)

**Targets:** All files in `crates/oxidized-server/src/network/play/`

Every `conn.send_packet(&pkt).await` and `conn.send_raw(id, &data).await`
call must be replaced with `conn_handle.send_packet(&pkt).await` or
`conn_handle.send_raw(id, data).await`.

**Affected files and approximate change count:**

| File | `send_packet` / `send_raw` calls | Notes |
|------|----------------------------------|-------|
| `play/join.rs` | ~32 | Heaviest — join sequence sends many packets |
| `play/mod.rs` | ~25 | Keepalive, state management |
| `play/movement.rs` | ~11 | Teleport confirms, position updates |
| `play/helpers.rs` | ~6 | Chunk sending (send_raw + flush) |
| `play/chat.rs` | ~4 | System messages |
| `play/inventory.rs` | ~3 | Equipment updates |
| `play/block_interaction.rs` | ~3 | Block change acks |
| `play/commands.rs` | ~1 | Tab completion response |
| `play/entity_tracking.rs` | ~2 | Entity spawn/despawn |
| `play/mining.rs` | ~2 | Block break acks |
| `play/placement.rs` | ~2 | Block placement acks |
| `play/sign_editing.rs` | ~1 | Sign open packet |
| `play/pick_block.rs` | ~1 | Creative inventory |
| `play/keepalive.rs` | 0 | Read-only (decode + validate) |

**Key change — remove explicit `flush()` calls:**

The current code often does `send_raw()` then `flush()`. With the writer
task, flushing is automatic — the writer batches and flushes. All manual
`conn.flush().await` calls in play handlers are removed.

**Chunk sending (helpers.rs) — biggest batching win:**

```rust
// BEFORE: N send_raw calls + 1 flush
for chunk in chunks {
    conn.send_raw(CHUNK_PACKET_ID, &encoded).await?;
}
conn.flush().await?;

// AFTER: N channel sends — writer batches automatically
for chunk in chunks {
    conn_handle.send_raw(CHUNK_PACKET_ID, encoded).await?;
}
// No explicit flush needed — writer task handles it
```

**Tests:**
- All existing play handler tests pass with `ConnectionHandle`
- Chunk sending through channel produces correct output
- Join sequence completes through channel

---

### R4.8: Migrate Play State — Broadcast Path

**Targets:** `crates/oxidized-server/src/network/play/mod.rs`

The current broadcast reception branch reads from a `broadcast::Receiver`
and calls `conn.send_raw()` + `conn.flush()` directly. This changes to
forward through the outbound channel.

**Current broadcast handling:**

```rust
broadcast_result = broadcast_rx.recv() => {
    match broadcast_result {
        Ok(msg) => {
            conn.send_raw(msg.packet_id, &msg.data).await?;
            conn.flush().await?;
        },
        Err(RecvError::Lagged(n)) => { /* log */ },
        Err(RecvError::Closed) => break,
    }
}
```

**New broadcast handling:**

```rust
broadcast_result = broadcast_rx.recv() => {
    match broadcast_result {
        Ok(msg) => {
            // Filter as before (exclude_entity, target_entity)
            if let Some(exclude) = msg.exclude_entity {
                if exclude == my_entity_id { continue; }
            }
            if let Some(target) = msg.target_entity {
                if target != my_entity_id { continue; }
            }
            // Queue on outbound channel — writer batches with other packets
            if conn_handle.try_send_raw(msg.packet_id, msg.data).is_err() {
                // Channel full — slow client, will be caught by R4.9
                warn!(peer = %addr, "Outbound channel full on broadcast");
                break;
            }
        },
        Err(RecvError::Lagged(n)) => { /* log */ },
        Err(RecvError::Closed) => break,
    }
}
```

**Key improvement:** Broadcast messages and direct sends now merge in
the same outbound channel. The writer flushes them together, reducing
syscalls from O(broadcasts × players) to O(players) per tick.

**Tests:**
- Broadcast messages arrive at the writer through outbound channel
- Entity filtering (exclude/target) still works
- Channel-full on broadcast triggers disconnect (slow client)
- Lagged broadcasts are logged, not fatal

---

### R4.9: Per-Connection Memory Budget and Slow-Client Detection

**Targets:** `crates/oxidized-server/src/network/writer.rs`,
`crates/oxidized-server/src/network/reader.rs`

**Memory budget (ADR-006: 256 KB per connection):**

Track combined buffer size in reader and writer:

```rust
// In writer_loop:
if self.batch_buf.len() > MAX_CONNECTION_MEMORY {
    return Err(ConnectionError::MemoryBudgetExceeded);
}
```

**Slow-client detection via outbound channel backpressure:**

If `conn_handle.send_packet().await` blocks for too long, the client
is too slow. Detection approaches:

1. **Channel full check (preferred):** Use `try_send()` for
   time-sensitive packets (broadcasts). If the channel is consistently
   full, disconnect after a configurable grace period.

2. **Writer-side timeout:** If `write_all().await` on the TCP socket
   takes longer than 30 seconds, disconnect.

```rust
// In writer_loop — add write timeout:
match tokio::time::timeout(
    WRITE_TIMEOUT,
    writer.write_all(&batch_buf),
).await {
    Ok(Ok(())) => { batch_buf.clear(); },
    Ok(Err(e)) => return Err(e.into()),
    Err(_) => return Err(ConnectionError::WriteTimeout),
}
```

**Keepalive interaction:** The existing keepalive timeout (30s) already
catches completely unresponsive clients. The memory budget and channel
backpressure catch *slow* clients that respond but can't keep up with
the data rate (e.g., chunk streaming).

**Tests:**
- Connection disconnected when memory budget exceeded
- Slow client (blocked writer) triggers timeout
- Channel-full detection works for broadcast sends
- Normal clients are not affected by budget/timeout checks

---

### R4.10: Compliance Tests and Benchmarks

**Targets:** `crates/oxidized-server/tests/network_compliance.rs` (new),
`crates/oxidized-protocol/tests/transport_compliance.rs` (new)

**Required by ADR-006 §Compliance:**

| Test | Description | Acceptance |
|------|-------------|------------|
| Throughput | Send 10,000 packets through writer task | >5,000 packets/sec |
| Memory | Monitor connection memory under load | <256 KB per connection |
| Backpressure | Simulate slow client (delayed reads) | Server disconnects, no OOM |
| Rate limit | Send 600 packets in 50ms | Connection terminated |
| TCP_NODELAY | Check socket option on accepted connections | Always set |
| Batch flush | Send 50 packets, verify single flush | 1 write syscall |
| Clean shutdown | Drop sender, verify tasks exit | No leaked tasks |
| Encryption roundtrip | Split connection, verify encrypted I/O | Packets decode correctly |

**Benchmark (criterion):**

```rust
// Measures: encode 100 packets → batch buffer → single flush
// vs. current: 100 × (encode + flush)
fn bench_batch_vs_individual(c: &mut Criterion) { ... }
```

**Integration test — full connection lifecycle:**

```rust
#[tokio::test]
async fn test_full_connection_lifecycle_with_task_pair() {
    // 1. Bind test server
    // 2. Connect client
    // 3. Complete handshake + login + config (single-task)
    // 4. Enter play state (task pair)
    // 5. Exchange keepalives
    // 6. Send/receive play packets
    // 7. Disconnect cleanly
    // 8. Verify both tasks exit
    // 9. Verify memory freed
}
```

---

## Migration Strategy

### Phase 1: Foundation (R4.1 + R4.2) — ✅ Complete

Added the new types and `into_split()` method. Nothing calls them in
production yet. All existing tests pass unchanged, plus 30 new tests
covering channel types, handle API, cipher/compression split, and
connection reader/writer halves (including encrypted + compressed
roundtrips).

### Phase 2: Writer task (R4.3) — testable in isolation

Implement and thoroughly test `writer_loop`. It reads from a channel
and writes to a `ConnectionWriter`. No integration with existing code
yet — tested with mock channels.

### Phase 3: Reader task (R4.4) — testable in isolation

Implement and test `reader_loop`. Reads from `ConnectionReader`,
dispatches to a channel. Rate limiting tested with burst sends.

### Phase 4: Integration (R4.5 + R4.6 + R4.7 + R4.8) — the big switch

This is the riskiest phase. The play state handler switches from
`&mut Connection` to `ConnectionHandle` + inbound channel. All play
handler files are touched. Do this in one commit to avoid a
half-migrated state.

**Risk mitigation:**
- Keep `Connection` for pre-play states (minimal change surface)
- Compile check after each file migration
- Run full test suite after migration
- Manual testing: join server, move, chat, break/place blocks, disconnect

### Phase 5: Hardening (R4.9 + R4.10) — safety nets

Add memory budgets, slow-client detection, and compliance tests.
These are additive — they don't change the happy path.

---

## Ordering & Dependencies

```
R4.1 (channel types)     ── foundation, do first
R4.2 (connection split)  ── depends on R4.1
R4.3 (writer task)       ── depends on R4.1, R4.2
R4.4 (reader task)       ── depends on R4.1, R4.2
R4.5 (pre-play migrate)  ── depends on R4.2 (minimal changes)
R4.6 (play inbound)      ── depends on R4.4
R4.7 (play outbound)     ── depends on R4.3, R4.6
R4.8 (play broadcast)    ── depends on R4.7
R4.9 (memory/slow)       ── depends on R4.3, R4.4
R4.10 (compliance)       ── depends on all above
```

**Critical path:** R4.1 → R4.2 → R4.3 + R4.4 (parallel) → R4.6 → R4.7 → R4.8 → R4.10

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Encryption state split breaks crypto | AES-CFB8 already uses independent encrypt/decrypt instances; verify with roundtrip tests before and after split |
| Play handler migration breaks packet ordering | Writer task preserves channel FIFO order; packets arrive in same order as sent |
| Keepalive sent through channel adds latency | Keepalive is not latency-critical (15s interval); sub-ms channel overhead is negligible |
| Broadcast + direct sends interleave incorrectly | Both go through same outbound channel — FIFO ordering preserved |
| Join sequence timing changes (batched vs immediate) | Client handles batched packets fine — Minecraft's own protocol uses batch markers (`ChunkBatchStart/Finished`) |
| Channel full during chunk loading disconnects player | Outbound channel is 512 packets — enough for ~30 chunks + overhead; monitor and tune if needed |
| Rate limiter false-positives on legitimate clients | 500 packets/50ms is very generous — vanilla client sends ~5-10/tick normally; only catches intentional floods |
| Two tasks per connection doubles Tokio task count | Tokio handles millions of tasks; 2000 tasks for 1000 players is trivial |
| `PlayContext` signature change touches every handler | Mechanical change (replace `conn: &mut Connection` with `conn_handle: &ConnectionHandle`); IDE rename helps |

---

## Files Changed Summary

### New Files

| File | Purpose |
|------|---------|
| `oxidized-protocol/src/transport/channel.rs` | `InboundPacket`, `OutboundPacket`, constants |
| `oxidized-protocol/src/transport/handle.rs` | `ConnectionHandle` API |
| `oxidized-server/src/network/reader.rs` | Reader task with rate limiting |
| `oxidized-server/src/network/writer.rs` | Writer task with batch flushing |
| `oxidized-server/tests/network_compliance.rs` | ADR-006 compliance tests |

### Modified Files

| File | Change |
|------|--------|
| `oxidized-protocol/src/transport/connection.rs` | Add `into_split()`, `ConnectionReader`, `ConnectionWriter` |
| `oxidized-protocol/src/transport/mod.rs` | Export new modules |
| `oxidized-server/src/network/mod.rs` | Split point in `handle_connection` |
| `oxidized-server/src/network/play/mod.rs` | New `handle_play_split`, channel-based loop |
| `oxidized-server/src/network/play/join.rs` | Use `ConnectionHandle` |
| `oxidized-server/src/network/play/movement.rs` | Use `ConnectionHandle` |
| `oxidized-server/src/network/play/chat.rs` | Use `ConnectionHandle` |
| `oxidized-server/src/network/play/helpers.rs` | Use `ConnectionHandle`, remove flush |
| `oxidized-server/src/network/play/inventory.rs` | Use `ConnectionHandle` |
| `oxidized-server/src/network/play/block_interaction.rs` | Use `ConnectionHandle` |
| `oxidized-server/src/network/play/commands.rs` | Use `ConnectionHandle` |
| `oxidized-server/src/network/play/entity_tracking.rs` | Use `ConnectionHandle` |
| `oxidized-server/src/network/play/mining.rs` | Use `ConnectionHandle` |
| `oxidized-server/src/network/play/placement.rs` | Use `ConnectionHandle` |
| `oxidized-server/src/network/play/sign_editing.rs` | Use `ConnectionHandle` |
| `oxidized-server/src/network/play/pick_block.rs` | Use `ConnectionHandle` |
| `oxidized-server/src/network/play/keepalive.rs` | Unchanged (read-only) |

---

## Acceptance Criteria

- [ ] `Connection::into_split()` produces `ConnectionReader` + `ConnectionWriter`
- [ ] Reader task dispatches packets through bounded inbound channel (128)
- [ ] Writer task batch-flushes from bounded outbound channel (512)
- [ ] Rate limiter disconnects clients sending >500 packets per 50ms
- [ ] Per-connection memory stays below 256 KB under normal load
- [ ] Slow-client detection disconnects unresponsive writers within 30s
- [ ] Pre-play states (Handshake/Status/Login/Config) still use single-task model
- [ ] All play handler `send_packet` calls go through `ConnectionHandle`
- [ ] No manual `flush()` calls in play handlers
- [ ] Broadcast messages route through outbound channel
- [ ] TCP_NODELAY set on all connections
- [ ] Keepalive timing unaffected (15s interval, 30s timeout)
- [ ] Clean shutdown: dropping senders causes both tasks to exit
- [ ] Throughput benchmark: >5000 packets/sec per connection
- [ ] `cargo test --workspace` passes with zero failures
- [ ] `cargo clippy --workspace -- -D warnings` produces zero warnings

---

## ADR Compliance Matrix (R4 scope)

| ADR | Requirement | Status |
|-----|-------------|--------|
| 006 | Per-connection reader/writer task pair | ❌ → target |
| 006 | Bounded inbound channel (128) | ❌ → target |
| 006 | Bounded outbound channel (512) | ❌ → target |
| 006 | Writer batch flushing | ❌ → target |
| 006 | TCP_NODELAY | ✅ Already compliant |
| 006 | Rate limiting (500/tick) | ❌ → target |
| 006 | Per-connection memory budget (256 KB) | ❌ → target |
| 006 | Throughput benchmark (>5000 pkt/s) | ❌ → target |
| 006 | Backpressure test | ❌ → target |
| 009 | Cipher state split for task pair | ✅ Complete |
| 020 | Network ↔ game channels | 🟡 Partial (outbound only; full ADR-020 in P15) |
