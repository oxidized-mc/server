# Phase 12 — Player Join + World Entry

**Crate:** `oxidized-game`  
**Reward:** A connecting player transitions from CONFIGURATION state to PLAY
state, receives the full login packet sequence in the correct order, and appears
in the world at the correct spawn position with their saved data loaded.

---

## Goal

Implement `ServerPlayer`, `PlayerList`, and the complete PLAY-state login
sequence. The player's NBT data is loaded from `playerdata/<uuid>.dat`, their
position/inventory/health are restored, and all required packets are sent before
the initial chunk batch begins.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Server player | `ServerPlayer` | `net.minecraft.server.level.ServerPlayer` |
| Player list | `PlayerList` | `net.minecraft.server.players.PlayerList` |
| Login handler | `ServerGamePacketListenerImpl.handleLogin` | `net.minecraft.server.network.ServerGamePacketListenerImpl` |
| Common spawn info | `CommonPlayerSpawnInfo` | `net.minecraft.network.protocol.game.CommonPlayerSpawnInfo` |
| Login packet | `ClientboundLoginPacket` | `net.minecraft.network.protocol.game.ClientboundLoginPacket` |
| Abilities packet | `ClientboundPlayerAbilitiesPacket` | `net.minecraft.network.protocol.game.ClientboundPlayerAbilitiesPacket` |
| Spawn pos packet | `ClientboundSetDefaultSpawnPositionPacket` | `net.minecraft.network.protocol.game.ClientboundSetDefaultSpawnPositionPacket` |
| Game event packet | `ClientboundGameEventPacket` | `net.minecraft.network.protocol.game.ClientboundGameEventPacket` |
| Player info update | `ClientboundPlayerInfoUpdatePacket` | `net.minecraft.network.protocol.game.ClientboundPlayerInfoUpdatePacket` |
| Chunk cache center | `ClientboundSetChunkCacheCenterPacket` | `net.minecraft.network.protocol.game.ClientboundSetChunkCacheCenterPacket` |
| Simulation dist | `ClientboundSetSimulationDistancePacket` | `net.minecraft.network.protocol.game.ClientboundSetSimulationDistancePacket` |
| Player position | `ClientboundPlayerPositionPacket` | `net.minecraft.network.protocol.game.ClientboundPlayerPositionPacket` |
| Accept teleport | `ServerboundAcceptTeleportationPacket` | `net.minecraft.network.protocol.game.ServerboundAcceptTeleportationPacket` |
| Game type | `GameType` | `net.minecraft.world.level.GameType` |

---

## Tasks

### 12.1 — `GameMode` enum

```rust
// crates/oxidized-game/src/player/game_mode.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum GameMode {
    #[default]
    Survival = 0,
    Creative = 1,
    Adventure = 2,
    Spectator = 3,
}

impl GameMode {
    pub fn from_id(id: i32) -> Self {
        match id {
            0 => Self::Survival,
            1 => Self::Creative,
            2 => Self::Adventure,
            3 => Self::Spectator,
            _ => Self::Survival,
        }
    }

    pub fn is_creative_or_spectator(self) -> bool {
        matches!(self, Self::Creative | Self::Spectator)
    }

    pub fn allow_flight(self) -> bool {
        matches!(self, Self::Creative | Self::Spectator)
    }
}
```

### 12.2 — `PlayerAbilities`

```rust
// crates/oxidized-game/src/player/abilities.rs

/// Mirrors `net.minecraft.world.entity.player.Abilities`.
#[derive(Debug, Clone)]
pub struct PlayerAbilities {
    pub invulnerable: bool,
    pub flying: bool,
    pub can_fly: bool,
    pub instabuild: bool, // creative
    /// Blocks per tick. Default 0.05.
    pub fly_speed: f32,
    /// Blocks per tick. Default 0.1.
    pub walk_speed: f32,
}

impl Default for PlayerAbilities {
    fn default() -> Self {
        Self {
            invulnerable: false,
            flying: false,
            can_fly: false,
            instabuild: false,
            fly_speed: 0.05,
            walk_speed: 0.1,
        }
    }
}

impl PlayerAbilities {
    pub fn for_game_mode(mode: GameMode) -> Self {
        let mut a = Self::default();
        match mode {
            GameMode::Creative => {
                a.invulnerable = true;
                a.can_fly = true;
                a.instabuild = true;
            }
            GameMode::Spectator => {
                a.invulnerable = true;
                a.can_fly = true;
                a.flying = true;
            }
            _ => {}
        }
        a
    }
}
```

