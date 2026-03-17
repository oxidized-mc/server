# Protocol Architecture

## Overview

Minecraft 26.1 uses a custom binary TCP protocol. This document describes the
full connection lifecycle, all 5 protocol states, packet format, encryption,
and compression.

**Java reference:** `net.minecraft.network.*`

---

## Wire Format

Every packet is framed with a VarInt length prefix:

```
[Packet Length: VarInt] [Packet Data: bytes]
```

Inside the packet data (before encryption/compression):

```
[Packet ID: VarInt] [Field 1] [Field 2] … [Field N]
```

### With Compression Enabled

```
[Data Length: VarInt] [Compressed Payload | Uncompressed Payload]
```

- If `Data Length == 0`: payload is uncompressed
- If `Data Length > 0`: payload is zlib-compressed; `Data Length` = uncompressed size

### With Encryption Enabled

After the `ClientboundLoginCompressionPacket` (or immediately after key exchange
for servers without compression), all bytes are encrypted with AES-128-CFB8.
The framing is applied _before_ encryption.

---

## VarInt / VarLong

Used everywhere — packet IDs, entity IDs, block state IDs, lengths.

```
Encoding: 7 bits per byte, little-endian, MSB = "more follows"

Examples:
  0      → 0x00
  127    → 0x7F
  128    → 0x80 0x01
  300    → 0xAC 0x02
  -1     → 0xFF 0xFF 0xFF 0xFF 0x0F  (32-bit two's complement as unsigned)

Max size:
  VarInt:  5 bytes  (32-bit value)
  VarLong: 10 bytes (64-bit value)
```

Java reference: `net.minecraft.network.VarInt`, `net.minecraft.network.VarLong`

---

## Protocol States

```
TCP connect
    │
    ▼ HANDSHAKING
[0x00 ClientIntentionPacket] next_state=1 → STATUS
                             next_state=2 → LOGIN
                             next_state=3 → LOGIN (transfer)
    │
    ├──► STATUS ──► ping/pong ──► disconnect
    │
    └──► LOGIN
              │
              ▼ CONFIGURATION  (after LoginFinished/LoginAcknowledged)
              │
              ▼ PLAY           (after FinishConfiguration/Acknowledged)
```

---

## State: HANDSHAKING

### Serverbound

| ID | Packet | Fields |
|----|--------|--------|
| `0x00` | `ClientIntentionPacket` | `protocol_version: VarInt`, `server_address: String(255)`, `server_port: u16`, `next_state: VarInt` |

Java: `net.minecraft.network.protocol.handshake.ClientIntentionPacket`

---

## State: STATUS

### Clientbound

| ID | Packet | Fields |
|----|--------|--------|
| `0x00` | `ClientboundStatusResponsePacket` | `status: String(32767)` (JSON) |
| `0x01` | `ClientboundPongResponsePacket` | `time: i64` |

### Serverbound

| ID | Packet | Fields |
|----|--------|--------|
| `0x00` | `ServerboundStatusRequestPacket` | _(empty)_ |
| `0x01` | `ServerboundPingRequestPacket` | `time: i64` |

### Status JSON Format

```json
{
  "version": { "name": "26.1-pre-3", "protocol": 1073742124 },
  "players": { "max": 20, "online": 3, "sample": [{"name":"Alice","id":"..."}] },
  "description": { "text": "An Oxidized Server" },
  "favicon": "data:image/png;base64,<64x64 PNG>",
  "enforcesSecureChat": false
}
```

Java: `net.minecraft.network.protocol.status.ServerStatus`

---

## State: LOGIN

### Clientbound

| ID | Packet | Fields |
|----|--------|--------|
| `0x00` | `ClientboundLoginDisconnectPacket` | `reason: Component(JSON)` |
| `0x01` | `ClientboundHelloPacket` | `server_id: String`, `public_key: ByteArray`, `challenge: ByteArray`, `should_authenticate: bool` |
| `0x02` | `ClientboundLoginFinishedPacket` | `uuid: Uuid`, `name: String(16)`, `properties: Vec<Property>` _(terminal)_ |
| `0x03` | `ClientboundLoginCompressionPacket` | `compression_threshold: VarInt` |
| `0x04` | `ClientboundCustomQueryPacket` | `message_id: VarInt`, `channel: ResourceLocation`, `data: ByteArray` |

### Serverbound

| ID | Packet | Fields |
|----|--------|--------|
| `0x00` | `ServerboundHelloPacket` | `name: String(16)`, `profile_id: Uuid` |
| `0x01` | `ServerboundKeyPacket` | `key_bytes: ByteArray` (RSA-enc secret), `encrypted_challenge: ByteArray` |
| `0x02` | `ServerboundLoginAcknowledgedPacket` | _(empty)_ _(terminal)_ |
| `0x03` | `ServerboundCustomQueryAnswerPacket` | `message_id: VarInt`, `data: Option<ByteArray>` |
| `0x04` | `ServerboundCookieResponsePacket` | `key: ResourceLocation`, `payload: Option<ByteArray(5120)>` |

