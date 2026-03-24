# Phase 14 — Player Movement

**Status:** ✅ Complete  
**Crate:** `oxidized-game`  
**Reward:** A player can walk, run, jump, and sneak. The server tracks their
position correctly, broadcasts movement to nearby players, loads and unloads
chunks as the player moves, and corrects invalid movement with a server-side
teleport.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-006: Network I/O](../adr/adr-006-network-io.md) — per-connection task pairs with mpsc channels
- [ADR-014: Chunk Storage](../adr/adr-014-chunk-storage.md) — DashMap + per-section RwLock for concurrent access
- [ADR-020: Player Session](../adr/adr-020-player-session.md) — split network actor + ECS entity architecture


## Goal

Handle the four serverbound movement packets (`Pos`, `Rot`, `PosRot`,
`StatusOnly`), validate each update, broadcast delta-encoded movement to
watching players, and maintain chunk tracking as the player moves across chunk
boundaries. Handle sprint/sneak state from `ServerboundPlayerCommandPacket`.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Move handler | `ServerGamePacketListenerImpl.handleMovePlayer` | `net.minecraft.server.network.ServerGamePacketListenerImpl` |
| Server-side move | `ServerPlayer.absMoveTo` | `net.minecraft.server.level.ServerPlayer` |
| Chunk tracking | `ChunkMap.move` | `net.minecraft.server.level.ChunkMap` |
| Move entity packet | `ClientboundMoveEntityPacket` | `net.minecraft.network.protocol.game.ClientboundMoveEntityPacket` |
| Entity sync packet | `ClientboundEntityPositionSyncPacket` | `net.minecraft.network.protocol.game.ClientboundEntityPositionSyncPacket` |
| Rotate head | `ClientboundRotateHeadPacket` | `net.minecraft.network.protocol.game.ClientboundRotateHeadPacket` |
| Set motion | `ClientboundSetEntityMotionPacket` | `net.minecraft.network.protocol.game.ClientboundSetEntityMotionPacket` |
| Player pos packet | `ClientboundPlayerPositionPacket` | `net.minecraft.network.protocol.game.ClientboundPlayerPositionPacket` |
| Player command | `ServerboundPlayerCommandPacket` | `net.minecraft.network.protocol.game.ServerboundPlayerCommandPacket` |
| Relative flags | `Relative` | `net.minecraft.world.entity.Relative` |

---

## Tasks

### 14.1 — Serverbound movement packet types

```rust
// crates/oxidized-game/src/net/packets/serverbound.rs

/// Sent every tick the client moves. `has_pos` and `has_rot` indicate which
/// fields are present. This models the four concrete Java inner classes:
/// `Pos`, `Rot`, `PosRot`, `StatusOnly`.
#[derive(Debug, Clone)]
pub struct ServerboundMovePlayerPacket {
    pub x: Option<f64>,       // present when has_pos
    pub y: Option<f64>,
    pub z: Option<f64>,
    pub yaw: Option<f32>,     // present when has_rot
    pub pitch: Option<f32>,
    pub on_ground: bool,
    pub horizontal_collision: bool,
}

impl ServerboundMovePlayerPacket {
    pub fn decode_pos(buf: &mut &[u8]) -> anyhow::Result<Self> {
        let x = read_f64(buf)?;
        let y = read_f64(buf)?;
        let z = read_f64(buf)?;
        let flags = read_u8(buf)?;
        Ok(Self {
            x: Some(x), y: Some(y), z: Some(z),
            yaw: None, pitch: None,
            on_ground: flags & 0x01 != 0,
            horizontal_collision: flags & 0x02 != 0,
        })
    }

    pub fn decode_rot(buf: &mut &[u8]) -> anyhow::Result<Self> {
        let yaw = read_f32(buf)?;
        let pitch = read_f32(buf)?;
        let flags = read_u8(buf)?;
        Ok(Self {
            x: None, y: None, z: None,
            yaw: Some(yaw), pitch: Some(pitch),
            on_ground: flags & 0x01 != 0,
            horizontal_collision: flags & 0x02 != 0,
        })
    }

    pub fn decode_pos_rot(buf: &mut &[u8]) -> anyhow::Result<Self> {
        let x = read_f64(buf)?;
        let y = read_f64(buf)?;
        let z = read_f64(buf)?;
        let yaw = read_f32(buf)?;
        let pitch = read_f32(buf)?;
        let flags = read_u8(buf)?;
        Ok(Self {
            x: Some(x), y: Some(y), z: Some(z),
            yaw: Some(yaw), pitch: Some(pitch),
            on_ground: flags & 0x01 != 0,
            horizontal_collision: flags & 0x02 != 0,
        })
    }

    pub fn decode_status_only(buf: &mut &[u8]) -> anyhow::Result<Self> {
        let flags = read_u8(buf)?;
        Ok(Self {
            x: None, y: None, z: None,
            yaw: None, pitch: None,
            on_ground: flags & 0x01 != 0,
            horizontal_collision: flags & 0x02 != 0,
        })
    }
}
```