### 12.3 — `ServerPlayer`

```rust
// crates/oxidized-game/src/player/server_player.rs

use std::sync::Arc;
use uuid::Uuid;
use oxidized_world::block::BlockPos;
use oxidized_protocol::types::{Vec3, GameProfile};

pub struct ServerPlayer {
    pub entity_id: i32,
    pub uuid: Uuid,
    pub name: String,
    pub profile: GameProfile,

    // World state
    pub pos: Vec3,
    pub yaw: f32,   // y-rotation (horizontal)
    pub pitch: f32, // x-rotation (vertical)
    pub on_ground: bool,

    // Game state
    pub game_mode: GameMode,
    pub previous_game_mode: Option<GameMode>,
    pub abilities: PlayerAbilities,
    pub food_level: i32,       // 0–20
    pub food_saturation: f32,
    pub health: f32,           // 0.0–20.0
    pub max_health: f32,

    // Inventory: 46 slots (0–8 hotbar, 9–35 main, 36–39 armour, 40 offhand, 45 crafting output)
    pub inventory: PlayerInventory,

    // Connection context
    pub view_distance: i32,
    pub simulation_distance: i32,

    // Teleport confirmation state
    /// Pending teleport IDs that the client has not yet confirmed.
    pub pending_teleports: std::collections::VecDeque<i32>,
    teleport_id_counter: i32,

    pub dimension: ResourceLocation,
    pub spawn_pos: BlockPos,
    pub spawn_angle: f32,
}

impl ServerPlayer {
    pub fn new(
        entity_id: i32,
        profile: GameProfile,
        dimension: ResourceLocation,
        game_mode: GameMode,
    ) -> Self {
        let abilities = PlayerAbilities::for_game_mode(game_mode);
        Self {
            entity_id,
            uuid: profile.uuid,
            name: profile.name.clone(),
            profile,
            pos: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
            game_mode,
            previous_game_mode: None,
            abilities,
            food_level: 20,
            food_saturation: 5.0,
            health: 20.0,
            max_health: 20.0,
            inventory: PlayerInventory::new(),
            view_distance: 10,
            simulation_distance: 10,
            pending_teleports: Default::default(),
            teleport_id_counter: 0,
            dimension,
            spawn_pos: BlockPos::new(0, 64, 0),
            spawn_angle: 0.0,
        }
    }

    pub fn next_teleport_id(&mut self) -> i32 {
        self.teleport_id_counter = self.teleport_id_counter.wrapping_add(1);
        self.teleport_id_counter
    }

    pub fn chunk_x(&self) -> i32 { (self.pos.x as i32) >> 4 }
    pub fn chunk_z(&self) -> i32 { (self.pos.z as i32) >> 4 }

    /// Load player data from `playerdata/<uuid>.dat` NBT.
    pub fn load_from_nbt(&mut self, nbt: &NbtCompound) -> anyhow::Result<()> {
        if let Some(pos_list) = nbt.get_list("Pos") {
            self.pos.x = pos_list[0].as_f64() as f32;
            self.pos.y = pos_list[1].as_f64() as f32;
            self.pos.z = pos_list[2].as_f64() as f32;
        }
        if let Some(rot_list) = nbt.get_list("Rotation") {
            self.yaw   = rot_list[0].as_f32();
            self.pitch = rot_list[1].as_f32();
        }
        self.health = nbt.get_float("Health").unwrap_or(20.0);
        self.food_level = nbt.get_int("foodLevel").unwrap_or(20);
        self.food_saturation = nbt.get_float("foodSaturationLevel").unwrap_or(5.0);
        let gm = nbt.get_int("playerGameType").unwrap_or(0);
        self.game_mode = GameMode::from_id(gm);
        self.abilities = PlayerAbilities::for_game_mode(self.game_mode);
        // TODO: deserialize inventory
        Ok(())
    }
}
```

