# ADR-020: Player Session Lifecycle

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P12, P14, P17, P20 |
| Deciders | Oxidized Core Team |

## Context

A player session encompasses everything that happens from the moment a client opens a TCP
connection to the moment it disconnects. During this lifecycle, the connection transitions
through multiple Minecraft protocol states — Handshake, Status, Login, Configuration, and
Play — each with distinct packet sets and behaviors. In the Login state, the server
authenticates the player with Mojang's session servers, exchanges encryption keys, and
enables compression. In Configuration, the server sends registry data, resource pack
prompts, and feature flags. In Play, the player entity is created (or loaded from disk),
chunks are streamed based on view distance, and the full game interaction loop begins.

In vanilla, this entire lifecycle is managed by a series of
`ServerCommonPacketListenerImpl` subclasses: `ServerHandshakePacketListenerImpl`,
`ServerStatusPacketListenerImpl`, `ServerLoginPacketListenerImpl`,
`ServerConfigurationPacketListenerImpl`, and `ServerGamePacketListenerImpl`. The Play
state handler alone (`ServerGamePacketListenerImpl`) is over 2,225 lines of code that
intermixes network protocol handling, game state mutation, anti-cheat validation, and
entity management. Methods like `handleMovePlayer()` directly modify the player entity's
position, check for illegal movement, update chunk tracking, and trigger block
interactions — all in one method, on the network thread. This tight coupling makes the
code nearly impossible to test, parallelize, or reason about independently.

Oxidized must cleanly separate concerns: network I/O (reading/writing bytes on sockets),
protocol state management (which packets are valid when), game state mutation (modifying
ECS components), and persistence (saving/loading player data). The design must handle edge
cases: what happens if the client disconnects during chunk loading? What if two clients
try to log in with the same UUID simultaneously? What if the game loop is running slower
than the network can deliver packets? These edge cases cause bugs and exploits in vanilla
that we can prevent architecturally.

## Decision Drivers

- **Separation of concerns**: Network I/O, protocol parsing, game logic, and persistence
  must be in separate modules with clear interfaces between them.
- **Backpressure handling**: If the game loop falls behind, inbound packets must queue
  with bounded capacity rather than causing unbounded memory growth or packet loss.
- **Graceful disconnect handling**: Whether the client disconnects cleanly (sending a
  disconnect packet), times out (no keepalive response), or crashes (TCP reset), the
  server must save player data, despawn the entity, and free resources.
- **Protocol state correctness**: Sending a Play-state packet during Login must be a
  compile-time error, not a runtime bug.
- **Authentication security**: Login must complete the Mojang authentication handshake
  correctly, including encryption and compression negotiation.
- **Chunk streaming efficiency**: Chunks must be sent to the player efficiently based on
  view distance, prioritizing chunks near the player and throttling to avoid network
  saturation.

## Considered Options

### Option 1: Monolithic Handler Like Vanilla

A single struct per player handles network I/O, protocol state, game logic, and
persistence. Methods on the struct process each packet type directly.

**Pros:**
- Simple to implement initially.
- Direct mapping to vanilla source for reference.

**Cons:**
- All of vanilla's problems: untestable, unparallelizable, tightly coupled.
- A bug in network parsing can corrupt game state.
- Cannot apply backpressure — packet handling is synchronous.

**Verdict: Rejected.** We're explicitly solving vanilla's architectural problems.

### Option 2: Actor Per Player

Each player is an independent actor (tokio task) with a mailbox. The actor owns the
player's network connection and game state. Messages to/from the game world are passed via
channels.

**Pros:**
- Clean isolation — each player's state is owned by one task.
- Natural async model — network I/O and game logic in the same task.
- No shared mutable state between players.

**Cons:**
- Player game state is isolated from the ECS world. Queries like "find all players within
  16 blocks" require message-passing round trips.
- Contradicts ADR-018 — player entities should be ECS entities with components, not
  actor-owned state.
- Tick synchronization is complex — the game loop must wait for all player actors to
  process their tick messages.

**Verdict: Rejected.** Conflicts with the ECS entity model.

### Option 3: Split Architecture — Network Task + ECS Entity + Bridge

