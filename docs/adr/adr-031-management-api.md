# ADR-031: Management & Remote Access APIs

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P33, P37 |
| Deciders | Oxidized Core Team |

## Context

Minecraft servers require remote management capabilities for operators who don't have direct
console access. Historically, two protocols have served this role: RCON (Remote Console), a
TCP-based protocol for executing commands and receiving responses, and GS4 Query, a UDP-based
protocol for retrieving server status information (player list, MOTD, game type, map name).
Both protocols date back to the Minecraft Beta era and are widely supported by hosting panels
(Pterodactyl, AMP, Multicraft), monitoring tools (Grafana dashboards), Discord bots, and
server list websites.

Version 26.1 introduces a new JSON-RPC management API over WebSocket, representing Mojang's
modern approach to server management. This API provides structured method calls (not raw
command strings), event subscriptions (real-time notifications for player join/leave, chat,
deaths), and proper authentication via Bearer tokens. The JSON-RPC API is expected to become
the primary management interface, with RCON and Query maintained for backward compatibility.
Tools like the official server monitoring dashboard and third-party management panels will
adopt it.

We must support all three protocols. The RCON and Query protocols are simple but have specific
quirks (RCON's fragmented response handling, Query's challenge-token rotation) that must be
implemented precisely for compatibility with existing tools. The JSON-RPC API is more complex
but well-structured. All three share a common need: executing server commands and querying
server state. Rather than implementing three separate codebases, we design a unified
management layer that the three protocols connect to as thin adapters.

## Decision Drivers

- **Tool compatibility**: RCON must work with existing tools (mcrcon, Pterodactyl, AMP,
  Crafty). Query must work with server list websites and monitoring tools. JSON-RPC must
  match vanilla 26.1's specification exactly.
- **Security**: All three protocols handle sensitive operations (command execution, player
  data). Authentication must be robust: timing-safe password comparison for RCON, challenge
  tokens for Query, Bearer token validation for JSON-RPC.
- **Unified management logic**: Command execution and status queries should be implemented
  once, not three times. Protocol adapters should be thin translation layers.
- **Performance**: Query is polled frequently by monitoring tools (every 5-30 seconds per
  tool, potentially dozens of tools). RCON commands may be issued rapidly by automation.
  Neither should impact game tick performance.
- **Configurability**: Each protocol should be independently enableable and configurable
  (bind address, port, authentication). Operators may want RCON without Query, or JSON-RPC
  without RCON.
- **TLS for JSON-RPC**: The modern API should support encryption. Web-based management tools
  require HTTPS/WSS.

## Considered Options

### Option 1: Three Separate Protocol Implementations

Implement each protocol independently with its own command execution and state query logic.

**Pros**: Simple conceptually — each protocol is self-contained. No abstraction overhead.

**Cons**: Code duplication (command execution, status formatting). Bug fixes must be applied
three times. Behavioral inconsistencies between protocols. Testing burden triples.

### Option 2: Unified Management Layer with Protocol Adapters

Define a `ManagementService` with methods like `execute_command()`, `get_server_status()`,
`list_players()`. Each protocol has a thin adapter that translates between its wire format
and the ManagementService API.

**Pros**: Single source of truth for management logic. Consistent behavior across protocols.
Adding new management features benefits all protocols automatically. Testing is focused on
one implementation.

**Cons**: Abstraction layer adds a small amount of indirection. Adapter code must handle
protocol-specific quirks that don't map cleanly to the unified API.

### Option 3: REST API (Non-Standard)

Replace or supplement RCON/Query with a RESTful HTTP API. JSON request/response, standard
HTTP methods, OpenAPI specification.

**Pros**: Familiar to web developers. Excellent tooling (curl, Postman, every HTTP library).
Self-documenting with OpenAPI.

**Cons**: Not compatible with existing Minecraft tools. The vanilla client doesn't use it.
Would be in addition to RCON/Query (not a replacement), adding a fourth protocol. Vanilla
26.1 chose JSON-RPC over REST, so we follow their design for client compatibility.

### Option 4: gRPC for Management

Use gRPC with Protocol Buffers for a strongly-typed, high-performance management API.

**Pros**: Strong typing, code generation, bidirectional streaming, excellent performance.

