# ADR-008: Connection State Machine Design

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P03, P04, P06, P12 |
| Deciders | Oxidized Core Team |

## Context

A Minecraft client connection progresses through a strict sequence of protocol states: **Handshaking** → **Status** (for server list ping) or **Login** → **Configuration** (since 1.20.2) → **Play**. Each state defines a different set of valid packets — a `LoginStart` packet is invalid during the `Play` state, and a `PlayerPosition` packet is meaningless during `Handshaking`. The server must reject or ignore packets that arrive in the wrong state.

Vanilla Java implements this with a single `Connection` class that holds a mutable `PacketListener` field. When the state changes (e.g., login completes), the listener is swapped to a new instance: `connection.setListener(new ServerPlayPacketListenerImpl(...))`. This works but provides no compile-time guarantees — it's entirely possible (and has historically caused bugs) to handle a packet in the wrong state, or to forget to swap the listener during a state transition.

Rust's type system offers a powerful alternative: the **typestate pattern**. By encoding the protocol state as a type parameter on the connection struct, we can make invalid state transitions and wrong-state packet handling into **compile-time errors**. A `Connection<Handshaking>` literally cannot call methods that handle `Play` packets — the methods don't exist on that type.

## Decision Drivers

- **Compile-time state safety**: handling a packet in the wrong state must be a compilation error, not a runtime bug
- **Exhaustive transitions**: every state must explicitly define which states it can transition to — no "forgot to handle this case" bugs
- **Shared resource transfer**: socket, encryption state, compression state, and connection metadata must transfer cleanly between states without cloning or re-initialization
- **Readability**: the state machine should be understandable by reading the type signatures — the code should document valid states and transitions
- **Async compatibility**: state transitions must work within async tasks — consuming `self` in an async method is fine with Tokio

## Considered Options

### Option 1: Enum state field with runtime checks (like vanilla)

```rust
struct Connection {
    state: ProtocolState,
    stream: TcpStream,
    // ...
}
```

Dispatch packets based on `self.state` at runtime. This mirrors vanilla's approach and is simple to implement. However, it provides no compile-time guarantees — a bug that processes a `Play` packet during `Login` would only be caught by testing or code review. The dispatch function would need a large match expression covering all state × packet combinations, which is error-prone.

### Option 2: Typestate pattern — different Rust types per state

```rust
struct Connection<S: State> {
    shared: SharedConnection,
    state_data: S,
}
```

Each protocol state is a separate type (`Handshaking`, `Login`, `Configuration`, `Play`). State transitions consume the old connection and produce a new one. Methods for handling packets are only defined on the relevant state type. Invalid transitions are compile-time errors.

### Option 3: Enum state with per-state packet dispatch tables

Use an enum for the state but define separate dispatch tables (function pointer maps) for each state. When a packet arrives, look up the handler in the current state's table. This provides runtime safety (unknown packets are rejected) but not compile-time safety — the dispatch tables are populated at runtime and could be misconfigured.

### Option 4: Actor per state

Model each state as a separate actor (async task). When the state transitions, the current actor sends the shared connection resources to a new actor via a oneshot channel and terminates. Each actor only handles packets for its state. This provides runtime isolation but adds complexity — task spawning overhead, channel coordination, and complex error handling when an actor fails during transition.

## Decision

**We adopt the typestate pattern (Option 2).** Each protocol state is encoded as a type parameter on the `Connection` struct. State transitions are methods that consume `self` and return a new `Connection` with the next state type. Packet handling methods are defined only on the appropriate state type.

### State Types

```rust
/// Marker trait for protocol states.
pub trait State: Send + 'static {}

/// Initial state — client sends Handshake packet.
pub struct Handshaking;
impl State for Handshaking {}

/// Server list ping — client queries MOTD, player count, etc.
pub struct Status;
impl State for Status {}

/// Authentication — username, encryption, compression setup.
pub struct Login;
impl State for Login {}

/// Post-login configuration — resource packs, registries, tags.
pub struct Configuration;
impl State for Configuration {}

/// Main gameplay — the long-lived state for connected players.
pub struct Play {
    pub player: PlayerData,
    pub last_keep_alive: Instant,
}
impl State for Play {}
```