Each player has two parts: (a) a network task (tokio task that owns the TCP stream and
handles serialization/deserialization) and (b) an ECS entity with player components in the
game world. They communicate via bounded channels. The game loop processes player events
from the channel during the NETWORK_RECEIVE phase and queues outbound packets during
NETWORK_SEND.

**Pros:**
- Clean separation — network code never touches ECS components; game code never touches
  sockets.
- Backpressure via bounded channels — if the game loop is slow, the channel fills up, and
  the network task stops reading (applying TCP backpressure to the client).
- Player state lives in ECS — all entity queries naturally include players.
- Network I/O is fully async (tokio), game logic is sync (ECS systems).
- Testable independently — network task can be tested with mock sockets, game systems can
  be tested with mock events.

**Cons:**
- Channel overhead — every packet crosses a channel boundary (serialized struct, not raw
  bytes). Typically < 1μs per crossing.
- Two-phase commit for disconnect — the network task detects disconnect, sends a
  disconnect event, the game loop processes it (despawn entity, save data), then
  acknowledges completion.
- Slightly more complex than a monolithic handler.

**Verdict: Selected.** Best alignment with ECS architecture and clean separation.

### Option 4: State Machine with Typed Phases

Use Rust's type system to model protocol states. Each state (Handshake, Status, Login,
Configuration, Play) is a separate type with only the packets valid for that state. State
transitions consume the current state and produce the next.

```rust
struct HandshakeState { stream: TcpStream }
struct LoginState { stream: TcpStream, username: String }
struct PlayState { stream: TcpStream, player_entity: Entity }

impl HandshakeState {
    fn handle_handshake(self, packet: HandshakePacket) -> Either<StatusState, LoginState>;
}
```

**Pros:**
- Compile-time protocol correctness — cannot send Play packets during Login.
- State transitions are explicit and auditable.

**Cons:**
- Does not address the separation of network vs. game logic.
- State machines get unwieldy when states can transition in multiple ways.

**Verdict: Partially adopted.** We use typed state machines within the network task, but
the overall architecture uses Option 3's split model.

## Decision

**Each player connection is split into a network task and an ECS entity, connected by
bounded mpsc channels.** The network task is a tokio task that owns the TCP stream and
handles all serialization/deserialization. The ECS entity holds all game state as
components. They communicate via two channels: an inbound event channel (network → game)
and an outbound packet channel (game → network).

### Connection Lifecycle

```
Client TCP Connect
       │
       ▼
┌──────────────┐
│  HANDSHAKE   │ ← Determines: Status query or Login
└──────┬───────┘
       │ (next_state = Login)
       ▼
┌──────────────┐
│    LOGIN     │ ← Authentication, encryption, compression
└──────┬───────┘
       │ (LoginSuccessPacket)
       ▼
┌──────────────┐
│CONFIGURATION │ ← Registry data, resource packs, feature flags
└──────┬───────┘
       │ (FinishConfigurationPacket)
       ▼
┌──────────────────────────────────────────────────┐
│                   PLAY STATE                      │
│                                                   │
│  ┌─────────────────┐    channel    ┌────────────┐│
│  │  Network Task    │◄────────────►│ ECS Entity  ││
│  │  (tokio task)    │  (events/    │ (game world)││
│  │                  │   packets)   │             ││
│  │ - Read packets   │              │ - Position  ││
│  │ - Write packets  │              │ - Inventory ││
│  │ - Keepalive      │              │ - GameMode  ││
│  │ - Compression    │              │ - Health    ││
│  │ - Encryption     │              │ - FoodData  ││
│  └─────────────────┘              └────────────┘│
└──────────────────────────────────────────────────┘
       │ (disconnect / timeout / kick)
       ▼
┌──────────────┐
│  DISCONNECT  │ ← Save player data, despawn entity, free resources
└──────────────┘
```

### Network Task Responsibilities

The network task is a `tokio::spawn`ed future that loops over socket reads. It uses a
typed state machine internally:

```rust
enum ConnectionState {
    Handshake(HandshakeHandler),
    Status(StatusHandler),
    Login(LoginHandler),
    Configuration(ConfigHandler),
    Play(PlayHandler),
}
```

In each state, the task reads raw bytes from the socket, decompresses (if enabled),
decrypts (if enabled), deserializes into the state-appropriate packet enum, and processes
it. In the Play state, processing means converting the packet into a game event and
sending it through the inbound channel:

