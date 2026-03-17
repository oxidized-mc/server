# Phase 37 — JSON-RPC Management API

**Crate:** `oxidized-server`  
**Reward:** Server manageable via WebSocket JSON-RPC; management tools (e.g.
Minecraft Bedrock Editor, custom dashboards) can connect and call methods.

**New in Minecraft 26.1 (Minecraft Java Edition snapshot).**

**Depends on:** Phase 18 (commands), Phase 33 (RCON/Query — same server mgmt
crate patterns)

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-031: Management API](../adr/adr-031-management-api.md) — unified management service with RCON/Query/JSON-RPC frontends


## Goal

Implement the JSON-RPC 2.0 management server that vanilla Minecraft 26.1
introduced. The server listens on a configurable port, authenticates clients via
a secret token, and exposes a rich set of methods for querying and controlling
the server. Server-to-client notifications (push events) are delivered over the
same WebSocket connection.

The Java reference implementation uses Netty + WebSockets. The Rust
implementation uses `tokio-tungstenite` and `axum` (or equivalent).

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Management server | `ManagementServer` | `net.minecraft.server.jsonrpc.ManagementServer` |
| WebSocket codec | `WebSocketToJsonCodec` | `net.minecraft.server.jsonrpc.websocket.WebSocketToJsonCodec` |
| Authentication handler | `AuthenticationHandler` | `net.minecraft.server.jsonrpc.security.AuthenticationHandler` |
| SSL context | `JsonRpcSslContextProvider` | `net.minecraft.server.jsonrpc.security.JsonRpcSslContextProvider` |
| Security config | `SecurityConfig` | `net.minecraft.server.jsonrpc.security.SecurityConfig` |
| Incoming method registry | `IncomingRpcMethods` | `net.minecraft.server.jsonrpc.IncomingRpcMethods` |
| Outgoing notifications | `OutgoingRpcMethods` | `net.minecraft.server.jsonrpc.OutgoingRpcMethods` |
| JSON-RPC errors | `JsonRPCErrors` | `net.minecraft.server.jsonrpc.JsonRPCErrors` |
| Player service | `PlayerService` | `net.minecraft.server.jsonrpc.methods.PlayerService` |
| Server state service | `ServerStateService` | `net.minecraft.server.jsonrpc.methods.ServerStateService` |
| Ban list service | `BanlistService` | `net.minecraft.server.jsonrpc.methods.BanlistService` |
| Allowlist service | `AllowlistService` | `net.minecraft.server.jsonrpc.methods.AllowlistService` |
| Operator service | `OperatorService` | `net.minecraft.server.jsonrpc.methods.OperatorService` |
| Game rules service | `GameRulesService` | `net.minecraft.server.jsonrpc.methods.GameRulesService` |
| Server settings service | `ServerSettingsService` | `net.minecraft.server.jsonrpc.methods.ServerSettingsService` |
| Discovery service | `DiscoveryService` | `net.minecraft.server.jsonrpc.methods.DiscoveryService` |

---

## Tasks

### 37.1 — `server.properties` configuration

| Property | Default | Notes |
|----------|---------|-------|
| `management-server-port` | (disabled) | Integer port; server does not start if absent |
| `management-server-tls-enabled` | `true` | Require TLS (rustls) |
| `management-server-cert-path` | `management.crt` | PEM certificate |
| `management-server-key-path` | `management.key` | PEM private key |
| `management-server-secret` | `""` | Required shared secret; refuse to start if empty |
| `management-server-allowed-origins` | `""` | Comma-separated allowed CORS origins |

### 37.2 — JSON-RPC 2.0 protocol

#### Request (client → server)

```json
{
  "jsonrpc": "2.0",
  "method": "players",
  "params": {},
  "id": 1
}
```

#### Response (server → client, on success)

```json
{
  "jsonrpc": "2.0",
  "result": { ... },
  "id": 1
}
```

#### Error response

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32601,
    "message": "Method not found",
    "data": null
  },
  "id": 1
}
```

#### Server notification (no `id` — push from server)

```json
{
  "jsonrpc": "2.0",
  "method": "players/joined",
  "params": { "player": { "id": "...", "name": "Steve" } }
}
```

#### Standard error codes

| Code | Meaning |
|------|---------|
| `-32700` | Parse error (invalid JSON) |
| `-32600` | Invalid Request (missing jsonrpc/method) |
| `-32601` | Method not found |
| `-32602` | Invalid params (wrong type/shape) |
| `-32603` | Internal error (server panicked etc.) |
| `-32099` to `-32000` | Server-defined application errors |

```rust
// crates/oxidized-server/src/jsonrpc/error.rs

