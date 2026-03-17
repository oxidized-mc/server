# Entity System

## Overview

Minecraft's entity system is a deep class hierarchy. This document maps the Java
hierarchy to the planned Rust design and details the synced entity data system,
AI system, and tracking.

**Java references:**
- `net.minecraft.world.entity.Entity`
- `net.minecraft.world.entity.LivingEntity`
- `net.minecraft.world.entity.Mob`
- `net.minecraft.server.level.ServerPlayer`
- `net.minecraft.world.entity.ai.*`

---

## Java Class Hierarchy (Server-Side)

```
Entity
└── LivingEntity
    ├── Mob
    │   ├── PathfinderMob
    │   │   ├── WaterAnimal (Squid, Dolphin, …)
    │   │   ├── AmbientCreature (Bat)
    │   │   └── Animal
    │   │       ├── Cow, Pig, Sheep, Chicken, Horse, …
    │   │       └── TamableAnimal (Wolf, Cat, Parrot)
    │   └── Monster
    │       ├── Zombie, Husk, DrownedZombie
    │       ├── Skeleton, Stray, WitherSkeleton
    │       ├── Creeper
    │       ├── Spider, CaveSpider
    │       ├── Enderman
    │       ├── Blaze
    │       ├── Witch
    │       └── (others)
    └── Player
        └── ServerPlayer   ← server-side player
```

---

## Rust Design

```rust
// Trait-based composition (not deep inheritance)

pub trait Entity: Send + Sync {
    fn id(&self) -> EntityId;
    fn uuid(&self) -> Uuid;
    fn pos(&self) -> Vec3;
    fn rot(&self) -> (f32, f32);   // yaw, pitch
    fn entity_type(&self) -> &EntityType;
    fn bounding_box(&self) -> AABB;
    fn tick(&mut self, level: &mut ServerLevel);
    fn save(&self) -> CompoundTag;
    fn load(&mut self, tag: &CompoundTag);
}

pub trait LivingEntity: Entity {
    fn health(&self) -> f32;
    fn max_health(&self) -> f32;
    fn hurt(&mut self, source: DamageSource, amount: f32) -> bool;
    fn kill(&mut self);
    fn active_effects(&self) -> &HashMap<MobEffectId, MobEffectInstance>;
}

pub trait Mob: LivingEntity {
    fn goal_selector(&self) -> &GoalSelector;
    fn target(&self) -> Option<EntityId>;
    fn navigation(&self) -> &PathNavigation;
}
```

In practice, concrete entity types are enums or structs that compose these
capabilities via `Arc<dyn Trait>` or a flat ECS-style approach using `hecs`.

---

## Entity Base Fields

From `net.minecraft.world.entity.Entity`:

```rust
pub struct EntityBase {
    pub id: EntityId,           // atomic i32 counter
    pub uuid: Uuid,
    pub pos: Vec3,
    pub old_pos: Vec3,          // previous tick position (for delta)
    pub rot: (f32, f32),        // yaw, pitch
    pub old_rot: (f32, f32),
    pub delta_movement: Vec3,   // velocity
    pub on_ground: bool,
    pub was_on_ground: bool,
    pub horizontal_collision: bool,
    pub vertical_collision: bool,
    pub no_physics: bool,
    pub removed: Option<RemovalReason>,
    pub fire_ticks: i32,        // -1 = fireproof
    pub air_supply: i32,        // breath (max 300 = 15 sec)
    pub portal_cooldown: i32,
    pub invulnerable: bool,
    pub fall_distance: f32,
    pub no_gravity: bool,
    pub glowing: bool,
    pub silent: bool,
    pub custom_name: Option<Component>,
    pub custom_name_visible: bool,
    pub tags: HashSet<String>,  // scoreboard tags
    pub passengers: Vec<EntityId>,
    pub vehicle: Option<EntityId>,
}
```

Java reference: `net.minecraft.world.entity.Entity` fields (lines 170–280)

---

## SynchedEntityData

Entity metadata synced to clients via `ClientboundSetEntityDataPacket`.

Each entity type registers data slots at startup with a type ID and default value.
Slots are identified by index (VarInt on the wire).

### Network Type IDs

| ID | Rust type | Java type |
|----|-----------|-----------|
| 0 | `u8` | `Byte` |
| 1 | `VarInt` | `Int` |
| 2 | `VarLong` | `Long` |
| 3 | `f32` | `Float` |
| 4 | `String` | `String` |
| 5 | `Component` | `Component` (JSON) |
| 6 | `Option<Component>` | `Optional<Component>` |
| 7 | `ItemStack` | `ItemStack` |
| 8 | `bool` | `Boolean` |
| 9 | `Vec3` | `Rotations` (3× float) |
| 10 | `BlockPos` | `BlockPos` |
| 11 | `Option<BlockPos>` | `Optional<BlockPos>` |
| 12 | `Direction` | `Direction` (VarInt 0–5) |
| 13 | `Option<Uuid>` | `Optional<UUID>` |
| 14 | `BlockState` | `BlockState` (VarInt state ID) |
| 15 | `Option<BlockState>` | `Optional<BlockState>` |
| 16 | `CompoundTag` | `CompoundTag` |
| 17 | `Particle` | `ParticleType` |
| 18 | `VarInt` | `VillagerData` (type, profession, level) |
| 19 | `Option<VarInt>` | `OptionalUnsignedInt` |
| 20 | `Pose` | `Pose` (VarInt 0–12) |
| 21 | `CatVariant` | (VarInt) |
| 22 | `WolfVariant` | |
| 23 | `FrogVariant` | |
| 24 | `Option<GlobalPos>` | |
| 25 | `PaintingVariant` | |
| 26 | `SnifferState` | |
| 27 | `ArmadilloState` | |
| 28 | `Vec3` | `Vector3f` |
| 29 | `Quaternion` | `Quaternionf` |