```rust
// In PlayHandler
match packet {
    ServerboundMovePlayerPosPacket { x, y, z, on_ground } => {
        inbound_tx.send(PlayerEvent::Move { x, y, z, on_ground }).await?;
    }
    ServerboundPlayerActionPacket { action, pos, direction, sequence } => {
        inbound_tx.send(PlayerEvent::Action { action, pos, direction, sequence }).await?;
    }
    // ... etc
}
```

The network task also runs a keepalive loop: every 15 seconds, send a
`ClientboundKeepAlivePacket` with a random `i64` ID. If the client doesn't respond with a
matching `ServerboundKeepAlivePacket` within 30 seconds, disconnect the player.

### ECS Entity Components

When a player enters the Play state, the game loop spawns an ECS entity with these
components:

- `PlayerMarker` — marker component for player-specific system queries
- `Position(DVec3)` — world position
- `Rotation { yaw: f32, pitch: f32 }` — look direction
- `Velocity(DVec3)` — movement velocity
- `Health { current: f32, max: f32 }` — health (default 20.0)
- `FoodData { food_level: i32, saturation: f32, exhaustion: f32 }` — hunger
- `GameMode(GameType)` — survival, creative, adventure, spectator
- `Abilities(PlayerAbilities)` — fly, instabuild, invulnerable, mayBuild, walkSpeed, flySpeed
- `PlayerInventory` — 36 hotbar+main slots, 4 armor, 1 offhand, 1 crafting output, 4 crafting grid
- `SelectedSlot(u8)` — active hotbar slot (0-8)
- `ExperienceData { level: i32, progress: f32, total: i32 }` — XP
- `Equipment(EquipmentSlots)` — armor and held items (for network sync)
- `ChunkTracker` — set of chunks currently sent to this player
- `NetworkBridge { inbound: Receiver<PlayerEvent>, outbound: Sender<OutboundPacket> }`
- `PlayerUuid(Uuid)` — persistent identity
- `PlayerProfile(GameProfile)` — username, UUID, skin properties
- `SynchedEntityData` — tracked data for network serialization
- `PermissionLevel(u8)` — op level 0-4

### Packet Flow

**Inbound (client → server):**
1. Network task reads bytes from socket.
2. Decrypts → decompresses → deserializes into packet struct.
3. Converts to `PlayerEvent` enum variant.
4. Sends via bounded `mpsc::Sender` to game world (capacity: 128 events).
5. During NETWORK_RECEIVE phase, the `process_player_events` system drains all inbound
   channels and applies events to ECS components.

**Outbound (server → client):**
1. During NETWORK_SEND phase, systems query dirty components and create `OutboundPacket`
   enum variants.
2. Packets are sent via bounded `mpsc::Sender` to the network task (capacity: 256 packets).
3. Network task serializes → compresses → encrypts → writes bytes to socket.

### Disconnect Handling

Three disconnect scenarios, all converging to the same cleanup path:

1. **Graceful**: Client sends disconnect packet. Network task sends `PlayerEvent::Disconnect`
   through the inbound channel, then closes the channel and terminates.
2. **Timeout**: Keepalive timer expires. Network task sends `PlayerEvent::Disconnect { reason:
   Timeout }`, closes the channel, terminates.
3. **TCP reset**: Socket read returns error. Network task sends `PlayerEvent::Disconnect {
   reason: ConnectionLost }`, closes the channel, terminates.

The game loop's `handle_disconnect_system` processes the disconnect event:
1. Save player data to disk (position, inventory, health, etc.) via `player_data_saver`.
2. Remove player from all scoreboards and teams.
3. Send `ClientboundPlayerInfoRemovePacket` to all other players.
4. Despawn the player entity (removes all components).
5. Log the disconnection.

### Player Data Persistence

Player data is saved in two scenarios:
- **On disconnect**: Always save immediately (blocking I/O is acceptable since the player
  is leaving).
- **Periodic auto-save**: Every 6000 ticks (5 minutes), the `auto_save_system` iterates
  all player entities and serializes their components to NBT, writing to
  `world/playerdata/<uuid>.dat`. This uses async file I/O to avoid blocking the tick loop.

Player data format matches vanilla's NBT structure for compatibility with existing worlds.

### Chunk Streaming