#[derive(Debug, Clone)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    pub const PARSE_ERROR: i32       = -32700;
    pub const INVALID_REQUEST: i32   = -32600;
    pub const METHOD_NOT_FOUND: i32  = -32601;
    pub const INVALID_PARAMS: i32    = -32602;
    pub const INTERNAL_ERROR: i32    = -32603;
    // Server-defined
    pub const UNAUTHORIZED: i32      = -32000;
    pub const SERVER_NOT_READY: i32  = -32001;
    pub const ENCODE_ERROR: i32      = -32002;
}
```

### 37.3 — Authentication

Authentication checks the HTTP `Authorization` header on the WebSocket upgrade
handshake. Clients must present:

```
Authorization: Bearer <secret>
```

OR the subprotocol field may carry the secret as:

```
Sec-WebSocket-Protocol: minecraft-v1,<secret>
```

CORS is validated against `management-server-allowed-origins`. Connections from
unlisted origins receive `403 Forbidden`.

```rust
// crates/oxidized-server/src/jsonrpc/auth.rs

pub fn check_auth(request: &http::Request<()>, secret: &str, allowed_origins: &[&str]) -> AuthResult {
    // CORS check
    if let Some(origin) = request.headers().get("Origin") {
        if !allowed_origins.is_empty() {
            let origin_str = origin.to_str().unwrap_or("");
            if !allowed_origins.contains(&origin_str) {
                return AuthResult::Forbidden("Origin not allowed");
            }
        }
    }
    // Bearer token check
    let auth = request.headers().get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));
    // Subprotocol fallback
    let subproto = request.headers().get("Sec-WebSocket-Protocol")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("minecraft-v1,"));
    let token = auth.or(subproto);
    match token {
        Some(t) if constant_time_eq(t.as_bytes(), secret.as_bytes()) => AuthResult::Allowed,
        _ => AuthResult::Unauthorized("Invalid or missing secret"),
    }
}
```

### 37.4 — Incoming RPC methods (request → response)

All methods operate on the main server thread via `channel` dispatch. Methods
tagged `notOnMainThread` in the Java source may be handled on the IO thread.

#### Player management

| Method | Params | Result | Notes |
|--------|--------|--------|-------|
| `players` | — | `[PlayerDto]` | List online players |
| `players/kick` | `{add: [{player, message?}]}` | `[PlayerDto]` | Kick one or more players |

`PlayerDto`:
```json
{ "id": "uuid-string", "name": "Steve" }
```

#### Ban list management

| Method | Params | Result |
|--------|--------|--------|
| `bans` | — | `[UserBanDto]` |
| `bans/add` | `{add: [UserBanDto]}` | `[UserBanDto]` |
| `bans/remove` | `{remove: [PlayerDto]}` | `[UserBanDto]` |
| `bans/set` | `{bans: [UserBanDto]}` | `[UserBanDto]` |
| `bans/clear` | — | `[UserBanDto]` |

`UserBanDto`:
```json
{ "player": { "id": "...", "name": "..." }, "reason": "griefing", "expires": "2027-01-01T00:00:00Z" }
```

#### IP ban management

| Method | Params | Result |
|--------|--------|--------|
| `ip_bans` | — | `[IpBanDto]` |
| `ip_bans/add` | `{add: [IpBanDto]}` | `[IpBanDto]` |
| `ip_bans/remove` | `{remove: [string]}` | `[IpBanDto]` |
| `ip_bans/set` | `{ip_bans: [IpBanDto]}` | `[IpBanDto]` |
| `ip_bans/clear` | — | `[IpBanDto]` |

#### Allowlist management

| Method | Params | Result |
|--------|--------|--------|
| `allowlist` | — | `[PlayerDto]` |
| `allowlist/add` | `{add: [PlayerDto]}` | `[PlayerDto]` |
| `allowlist/remove` | `{remove: [PlayerDto]}` | `[PlayerDto]` |
| `allowlist/set` | `{players: [PlayerDto]}` | `[PlayerDto]` |
| `allowlist/clear` | — | `[PlayerDto]` |

#### Operator management

| Method | Params | Result |
|--------|--------|--------|
| `operators` | — | `[OperatorDto]` |
| `operators/add` | `{add: [OperatorDto]}` | `[OperatorDto]` |
| `operators/remove` | `{remove: [PlayerDto]}` | `[OperatorDto]` |
| `operators/set` | `{operators: [OperatorDto]}` | `[OperatorDto]` |
| `operators/clear` | — | `[OperatorDto]` |

`OperatorDto`:
```json
{ "player": { "id": "...", "name": "..." }, "level": 4, "bypassesPlayerLimit": false }
```

#### Server control

| Method | Params | Result |
|--------|--------|--------|
| `server/status` | — | `ServerState` |
| `server/save` | `{flush: bool}` | `bool` |
| `server/stop` | — | `bool` |
| `server/system_message` | `{message, overlay, receivingPlayers?}` | `bool` |

`ServerState`:
```json
{
  "started": true,
  "players": [...],
  "version": { "name": "26.1-pre-3", "protocol": 1073742124 }
}
```

#### Server settings (get/set pairs)

Each setting has a `serversettings/<name>` getter and `serversettings/<name>/set` setter:

| Setting | Type | Notes |
|---------|------|-------|
| `autosave` | `bool` | Auto-save enabled |
| `difficulty` | `string` | peaceful/easy/normal/hard |
| `enforce_allowlist` | `bool` | Kick players not on allowlist |
| `use_allowlist` | `bool` | Whether allowlist is active |
| `max_players` | `int` | Max concurrent players |
| `pause_when_empty_seconds` | `int?` | Null = disabled |
| `player_idle_timeout` | `int` | Minutes; 0=disabled |
| `allow_flight` | `bool` | Anti-cheat flight check |
| `motd` | `string` | Server description |
| `spawn_protection_radius` | `int` | Blocks around spawn |
| `force_game_mode` | `bool` | Force default game mode on join |
| `game_mode` | `string` | Default game mode |
| `view_distance` | `int` | 2–32 |
| `simulation_distance` | `int` | 2–32 |
| `accept_transfers` | `bool` | Accept transfer packets |
| `status_heartbeat_interval` | `int?` | Seconds between status notifications |
| `operator_user_permission_level` | `int` | Default op permission level |

#### Game rules

| Method | Params | Result |
|--------|--------|--------|
| `gamerules` | — | `[GameRuleUpdate]` |
| `gamerules/update` | `GameRuleUpdate` | `GameRuleUpdate` |

`GameRuleUpdate`:
```json
{ "key": "doDaylightCycle", "value": true }
```

#### Discovery

| Method | Params | Result |
|--------|--------|--------|
| `rpc.discover` | — | OpenRPC discovery document |

Returns the full OpenRPC schema listing all available methods, their parameters
and return types.

### 37.5 — Outgoing notifications (server → client push)

Server push notifications are sent to all connected authenticated clients as
JSON-RPC notifications (no `id`):

| Method | When sent | Params |
|--------|-----------|--------|
| `server/started` | Server finishes startup | — |
| `server/stopping` | Server begins shutdown | — |
| `server/saving` | Save started | — |
| `server/saved` | Save completed | — |
| `server/activity` | Any player activity (rate limited to 1/30s) | — |
| `players/joined` | Player connects | `{player: PlayerDto}` |
| `players/left` | Player disconnects | `{player: PlayerDto}` |
| `operators/added` | Player opped | `{player: OperatorDto}` |
| `operators/removed` | Player de-opped | `{player: OperatorDto}` |
| `allowlist/added` | Added to allowlist | `{player: PlayerDto}` |
| `allowlist/removed` | Removed from allowlist | `{player: PlayerDto}` |
| `ip_bans/added` | IP banned | `{player: IpBanDto}` |
| `ip_bans/removed` | IP unbanned | `{player: string}` |
| `bans/added` | Player banned | `{player: UserBanDto}` |
| `bans/removed` | Player unbanned | `{player: PlayerDto}` |
| `server/status` | Heartbeat (configurable interval) | `ServerState` |

### 37.6 — Server implementation (`oxidized-server/src/jsonrpc/server.rs`)

```rust
// crates/oxidized-server/src/jsonrpc/server.rs

