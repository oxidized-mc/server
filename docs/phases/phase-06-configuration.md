# Phase 6 — Configuration State

**Crate:** `oxidized-protocol`  
**Reward:** A connected and authenticated client transitions through CONFIGURATION
and reaches PLAY — the final handshake before gameplay.

---

## Goal

Implement the CONFIGURATION protocol state, which sends registry data, feature
flags, and known packs to the client before switching to the PLAY state.

---

## Java Reference

| Concept | Java class |
|---------|-----------|
| Configuration handler | `net.minecraft.server.network.ServerConfigurationPacketListenerImpl` |
| Registry data | `net.minecraft.network.protocol.configuration.ClientboundRegistryDataPacket` |
| Feature flags | `net.minecraft.network.protocol.configuration.ClientboundUpdateEnabledFeaturesPacket` |
| Known packs | `net.minecraft.network.protocol.configuration.ClientboundSelectKnownPacksPacket` |
| Finish config (server) | `net.minecraft.network.protocol.configuration.ClientboundFinishConfigurationPacket` |
| Finish config (client) | `net.minecraft.network.protocol.configuration.ServerboundFinishConfigurationPacket` |
| Cookie store | `net.minecraft.network.protocol.configuration.ClientboundStoreCookiePacket` |
| Cookie response | `net.minecraft.network.protocol.configuration.ServerboundCookieResponsePacket` |

---

## Packet List

### Clientbound Configuration

| ID | Packet | Purpose |
|----|--------|---------|
| 0x00 | `ClientboundCustomPayloadPacket` | Plugin channels |
| 0x01 | `ClientboundDisconnectPacket` | Kick during config |
| 0x02 | `ClientboundFinishConfigurationPacket` | Terminal — signals switch to PLAY |
| 0x03 | `ClientboundKeepAlivePacket` | Connection keepalive |
| 0x04 | `ClientboundPingPacket` | Latency check |
| 0x05 | `ClientboundResetChatPacket` | Clear chat history |
| 0x06 | `ClientboundRegistryDataPacket` | Registry sync |
| 0x07 | `ClientboundRemoveResourcePackPacket` | Remove resource pack |
| 0x08 | `ClientboundAddResourcePackPacket` | Add resource pack |
| 0x09 | `ClientboundStoreCookiePacket` | Store a cookie |
| 0x0A | `ClientboundTransferPacket` | Server transfer (1.20.5+) |
| 0x0B | `ClientboundUpdateEnabledFeaturesPacket` | Feature flags |
| 0x0C | `ClientboundUpdateTagsPacket` | Block/item/fluid tags |
| 0x0D | `ClientboundSelectKnownPacksPacket` | Data pack negotiation |
| 0x0E | `ClientboundCustomReportDetailsPacket` | Crash report metadata |
| 0x0F | `ClientboundServerLinksPacket` | Server links (bug reports, etc.) |

### Serverbound Configuration

| ID | Packet | Purpose |
|----|--------|---------|
| 0x00 | `ServerboundClientInformationPacket` | Client locale, render distance, etc. |
| 0x01 | `ServerboundCookieResponsePacket` | Cookie data |
| 0x02 | `ServerboundCustomPayloadPacket` | Plugin channels |
| 0x03 | `ServerboundFinishConfigurationPacket` | Terminal — client is ready |
| 0x04 | `ServerboundKeepAlivePacket` | Keep alive response |
| 0x05 | `ServerboundPongPacket` | Ping response |
| 0x06 | `ServerboundResourcePackPacket` | Resource pack status |
| 0x07 | `ServerboundSelectKnownPacksPacket` | Known packs response |

---

## Tasks

### 6.1 — Registry Data Packet

The `ClientboundRegistryDataPacket` sends the server's data-driven registry to
the client. This includes biomes, dimensions, damage types, etc.

```rust
pub struct ClientboundRegistryDataPacket {
    pub registry_id: ResourceLocation,   // e.g. "minecraft:worldgen/biome"
    pub entries: Vec<RegistryEntry>,
}

pub struct RegistryEntry {
    pub id: ResourceLocation,
    pub data: Option<NbtCompound>,  // None if using a known pack baseline
}
```

