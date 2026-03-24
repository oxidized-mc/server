# Phase 33 — RCON + Query

**Status:** 📋 Planned  
**Crate:** `oxidized-server`  
**Reward:** Server manageable remotely via RCON; Query protocol works for server
list tools (e.g. MCStat, minequery).

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-031: Management API](../adr/adr-031-management-api.md) — unified management service with RCON/Query/JSON-RPC frontends


## Goal

Implement two server management protocols that have been part of Minecraft's
network surface since the Beta era: RCON (TCP, binary) for remote command
execution and the GameSpy4-compatible UDP Query protocol for server status
polling. Both run alongside the main game server on dedicated ports.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| RCON TCP server | `RconServer` | `net.minecraft.server.rcon.thread.RconThread` |
| RCON client handler | `RconClient` | `net.minecraft.server.rcon.thread.RconClient` |
| Query server | `QueryThreadGs4` | `net.minecraft.server.rcon.thread.QueryThreadGs4` |
| Packet utilities | `PktUtils` | `net.minecraft.server.rcon.PktUtils` |
| Console command source | `RconConsoleSource` | `net.minecraft.server.rcon.RconConsoleSource` |
| Server interface | `ServerInterface` | `net.minecraft.server.ServerInterface` |

---

## Tasks

### 33.1 — RCON packet codec (`oxidized-server/src/rcon/codec.rs`)

RCON uses a simple little-endian binary framing. Each packet on the wire:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                    Length (i32 LE)                            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                  Request ID (i32 LE)                          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                    Type (i32 LE)                              |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|            Payload (null-terminated UTF-8 string)             |
|                        ...                                    |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  0x00  |  0x00  |  (null terminator + 1 pad byte)            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

`Length` = total bytes of `RequestID + Type + Payload + 2` (does **not** include
the 4-byte Length field itself).  Minimum packet is 10 bytes (empty payload).

```rust
// crates/oxidized-server/src/rcon/codec.rs

use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const SERVERDATA_AUTH: i32 = 3;
pub const SERVERDATA_AUTH_RESPONSE: i32 = 2;
pub const SERVERDATA_EXECCOMMAND: i32 = 2;
pub const SERVERDATA_RESPONSE_VALUE: i32 = 0;
pub const SERVERDATA_AUTH_FAILURE: i32 = -1;

/// Maximum payload size before splitting (vanilla uses 4096).
pub const MAX_RESPONSE_SIZE: usize = 4096;

#[derive(Debug, Clone)]
pub struct RconPacket {
    pub request_id: i32,
    pub pkt_type: i32,
    pub payload: String,
}

impl RconPacket {
    pub async fn read<R: AsyncReadExt + Unpin>(r: &mut R) -> anyhow::Result<Self> {
        let len = r.read_i32_le().await? as usize;
        anyhow::ensure!(len >= 10, "RCON packet too short: {len}");
        anyhow::ensure!(len <= 4110, "RCON packet too long: {len}");
        let request_id = r.read_i32_le().await?;
        let pkt_type = r.read_i32_le().await?;
        let payload_len = len - 10;
        let mut payload_bytes = vec![0u8; payload_len];
        r.read_exact(&mut payload_bytes).await?;
        let _ = r.read_u8().await?; // null terminator
        let _ = r.read_u8().await?; // pad byte
        let payload = String::from_utf8_lossy(&payload_bytes).into_owned();
        Ok(Self { request_id, pkt_type, payload })
    }

    pub async fn write<W: AsyncWriteExt + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
        let payload_bytes = self.payload.as_bytes();
        let len = (payload_bytes.len() + 10) as i32;
        w.write_i32_le(len).await?;
        w.write_i32_le(self.request_id).await?;
        w.write_i32_le(self.pkt_type).await?;
        w.write_all(payload_bytes).await?;
        w.write_u8(0x00).await?; // null terminator
        w.write_u8(0x00).await?; // pad byte
        Ok(())
    }
}
```

### 33.2 — RCON client handler (`oxidized-server/src/rcon/client.rs`)

Each accepted TCP connection gets its own task.