### Connection Structure

```rust
/// Data shared across all states — transferred during transitions.
struct SharedConnection {
    read_half: OwnedReadHalf,
    write_half: OwnedWriteHalf,
    addr: SocketAddr,
    cipher: Option<CipherState>,
    compression: Option<CompressionState>,
    inbound_tx: mpsc::Sender<InboundPacket>,
    outbound_rx: mpsc::Receiver<OutboundPacket>,
}

/// A connection in a specific protocol state.
pub struct Connection<S: State> {
    shared: SharedConnection,
    state: S,
}
```

### State Transitions

Transitions consume the old connection and produce a new typed one:

```rust
impl Connection<Handshaking> {
    /// Transition to Status state (server list ping).
    pub fn into_status(self) -> Connection<Status> {
        Connection {
            shared: self.shared,
            state: Status,
        }
    }

    /// Transition to Login state (player joining).
    pub fn into_login(self) -> Connection<Login> {
        Connection {
            shared: self.shared,
            state: Login,
        }
    }

    // Cannot transition to Play or Configuration from Handshaking —
    // those methods simply don't exist on Connection<Handshaking>.
}

impl Connection<Login> {
    /// Enable encryption on the connection.
    pub fn enable_encryption(&mut self, shared_secret: &[u8]) {
        self.shared.cipher = Some(CipherState::new(shared_secret));
    }

    /// Enable compression on the connection.
    pub fn enable_compression(&mut self, threshold: usize) {
        self.shared.compression = Some(CompressionState::new(threshold));
    }

    /// Transition to Configuration state after successful authentication.
    pub fn into_configuration(self) -> Connection<Configuration> {
        Connection {
            shared: self.shared,
            state: Configuration,
        }
    }
}

impl Connection<Configuration> {
    /// Transition to Play state after configuration is complete.
    pub fn into_play(self, player: PlayerData) -> Connection<Play> {
        Connection {
            shared: self.shared,
            state: Play {
                player,
                last_keep_alive: Instant::now(),
            },
        }
    }

    /// Return to Configuration from Play (for resource pack changes in 1.20.4+).
    /// This is a special re-configuration transition.
    pub fn from_play(shared: SharedConnection, player: PlayerData) -> Self {
        Connection {
            shared,
            state: Configuration,
        }
    }
}
```

### Packet Handling

Each state defines its own `handle_packet` method that only accepts packets valid in that state:

```rust
impl Connection<Handshaking> {
    pub async fn handle_packet(&mut self, packet: HandshakingPacket) -> Result<Transition> {
        match packet {
            HandshakingPacket::Handshake(hs) => {
                match hs.next_state.0 {
                    1 => Ok(Transition::ToStatus),
                    2 => Ok(Transition::ToLogin),
                    _ => Err(ProtocolError::InvalidNextState(hs.next_state.0).into()),
                }
            }
        }
    }
}

impl Connection<Play> {
    pub async fn handle_packet(&mut self, packet: PlayPacket) -> Result<()> {
        match packet {
            PlayPacket::PlayerPosition(pos) => self.handle_player_position(pos).await,
            PlayPacket::ChatMessage(msg) => self.handle_chat_message(msg).await,
            PlayPacket::KeepAliveResponse(ka) => self.handle_keep_alive(ka),
            // ... all Play-state packets
        }
    }
}

// This won't compile — Connection<Handshaking> has no handle_player_position method:
// let conn: Connection<Handshaking> = ...;
// conn.handle_player_position(pos); // ERROR: method not found
```

### Connection Lifecycle

The full lifecycle in the reader task:

