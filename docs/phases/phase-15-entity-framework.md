# Phase 15 — Entity Framework + Tracking

**Crate:** `oxidized-game`  
**Reward:** Entities appear in the world and are synchronised to all nearby
players. Spawning any entity causes `ClientboundAddEntityPacket` to be sent to
watching players; moving out of tracking range sends
`ClientboundRemoveEntitiesPacket`. `SynchedEntityData` changes are batched and
flushed each tick.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-018: Entity System](../adr/adr-018-entity-system.md) — ECS with bevy_ecs for data-oriented entity management


## Goal

Build the entity base infrastructure: atomic ID allocation, the
`SynchedEntityData` system (with all 31 registered serializer types), per-entity
bounding boxes, and the `EntityTracker` that mirrors Java's
`ChunkMap.TrackedEntity`. All entity packets must be serialised to the exact wire
format the vanilla client expects.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Entity base | `Entity` | `net.minecraft.world.entity.Entity` |
| Entity data | `SynchedEntityData` | `net.minecraft.network.syncher.SynchedEntityData` |
| Data accessor | `EntityDataAccessor` | `net.minecraft.network.syncher.EntityDataAccessor` |
| Serializers | `EntityDataSerializers` | `net.minecraft.network.syncher.EntityDataSerializers` |
| Entity type | `EntityType` | `net.minecraft.world.entity.EntityType` |
| Entity tracker | `EntityTrackerEntry` | `net.minecraft.server.level.EntityTrackerEntry` |
| Tracked entity | `ChunkMap.TrackedEntity` | `net.minecraft.server.level.ChunkMap.TrackedEntity` |
| Add entity pkt | `ClientboundAddEntityPacket` | `net.minecraft.network.protocol.game.ClientboundAddEntityPacket` |
| Remove entities | `ClientboundRemoveEntitiesPacket` | `net.minecraft.network.protocol.game.ClientboundRemoveEntitiesPacket` |
| Entity data pkt | `ClientboundSetEntityDataPacket` | `net.minecraft.network.protocol.game.ClientboundSetEntityDataPacket` |

---

## Tasks

### 15.1 — Atomic entity ID counter

```rust
// crates/oxidized-game/src/entity/id.rs

use std::sync::atomic::{AtomicI32, Ordering};

static NEXT_ENTITY_ID: AtomicI32 = AtomicI32::new(1);

/// Allocate a globally unique entity ID.
pub fn next_entity_id() -> i32 {
    NEXT_ENTITY_ID.fetch_add(1, Ordering::Relaxed)
}

/// Reset counter (test-only).
#[cfg(test)]
pub fn reset_counter(value: i32) {
    NEXT_ENTITY_ID.store(value, Ordering::SeqCst);
}
```

### 15.2 — `SynchedEntityData` serializer types

All 31 types registered in `EntityDataSerializers`, in registration order
(0–30). The integer is the serializer ID written on the wire.

```rust
// crates/oxidized-game/src/entity/synched_data.rs

/// Wire type ID for each EntityDataSerializer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum DataSerializerType {
    Byte            = 0,
    VarInt          = 1,
    VarLong         = 2,
    Float           = 3,
    String          = 4,
    Component       = 5,   // TextComponent (JSON)
    OptComponent    = 6,   // Optional<TextComponent>
    ItemStack       = 7,
    Boolean         = 8,
    Rotations       = 9,   // 3 × f32
    BlockPos        = 10,
    OptBlockPos     = 11,
    Direction       = 12,  // enum 0–5
    OptUuid         = 13,
    BlockState      = 14,  // VarInt global block state ID
    OptBlockState   = 15,  // VarInt, 0 = absent
    CompoundTag     = 16,
    ParticleOptions = 17,
    ParticleList    = 18,
    VillagerData    = 19,  // type VarInt + profession VarInt + level VarInt
    OptVarInt       = 20,  // 0 = absent, else value+1
    Pose            = 21,  // VarInt enum
    CatVariant      = 22,
    WolfVariant     = 23,
    FrogVariant     = 24,
    OptGlobalPos    = 25,  // Optional<ResourceKey<Level> + BlockPos>
    PaintingVariant = 26,
    SnifferState    = 27,
    ArmadilloState  = 28,
    Vector3f        = 29,  // 3 × f32
    Quaternionf     = 30,  // 4 × f32
}
```