Registries to send:
- `minecraft:dimension_type`
- `minecraft:worldgen/biome`
- `minecraft:chat_type`
- `minecraft:trim_pattern`
- `minecraft:trim_material`
- `minecraft:wolf_variant`
- `minecraft:painting_variant`
- `minecraft:damage_type`
- `minecraft:banner_pattern`
- `minecraft:enchantment`
- `minecraft:jukebox_song`
- `minecraft:instrument`

These are loaded from embedded JSON at compile time (vanilla data extracted from
the server JAR using `java -DbundlerMainClass=net.minecraft.data.Main -jar server.jar
--all`).

### 6.2 — Feature Flags

```rust
pub struct ClientboundUpdateEnabledFeaturesPacket {
    pub features: Vec<ResourceLocation>,
}
```

Default features to send:
- `minecraft:vanilla`

For experimental features (if enabled in server config):
- `minecraft:bundle`
- `minecraft:trade_rebalance`
- `minecraft:winter_drop`

### 6.3 — Known Packs Negotiation

The client and server negotiate which built-in data packs are known
(to avoid resending their registry data in full).

```rust
pub struct ClientboundSelectKnownPacksPacket {
    pub packs: Vec<KnownPack>,
}

pub struct KnownPack {
    pub namespace: String,   // "minecraft"
    pub id: String,          // "core"
    pub version: String,     // "26.1-pre-3"
}
```

Server sends list of known packs → client responds with which ones it knows →
server sends only registry data for packs the client doesn't know.

For simplicity in the initial implementation, always send full registry data.

### 6.4 — Tags Packet

```rust
pub struct ClientboundUpdateTagsPacket {
    pub tags: HashMap<ResourceLocation, Vec<TagData>>,
}

pub struct TagData {
    pub name: ResourceLocation,
    pub entries: Vec<i32>,  // registry element IDs
}
```

Load from `data/minecraft/tags/` JSON files extracted from vanilla.

### 6.5 — Configuration State Machine

```rust
enum ConfigState {
    AwaitingKnownPacks,
    SendingRegistries,
    AwaitingFinish,
    Done,
}

async fn handle_configuration(conn: &mut Connection, server: &Server) {
    // 1. Send feature flags
    // 2. Send ClientboundSelectKnownPacks
    // 3. Wait for ServerboundSelectKnownPacks
    // 4. Send all registry data (ClientboundRegistryDataPacket × N)
    // 5. Send tags
    // 6. Send ClientboundFinishConfiguration
    // 7. Wait for ServerboundFinishConfiguration
    // 8. Transition to PLAY state
}
```

### 6.6 — Client Information

```rust
pub struct ServerboundClientInformationPacket {
    pub language: String,               // e.g. "en_us"
    pub view_distance: u8,             // chunks, 2–32
    pub chat_mode: ChatMode,
    pub chat_colors: bool,
    pub displayed_skin_parts: u8,      // bitmask
    pub main_hand: HumanoidArm,
    pub text_filtering: bool,
    pub allows_listing: bool,
    pub particle_status: ParticleStatus,
}
```

Store per-connection; used for view distance per player (Phase 13).

---

## Embedded Registry Data

Bundle vanilla registry JSON as static bytes:

```rust
// In oxidized-protocol/src/registry/
include!(concat!(env!("OUT_DIR"), "/registry_data.rs"));
// OR use include_str! + lazy_static deserialization
static REGISTRY_DATA: &[u8] = include_bytes!("../../data/registries.nbt");
```

Extract registry data from vanilla server:
```bash
java -DbundlerMainClass=net.minecraft.data.Main \
     -jar mc-server-ref/server.jar --all --output mc-server-ref/data-output/
```

---

## Tests

```rust
#[test]
fn test_configuration_sequence_mock() {
    // Mock connection with pre-queued serverbound packets
    // Verify clientbound sequence matches expected order
}

#[test]
fn test_known_packs_packet_encode() {
    let pkt = ClientboundSelectKnownPacksPacket { ... };
    let bytes = pkt.encode();
    let decoded = ClientboundSelectKnownPacksPacket::decode(&bytes).unwrap();
    assert_eq!(pkt, decoded);
}
```