```rust
// crates/oxidized-server/src/rcon/client.rs

pub async fn handle_rcon_client(
    stream: TcpStream,
    password: Arc<String>,
    command_sender: mpsc::Sender<(String, oneshot::Sender<String>)>,
) {
    let (mut reader, mut writer) = stream.into_split();
    let mut authed = false;

    loop {
        let pkt = match RconPacket::read(&mut reader).await {
            Ok(p) => p,
            Err(_) => return,
        };

        match pkt.pkt_type {
            SERVERDATA_AUTH => {
                if pkt.payload == *password {
                    authed = true;
                    // Auth response: echo request_id on success
                    RconPacket { request_id: pkt.request_id,
                                 pkt_type: SERVERDATA_RESPONSE_VALUE,
                                 payload: String::new() }
                        .write(&mut writer).await.ok();
                    RconPacket { request_id: pkt.request_id,
                                 pkt_type: SERVERDATA_AUTH_RESPONSE,
                                 payload: String::new() }
                        .write(&mut writer).await.ok();
                } else {
                    // Auth failure: request_id = -1
                    RconPacket { request_id: SERVERDATA_AUTH_FAILURE,
                                 pkt_type: SERVERDATA_AUTH_RESPONSE,
                                 payload: String::new() }
                        .write(&mut writer).await.ok();
                }
            }
            SERVERDATA_EXECCOMMAND if authed => {
                let (tx, rx) = oneshot::channel();
                let _ = command_sender.send((pkt.payload.clone(), tx)).await;
                let output = rx.await.unwrap_or_default();
                // Split large responses at MAX_RESPONSE_SIZE boundary
                for chunk in output.as_bytes().chunks(MAX_RESPONSE_SIZE) {
                    let payload = String::from_utf8_lossy(chunk).into_owned();
                    RconPacket { request_id: pkt.request_id,
                                 pkt_type: SERVERDATA_RESPONSE_VALUE,
                                 payload }
                        .write(&mut writer).await.ok();
                }
            }
            _ if !authed => {
                RconPacket { request_id: SERVERDATA_AUTH_FAILURE,
                             pkt_type: SERVERDATA_AUTH_RESPONSE,
                             payload: String::new() }
                    .write(&mut writer).await.ok();
            }
            _ => {}
        }
    }
}
```

### 33.3 — RCON TCP server (`oxidized-server/src/rcon/server.rs`)

- [ ] Bind to `0.0.0.0:rcon.port` (default 25575) when `enable-rcon = true`
- [ ] Accept up to 4 concurrent connections (vanilla maximum); log a warning and
      drop the socket when at capacity
- [ ] Read `rcon.password` from `oxidized.toml`; refuse to start with an
      empty password
- [ ] Each accepted connection spawns `tokio::spawn(handle_rcon_client(...))`
- [ ] `command_sender` forwards commands to the main tick thread; output is
      captured from the `RconConsoleSource` equivalent

```rust
// crates/oxidized-server/src/rcon/server.rs

const MAX_RCON_CLIENTS: usize = 4;

pub async fn run_rcon_server(
    port: u16,
    password: Arc<String>,
    command_sender: mpsc::Sender<(String, oneshot::Sender<String>)>,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!("RCON listening on port {port}");
    let active = Arc::new(AtomicUsize::new(0));

    loop {
        let (stream, addr) = listener.accept().await?;
        let count = active.load(Ordering::Relaxed);
        if count >= MAX_RCON_CLIENTS {
            tracing::warn!("RCON: dropping connection from {addr} (max {MAX_RCON_CLIENTS} reached)");
            continue;
        }
        active.fetch_add(1, Ordering::Relaxed);
        let active2 = active.clone();
        let password2 = password.clone();
        let cmd_tx = command_sender.clone();
        tokio::spawn(async move {
            handle_rcon_client(stream, password2, cmd_tx).await;
            active2.fetch_sub(1, Ordering::Relaxed);
        });
    }
}
```

### 33.4 — Query UDP protocol (`oxidized-server/src/query/server.rs`)

The GameSpy4 (GS4) Query protocol uses a challenge/response handshake over UDP.

#### 33.4.1 — Handshake sequence

```
Client → Server: FE FD 09 <session_id: 4 bytes BE i32>
Server → Client: 09 <session_id: 4 bytes BE i32> <challenge_token: null-terminated ASCII string>
```