pub async fn run_management_server(
    addr: SocketAddr,
    tls_config: Option<Arc<ServerConfig>>,
    secret: Arc<String>,
    allowed_origins: Arc<Vec<String>>,
    api: Arc<ServerApi>,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("Management server listening on {addr}");

    loop {
        let (stream, remote_addr) = listener.accept().await?;
        let secret2 = secret.clone();
        let origins2 = allowed_origins.clone();
        let api2 = api.clone();
        let tls2 = tls_config.clone();

        tokio::spawn(async move {
            let result = handle_connection(
                stream, remote_addr, tls2, secret2, origins2, api2).await;
            if let Err(e) = result {
                tracing::debug!("Management client {remote_addr} error: {e}");
            }
        });
    }
}

async fn handle_connection(
    stream: TcpStream,
    remote_addr: SocketAddr,
    tls_config: Option<Arc<ServerConfig>>,
    secret: Arc<String>,
    allowed_origins: Arc<Vec<String>>,
    api: Arc<ServerApi>,
) -> anyhow::Result<()> {
    // Upgrade to TLS if configured
    let ws_stream = if let Some(tls) = tls_config {
        let acceptor = TlsAcceptor::from(tls);
        let tls_stream = acceptor.accept(stream).await?;
        tokio_tungstenite::accept_hdr_async(tls_stream, |req, res| {
            auth_callback(req, res, &secret, &allowed_origins)
        }).await?
    } else {
        tokio_tungstenite::accept_hdr_async(stream, |req, res| {
            auth_callback(req, res, &secret, &allowed_origins)
        }).await?
    };

    handle_ws_client(ws_stream, remote_addr, api).await
}
```

### 37.7 — Request dispatch loop

```rust
async fn handle_ws_client(
    mut ws: WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>,
    addr: SocketAddr,
    api: Arc<ServerApi>,
) -> anyhow::Result<()> {
    while let Some(msg) = ws.next().await {
        let msg = msg?;
        if let Message::Text(text) = msg {
            let response = dispatch_request(&text, &api).await;
            ws.send(Message::Text(serde_json::to_string(&response)?)).await?;
        }
    }
    Ok(())
}