### 15.3 — Common entity data slot indices

These match `Entity.class` field definitions in Java (via `defineId`):

```rust
// crates/oxidized-game/src/entity/data_slots.rs

/// Slot 0: Byte flags field.
/// Bit 0: on fire, Bit 1: crouching, Bit 3: sprinting,
/// Bit 4: swimming, Bit 5: invisible, Bit 6: glowing, Bit 7: fall flying.
pub const DATA_FLAGS: u8 = 0;
/// Slot 1: Air supply ticks (VarInt). Max = 300 (15 s).
pub const DATA_AIR_SUPPLY: u8 = 1;
/// Slot 2: Custom name (OptComponent).
pub const DATA_CUSTOM_NAME: u8 = 2;
/// Slot 3: Custom name visible (Boolean).
pub const DATA_CUSTOM_NAME_VISIBLE: u8 = 3;
/// Slot 4: Silent (Boolean).
pub const DATA_SILENT: u8 = 4;
/// Slot 5: No gravity (Boolean).
pub const DATA_NO_GRAVITY: u8 = 5;
/// Slot 6: Pose (VarInt enum). 0=Standing, 1=FallFlying, 2=Sleeping,
///   3=Swimming, 4=SpinAttack, 5=Sneaking, 6=LongJumping, 7=Dying,
///   8=Croaking, 9=UsingTongue, 10=Sitting, 11=Roaring, 12=Sniffing,
///   13=Emerging, 14=Digging.
pub const DATA_POSE: u8 = 6;
/// Slot 7: Freeze ticks (VarInt). Used by Powder Snow.
pub const DATA_FREEZE_TICKS: u8 = 7;
```

### 15.4 — `SynchedEntityData` runtime store

```rust
// crates/oxidized-game/src/entity/synched_data.rs (continued)

use std::any::Any;

pub struct DataItem {
    pub serializer_type: DataSerializerType,
    pub value: Box<dyn Any + Send + Sync>,
    pub dirty: bool,
}

pub struct SynchedEntityData {
    items: Vec<Option<DataItem>>,
    is_dirty: bool,
}

impl SynchedEntityData {
    pub fn new() -> Self {
        // 255 slots maximum (Java uses class-tree inheritance for IDs).
        Self { items: Vec::new(), is_dirty: false }
    }

    pub fn define<T: Any + Clone + Send + Sync>(
        &mut self,
        slot: u8,
        serializer: DataSerializerType,
        default: T,
    ) {
        let idx = slot as usize;
        while self.items.len() <= idx {
            self.items.push(None);
        }
        self.items[idx] = Some(DataItem {
            serializer_type: serializer,
            value: Box::new(default),
            dirty: false,
        });
    }

    pub fn get<T: Any + Clone>(&self, slot: u8) -> T {
        let item = self.items[slot as usize].as_ref()
            .expect("undefined data slot");
        item.value.downcast_ref::<T>()
            .expect("type mismatch in SynchedEntityData::get")
            .clone()
    }

    pub fn set<T: Any + PartialEq + Clone + Send + Sync>(
        &mut self, slot: u8, value: T,
    ) {
        let item = self.items[slot as usize].as_mut()
            .expect("undefined data slot");
        if let Some(existing) = item.value.downcast_ref::<T>() {
            if existing == &value { return; }
        }
        item.value = Box::new(value);
        item.dirty = true;
        self.is_dirty = true;
    }

    pub fn is_dirty(&self) -> bool { self.is_dirty }

    /// Collect all dirty slots and reset dirty flags.
    pub fn pack_dirty(&mut self) -> Vec<DirtyDataValue> {
        if !self.is_dirty { return Vec::new(); }
        self.is_dirty = false;
        self.items.iter_mut().enumerate()
            .filter_map(|(i, maybe)| {
                let item = maybe.as_mut()?;
                if item.dirty {
                    item.dirty = false;
                    Some(DirtyDataValue {
                        slot: i as u8,
                        serializer_type: item.serializer_type,
                        value: &item.value,
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

pub struct DirtyDataValue<'a> {
    pub slot: u8,
    pub serializer_type: DataSerializerType,
    pub value: &'a Box<dyn Any + Send + Sync>,
}
```

