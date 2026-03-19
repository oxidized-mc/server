# ADR-036: Packet Handler Architecture

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-19 |
| Phases | R1 (Refactoring), P19+, All future network phases |
| Deciders | Oxidized Core Team |

## Context

The `oxidized-server/src/network.rs` file has grown to 2079 lines with a single
`handle_play_entry` function spanning 715 lines. At its core is a 433-line if-else chain
that dispatches serverbound Play packets by comparing `pkt.id` against known constants:

```rust
if pkt.id == ServerboundKeepAlivePacket::PACKET_ID {
    match ServerboundKeepAlivePacket::decode(pkt.data) {
        Ok(ka) => { /* 20 lines of handling */ },
        Err(e) => { debug!(...); },
    }
} else if pkt.id == ServerboundMovePlayerPacket::PACKET_ID_POS || ... {
    // 157 lines of movement handling
} else if pkt.id == ServerboundChatPacket::PACKET_ID {
    // 48 lines of chat handling
} // ... 10 more branches
```

This pattern has three problems:

1. **Scalability**: Play state has 58 serverbound packets in 26.1. We currently handle ~12.
   Each new packet adds another `else if` branch with inline decode + handle + error log.
   At full coverage, this will be 1000+ lines of dispatch.

2. **Repetition**: Every branch repeats the same pattern — `PacketType::decode(pkt.data)`
   → `match Ok/Err` → handle/log. This is 6-8 lines of boilerplate per packet × 58 packets.

3. **Testability**: Handlers are closures inside a giant function, making them impossible to
   unit-test without driving the entire connection lifecycle.

Additionally, ADR-008 specified a typestate pattern for connections (`Connection<S: State>`)
but the implementation uses a runtime `ConnectionState` enum. This was a pragmatic choice —
typestate adds complexity to async handler signatures — but the safety benefit (preventing
wrong-state packet handling) can be achieved architecturally by splitting handlers into
per-state modules where each module only handles its own packets.

## Decision Drivers

- **Extensibility**: adding a new packet handler should require touching only one file
- **Testability**: each handler must be independently unit-testable
- **Readability**: the dispatch logic should be a concise routing table, not 433 lines of
  if-else
- **DRY**: the decode-match-log pattern should be written once and reused
- **Safety**: wrong-state packet handling should be prevented by module structure, not
  runtime checks
- **Performance**: dispatch overhead must be negligible compared to packet handling cost

## Considered Options

### Option 1: Keep if-else chain, just split into functions

Extract each handler body into a named function but keep the if-else chain for dispatch.
This improves readability and testability but doesn't solve the scalability or DRY problems.
The dispatch chain still grows linearly with packet count.

### Option 2: Match statement with function calls

Replace if-else with `match pkt.id { ... }` where each arm calls a handler function.
Combined with a `decode_and_handle` helper, this is concise and idiomatic Rust. The match
statement serves as a routing table. Each handler function lives in a dedicated module file.

### Option 3: Trait-based dispatch with registry

Define a `PacketHandler` trait and register implementations in a `HashMap<PacketId, Box<dyn PacketHandler>>`. Dispatch is a single lookup. This is highly extensible but adds runtime
overhead (vtable dispatch, HashMap lookup) and is unidiomatic for Rust where the set of
packets is known at compile time.

### Option 4: Enum-based dispatch with derive macro

Create a `ServerboundPlayPacket` enum with a variant per packet type, derive `Decode`, and
pattern match. This provides the best type safety but requires a large enum and custom
derive infrastructure that doesn't yet exist.

## Decision

**We adopt Option 2: Match statement with function calls and a decode helper.** This is the
simplest approach that solves all three problems (scalability, repetition, testability)
without introducing framework complexity.

### Module Structure

```
src/network/
├── mod.rs                  # TCP listener, accept loop, shared types & context structs
├── helpers.rs              # decode_packet<T>(), disconnect(), utility functions
├── handshake.rs            # handle_handshake() — Handshaking state
├── status.rs               # handle_status() — Status state  
├── login.rs                # handle_login(), authenticate_online() — Login state
├── configuration.rs        # handle_configuration() — Configuration state
└── play/
    ├── mod.rs              # handle_play() — main select! loop + packet dispatch match
    ├── movement.rs         # handle_movement() — position, rotation, chunk tracking
    ├── chat.rs             # handle_chat(), handle_chat_command() — messaging
    ├── commands.rs         # handle_command_suggestion(), make_command_source() — commands
    └── helpers.rs          # send_initial_chunks(), commands_packet_from_tree()
```

### Decode Helper

A generic function eliminates the repeated decode + error log pattern:

```rust
/// Decode a packet from raw bytes, logging failures with connection context.
pub fn decode_packet<T: McRead>(
    data: bytes::Bytes,
    addr: SocketAddr,
    player_name: &str,
    packet_name: &str,
) -> Result<T, ConnectionError> {
    T::decode(data).map_err(|e| {
        debug!(
            peer = %addr,
            name = %player_name,
            error = %e,
            "Failed to decode {packet_name}"
        );
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Failed to decode {packet_name}: {e}"),
        ))
    })
}
```

Usage in handlers reduces each packet decode from 6-8 lines to 1:

```rust
let ka: ServerboundKeepAlivePacket = decode_packet(pkt.data, addr, name, "KeepAlive")?;
```

### Play State Dispatch

The play loop's dispatch is a compact match statement that delegates to handler functions:

```rust
// In play/mod.rs — the main dispatch
match pkt.id {
    ServerboundKeepAlivePacket::PACKET_ID => {
        handle_keepalive(&mut ctx, pkt.data).await?;
    }
    id if is_movement_packet(id) => {
        movement::handle_movement(&mut ctx, pkt.id, pkt.data).await?;
    }
    ServerboundChatPacket::PACKET_ID => {
        chat::handle_chat(&mut ctx, pkt.data).await?;
    }
    ServerboundChatCommandPacket::PACKET_ID
    | ServerboundChatCommandSignedPacket::PACKET_ID => {
        chat::handle_chat_command(&mut ctx, pkt.id, pkt.data).await?;
    }
    ServerboundCommandSuggestionPacket::PACKET_ID => {
        commands::handle_suggestion(&mut ctx, pkt.data).await?;
    }
    ServerboundPlayerCommandPacket::PACKET_ID => {
        handle_player_command(&mut ctx, pkt.data).await?;
    }
    // ... future packets go here — one line each
    unknown if unknown < 0 || unknown > MAX_PLAY_PACKET_ID => {
        warn!(peer = %ctx.addr, id = unknown, "Invalid packet ID");
        break;
    }
    unhandled => {
        trace!(peer = %ctx.addr, id = unhandled, "Unhandled play packet");
    }
}
```

Each match arm is 1-3 lines. Adding a new packet requires:
1. Create a handler function in the appropriate submodule
2. Add one match arm to the dispatch

### Handler Context

Instead of passing 8+ parameters to every handler, bundle them into a context struct:

```rust
/// Shared context passed to all play-state packet handlers.
pub struct PlayContext<'a> {
    pub conn: &'a mut Connection,
    pub player: &'a Arc<RwLock<ServerPlayer>>,
    pub server_ctx: &'a Arc<ServerContext>,
    pub player_name: &'a str,
    pub player_uuid: uuid::Uuid,
    pub addr: SocketAddr,
    pub chunk_tracker: &'a mut PlayerChunkTracker,
    pub rate_limiter: &'a mut ChatRateLimiter,
    pub pending_teleport: &'a mut Option<(i32, /* ... */)>,
}
```

### Connection State Handling (ADR-008 Amendment)

ADR-008's typestate pattern is **not retroactively implemented**. The runtime
`ConnectionState` enum remains. However, the module structure achieves the same safety goal:

- `handshake.rs` only imports Handshaking packets
- `login.rs` only imports Login packets
- `play/movement.rs` only imports Play movement packets

A developer physically cannot handle a Login packet in `play/movement.rs` because the
packet types aren't imported. This is **file-level type safety** rather than generic-parameter
type safety — less formal but equally effective in practice.

ADR-008's status is updated to note this architectural amendment.

## Consequences

### Positive

- **Scalability**: Adding a new packet is a 1-file change (handler) + 1-line change (dispatch arm)
- **Testability**: Each handler function can be unit-tested by constructing a `PlayContext`
  with mock components
- **Readability**: The dispatch match is a ~60-line routing table, not a 433-line logic block
- **DRY**: The decode pattern is written once in `decode_packet<T>()`
- **Safety**: Module structure prevents cross-state packet handling without typestate complexity
- **File sizes**: `network.rs` (2079 LOC) becomes ~10 files averaging ~150 LOC each

### Negative

- **Refactoring cost**: Splitting the existing file requires careful moves to preserve
  behavior — must be done with comprehensive test coverage
- **Context struct passing**: Handler functions receive a `PlayContext` reference, adding one
  level of indirection compared to direct variable access
- **Import management**: Each handler file must import its packet types — more `use` statements
  than a single-file approach

### Neutral

- The match dispatch has effectively the same performance as the if-else chain — both
  compile to a jump table or branch chain
- Future evolution to Option 4 (enum dispatch) remains possible — the handler functions
  would stay the same, only the dispatch mechanism changes

## Compliance

- **New packet handlers**: Must be added as functions in the appropriate submodule, not
  inline in the dispatch match
- **Handler signature**: All play handlers take `&mut PlayContext` and return
  `Result<(), ConnectionError>` (or `Result<ControlFlow, ConnectionError>` for handlers
  that may signal disconnection)
- **Decode pattern**: All packet decoding must use `decode_packet<T>()` — no inline
  `match Packet::decode() { Ok/Err }` blocks
- **File size**: Each handler file should stay under 200 LOC; split further if exceeded
- **Tests**: Each handler module includes unit tests for its handler functions

## Related ADRs

- [ADR-006: Network I/O Architecture](adr-006-network-io.md) — the reader/writer task pair
  remains unchanged; this ADR restructures the packet handling within the reader task
- [ADR-007: Packet Codec Framework](adr-007-packet-codec.md) — packet types and their
  `decode`/`encode` methods are used by the handlers
- [ADR-008: Connection State Machine Design](adr-008-connection-state-machine.md) — typestate
  pattern is not implemented; module structure provides equivalent safety (see amendment)
- [ADR-035: Module Structure & File Size Policy](adr-035-module-structure.md) — this ADR is
  a specific application of the module structure policy to the network layer

## References

- [Rust match ergonomics](https://doc.rust-lang.org/reference/expressions/match-expr.html)
- [Context pattern in Rust](https://rust-unofficial.github.io/patterns/patterns/behavioural/context.html)
- [Netty ChannelHandler architecture](https://netty.io/4.1/api/io/netty/channel/ChannelHandler.html) — inspiration for per-packet handler separation
