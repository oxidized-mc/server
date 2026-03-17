# Phase 3 — Handshake + Status (Server List Ping)

**Status:** ✅ Complete

**Crate:** `oxidized-protocol`  
**Reward:** 🎉 The Oxidized server appears in Minecraft's multiplayer server list
with correct MOTD, player count, and version.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-006: Network I/O](../adr/adr-006-network-io.md) — per-connection task pairs with mpsc channels
- [ADR-007: Packet Codec](../adr/adr-007-packet-codec.md) — #[derive(McPacket)] macro for wire format
- [ADR-008: Connection State Machine](../adr/adr-008-connection-state-machine.md) — typestate pattern for protocol state transitions


## Goal

Implement the HANDSHAKING and STATUS protocol states so that a vanilla 26.1 client
can ping the server and display it in the server list.

---

## Java Reference

| Concept | Java class |
|---------|-----------|
| Handshake packet | `net.minecraft.network.protocol.handshake.ClientIntentionPacket` |
| Client intent | `net.minecraft.network.protocol.handshake.ClientIntent` |
| Status response | `net.minecraft.network.protocol.status.ClientboundStatusResponsePacket` |
| Pong | `net.minecraft.network.protocol.status.ClientboundPongResponsePacket` |
| Server status | `net.minecraft.network.protocol.status.ServerStatus` |
| Handler | `net.minecraft.server.network.ServerHandshakePacketListenerImpl` |
| Status handler | `net.minecraft.server.network.ServerStatusPacketListenerImpl` |

---

## Tasks

### 3.1 — Handshake Packet (`packets/handshake/`)

```rust
/// Serverbound 0x00 — first packet from client
pub struct ClientIntentionPacket {
    pub protocol_version: i32,
    pub server_address: String,   // max 255 chars
    pub server_port: u16,
    pub next_state: ClientIntent,
}

pub enum ClientIntent {
    Status = 1,
    Login  = 2,
    Transfer = 3,  // new in 1.20.5+ (server transfer)
}
```

Decode:
```rust
let protocol_version = buf.read_varint()?;
let server_address   = buf.read_string(255)?;
let server_port      = buf.read_u16()?;
let next_state       = ClientIntent::try_from(buf.read_varint()?)?;
```

### 3.2 — Status Packets

```rust
/// Clientbound 0x00
pub struct ClientboundStatusResponsePacket {
    pub status_json: String,
}

/// Clientbound 0x01
pub struct ClientboundPongResponsePacket {
    pub time: i64,
}

/// Serverbound 0x00
pub struct ServerboundStatusRequestPacket;

/// Serverbound 0x01
pub struct ServerboundPingRequestPacket {
    pub time: i64,
}
```

### 3.3 — ServerStatus JSON builder

```rust
pub struct ServerStatus {
    pub version: StatusVersion,
    pub players: StatusPlayers,
    pub description: Component,
    pub favicon: Option<String>,   // "data:image/png;base64,..."
    pub enforces_secure_chat: bool,
}

pub struct StatusVersion { pub name: String, pub protocol: i32 }
pub struct StatusPlayers { pub max: u32, pub online: u32, pub sample: Vec<PlayerSample> }
pub struct PlayerSample { pub name: String, pub id: Uuid }

impl ServerStatus {
    pub fn to_json(&self) -> String;  // serde_json serialize
}
```

### 3.4 — Favicon loading

- Load `server-icon.png` from server root (must be 64×64 PNG)
- Base64-encode with `base64` crate
- Prepend `data:image/png;base64,`
- Cache on startup; reload on `/reload` (Phase 18)

### 3.5 — Protocol dispatch

Extend `handle_connection` from Phase 2:

```rust
match conn.state {
    ConnectionState::Handshaking => handle_handshake(&mut conn, pkt).await?,
    ConnectionState::Status => handle_status(&mut conn, pkt, &server_status).await?,
    ConnectionState::Login => todo!("Phase 4"),
    _ => {}
}
```

```rust
async fn handle_handshake(conn: &mut Connection, pkt: RawPacket, ...) {
    let intention = ClientIntentionPacket::decode(pkt)?;
    conn.protocol_version = intention.protocol_version;
    conn.state = match intention.next_state {
        ClientIntent::Status => ConnectionState::Status,
        ClientIntent::Login  => ConnectionState::Login,
        ClientIntent::Transfer => ConnectionState::Login,
    };
}

async fn handle_status(conn: &mut Connection, pkt: RawPacket, status: &ServerStatus) {
    match pkt.id {
        0x00 => {   // StatusRequest
            let json = status.to_json();
            conn.send(ClientboundStatusResponsePacket { status_json: json }).await?;
        }
        0x01 => {   // Ping
            let ping = ServerboundPingRequestPacket::decode(pkt)?;
            conn.send(ClientboundPongResponsePacket { time: ping.time }).await?;
            conn.close();
        }
        _ => {}
    }
}
```

### 3.6 — Protocol version mismatch

If `intention.protocol_version != PROTOCOL_VERSION`, still respond with status
(the client will show "Outdated server!" but the server appears in the list).
This is vanilla behaviour.

---

## Component JSON Format

The `description` field uses Minecraft's chat component format:

```json
{ "text": "An Oxidized Server" }
{ "text": "§aGreen §rNormal", "extra": [...] }
{ "translate": "multiplayer.disconnect.server_full" }
```

Implement a minimal `Component` type for Phase 3 (just `TextComponent`);
expand in Phase 17.

---

## Tests

```rust
#[tokio::test]
async fn test_handshake_to_status() {
    // Send ClientIntentionPacket(next_state=Status)
    // Expect connection state changes to Status
}

#[test]
fn test_status_json_serialization() {
    let status = ServerStatus { version: ..., players: ..., description: ... };
    let json: serde_json::Value = serde_json::from_str(&status.to_json()).unwrap();
    assert_eq!(json["version"]["protocol"], 1073742124);
}

#[test]
fn test_protocol_mismatch_still_responds() {
    // Protocol version 999 → still get status JSON back
}
```

---

## Files Created

```
crates/oxidized-protocol/src/packets/
├── mod.rs
├── handshake/
│   ├── mod.rs
│   └── client_intention.rs
└── status/
    ├── mod.rs
    ├── clientbound_status_response.rs
    ├── clientbound_pong_response.rs
    ├── serverbound_status_request.rs
    └── serverbound_ping_request.rs

crates/oxidized-protocol/src/
└── status.rs   ← ServerStatus, StatusVersion, StatusPlayers
```