### 12.4 — Player data NBT schema

Player data is stored as a GZip-compressed NBT compound in
`<world>/playerdata/<uuid>.dat`:

```
Root Compound:
  DataVersion: Int
  Pos: List<Double> [x, y, z]
  Rotation: List<Float> [yaw, pitch]
  Motion: List<Double> [vx, vy, vz]
  Health: Float
  FoodLevel: Int
  foodSaturationLevel: Float
  XpLevel: Int
  XpP: Float  (progress 0.0–1.0)
  Score: Int
  playerGameType: Int
  Inventory: List<Compound>
    each item:
      Slot: Byte
      id: String
      Count: Byte
      tag: Compound (optional)
  SpawnX: Int (optional)
  SpawnY: Int (optional)
  SpawnZ: Int (optional)
  SpawnForced: Byte (optional)
  Dimension: String
  SeenCredits: Byte
```

### 12.5 — `PlayerList`

```rust
// crates/oxidized-game/src/player/player_list.rs

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

pub struct PlayerList {
    players: HashMap<Uuid, Arc<RwLock<ServerPlayer>>>,
    /// Order preserved for tab-list purposes.
    ordered: Vec<Uuid>,
    max_players: usize,
    entity_id_counter: std::sync::atomic::AtomicI32,
}

impl PlayerList {
    pub fn new(max_players: usize) -> Self {
        Self {
            players: HashMap::new(),
            ordered: Vec::new(),
            max_players,
            entity_id_counter: std::sync::atomic::AtomicI32::new(1),
        }
    }

    pub fn next_entity_id(&self) -> i32 {
        self.entity_id_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    pub fn player_count(&self) -> usize { self.players.len() }
    pub fn max_players(&self) -> usize { self.max_players }
    pub fn is_full(&self) -> bool { self.player_count() >= self.max_players }

    pub fn add(&mut self, player: ServerPlayer) -> Arc<RwLock<ServerPlayer>> {
        let uuid = player.uuid;
        let arc = Arc::new(RwLock::new(player));
        self.players.insert(uuid, Arc::clone(&arc));
        self.ordered.push(uuid);
        arc
    }

    pub fn remove(&mut self, uuid: &Uuid) -> Option<Arc<RwLock<ServerPlayer>>> {
        self.ordered.retain(|u| u != uuid);
        self.players.remove(uuid)
    }

    pub fn get(&self, uuid: &Uuid) -> Option<Arc<RwLock<ServerPlayer>>> {
        self.players.get(uuid).map(Arc::clone)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<RwLock<ServerPlayer>>> {
        self.ordered.iter().filter_map(|u| self.players.get(u))
    }
}
```

### 12.6 — Full login packet sequence

This is the exact order mandated by `ServerGamePacketListenerImpl.handleLogin`:

```rust
// crates/oxidized-game/src/player/login.rs

use oxidized_protocol::packets::game::clientbound::*;

/// Send the complete PLAY-state login sequence to a newly joined player.
/// Called once the player has transitioned from CONFIGURATION → PLAY.
pub async fn send_login_sequence(
    conn: &mut PlayerConnection,
    player: &ServerPlayer,
    level_data: &PrimaryLevelData,
    all_players: &PlayerList,
    dimensions: &[ResourceLocation],
) -> anyhow::Result<()> {
    // 1. ClientboundLoginPacket (0x2B)
    conn.send(ClientboundLoginPacket {
        player_id: player.entity_id,
        hardcore: level_data.hardcore,
        dimensions: dimensions.to_vec(),
        max_players: all_players.max_players() as i32,
        chunk_radius: player.view_distance,
        simulation_distance: player.simulation_distance,
        reduced_debug_info: false,
        show_death_screen: true,
        do_limited_crafting: false,
        common_spawn_info: CommonPlayerSpawnInfo {
            dimension_type: player.dimension.clone(),
            dimension: player.dimension.clone(),
            seed: 0, // hashed seed
            game_mode: player.game_mode as u8,
            previous_game_mode: -1i8, // -1 = none
            is_debug: false,
            is_flat: false,
            last_death_location: None,
            portal_cooldown: 0,
            sea_level: level_data.sea_level,
        },
        enforces_secure_chat: false,
    }).await?;

    // 2. ClientboundPlayerAbilitiesPacket
    conn.send(ClientboundPlayerAbilitiesPacket {
        flags: {
            let mut f = 0u8;
            if player.abilities.invulnerable { f |= 0x01; }
            if player.abilities.flying       { f |= 0x02; }
            if player.abilities.can_fly      { f |= 0x04; }
            if player.abilities.instabuild   { f |= 0x08; }
            f
        },
        fly_speed:  player.abilities.fly_speed,
        walk_speed: player.abilities.walk_speed,
    }).await?;

    // 3. ClientboundSetDefaultSpawnPositionPacket
    conn.send(ClientboundSetDefaultSpawnPositionPacket {
        pos: level_data.spawn_pos(),
        angle: level_data.spawn_angle,
    }).await?;

    // 4. ClientboundGameEventPacket: CHANGE_GAME_MODE (type=3)
    conn.send(ClientboundGameEventPacket {
        event: GameEvent::ChangeGameMode,
        value: player.game_mode as f32,
    }).await?;

    // 5. ClientboundPlayerInfoUpdatePacket — ADD_PLAYER for all online players,
    //    then INITIALIZE_CHAT, UPDATE_GAME_MODE, UPDATE_LISTED, UPDATE_LATENCY.
    //    The new player is included in the list so others see them.
    let info_entries: Vec<PlayerInfoEntry> = all_players.iter()
        .map(|p| {
            let p = p.read().unwrap();
            PlayerInfoEntry {
                uuid: p.uuid,
                name: p.name.clone(),
                properties: p.profile.properties.clone(),
                game_mode: p.game_mode as i32,
                latency: 0,
                display_name: None,
                listed: true,
            }
        })
        .collect();
    conn.send(ClientboundPlayerInfoUpdatePacket {
        actions: PlayerInfoActions::ADD_PLAYER
            | PlayerInfoActions::INITIALIZE_CHAT
            | PlayerInfoActions::UPDATE_GAME_MODE
            | PlayerInfoActions::UPDATE_LISTED
            | PlayerInfoActions::UPDATE_LATENCY,
        entries: info_entries,
    }).await?;

    // 6. ClientboundSetChunkCacheCenterPacket
    conn.send(ClientboundSetChunkCacheCenterPacket {
        chunk_x: player.chunk_x(),
        chunk_z: player.chunk_z(),
    }).await?;

    // 7. ClientboundSetSimulationDistancePacket
    conn.send(ClientboundSetSimulationDistancePacket {
        simulation_distance: player.simulation_distance,
    }).await?;

    // 8. Initial chunks are sent here by the chunk-sending subsystem (Phase 13).

    // 9. ClientboundPlayerPositionPacket (initial position + teleport_id)
    let teleport_id = {
        // Requires mutable borrow; caller manages player mutability.
        0 // placeholder — assign real ID in caller
    };
    conn.send(ClientboundPlayerPositionPacket {
        teleport_id,
        x: player.pos.x as f64,
        y: player.pos.y as f64,
        z: player.pos.z as f64,
        vx: 0.0,
        vy: 0.0,
        vz: 0.0,
        yaw: player.yaw,
        pitch: player.pitch,
        relative_flags: RelativeFlags::empty(),
    }).await?;

    Ok(())
}
```

### 12.7 — `ServerboundAcceptTeleportationPacket` handler

```rust
// crates/oxidized-game/src/player/login.rs (continued)

/// Called when the client sends ServerboundAcceptTeleportationPacket.
/// The player is not considered "fully in world" until the first teleport is acked.
pub fn handle_accept_teleportation(
    player: &mut ServerPlayer,
    teleport_id: i32,
) {
    if let Some(pos) = player.pending_teleports.front().copied() {
        if pos == teleport_id {
            player.pending_teleports.pop_front();
        }
    }
    // If the pending queue is empty, the player has confirmed their initial position.
}
```