### 15.5 — `Entity` base struct

```rust
// crates/oxidized-game/src/entity/entity.rs

use uuid::Uuid;
use std::sync::Arc;

pub struct AABB {
    pub min_x: f64, pub min_y: f64, pub min_z: f64,
    pub max_x: f64, pub max_y: f64, pub max_z: f64,
}

impl AABB {
    pub fn from_center(x: f64, y: f64, z: f64, w: f64, h: f64) -> Self {
        Self {
            min_x: x - w / 2.0, min_y: y, min_z: z - w / 2.0,
            max_x: x + w / 2.0, max_y: y + h, max_z: z + w / 2.0,
        }
    }

    pub fn contains(&self, x: f64, y: f64, z: f64) -> bool {
        x >= self.min_x && x <= self.max_x
            && y >= self.min_y && y <= self.max_y
            && z >= self.min_z && z <= self.max_z
    }

    pub fn intersects(&self, other: &AABB) -> bool {
        self.max_x > other.min_x && self.min_x < other.max_x
            && self.max_y > other.min_y && self.min_y < other.max_y
            && self.max_z > other.min_z && self.min_z < other.max_z
    }
}

pub struct Entity {
    pub id: i32,
    pub uuid: Uuid,
    pub entity_type: ResourceLocation,

    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,

    pub vx: f64,
    pub vy: f64,
    pub vz: f64,

    pub on_ground: bool,
    pub removed: bool,

    pub bounding_box: AABB,
    pub synched_data: SynchedEntityData,

    /// Width of entity's hitbox (meters).
    pub width: f32,
    /// Height of entity's hitbox (meters).
    pub height: f32,
}

impl Entity {
    pub fn new(entity_type: ResourceLocation, width: f32, height: f32) -> Self {
        let id = next_entity_id();
        let mut synched_data = SynchedEntityData::new();
        // Define base entity slots.
        synched_data.define(DATA_FLAGS, DataSerializerType::Byte, 0u8);
        synched_data.define(DATA_AIR_SUPPLY, DataSerializerType::VarInt, 300i32);
        synched_data.define(DATA_CUSTOM_NAME, DataSerializerType::OptComponent, None::<String>);
        synched_data.define(DATA_CUSTOM_NAME_VISIBLE, DataSerializerType::Boolean, false);
        synched_data.define(DATA_SILENT, DataSerializerType::Boolean, false);
        synched_data.define(DATA_NO_GRAVITY, DataSerializerType::Boolean, false);
        synched_data.define(DATA_POSE, DataSerializerType::VarInt, 0i32);
        synched_data.define(DATA_FREEZE_TICKS, DataSerializerType::VarInt, 0i32);

        Self {
            id,
            uuid: Uuid::new_v4(),
            entity_type,
            x: 0.0, y: 0.0, z: 0.0,
            yaw: 0.0, pitch: 0.0, head_yaw: 0.0,
            vx: 0.0, vy: 0.0, vz: 0.0,
            on_ground: false,
            removed: false,
            bounding_box: AABB::from_center(0.0, 0.0, 0.0, width as f64, height as f64),
            synched_data,
            width,
            height,
        }
    }

    pub fn set_pos(&mut self, x: f64, y: f64, z: f64) {
        self.x = x;
        self.y = y;
        self.z = z;
        self.bounding_box = AABB::from_center(x, y, z, self.width as f64, self.height as f64);
    }

    /// Get entity flag bit.
    pub fn get_flag(&self, bit: u8) -> bool {
        let flags: u8 = self.synched_data.get(DATA_FLAGS);
        flags & (1 << bit) != 0
    }

    pub fn set_flag(&mut self, bit: u8, value: bool) {
        let mut flags: u8 = self.synched_data.get(DATA_FLAGS);
        if value { flags |= 1 << bit; } else { flags &= !(1 << bit); }
        self.synched_data.set(DATA_FLAGS, flags);
    }

    pub fn is_on_fire(&self) -> bool { self.get_flag(0) }
    pub fn is_crouching(&self) -> bool { self.get_flag(1) }
    pub fn is_sprinting(&self) -> bool { self.get_flag(3) }
    pub fn is_invisible(&self) -> bool { self.get_flag(5) }
}
```