### 14.2 — Server-side movement validation

```rust
// crates/oxidized-game/src/player/movement.rs

/// Maximum distance a player may travel in a single tick (100 m).
/// Exceeding this triggers a server correction.
pub const MAX_MOVEMENT_PER_TICK: f64 = 100.0;

pub struct MovementResult {
    pub accepted: bool,
    /// If false, send a ClientboundPlayerPositionPacket correction.
    pub needs_correction: bool,
    pub new_pos: Vec3,
    pub new_yaw: f32,
    pub new_pitch: f32,
}

pub fn validate_movement(
    player: &ServerPlayer,
    packet: &ServerboundMovePlayerPacket,
) -> MovementResult {
    // If client reported no position, retain current.
    let new_pos = Vec3 {
        x: packet.x.unwrap_or(player.pos.x as f64) as f32,
        y: packet.y.unwrap_or(player.pos.y as f64) as f32,
        z: packet.z.unwrap_or(player.pos.z as f64) as f32,
    };
    let new_yaw   = packet.yaw.unwrap_or(player.yaw);
    let new_pitch = packet.pitch.unwrap_or(player.pitch);

    let dx = (new_pos.x - player.pos.x) as f64;
    let dy = (new_pos.y - player.pos.y) as f64;
    let dz = (new_pos.z - player.pos.z) as f64;
    let dist_sq = dx * dx + dy * dy + dz * dz;

    // Flag as invalid if movement exceeds limit.
    let needs_correction = dist_sq > MAX_MOVEMENT_PER_TICK * MAX_MOVEMENT_PER_TICK;

    MovementResult {
        accepted: !needs_correction,
        needs_correction,
        new_pos,
        new_yaw,
        new_pitch,
    }
}
```

### 14.3 — Delta encoding for `ClientboundMoveEntityPacket`

```
// Formula (from net.minecraft.network.protocol.game.ClientboundMoveEntityPacket):
// xa = (i16)(new_x * 32 - old_x * 32) * 128 = (i16)((new_x - old_x) * 4096.0)
// Max representable delta: 32767 / 4096 ≈ 7.999 blocks
// If |delta| > 8 blocks, use ClientboundEntityPositionSyncPacket instead.
```

```rust
// crates/oxidized-game/src/net/entity_movement.rs

pub const DELTA_SCALE: f64 = 4096.0;
pub const MAX_DELTA_BLOCKS: f64 = 7.999; // ~32767/4096

/// Encode a position delta as a short.
/// Returns None if the delta is too large for a short encoding.
pub fn encode_delta(old: f64, new: f64) -> Option<i16> {
    let raw = (new * DELTA_SCALE) as i64 - (old * DELTA_SCALE) as i64;
    if raw < i16::MIN as i64 || raw > i16::MAX as i64 {
        None
    } else {
        Some(raw as i16)
    }
}

/// Pack a rotation angle (degrees) into a byte.
/// 0–255 maps to 0–360°; same as Mth.packDegrees.
pub fn pack_degrees(angle: f32) -> u8 {
    ((angle * 256.0 / 360.0) as i32 & 0xFF) as u8
}

pub fn unpack_degrees(byte: u8) -> f32 {
    byte as f32 * 360.0 / 256.0
}

pub enum EntityMovePacket {
    /// Small delta — use delta-encoded shorts.
    Delta {
        entity_id: i32,
        dx: i16, dy: i16, dz: i16,
        yaw: Option<u8>,
        pitch: Option<u8>,
        on_ground: bool,
    },
    /// Large teleport (> 8 blocks) — use absolute coordinates.
    Sync {
        entity_id: i32,
        x: f64, y: f64, z: f64,
        vx: i16, vy: i16, vz: i16,
        yaw: u8, pitch: u8,
        on_ground: bool,
    },
}

pub fn make_entity_move_packet(
    entity_id: i32,
    old_pos: Vec3,
    new_pos: Vec3,
    old_yaw: f32,
    new_yaw: f32,
    new_pitch: f32,
    on_ground: bool,
    include_rotation: bool,
) -> EntityMovePacket {
    let dx = encode_delta(old_pos.x as f64, new_pos.x as f64);
    let dy = encode_delta(old_pos.y as f64, new_pos.y as f64);
    let dz = encode_delta(old_pos.z as f64, new_pos.z as f64);

    match (dx, dy, dz) {
        (Some(dx), Some(dy), Some(dz)) => EntityMovePacket::Delta {
            entity_id,
            dx, dy, dz,
            yaw: if include_rotation { Some(pack_degrees(new_yaw)) } else { None },
            pitch: if include_rotation { Some(pack_degrees(new_pitch)) } else { None },
            on_ground,
        },
        _ => EntityMovePacket::Sync {
            entity_id,
            x: new_pos.x as f64,
            y: new_pos.y as f64,
            z: new_pos.z as f64,
            vx: 0, vy: 0, vz: 0,
            yaw: pack_degrees(new_yaw),
            pitch: pack_degrees(new_pitch),
            on_ground,
        },
    }
}
```