### Authentication Flow (Online Mode)

```
Server                              Client
  │── ClientboundHelloPacket ──────────►│
  │   (server_id="", rsa_pubkey,         │
  │    nonce/challenge)                  │
  │                                      │
  │◄─ ServerboundKeyPacket ─────────────│
  │   (RSA_enc(shared_secret),           │
  │    RSA_enc(challenge))               │
  │                                      │
  │  [Server verifies challenge]         │
  │  [Server POSTs to Mojang]            │
  │  GET sessionserver.mojang.com/session/minecraft/hasJoined
  │      ?username=<name>
  │      &serverId=<sha1(serverId+sharedSecret+pubkey)>
  │      [→ 200 OK with profile JSON]    │
  │                                      │
  │  [AES-128-CFB8 enabled both ends]    │
  │  [Compression threshold sent]        │
  │── ClientboundLoginFinishedPacket ──►│
```

Java: `net.minecraft.server.network.ServerLoginPacketListenerImpl`

---

## State: CONFIGURATION

### Clientbound

| ID | Packet | Fields |
|----|--------|--------|
| `0x00` | `ClientboundCookieRequestPacket` | `key: ResourceLocation` |
| `0x01` | `ClientboundCustomPayloadPacket` | `channel: ResourceLocation`, `data: ByteArray` |
| `0x02` | `ClientboundDisconnectPacket` | `reason: Component` |
| `0x03` | `ClientboundFinishConfigurationPacket` | _(empty)_ _(terminal)_ |
| `0x04` | `ClientboundKeepAlivePacket` | `id: i64` |
| `0x05` | `ClientboundPingPacket` | `id: i32` |
| `0x06` | `ClientboundRegistryDataPacket` | `registry_id: ResourceLocation`, `entries: Vec<RegistryEntry>` |
| `0x07` | `ClientboundRemoveResourcePackPacket` | `ids: Vec<Uuid>` |
| `0x08` | `ClientboundResetChatPacket` | _(empty)_ |
| `0x09` | `ClientboundResourcePackPushPacket` | `uuid, url, hash, required, prompt` |
| `0x0A` | `ClientboundSelectKnownPacksPacket` | `known_packs: Vec<KnownPack>` |
| `0x0B` | `ClientboundStoreCookiePacket` | `key: ResourceLocation`, `payload: ByteArray(5120)` |
| `0x0C` | `ClientboundTransferPacket` | `host: String`, `port: VarInt` |
| `0x0D` | `ClientboundUpdateEnabledFeaturesPacket` | `features: Vec<ResourceLocation>` |
| `0x0E` | `ClientboundUpdateTagsPacket` | `tags: Map<ResourceLocation, Map<ResourceLocation, Vec<VarInt>>>` |
| `0x0F` | `ClientboundCodeOfConductPacket` | `requires_acceptance: bool` |

### Serverbound

| ID | Packet | Fields |
|----|--------|--------|
| `0x00` | `ServerboundClientInformationPacket` | `locale`, `view_distance`, `chat_mode`, `chat_colors`, `skin_parts`, `main_hand`, `text_filtering`, `allow_listing` |
| `0x01` | `ServerboundCookieResponsePacket` | `key: ResourceLocation`, `payload: Option<ByteArray>` |
| `0x02` | `ServerboundCustomPayloadPacket` | `channel: ResourceLocation`, `data: ByteArray` |
| `0x03` | `ServerboundFinishConfigurationPacket` | _(empty)_ _(terminal)_ |
| `0x04` | `ServerboundKeepAlivePacket` | `id: i64` |
| `0x05` | `ServerboundPongPacket` | `id: i32` |
| `0x06` | `ServerboundResourcePackPacket` | `uuid: Uuid`, `action: VarInt` |
| `0x07` | `ServerboundSelectKnownPacksPacket` | `known_packs: Vec<KnownPack>` |
| `0x08` | `ServerboundAcceptCodeOfConductPacket` | `accepted: bool` |

---

## State: PLAY

See [reference/protocol-packets.md](../reference/protocol-packets.md) for the full list of
127 clientbound + 58 serverbound play packets.

---

## Encryption Details

**Algorithm:** AES-128-CFB8 (cipher feedback, 8-bit segment)

```
Key size:    128 bits (16 bytes) — the shared secret
IV:          Same 16 bytes as the key (Minecraft quirk)
Direction:   Same key+IV for both enc and dec
When active: All bytes after key exchange, including framing bytes
```