### 15.6 — `EntityTracker`

Mirrors `ChunkMap.TrackedEntity`. Determines which players see each entity and
sends spawn/despawn packets when that set changes.

```rust
// crates/oxidized-game/src/entity/tracker.rs

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

/// Default tracking ranges (in blocks) per entity type category.
pub const TRACKING_RANGE_PLAYER: i32 = 64;
pub const TRACKING_RANGE_ANIMAL: i32 = 80;
pub const TRACKING_RANGE_MONSTER: i32 = 48;
pub const TRACKING_RANGE_MISC: i32 = 64;
pub const TRACKING_RANGE_OTHER: i32 = 64;

pub struct EntityTracker {
    /// entity_id → set of player UUIDs currently watching it.
    watching: HashMap<i32, HashSet<uuid::Uuid>>,
    /// Tracking range per entity.
    range: HashMap<i32, i32>,
}

impl EntityTracker {
    pub fn new() -> Self {
        Self { watching: HashMap::new(), range: HashMap::new() }
    }

    pub fn register(&mut self, entity_id: i32, tracking_range: i32) {
        self.watching.insert(entity_id, HashSet::new());
        self.range.insert(entity_id, tracking_range);
    }

    pub fn unregister(&mut self, entity_id: i32) -> HashSet<uuid::Uuid> {
        self.range.remove(&entity_id);
        self.watching.remove(&entity_id).unwrap_or_default()
    }

    /// Returns (players to send spawn, players to send despawn) for an entity
    /// given the updated set of players within tracking range.
    pub fn update(
        &mut self,
        entity_id: i32,
        now_watching: HashSet<uuid::Uuid>,
    ) -> (Vec<uuid::Uuid>, Vec<uuid::Uuid>) {
        let current = self.watching.entry(entity_id).or_default();
        let to_add: Vec<_> = now_watching.difference(current).copied().collect();
        let to_remove: Vec<_> = current.difference(&now_watching).copied().collect();
        *current = now_watching;
        (to_add, to_remove)
    }

    pub fn is_tracking(
        &self, entity_id: i32, player_uuid: &uuid::Uuid,
    ) -> bool {
        self.watching.get(&entity_id)
            .map(|s| s.contains(player_uuid))
            .unwrap_or(false)
    }
}
```

### 15.7 — `ClientboundAddEntityPacket` wire format

```rust
pub struct ClientboundAddEntityPacket {
    pub entity_id: i32,       // VarInt
    pub uuid: uuid::Uuid,     // 16 bytes
    pub entity_type: i32,     // VarInt (registry ID)
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vx: i16,              // velocity * 8000, clamped to [-30000, 30000]
    pub vy: i16,
    pub vz: i16,
    pub pitch: u8,            // packed degrees
    pub yaw: u8,
    pub head_yaw: u8,
    pub data: i32,            // VarInt, entity-type-specific data
}

impl ClientboundAddEntityPacket {
    /// Velocity scale: 1.0 m/tick → 8000.
    pub fn encode_velocity(v: f64) -> i16 {
        (v * 8000.0).clamp(-30000.0, 30000.0) as i16
    }
}
```

---

## Data Structures Summary