### Common Entity Slots

| Slot | Type | Description |
|------|------|-------------|
| 0 | `u8` | Entity flags (on_fire=0x01, crouching=0x02, sprinting=0x08, swimming=0x10, invisible=0x20, glowing=0x40, flying_with_elytra=0x80) |
| 1 | `VarInt` | Air supply (0–300) |
| 2 | `Option<Component>` | Custom name |
| 3 | `bool` | Custom name visible |
| 4 | `bool` | Silent |
| 5 | `bool` | No gravity |
| 6 | `Pose` | Pose (STANDING, FALL_FLYING, SLEEPING, SWIMMING, SPIN_ATTACK, CROUCHING, LONG_JUMPING, DYING, CROAKING, USING_TONGUE, SITTING, ROARING, SNIFFING, EMERGING, DIGGING) |
| 7 | `VarInt` | Ticks frozen (0–140) |

Java reference: `net.minecraft.world.entity.EntityDataAccessors`

---

## AI System

### Legacy AI (GoalSelector — most mobs)

```rust
pub struct GoalSelector {
    goals: Vec<WrappedGoal>,    // sorted by priority (lower = higher priority)
}

pub struct WrappedGoal {
    priority: i32,
    goal: Box<dyn PathfinderGoal>,
    running: bool,
}

pub trait PathfinderGoal: Send + Sync {
    fn can_use(&self) -> bool;
    fn can_continue_to_use(&self) -> bool { self.can_use() }
    fn is_interruptable(&self) -> bool { true }
    fn start(&mut self);
    fn stop(&mut self);
    fn requires_update_every_tick(&self) -> bool { false }
    fn tick(&mut self);
    fn flags(&self) -> EnumSet<GoalFlag>;
}
```

Each tick: evaluate all goals, start the highest-priority applicable one per flag,
tick currently running goals.

**Core goal implementations:**

| Goal | Description | Java |
|------|-------------|------|
| `FloatGoal` | Swim upward in fluids | `net.minecraft.world.entity.ai.goal.FloatGoal` |
| `MeleeAttackGoal` | Chase + melee attack target | `MeleeAttackGoal` |
| `RangedBowAttackGoal` | Skeleton bow attack | `RangedBowAttackGoal` |
| `WaterAvoidingRandomWalkGoal` | Random wandering | `WaterAvoidingRandomWalkingGoal` |
| `LookAtPlayerGoal` | Face nearest player | `LookAtPlayerGoal` |
| `RandomLookAroundGoal` | Random head rotation | `RandomLookAroundGoal` |
| `NearestAttackableTargetGoal<T>` | Acquire T as target | `NearestAttackableTargetGoal` |
| `BreedGoal` | Animal breeding | `BreedGoal` |
| `TemptGoal` | Follow player with food | `TemptGoal` |
| `PanicGoal` | Flee from attacker | `PanicGoal` |
| `FollowParentGoal` | Baby follows parent | `FollowParentGoal` |
| `EatBlockGoal` | Sheep eating grass | `EatBlockGoal` |
| `OpenDoorGoal` | Villager opens doors | `OpenDoorGoal` |

### Modern AI (Brain — villagers, piglins)

```rust
pub struct Brain {
    memories: HashMap<MemoryModuleType, MemoryValue>,
    sensors: Vec<Box<dyn Sensor>>,
    behaviors: HashMap<Activity, Vec<BehaviorControl>>,
    core_activities: HashSet<Activity>,
    active_activities: HashSet<Activity>,
}
```

Brain-based mobs don't use `GoalSelector` — they use the `Brain::tick()` pipeline.

---

## Entity Tracking

The server tracks which players are near each entity and sends
spawn/despawn/update packets accordingly.

```rust
pub struct EntityTracker {
    entity: Arc<RwLock<dyn Entity>>,
    tracking_range: i32,           // blocks (varies by entity type)
    update_interval: i32,          // ticks between delta updates
    tracked_by: HashSet<PlayerId>, // players currently tracking this entity
    last_sent_pos: Vec3,
    last_sent_rot: (f32, f32),
}
```

**Default tracking ranges (Java `EntityType`):**

| Type | Range (blocks) |
|------|----------------|
| Player | 128 |
| Mob | 64 |
| Animal | 64 |
| Item (dropped) | 64 |
| Arrow | 64 |
| Boat / Minecart | 64 |
| FallingBlock | 64 |
| TNT | 64 |
| XP orb | 64 |
| Lightning bolt | 16 384 (global) |

Java reference: `net.minecraft.server.level.ChunkMap.TrackedEntity`

---

## Entity Tick Order (per ServerLevel tick)

1. Tick entities in `entityTickList` (loaded + tracked entities)
2. For each entity:
   a. `entity.tick()` (base physics, fire, air, portal)
   b. `entity.rideTick()` if vehicle
   c. Specific type tick (mob AI, player input, projectile flight)
3. Remove entities flagged for removal
4. Send `ClientboundMoveEntityPacket` deltas to tracking players
5. Send `ClientboundSetEntityDataPacket` for dirty metadata

Java reference: `net.minecraft.server.level.ServerLevel.tick()` (entity section)