---

## Data Structures Summary

```
oxidized-game::player
  ├── GameMode              — Survival/Creative/Adventure/Spectator
  ├── PlayerAbilities       — fly_speed, walk_speed, can_fly, invulnerable…
  ├── ServerPlayer          — full player state: pos, rot, health, inventory…
  ├── PlayerList            — HashMap<Uuid, Arc<RwLock<ServerPlayer>>>
  └── login::send_login_sequence — ordered packet sender
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Login packet field round-trip through encode → decode.
    #[test]
    fn login_packet_serialization() {
        let pkt = ClientboundLoginPacket {
            player_id: 42,
            hardcore: false,
            dimensions: vec![
                ResourceLocation::minecraft("overworld"),
                ResourceLocation::minecraft("the_nether"),
                ResourceLocation::minecraft("the_end"),
            ],
            max_players: 20,
            chunk_radius: 10,
            simulation_distance: 10,
            reduced_debug_info: false,
            show_death_screen: true,
            do_limited_crafting: false,
            common_spawn_info: CommonPlayerSpawnInfo {
                dimension_type: ResourceLocation::minecraft("overworld"),
                dimension: ResourceLocation::minecraft("overworld"),
                seed: 0,
                game_mode: 0,
                previous_game_mode: -1,
                is_debug: false,
                is_flat: false,
                last_death_location: None,
                portal_cooldown: 0,
                sea_level: 63,
            },
            enforces_secure_chat: false,
        };

        let mut buf = Vec::new();
        pkt.encode(&mut buf);
        let decoded = ClientboundLoginPacket::decode(&mut buf.as_slice()).unwrap();

        assert_eq!(decoded.player_id, 42);
        assert_eq!(decoded.chunk_radius, 10);
        assert_eq!(decoded.dimensions.len(), 3);
        assert!(!decoded.hardcore);
    }

    /// Player NBT schema: Pos list with 3 doubles, Rotation with 2 floats.
    #[test]
    fn player_nbt_load_pos_rotation() {
        let mut nbt = NbtCompound::new();
        nbt.put_list_double("Pos", vec![10.5, 64.0, -20.5]);
        nbt.put_list_float("Rotation", vec![90.0f32, 15.0f32]);
        nbt.put_float("Health", 18.0);
        nbt.put_int("foodLevel", 17);
        nbt.put_int("playerGameType", 1);

        let profile = GameProfile::new(Uuid::nil(), "TestPlayer".into());
        let mut player = ServerPlayer::new(
            1, profile,
            ResourceLocation::minecraft("overworld"),
            GameMode::Survival,
        );
        player.load_from_nbt(&nbt).unwrap();

        assert!((player.pos.x - 10.5).abs() < 0.001);
        assert!((player.pos.y - 64.0).abs() < 0.001);
        assert!((player.pos.z + 20.5).abs() < 0.001);
        assert!((player.yaw - 90.0).abs() < 0.001);
        assert_eq!(player.health as i32, 18);
        assert_eq!(player.food_level, 17);
        assert_eq!(player.game_mode, GameMode::Creative);
    }

    /// PlayerList add/remove and max_players enforcement.
    #[test]
    fn player_list_add_remove() {
        let mut list = PlayerList::new(2);
        let p1 = make_test_player(1, "Alice");
        let p2 = make_test_player(2, "Bob");
        let uuid1 = p1.uuid;
        let uuid2 = p2.uuid;

        list.add(p1);
        list.add(p2);
        assert_eq!(list.player_count(), 2);
        assert!(list.is_full());

        list.remove(&uuid1);
        assert_eq!(list.player_count(), 1);
        assert!(!list.is_full());
        assert!(list.get(&uuid2).is_some());
    }

    fn make_test_player(id: i32, name: &str) -> ServerPlayer {
        let uuid = Uuid::new_v4();
        let profile = GameProfile::new(uuid, name.into());
        ServerPlayer::new(id, profile, ResourceLocation::minecraft("overworld"), GameMode::Survival)
    }
}
```