```rust
pub async fn run_connection(stream: TcpStream, addr: SocketAddr) -> Result<()> {
    let conn = Connection::<Handshaking>::new(stream, addr);

    // Phase 1: Handshaking (exactly one packet)
    let transition = conn.handle_handshake().await?;
    match transition {
        Transition::ToStatus => {
            let mut conn = conn.into_status();
            conn.run_status_exchange().await?;
            // Status connections end here
        }
        Transition::ToLogin => {
            let mut conn = conn.into_login();
            conn.run_login_sequence().await?;
            // Login sets up encryption + compression

            let mut conn = conn.into_configuration();
            conn.run_configuration().await?;
            // Configuration sends registries, tags, resource packs

            let mut conn = conn.into_play(player_data);
            conn.run_play_loop().await?;
            // Play loop runs until disconnect
        }
    }
    Ok(())
}
```

### Play ↔ Configuration Re-entry (1.20.4+)

Since Minecraft 1.20.2, the server can send players back to the Configuration state (e.g., to apply new resource packs). This is modeled as an explicit transition:

```rust
impl Connection<Play> {
    pub fn into_reconfiguration(self) -> (Connection<Configuration>, PlayerData) {
        let player = self.state.player;
        let conn = Connection {
            shared: self.shared,
            state: Configuration,
        };
        (conn, player)
    }
}
```

## Consequences

### Positive

- Invalid state transitions are compile-time errors — if `Connection<Handshaking>` doesn't have an `into_play()` method, that transition literally cannot be written
- Packet handling methods are scoped to their state type — impossible to accidentally handle a `Play` packet during `Login`
- State-specific data (e.g., `PlayerData` only exists in `Play`) is guaranteed to be available by the type system — no `Option<Player>` null checks
- The transition chain is readable as a sequence of typed method calls — the code documents the protocol state machine
- Shared resources (socket, cipher, compression) transfer between states by moving a struct — zero-cost, no cloning

### Negative

- The typestate pattern is unfamiliar to many developers — the "consuming self" pattern requires understanding Rust's ownership model
- Adding a new state requires defining a new type, implementing `State`, and adding transition methods on the relevant existing states — more boilerplate than changing an enum variant
- Dynamic dispatch is needed in a few places (e.g., the server's connection list contains connections in different states) — requires `Box<dyn ConnectionHandle>` or similar trait object

### Neutral

- The pattern naturally enforces the single-packet-per-state-transition rule for `Handshaking` — the handler returns a `Transition` and consumes the connection
- Re-configuration (Play → Configuration → Play) creates a temporary `Configuration` connection, which is semantically correct but may seem unusual

## Compliance

- **Compile-time check**: any PR that adds a new packet handler must place it in the correct state's `impl` block — the compiler rejects it if the state is wrong
- **State transition audit**: code review verifies that every `into_*` transition method transfers all shared resources and doesn't leak file descriptors or allocated buffers
- **Integration test**: a test drives a connection through the full Handshaking → Login → Configuration → Play lifecycle using a mock client, verifying state transitions occur in the correct order
- **Re-configuration test**: a test verifies the Play → Configuration → Play transition preserves player data and connection state
- **No runtime state checks**: code review rejects any `if self.state == Play { ... }` patterns — state-specific logic must be in the typed `impl` block

## Related ADRs

- [ADR-001: Async Runtime Selection](adr-001-async-runtime.md) — connection tasks are async functions running on Tokio
- [ADR-006: Network I/O Architecture](adr-006-network-io.md) — the reader/writer task pair owns the connection state machine
- [ADR-007: Packet Codec Framework](adr-007-packet-codec.md) — packet types are state-scoped enums decoded by the registry
- [ADR-009: Encryption & Compression Pipeline](adr-009-encryption-compression.md) — cipher and compression are in SharedConnection, enabled during Login

## References

- [Typestate pattern in Rust](https://cliffle.com/blog/rust-typestate/)
- [Ana Hoverbear — "Rust Typestates"](https://hoverbear.org/blog/rust-state-machine-pattern/)
- [wiki.vg — Protocol States](https://wiki.vg/Protocol#Packet_format)
- [Minecraft 1.20.2 Configuration phase](https://minecraft.wiki/w/Java_Edition_1.20.2#Protocol)
- [Yandros — "Typestates in Rust"](https://docs.rs/typestate/latest/typestate/)