**Cons**: Not compatible with vanilla tools. Requires gRPC client libraries (not available in
all languages). Heavy dependency (tonic, prost). Vanilla chose JSON-RPC, and we must match
that for compatibility. Could be offered as an optional fourth interface later.

## Decision

**Unified management service with three protocol frontends.** A single `ManagementService`
holds all management logic. Three protocol adapters (RCON, Query, JSON-RPC) translate between
their wire formats and the service.

### ManagementService

```rust
pub struct ManagementService {
    server: Arc<MinecraftServer>,
    command_dispatcher: Arc<CommandDispatcher>,
}

impl ManagementService {
    /// Execute a command as the server console and return the text output.
    pub async fn execute_command(&self, command: &str) -> CommandResult {
        let source = CommandSource::console();
        self.command_dispatcher.dispatch(source, command).await
    }

    /// Get current server status (player count, TPS, MOTD, etc.).
    pub fn get_server_status(&self) -> ServerStatus {
        ServerStatus {
            motd: self.server.motd().clone(),
            player_count: self.server.player_count(),
            max_players: self.server.max_players(),
            players: self.server.online_player_names(),
            version: self.server.version().to_string(),
            protocol_version: PROTOCOL_VERSION,
            game_type: self.server.default_game_type(),
            map_name: self.server.level_name().to_string(),
            server_port: self.server.port(),
            tps: self.server.tps(),
        }
    }

    /// List online players with details.
    pub fn list_players(&self) -> Vec<PlayerInfo> {
        self.server.online_players().iter().map(|p| PlayerInfo {
            uuid: p.uuid(),
            name: p.name().to_string(),
            address: p.remote_address().to_string(),
            latency_ms: p.latency_ms(),
        }).collect()
    }

    /// Subscribe to server events (for JSON-RPC).
    pub fn subscribe_events(&self, filter: EventFilter) -> EventStream {
        self.server.event_bus().subscribe(filter)
    }
}
```

### RCON Protocol Adapter

RCON uses a simple binary frame protocol over TCP:

```
Frame format (little-endian):
┌──────────┬──────────┬──────────┬──────────────────┬──────┐
│ Length    │ Request  │ Type     │ Payload          │ Pad  │
│ (i32)    │ ID (i32) │ (i32)    │ (variable, ASCII)│ (2B) │
└──────────┴──────────┴──────────┴──────────────────┴──────┘
Length = RequestID(4) + Type(4) + Payload(N) + Pad(2)
```

| Type | Value | Direction |
|------|-------|-----------|
| Login | 3 | Client → Server |
| Login Response | 2 | Server → Client |
| Command | 2 | Client → Server |
| Command Response | 0 | Server → Client |

Authentication handshake:
1. Client sends Login frame (type 3) with password as payload.
2. Server responds with Login Response (type 2). If auth succeeds, request ID matches. If
   auth fails, request ID is -1.
3. After successful auth, client sends Command frames (type 2).
4. Server responds with Command Response frames (type 0).

Implementation details:
- **Max connections**: 4 simultaneous RCON connections (configurable). Additional connections
  are rejected immediately.
- **Max payload size**: 4096 bytes (vanilla limit). Responses exceeding this are split into
  multiple frames with the same request ID.
- **Password comparison**: Timing-safe comparison using `constant_time_eq` to prevent timing
  attacks on the password.
- **Bind configuration**: `rcon.port` (default 25575), `rcon.password` (required to enable),
  `rcon.address` (default 0.0.0.0).

```rust
pub struct RconAdapter {
    listener: TcpListener,
    management: Arc<ManagementService>,
    password_hash: [u8; 32],  // SHA-256 of configured password
    max_connections: usize,
    active_connections: AtomicUsize,
}
```

### Query Protocol Adapter

GS4 Query uses UDP with a challenge-token mechanism to prevent amplification attacks:

**Handshake**:
1. Client sends Handshake request (type 0x09) with session ID.
2. Server responds with a challenge token (random i32, ASCII-encoded).
3. Client includes the challenge token in subsequent requests.
4. Challenge tokens expire after 30 seconds and are rotated.