### 14.4 — `RelativeFlags` for position corrections

```rust
// crates/oxidized-game/src/player/teleport.rs

bitflags::bitflags! {
    /// Mirrors `net.minecraft.world.entity.Relative`.
    #[derive(Debug, Clone, Copy)]
    pub struct RelativeFlags: u32 {
        const X     = 0x0001;
        const Y     = 0x0002;
        const Z     = 0x0004;
        const YAW   = 0x0008;
        const PITCH = 0x0010;
    }
}

pub struct PendingTeleport {
    pub id: i32,
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

/// Send an absolute position correction to the player.
pub async fn send_position_correction(
    conn: &mut PlayerConnection,
    player: &mut ServerPlayer,
    pos: Vec3,
    yaw: f32,
    pitch: f32,
) -> anyhow::Result<()> {
    let tid = player.next_teleport_id();
    player.pending_teleports.push_back(tid);

    conn.send(ClientboundPlayerPositionPacket {
        teleport_id: tid,
        x: pos.x as f64,
        y: pos.y as f64,
        z: pos.z as f64,
        vx: 0.0, vy: 0.0, vz: 0.0,
        yaw,
        pitch,
        relative_flags: RelativeFlags::empty(),
    }).await
}
```

### 14.5 — Sprint/sneak state via `ServerboundPlayerCommandPacket`

```rust
// crates/oxidized-game/src/player/movement.rs (continued)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerCommandAction {
    PressShiftKey = 0,
    ReleaseShiftKey = 1,
    StopSleeping = 2,
    StartSprinting = 3,
    StopSprinting = 4,
    StartRidingJump = 5,
    StopRidingJump = 6,
    OpenInventory = 7,
    StartFallFlying = 8,
}

pub fn handle_player_command(
    player: &mut ServerPlayer,
    action: PlayerCommandAction,
    _data: i32,
) {
    match action {
        PlayerCommandAction::PressShiftKey   => player.sneaking = true,
        PlayerCommandAction::ReleaseShiftKey => player.sneaking = false,
        PlayerCommandAction::StartSprinting  => player.sprinting = true,
        PlayerCommandAction::StopSprinting   => player.sprinting = false,
        _ => {}
    }
}
```

### 14.6 — Chunk tracking on move

```rust
// crates/oxidized-game/src/chunk/chunk_tracker.rs

use oxidized_world::chunk::ChunkPos;

/// Maintains the set of chunk positions sent to a specific player.
pub struct PlayerChunkTracker {
    pub center: ChunkPos,
    pub view_distance: i32,
    /// All chunks currently loaded by this player.
    pub loaded: std::collections::HashSet<ChunkPos>,
}

impl PlayerChunkTracker {
    pub fn new(center: ChunkPos, view_distance: i32) -> Self {
        Self {
            center,
            view_distance,
            loaded: std::collections::HashSet::new(),
        }
    }

    /// Called when the player moves to a new chunk center.
    /// Returns (chunks to load, chunks to unload).
    pub fn update_center(
        &mut self,
        new_center: ChunkPos,
    ) -> (Vec<ChunkPos>, Vec<ChunkPos>) {
        if new_center == self.center {
            return (vec![], vec![]);
        }
        let to_load = chunks_to_load(self.center, new_center, self.view_distance);
        let to_unload = chunks_to_unload(self.center, new_center, self.view_distance);
        self.center = new_center;
        for p in &to_load   { self.loaded.insert(*p); }
        for p in &to_unload { self.loaded.remove(p); }
        (to_load, to_unload)
    }
}
```