The `chunk_tracker_system` runs in the NETWORK_SEND phase. For each player, it:
1. Calculates the set of chunks within view distance of the player's current chunk.
2. Diffs against the `ChunkTracker` component's sent set.
3. Queues `ClientboundLevelChunkWithLightPacket` for new chunks (closest first, up to 4
   per tick to avoid network saturation).
4. Queues `ClientboundForgetLevelChunkPacket` for chunks that left view distance.
5. Updates the `ChunkTracker` sent set.

Chunk data is serialized on a worker thread (via `rayon`) to avoid blocking the tick loop.

### Duplicate Login Prevention

If a client connects with a UUID that is already logged in:
1. The existing player is kicked with "You logged in from another location."
2. The existing player's data is saved.
3. The new connection proceeds with login normally.
4. The new player entity is spawned with freshly loaded data from disk.

This matches vanilla behavior and prevents UUID-based session hijacking.

## Consequences

### Positive

- **Clean separation**: Network code is 100% async I/O with zero game logic. Game systems
  are 100% ECS with zero socket operations. Each can be tested independently.
- **Natural backpressure**: Bounded channels prevent runaway memory growth when the game
  loop is slow. If the inbound channel is full, the network task blocks on send, which
  stops reading from the socket, which applies TCP backpressure to the client.
- **Player entities are first-class ECS entities**: All systems that operate on entities
  naturally include players. A "damage all entities" command works on players without
  special-casing.
- **Graceful disconnect in all cases**: The three-scenario disconnect model ensures player
  data is always saved, even during network errors or server overload.

### Negative

- **Channel latency**: Every inbound packet has ~1-5μs of channel overhead (enqueue +
  dequeue). For a server with 200 players sending 20 packets/second each, this is ~4000
  channel operations per tick — approximately 4-20ms total, which is measurable.
- **Deferred packet processing**: Packets are not processed immediately on receipt but
  queued until the next NETWORK_RECEIVE phase. This adds up to 50ms of latency (one tick).
  Vanilla has the same behavior for most packets (they queue too), so this is generally
  acceptable.
- **Complex disconnect coordination**: The two-part cleanup (network task closes → game
  loop despawns) requires careful handling of the case where the game loop is behind and
  hasn't processed the disconnect yet when the next tick starts.

### Neutral

- **Resource pack negotiation**: The Configuration state handles resource pack prompts
  before the player enters Play. This is simpler than vanilla's approach (which can
  reconfigure mid-Play in 1.20.2+).
- **Encryption overhead**: AES-128 CFB8 encryption/decryption runs in the network task,
  not the game loop. This is ~0.5μs per packet — negligible.

## Compliance

- **No game state in network tasks**: Code review must verify that network task code never
  imports or references ECS component types directly. All communication goes through the
  channel.
- **Disconnect safety test**: Integration test that kills TCP connections at every protocol
  state and verifies player data is saved and entity is despawned.
- **Backpressure test**: Simulate a slow game loop (200ms ticks) and verify that inbound
  channels fill up and network tasks block rather than consuming unbounded memory.
- **Keepalive timing test**: Verify that a client that stops responding is disconnected
  within 30-35 seconds (30s timeout + up to one tick of processing delay).
- **Chunk streaming test**: Connect a player, verify chunks arrive closest-first, verify
  chunks outside view distance are unloaded when the player moves.

## Related ADRs

- **ADR-018**: Entity System Architecture — player entities are ECS entities with
  PlayerMarker and player-specific components
- **ADR-019**: Tick Loop Design — NETWORK_RECEIVE and NETWORK_SEND phases bracket the
  game simulation
- **ADR-024**: Inventory & Container Transactions — PlayerInventory component and
  container interaction flow

## References

- Vanilla source: `net.minecraft.server.network.ServerGamePacketListenerImpl`
- Vanilla source: `net.minecraft.server.network.ServerLoginPacketListenerImpl`
- Vanilla source: `net.minecraft.server.network.ServerConfigurationPacketListenerImpl`
- Vanilla source: `net.minecraft.server.level.ServerPlayer`
- [Minecraft Protocol — Protocol States](https://wiki.vg/Protocol#Packet_format)
- [Mojang Authentication — Session Server](https://wiki.vg/Protocol_Encryption)
- [Tokio mpsc channels](https://docs.rs/tokio/latest/tokio/sync/mpsc/)