**Basic Stat** (type 0x00, short response):
- MOTD, game type, map name, current players, max players, host port, host IP.

**Full Stat** (type 0x00 with additional padding bytes):
- All basic stat fields plus: game ID, version, plugin list, and full player name list.

```rust
pub struct QueryAdapter {
    socket: UdpSocket,
    management: Arc<ManagementService>,
    challenges: DashMap<SocketAddr, (i32, Instant)>,  // addr → (token, expiry)
    challenge_rotation_interval: Duration,  // 30s
}
```

Implementation details:
- **Challenge token rotation**: Every 30 seconds, expired tokens are purged. Each client
  address gets a unique token.
- **Response caching**: Server status is cached for 5 seconds to avoid recomputing on every
  query (monitoring tools may poll frequently).
- **Max response size**: UDP responses are limited to ~1400 bytes (MTU-safe). Player lists
  exceeding this are truncated.
- **Bind configuration**: `query.port` (default 25565), `enable-query` (default false).

### JSON-RPC Protocol Adapter (WebSocket)

The modern management API uses JSON-RPC 2.0 over WebSocket, optionally with TLS:

```rust
pub struct JsonRpcAdapter {
    management: Arc<ManagementService>,
    tls_config: Option<Arc<rustls::ServerConfig>>,
    bearer_token_hash: [u8; 32],
    subscriptions: DashMap<ConnectionId, Vec<EventFilter>>,
}
```

**Connection flow**:
1. Client connects via WebSocket (`ws://` or `wss://`).
2. Client sends authentication: `Authorization: Bearer <token>` in the upgrade request
   headers, or as the first JSON-RPC call (`auth.login`).
3. After authentication, client sends JSON-RPC 2.0 requests.
4. Server responds with JSON-RPC 2.0 responses.
5. Client can subscribe to events; server sends JSON-RPC notifications for subscribed events.

**JSON-RPC Methods** (matching vanilla 26.1):

| Method | Description | Parameters |
|--------|-------------|------------|
| `server.status` | Get server status | — |
| `server.stop` | Stop the server | — |
| `server.tps` | Get TPS info | — |
| `players.list` | List online players | `{ offset?, limit? }` |
| `players.get` | Get player details | `{ uuid }` or `{ name }` |
| `players.kick` | Kick a player | `{ uuid, reason? }` |
| `players.ban` | Ban a player | `{ uuid, reason?, duration? }` |
| `command.execute` | Execute a command | `{ command }` |
| `command.completions` | Get tab completions | `{ command, cursor? }` |
| `world.info` | Get world info | `{ dimension? }` |
| `events.subscribe` | Subscribe to events | `{ events: [...] }` |
| `events.unsubscribe` | Unsubscribe | `{ subscription_id }` |

**Event Notifications** (server → client, JSON-RPC notification format):

```json
{
    "jsonrpc": "2.0",
    "method": "event.player_join",
    "params": {
        "uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5",
        "name": "Notch",
        "timestamp": "2026-03-17T14:23:45.123Z"
    }
}
```

Subscribable events: `player_join`, `player_leave`, `player_chat`, `player_death`,
`player_advancement`, `server_tps_change`, `command_executed`.

**TLS Configuration**:
- Certificate and key files specified in `oxidized.toml`:
  `management-api.tls-cert=/path/to/cert.pem`, `management-api.tls-key=/path/to/key.pem`.
- If TLS is not configured, the WebSocket runs unencrypted (with a warning in the log).
- TLS is provided by `rustls` (no OpenSSL dependency).

**CORS Headers** (for web-based management tools):
- `Access-Control-Allow-Origin`: Configurable (default: same-origin only).
- `Access-Control-Allow-Headers`: `Authorization, Content-Type`.
- `Access-Control-Allow-Methods`: `GET` (WebSocket upgrade only).

### Authentication Security

All three protocols implement secure authentication:

| Protocol | Auth Method | Security Measure |
|----------|-------------|------------------|
| RCON | Password in login frame | Timing-safe comparison (`constant_time_eq`) |
| Query | Challenge token | 30s rotation, per-client tokens, anti-amplification |
| JSON-RPC | Bearer token in header | Timing-safe comparison, TLS recommended |