async fn dispatch_request(text: &str, api: &ServerApi) -> serde_json::Value {
    let req: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return json_rpc_error(serde_json::Value::Null,
                                        JsonRpcError::PARSE_ERROR, "Parse error"),
    };
    let id = req.get("id").cloned().unwrap_or(serde_json::Value::Null);
    let method = match req.get("method").and_then(|v| v.as_str()) {
        Some(m) => m,
        None => return json_rpc_error(id, JsonRpcError::INVALID_REQUEST,
                                      "Missing method"),
    };
    let params = req.get("params").cloned().unwrap_or(json!({}));

    match api.dispatch(method, params).await {
        Ok(result) => json!({ "jsonrpc": "2.0", "result": result, "id": id }),
        Err(e) => json_rpc_error(id, e.code, &e.message),
    }
}
```

### 37.8 — Notification broadcasting

```rust
// crates/oxidized-server/src/jsonrpc/notifications.rs

pub struct NotificationBroadcaster {
    /// Channel for each connected client; sender cloned per connection.
    clients: Arc<RwLock<Vec<mpsc::UnboundedSender<String>>>>,
}

impl NotificationBroadcaster {
    pub fn broadcast(&self, method: &str, params: serde_json::Value) {
        let msg = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        })).unwrap_or_default();
        let clients = self.clients.read().unwrap();
        clients.iter().for_each(|tx| { let _ = tx.send(msg.clone()); });
    }
}
```

Hook `NotificationBroadcaster` into server events:

```rust
// In player join handler:
broadcaster.broadcast("players/joined", json!({ "player": PlayerDto::from(&player) }));

// In server shutdown:
broadcaster.broadcast("server/stopping", json!({}));
```

---

## Acceptance Criteria

- [ ] `wscat -c 'wss://localhost:<port>' -H 'Authorization: Bearer secret'`
      connects successfully with TLS
- [ ] `{"jsonrpc":"2.0","method":"players","id":1}` returns the player list
- [ ] `{"jsonrpc":"2.0","method":"server/status","id":2}` returns status with
      `started: true`
- [ ] `{"jsonrpc":"2.0","method":"server/system_message","params":{"message":{"text":"hello"},"overlay":false},"id":3}`
      broadcasts `hello` to all players
- [ ] `{"jsonrpc":"2.0","method":"server/stop","id":4}` stops the server
- [ ] Wrong secret → `401 Unauthorized` on WebSocket upgrade
- [ ] Origin not in allowed list → `403 Forbidden`
- [ ] Unknown method → error code `-32601`
- [ ] Player join triggers `players/joined` notification to all connected clients
- [ ] `rpc.discover` returns an OpenRPC document listing all methods
- [ ] `management-server-port` absent in `server.properties` → server does not
      start; no error logged