```
oxidized-game::entity
  ├── next_entity_id()             — atomic i32 counter
  ├── DataSerializerType (0..=30)  — all 31 wire types
  ├── SynchedEntityData            — slot store + dirty tracking
  ├── AABB                         — axis-aligned bounding box
  ├── Entity                       — base struct (id, pos, vel, flags, data…)
  └── EntityTracker                — watching set per entity_id
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Each call to next_entity_id() returns a unique, incrementing value.
    #[test]
    fn entity_id_uniqueness() {
        reset_counter(1);
        let ids: Vec<i32> = (0..100).map(|_| next_entity_id()).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), 100, "IDs are not unique");
        assert!(ids.windows(2).all(|w| w[1] == w[0] + 1), "IDs are not sequential");
    }

    /// SynchedEntityData get/set round-trip for Boolean.
    #[test]
    fn synched_data_bool_roundtrip() {
        let mut data = SynchedEntityData::new();
        data.define(DATA_SILENT, DataSerializerType::Boolean, false);
        assert_eq!(data.get::<bool>(DATA_SILENT), false);
        data.set(DATA_SILENT, true);
        assert_eq!(data.get::<bool>(DATA_SILENT), true);
    }

    /// Setting to the same value does not mark the slot dirty.
    #[test]
    fn synched_data_no_dirty_on_same_value() {
        let mut data = SynchedEntityData::new();
        data.define(DATA_SILENT, DataSerializerType::Boolean, false);
        data.set(DATA_SILENT, false); // same as default
        assert!(!data.is_dirty());
    }

    /// pack_dirty collects only changed slots and resets their dirty flag.
    #[test]
    fn synched_data_pack_dirty() {
        let mut data = SynchedEntityData::new();
        data.define(0, DataSerializerType::Byte, 0u8);
        data.define(1, DataSerializerType::VarInt, 300i32);
        data.set(0u8, 4u8); // set on fire (bit 0)
        let dirty = data.pack_dirty();
        assert_eq!(dirty.len(), 1);
        assert_eq!(dirty[0].slot, 0);
        // Second call should return empty.
        let dirty2 = data.pack_dirty();
        assert!(dirty2.is_empty());
    }

    /// AABB intersection logic.
    #[test]
    fn aabb_intersection() {
        let a = AABB::from_center(0.0, 0.0, 0.0, 1.0, 2.0);
        let b = AABB::from_center(0.5, 0.0, 0.0, 1.0, 2.0); // overlaps
        let c = AABB::from_center(5.0, 0.0, 0.0, 1.0, 2.0); // far away
        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }

    /// EntityTracker correctly identifies new watchers and departed watchers.
    #[test]
    fn entity_tracker_watch_unwatch() {
        let mut tracker = EntityTracker::new();
        tracker.register(42, 64);

        let p1 = uuid::Uuid::new_v4();
        let p2 = uuid::Uuid::new_v4();

        let (add, remove) = tracker.update(42, [p1].into_iter().collect());
        assert_eq!(add, vec![p1]);
        assert!(remove.is_empty());

        let (add2, remove2) = tracker.update(42, [p1, p2].into_iter().collect());
        assert_eq!(add2, vec![p2]);
        assert!(remove2.is_empty());

        let (add3, remove3) = tracker.update(42, [p2].into_iter().collect());
        assert!(add3.is_empty());
        assert_eq!(remove3, vec![p1]);
    }

    /// encode_velocity clamps correctly.
    #[test]
    fn add_entity_velocity_encoding() {
        assert_eq!(ClientboundAddEntityPacket::encode_velocity(0.0), 0);
        assert_eq!(ClientboundAddEntityPacket::encode_velocity(1.0), 8000);
        assert_eq!(ClientboundAddEntityPacket::encode_velocity(-1.0), -8000);
        // Extreme values clamp.
        assert_eq!(ClientboundAddEntityPacket::encode_velocity(100.0), -30000i16.wrapping_neg());
        // Negative extreme.
        assert_eq!(ClientboundAddEntityPacket::encode_velocity(-100.0), -30000);
    }
}
```