Passwords and tokens are never logged, even at debug level. Failed authentication attempts
are logged with the source IP and rate-limited (max 5 failures per IP per minute; subsequent
attempts are silently dropped for 60 seconds).

### Configuration

```properties
# RCON
enable-rcon=false
rcon.port=25575
rcon.password=
rcon.max-connections=4

# Query
enable-query=false
query.port=25565

# JSON-RPC Management API
enable-management-api=false
management-api.port=25580
management-api.token=
management-api.tls-cert=
management-api.tls-key=
management-api.cors-origins=
```

All three protocols are disabled by default and require explicit opt-in. This follows the
principle of least privilege and matches vanilla's default configuration.

## Consequences

### Positive

- **Full compatibility**: All three protocols work with their respective ecosystems — RCON
  tools, Query-based monitoring, and vanilla 26.1's JSON-RPC clients.
- **Single management logic**: Command execution and status queries are implemented once.
  Behavioral consistency is guaranteed across protocols.
- **Modern and legacy**: JSON-RPC provides a modern, structured API with event subscriptions.
  RCON and Query maintain backward compatibility. Operators can migrate at their own pace.
- **Security**: Timing-safe authentication, TLS support, rate limiting, and connection caps
  provide defense in depth.

### Negative

- **Three protocols to maintain**: Despite the unified backend, each protocol adapter has its
  own wire format, framing, and quirks. Bug reports may be protocol-specific.
- **WebSocket complexity**: JSON-RPC over WebSocket with TLS and event subscriptions is the
  most complex adapter. Connection lifecycle management (reconnection, subscription cleanup)
  adds code.
- **Configuration surface**: Three protocols means three sets of ports, toggles, and auth
  settings in `oxidized.toml`. Documentation must clearly explain each.

### Neutral

- The JSON-RPC method list will grow with future Minecraft versions. The adapter pattern makes
  adding new methods straightforward (implement in ManagementService, expose in adapter).
- A fourth protocol adapter (e.g., gRPC, REST) could be added later without changing the
  management service. The architecture is open for extension.

## Compliance

- [ ] RCON: `mcrcon` connects, authenticates, and executes commands successfully.
- [ ] RCON: Failed auth returns request ID -1. Timing-safe comparison verified.
- [ ] RCON: Max 4 simultaneous connections enforced; 5th connection is rejected.
- [ ] RCON: Response fragmentation works for outputs exceeding 4096 bytes.
- [ ] Query: Basic stat response matches vanilla format (verified with `mcstatus` tool).
- [ ] Query: Full stat response includes all fields including player list.
- [ ] Query: Challenge token rotation occurs every 30 seconds.
- [ ] Query: Expired challenge tokens are rejected.
- [ ] JSON-RPC: All methods in the method table return correct responses.
- [ ] JSON-RPC: Event subscriptions deliver notifications for join/leave/chat/death.
- [ ] JSON-RPC: TLS works with a self-signed certificate.
- [ ] JSON-RPC: Unauthenticated requests are rejected with appropriate error code.
- [ ] All protocols: `execute_command("list")` returns the same player list.
- [ ] Auth rate limiting: 6th failed attempt from same IP within 1 minute is dropped.

## Related ADRs

- **ADR-003**: Packet Codec Architecture (RCON uses a similar framing model)
- **ADR-010**: Command System (ManagementService delegates to CommandDispatcher)
- **ADR-017**: Player Session & Authentication (player data referenced by management API)
- **ADR-030**: Graceful Shutdown & Crash Recovery (`server.stop` via JSON-RPC triggers
  graceful shutdown)
- **ADR-032**: Performance & Scalability (TPS metrics exposed via management API)

## References

- [wiki.vg — RCON Protocol](https://wiki.vg/RCON)
- [wiki.vg — Query Protocol](https://wiki.vg/Query)
- [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification)
- [Minecraft 26.1 JSON-RPC Management API](https://minecraft.wiki/w/Management_API)
- [rustls — modern TLS in Rust](https://github.com/rustls/rustls)
- [tokio-tungstenite — async WebSocket](https://github.com/snapview/tokio-tungstenite)
- [constant_time_eq crate](https://crates.io/crates/constant_time_eq)