Java reference: `net.minecraft.network.CipherBase`, `CipherEncoder`, `CipherDecoder`

---

## Compression Details

**Algorithm:** Deflate (RFC 1951) via Java `Deflater`/`Inflater`
**Threshold:** Default 256 bytes. Configurable in `oxidized.toml`.

```
Packet format with compression:
  [Total length: VarInt]          ← length of everything below
  [Data length: VarInt]           ← 0 = not compressed, N = original size
  [Payload: bytes]                ← compressed if Data length > 0
```

Limits:
- `MAX_COMPRESSED_LENGTH = 2 097 152` (2 MB)
- `MAX_UNCOMPRESSED_LENGTH = 8 388 608` (8 MB)

Java reference: `net.minecraft.network.CompressionEncoder`, `CompressionDecoder`

---

## Netty Pipeline (Java → Rust Equivalent)

| Java handler | Rust equivalent |
|---|---|
| `ReadTimeoutHandler` | `tokio::time::timeout` on reads |
| `Varint21FrameDecoder` | `FrameDecoder` in `codec/frame.rs` |
| `Varint21LengthFieldPrepender` | `FrameEncoder` in `codec/frame.rs` |
| `CompressionDecoder` | `compress.rs::CompressionDecoder` |
| `CompressionEncoder` | `compress.rs::CompressionEncoder` |
| `CipherDecoder` | `cipher.rs::CipherDecoder` |
| `CipherEncoder` | `cipher.rs::CipherEncoder` |
| `PacketDecoder<T>` | `connection.rs`: typed decode from buffer |
| `PacketEncoder<T>` | `connection.rs`: typed encode to buffer |

The Rust implementation uses a single `tokio::io::AsyncRead`/`AsyncWrite` pair
rather than a Netty pipeline. The codec transformations are applied in sequence
inside the connection read/write loops.

---

## Rust Implementation: Typestate Connection

Per [ADR-008](../adr/adr-008-connection-state-machine.md), connections use the
**typestate pattern** to enforce valid state transitions at compile time:

```rust
pub struct Connection<S: ProtocolState> {
    reader: OwnedReadHalf,
    writer: OwnedWriteHalf,
    compression: Option<CompressionState>,
    cipher: Option<CipherState>,
    _state: PhantomData<S>,
}

// State types
pub struct Handshaking;
pub struct Status;
pub struct Login;
pub struct Configuration;
pub struct Play;

impl Connection<Handshaking> {
    pub fn into_status(self) -> Connection<Status> { ... }
    pub fn into_login(self) -> Connection<Login> { ... }
}

impl Connection<Login> {
    pub fn into_configuration(self) -> Connection<Configuration> { ... }
}

impl Connection<Configuration> {
    pub fn into_play(self) -> Connection<Play> { ... }
}
```

Each state only exposes the packets valid for that state. A `Connection<Login>`
cannot send play packets — the compiler prevents it.

---

## Per-Connection Task Pair

Per [ADR-006](../adr/adr-006-network-io.md), each client connection spawns
**two Tokio tasks** (reader + writer) communicating via bounded `mpsc` channels:

```
┌──────────────────────────────────────────────────┐
│ Per-connection (Tokio tasks)                      │
│                                                    │
│  Reader task                 Writer task           │
│  ┌─────────────┐            ┌──────────────┐      │
│  │ TCP read     │            │ mpsc::recv   │      │
│  │ frame decode │            │ packet encode│      │
│  │ decompress   │──inbound──►│ compress     │      │
│  │ decrypt      │  channel   │ encrypt      │      │
│  │ packet parse │            │ TCP write    │      │
│  └─────────────┘            └──────────────┘      │
│         │                          ▲               │
│         ▼                          │               │
│    inbound mpsc              outbound mpsc         │
└─────────┬──────────────────────────┬───────────────┘
          │                          │
          ▼                          │
   ┌──────────────────────────────────┐
   │       Game tick thread           │
   │  (NETWORK_RECEIVE drains         │
   │   inbound; NETWORK_SEND fills    │
   │   outbound)                      │
   └──────────────────────────────────┘
```

This separates network I/O (async, Tokio) from game logic (synchronous, tick thread).
Bounded channels provide natural backpressure when the game loop is overloaded.

---

## Related ADRs

- [ADR-006: Network I/O](../adr/adr-006-network-io.md) — per-connection task pair
- [ADR-008: Connection State Machine](../adr/adr-008-connection-state-machine.md) — typestate pattern
- [ADR-009: Encryption & Compression](../adr/adr-009-encryption-compression.md) — AES-CFB8, zlib