`challenge_token` is a random i32 formatted as decimal ASCII, e.g. `"12345678\0"`.

#### 33.4.2 — Full stat request/response

```
Client → Server:
  FE FD 00
  <session_id: 4 bytes BE i32>
  <challenge_token: 4 bytes BE i32>
  00 00 00 00          ← padding (full stat request)

Server → Client:
  00
  <session_id: 4 bytes BE i32>
  73 70 6C 69 74 6E 75 6D 00 80 00   ← magic "splitnum\0\x80\0"
  <key=value pairs, each null-terminated>
  00                   ← end of k/v section
  01 70 6C 61 79 65 72 5F 00 00   ← magic "\x01player_\0\0"
  <player names, each null-terminated>
  00                   ← end of player list
```

Required key/value fields (in order):

| Key | Value source |
|-----|-------------|
| `hostname` | `motd` from oxidized.toml (stripped of color codes) |
| `gametype` | `"SMP"` |
| `game_id` | `"MINECRAFT"` |
| `version` | server version string |
| `plugins` | `""` (or mod list if applicable) |
| `map` | name of the default world |
| `numplayers` | current online player count |
| `maxplayers` | `max-players` from oxidized.toml |
| `hostport` | game port (ASCII decimal) |
| `hostip` | bound server IP |

#### 33.4.3 — Challenge token rotation

- [ ] Generate a new `challenge_token` (random `i32`) every 30 seconds
- [ ] Keep previous token valid for 5 minutes total (accept either current or
      previous during the overlap window)
- [ ] Store per-session state keyed by `(remote_addr, session_id)`

```rust
// crates/oxidized-server/src/query/server.rs

#[derive(Default)]
struct QueryState {
    /// current_token and the instant it was generated
    current_token: (i32, Instant),
    /// previous token still valid for up to 5 min
    previous_token: Option<(i32, Instant)>,
}

pub async fn run_query_server(port: u16, stats: Arc<QueryStats>) -> anyhow::Result<()> {
    let socket = UdpSocket::bind(("0.0.0.0", port)).await?;
    tracing::info!("Query listening on port {port}");
    let mut buf = [0u8; 1460];
    let mut sessions: HashMap<(std::net::IpAddr, i32), i32> = HashMap::new();
    let mut state = QueryState::default();
    let mut rotation = tokio::time::interval(Duration::from_secs(30));

    loop {
        tokio::select! {
            _ = rotation.tick() => { rotate_tokens(&mut state); }
            Ok((len, addr)) = socket.recv_from(&mut buf) => {
                handle_query_packet(&socket, addr, &buf[..len],
                                    &mut sessions, &state, &stats).await;
            }
        }
    }
}
```

### 33.5 — `oxidized.toml` configuration

| Property | Default | Notes |
|----------|---------|-------|
| `enable-rcon` | `false` | Start RCON server |
| `rcon.port` | `25575` | TCP port |
| `rcon.password` | `""` | Required to start; error if empty |
| `enable-query` | `false` | Start Query server |
| `query.port` | `25565` | UDP port (may share game port) |

### 33.6 — Console capture (`oxidized-server/src/rcon/console.rs`)

- [ ] Implement `RconConsoleSource`: a command source whose output is buffered
      into a `String` rather than written to the log
- [ ] Commands executed via RCON run on the main tick thread and wait for their
      output via `oneshot::channel`
- [ ] Log each command at `INFO` level: `RCON from <addr>: <command>`

---

## Acceptance Criteria

- [ ] `mcrcon -H localhost -P 25575 -p secret "list"` returns the player list
- [ ] `mcrcon` auth failure returns request_id `-1`
- [ ] Large command output (> 4096 bytes) is split into multiple packets with
      the same request_id
- [ ] After 4 clients the 5th connection is silently dropped (not crashed)
- [ ] `nmap -sU -p 25565 --script minecraft-info` (or equivalent tool) returns
      server info via Query
- [ ] Challenge tokens rotate; stale tokens (> 5 min) are rejected
- [ ] `enable-query=false` or `enable-rcon=false` → corresponding server does
      not start