---

## Data Structures Summary

```
oxidized-game::player
  ├── ServerboundMovePlayerPacket — decoded pos/rot/pos_rot/status_only
  ├── MovementResult              — validated output of validate_movement
  ├── PlayerCommandAction         — sneak/sprint/sleep/etc.
  └── PlayerChunkTracker          — set of loaded chunk positions per player

oxidized-game::net
  ├── encode_delta(old, new)      — f64 → i16 or None
  ├── pack_degrees(angle)         — f32 → u8
  ├── EntityMovePacket            — Delta or Sync variant
  └── RelativeFlags               — bitmask for ClientboundPlayerPositionPacket
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Delta encoding: 1.0 block → 4096 as i16.
    #[test]
    fn delta_encoding_one_block() {
        assert_eq!(encode_delta(0.0, 1.0), Some(4096));
        assert_eq!(encode_delta(0.0, -1.0), Some(-4096));
    }

    /// Delta encoding: values > 8 blocks return None.
    #[test]
    fn delta_encoding_too_large_returns_none() {
        assert_eq!(encode_delta(0.0, 8.001), None);
        assert_eq!(encode_delta(100.0, 108.5), None);
    }

    /// Delta encoding: exactly 7.999 blocks fits in an i16.
    #[test]
    fn delta_encoding_max_valid() {
        let result = encode_delta(0.0, 7.999);
        assert!(result.is_some());
        let val = result.unwrap() as i64;
        assert!(val <= i16::MAX as i64, "delta {val} exceeds i16::MAX");
    }

    /// pack_degrees: 0° → 0, 180° → 128, 360° → 0 (wraps).
    #[test]
    fn pack_degrees_values() {
        assert_eq!(pack_degrees(0.0), 0);
        assert_eq!(pack_degrees(180.0), 128);
        // 360° wraps back to 0.
        assert_eq!(pack_degrees(360.0), 0);
    }

    /// Movement exceeding MAX_MOVEMENT_PER_TICK triggers correction flag.
    #[test]
    fn validate_movement_too_fast() {
        let mut player = make_test_player_at(Vec3::ZERO);
        let pkt = ServerboundMovePlayerPacket {
            x: Some(200.0), y: Some(0.0), z: Some(0.0),
            yaw: None, pitch: None,
            on_ground: false, horizontal_collision: false,
        };
        let result = validate_movement(&player, &pkt);
        assert!(result.needs_correction);
        assert!(!result.accepted);
    }

    /// Normal 1-block movement is accepted without correction.
    #[test]
    fn validate_movement_normal_step() {
        let player = make_test_player_at(Vec3::ZERO);
        let pkt = ServerboundMovePlayerPacket {
            x: Some(0.1), y: Some(0.0), z: Some(0.0),
            yaw: None, pitch: None,
            on_ground: true, horizontal_collision: false,
        };
        let result = validate_movement(&player, &pkt);
        assert!(!result.needs_correction);
        assert!(result.accepted);
    }

    /// chunk_tracker.update_center yields correct load/unload sets.
    #[test]
    fn chunk_tracker_update_center() {
        let mut tracker = PlayerChunkTracker::new(ChunkPos::new(0, 0), 2);
        // Pre-populate loaded with all chunks in view distance 2.
        for pos in spiral_chunks(ChunkPos::new(0, 0), 2) {
            tracker.loaded.insert(pos);
        }

        let (to_load, to_unload) = tracker.update_center(ChunkPos::new(1, 0));
        // Moving +1 in X: new column at x=3 loads, old column at x=-2 unloads.
        assert!(to_load.iter().all(|p| p.x == 3),
            "load column should be x=3: {to_load:?}");
        assert!(to_unload.iter().all(|p| p.x == -2),
            "unload column should be x=-2: {to_unload:?}");
    }

    fn make_test_player_at(pos: Vec3) -> ServerPlayer {
        let mut p = ServerPlayer::new(
            1,
            GameProfile::new(uuid::Uuid::nil(), "Test".into()),
            ResourceLocation::minecraft("overworld"),
            GameMode::Survival,
        );
        p.pos = pos;
        p
    }
}
```
