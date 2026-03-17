# Entity System

## Design Philosophy

Oxidized uses **[bevy_ecs](https://docs.rs/bevy_ecs/latest/bevy_ecs/)** as a
standalone library for all entity representation and processing (see
[ADR-018](../adr/adr-018-entity-system.md)). There is no trait inheritance
hierarchy. Instead:

- **Entities** are opaque `Entity` IDs — lightweight handles with no data.
- **Components** are plain Rust structs annotated with `#[derive(Component)]`.
  All entity state is decomposed into components.
- **Systems** are functions that declare their data access via `Query<>`
  parameters. `bevy_ecs` analyzes queries and schedules non-conflicting systems
  for automatic parallel execution.

This architecture gives us cache-friendly iteration over contiguous component
arrays, safe multi-threaded ticking (independent systems run on different cores),
and trivial behavioral composition — adding a capability to any entity is just
inserting a new component.

> **Why not trait objects?** ADR-018 explicitly evaluated and **rejected** the
> OOP approach (`pub trait Entity`, `Arc<dyn Trait>`, `hecs`). Trait objects
> preserve all of vanilla Java's problems (pointer chasing, no parallelism,
> composition rigidity) while fighting Rust's ownership model. See the ADR for
> the full rationale.

**Java references:**
- `net.minecraft.world.entity.Entity`
- `net.minecraft.world.entity.LivingEntity`
- `net.minecraft.world.entity.Mob`
- `net.minecraft.server.level.ServerPlayer`
- `net.minecraft.world.entity.ai.*`

---

## Vanilla Java → ECS Mapping

Vanilla's deep class hierarchy maps to flat component composition. Each level of
the Java hierarchy becomes a set of components plus a marker component.

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Java Class            │ ECS Representation                             │
├───────────────────────┼────────────────────────────────────────────────┤
│ Entity (base)         │ Position, Rotation, Velocity, OnGround,       │
│                       │ FallDistance, EntityFlags, BoundingBox,        │
│                       │ EntityType, TickCount, NoGravity*, Silent*,   │
│                       │ CustomName*, SynchedData, …                   │
├───────────────────────┼────────────────────────────────────────────────┤
│ LivingEntity          │ LivingEntityMarker + Health, Equipment,       │
│                       │ ActiveEffects, Attributes, ArmorValue,        │
│                       │ AbsorptionAmount, DeathTime, HurtTime,        │
│                       │ LastDamageSource                              │
├───────────────────────┼────────────────────────────────────────────────┤
│ Mob                   │ MobMarker + AiGoals, NavigationPath, Target   │
├───────────────────────┼────────────────────────────────────────────────┤
│ PathfinderMob         │ PathfinderMobMarker (inherits Mob components) │
├───────────────────────┼────────────────────────────────────────────────┤
│ Monster               │ MonsterMarker                                 │
│ Animal                │ AnimalMarker + BreedCooldown                  │
│ TamableAnimal         │ TamableMarker + Owner(Entity)                 │
├───────────────────────┼────────────────────────────────────────────────┤
│ Player                │ PlayerMarker + PlayerInventory, GameMode,     │
│                       │ FoodData, ExperienceData, Abilities,          │
│                       │ SelectedSlot                                  │
├───────────────────────┼────────────────────────────────────────────────┤
│ Zombie                │ ZombieMarker + shared components above        │
│ Skeleton              │ SkeletonMarker + shared components above      │
│ Creeper               │ CreeperMarker + FuseTime, ExplosionRadius     │
│ Villager              │ VillagerMarker + BrainAi, VillagerData        │
└───────────────────────┴────────────────────────────────────────────────┘

  * NoGravity, Silent, CustomName are optional components — inserted only when
    the flag is set, removed when cleared. Systems filter with With<>/Without<>.
```

A zombie entity in the ECS `World` has the following components: `Position`,
`Rotation`, `Velocity`, `OnGround`, `FallDistance`, `EntityFlags`, `BoundingBox`,
`EntityType`, `TickCount`, `SynchedData`, `Health`, `Equipment`, `ActiveEffects`,
`Attributes`, `AiGoals`, `NavigationPath`, `LivingEntityMarker`, `MobMarker`,
`PathfinderMobMarker`, `MonsterMarker`, `ZombieMarker`.

Systems that implement zombie-specific logic query `With<ZombieMarker>`:

```rust
fn zombie_sunlight_burning_system(
    mut commands: Commands,
    query: Query<(Entity, &Position), (With<ZombieMarker>, Without<Burning>)>,
    time: Res<DayTime>,
) {
    for (entity, pos) in &query {
        if time.is_day() && pos.0.y > 64.0 && /* sky visible check */ true {
            commands.entity(entity).insert(Burning { ticks_remaining: 80 });
        }
    }
}
```

---

## Component Catalog

All fields from vanilla's `Entity` base class are decomposed into individual
components. The data is identical to vanilla — only the storage model changes.

### Core Components (all entities)

These correspond to fields from `net.minecraft.world.entity.Entity`:

```rust
#[derive(Component)]
pub struct Position(pub DVec3);          // pos — current world position

#[derive(Component)]
pub struct OldPosition(pub DVec3);       // previous tick position (for delta packets)

#[derive(Component)]
pub struct Rotation {
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Component)]
pub struct OldRotation {
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Component)]
pub struct Velocity(pub DVec3);          // delta_movement

#[derive(Component)]
pub struct OnGround(pub bool);

#[derive(Component)]
pub struct WasOnGround(pub bool);

#[derive(Component)]
pub struct HorizontalCollision(pub bool);

#[derive(Component)]
pub struct VerticalCollision(pub bool);

#[derive(Component)]
pub struct NoPhysics;                    // marker — entity ignores collisions

#[derive(Component)]
pub struct Removed(pub RemovalReason);

#[derive(Component)]
pub struct FireTicks(pub i32);           // -1 = fireproof

#[derive(Component)]
pub struct AirSupply(pub i32);           // breath (max 300 = 15 sec)

#[derive(Component)]
pub struct PortalCooldown(pub i32);

#[derive(Component)]
pub struct Invulnerable;                 // marker

#[derive(Component)]
pub struct FallDistance(pub f32);

#[derive(Component)]
pub struct NoGravity;                    // marker — optional, inserted when set

#[derive(Component)]
pub struct Glowing;                      // marker

#[derive(Component)]
pub struct Silent;                       // marker

#[derive(Component)]
pub struct CustomName(pub TextComponent);

#[derive(Component)]
pub struct CustomNameVisible;            // marker

#[derive(Component)]
pub struct EntityFlags(pub u8);          // on_fire=0x01, crouching=0x02, sprinting=0x08,
                                         // swimming=0x10, invisible=0x20, glowing=0x40,
                                         // flying_with_elytra=0x80

#[derive(Component)]
pub struct ScoreboardTags(pub HashSet<String>);

#[derive(Component)]
pub struct Passengers(pub Vec<Entity>);

#[derive(Component)]
pub struct Vehicle(pub Entity);

#[derive(Component)]
pub struct BoundingBox(pub AABB);

#[derive(Component)]
pub struct EntityType(pub ResourceLocation);

#[derive(Component)]
pub struct TickCount(pub u32);
```

Java reference: `net.minecraft.world.entity.Entity` fields (lines 170–280)

### LivingEntity Components

Added alongside `LivingEntityMarker` for all living entities:

```rust
#[derive(Component)]
pub struct LivingEntityMarker;

#[derive(Component)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

#[derive(Component)]
pub struct Equipment(pub EquipmentSlots);

#[derive(Component)]
pub struct ActiveEffects(pub HashMap<MobEffect, EffectInstance>);

#[derive(Component)]
pub struct Attributes(pub AttributeMap);

#[derive(Component)]
pub struct ArmorValue(pub f32);

#[derive(Component)]
pub struct AbsorptionAmount(pub f32);

#[derive(Component)]
pub struct DeathTime(pub u16);

#[derive(Component)]
pub struct HurtTime(pub u16);

#[derive(Component)]
pub struct LastDamageSource(pub DamageSource);
```

### Player Components

Added alongside `PlayerMarker` for server-side player entities:

```rust
#[derive(Component)]
pub struct PlayerMarker;

#[derive(Component)]
pub struct PlayerInventory { /* slots, armor, offhand, crafting */ }

#[derive(Component)]
pub struct GameMode(pub GameModeType);

#[derive(Component)]
pub struct FoodData {
    pub food_level: i32,
    pub saturation: f32,
    pub exhaustion: f32,
}

#[derive(Component)]
pub struct ExperienceData {
    pub level: i32,
    pub progress: f32,
    pub total: i32,
}

#[derive(Component)]
pub struct Abilities(pub PlayerAbilities);

#[derive(Component)]
pub struct SelectedSlot(pub u8);
```

### Mob Components

Added alongside `MobMarker`:

```rust
#[derive(Component)]
pub struct MobMarker;

#[derive(Component)]
pub struct AiGoals {
    pub goals: Vec<PrioritizedGoal>,
    pub active_goals: SmallVec<[usize; 4]>,
    pub disabled_flags: GoalFlags,
}

#[derive(Component)]
pub struct NavigationPath {
    pub path: Option<Path>,
    pub navigation_type: NavigationType,
    pub max_distance: f32,
    pub can_open_doors: bool,
    pub can_pass_doors: bool,
    pub can_float: bool,
}

#[derive(Component)]
pub struct Target(pub Entity);
```

---

## System Examples

Systems are plain functions that declare data access via `Query<>`. `bevy_ecs`
runs non-conflicting systems in parallel automatically.

```rust
/// Applies gravity to all entities that are not marked NoGravity.
/// Runs in the Physics sub-phase of ENTITY_TICK.
fn gravity_system(
    mut query: Query<&mut Velocity, (With<EntityFlags>, Without<NoGravity>)>,
) {
    for mut vel in &mut query {
        vel.0.y -= 0.08; // GRAVITY constant (vanilla)
        vel.0.y *= 0.98; // VERTICAL_DRAG
    }
}

/// Tracks entity position changes and queues delta-movement packets for
/// players within tracking range. Runs in the NETWORK_SEND phase.
fn entity_tracking_system(
    query: Query<(&Position, &EntityType, &TrackedBy), Changed<Position>>,
    // ... packet queue resources
) {
    for (pos, etype, tracked_by) in &query {
        // Serialize ClientboundMoveEntityPacket for each tracking player
    }
}

/// Evaluates AI goals for all mobs every tick. Runs in the AI sub-phase
/// of ENTITY_TICK (after physics, before entity behavior).
fn ai_goal_system(
    mut query: Query<(Entity, &mut AiGoals, &Position, Option<&Target>), With<MobMarker>>,
    world: &World,
) {
    for (entity, mut goals, pos, target) in &mut query {
        // 1. Stop goals that can no longer continue
        // 2. Evaluate inactive goals — start highest-priority applicable
        // 3. Tick all active goals
    }
}
```

---

## SynchedEntityData

Entity metadata synced to clients via `ClientboundSetEntityDataPacket`. In vanilla
this is a per-entity map of tracked values indexed by `EntityDataAccessor<T>` that
flags dirty entries for network serialization.

In Oxidized, `SynchedData` is a **component** containing a `SmallVec` of typed
data entries. Each entry stores its current value and a dirty flag. When any system
modifies a tracked value (health, pose, entity flags, custom name, etc.), it sets
the dirty flag. The `entity_data_sync_system` runs in the NETWORK_SEND phase,
iterates all entities with dirty `SynchedData`, serializes changed entries into
`ClientboundSetEntityDataPacket`, and clears the dirty flags.

```rust
#[derive(Component)]
pub struct SynchedData {
    entries: SmallVec<[DataEntry; 8]>,
}

struct DataEntry {
    type_id: u8,        // network type ID (see table below)
    slot: u8,           // index within entity's metadata
    value: DataValue,   // type-erased current value
    dirty: bool,        // needs network sync
}
```

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

Mob AI runs as ECS systems during the AI sub-phase of ENTITY_TICK (after physics,
before entity behavior). See [ADR-023](../adr/adr-023-ai-pathfinding.md) for the
full design. Two complementary AI models exist:

### GoalSelector (most mobs)

The `AiGoals` component holds a priority-sorted list of goals. The
`goal_selector_system` evaluates, starts, ticks, and stops goals each tick:

```rust
fn goal_selector_system(
    mut query: Query<(Entity, &mut AiGoals, &Position, Option<&Target>), With<MobMarker>>,
    world: &World,
) {
    for (entity, mut goals, pos, target) in &mut query {
        // 1. Stop goals that can no longer continue
        // 2. Evaluate inactive goals — start highest-priority applicable
        // 3. Stop conflicting lower-priority goals (flag-based mutual exclusion)
        // 4. Tick all active goals
    }
}
```

Goals implement the `Goal` trait and declare flag-based mutual exclusion
(MOVE, LOOK, JUMP, TARGET). Each tick, the selector starts the highest-priority
applicable goal per flag set and preempts lower-priority conflicting goals.

**Core goal implementations:**

| Goal | Description | Flags | Java class |
|------|-------------|-------|------------|
| `FloatGoal` | Swim upward in fluids | JUMP | `FloatGoal` |
| `MeleeAttackGoal` | Chase + melee attack target | MOVE, LOOK | `MeleeAttackGoal` |
| `RangedBowAttackGoal` | Skeleton bow attack | MOVE, LOOK | `RangedBowAttackGoal` |
| `WaterAvoidRandomStrollGoal` | Random wandering | MOVE | `WaterAvoidingRandomWalkingGoal` |
| `LookAtPlayerGoal` | Face nearest player | LOOK | `LookAtPlayerGoal` |
| `RandomLookAroundGoal` | Random head rotation | LOOK | `RandomLookAroundGoal` |
| `NearestAttackableTargetGoal` | Acquire target by type | TARGET | `NearestAttackableTargetGoal` |
| `HurtByTargetGoal` | Retaliate against attacker | TARGET | `HurtByTargetGoal` |
| `BreedGoal` | Animal breeding | MOVE | `BreedGoal` |
| `TemptGoal` | Follow player holding food | MOVE, LOOK | `TemptGoal` |
| `PanicGoal` | Flee from attacker | MOVE | `PanicGoal` |
| `FollowParentGoal` | Baby follows parent | MOVE | `FollowParentGoal` |
| `EatBlockGoal` | Sheep eating grass | (none) | `EatBlockGoal` |
| `OpenDoorGoal` | Villager opens doors | (none) | `OpenDoorGoal` |
| `AvoidEntityGoal` | Flee from specific entity type | MOVE | `AvoidEntityGoal` |

### Brain System (villagers, piglins, wardens)

Brain-based mobs use a `BrainAi` component instead of `AiGoals`. The brain
uses activities (IDLE, WORK, REST, FIGHT), sensors, and memories:

```rust
#[derive(Component)]
pub struct BrainAi {
    pub memories: HashMap<MemoryModuleType, MemoryValue>,
    pub sensors: Vec<Box<dyn Sensor>>,
    pub behaviors: HashMap<Activity, Vec<BehaviorControl>>,
    pub core_activities: HashSet<Activity>,
    pub active_activities: HashSet<Activity>,
}
```

A dedicated `brain_tick_system` processes brain-based mobs:

```rust
fn brain_tick_system(
    mut query: Query<(Entity, &mut BrainAi, &Position), Without<AiGoals>>,
    world: &World,
) {
    for (entity, mut brain, pos) in &mut query {
        // 1. Tick sensors (perceive nearby entities/blocks)
        // 2. Update memories from sensor output
        // 3. Select active activities based on schedule + conditions
        // 4. Run behavior controls for active activities
    }
}
```

### Pathfinding

Both AI models use A* pathfinding on the block grid via the `NavigationPath`
component. Path requests are evaluated by `find_path()` with a maximum node
expansion limit (default 400). Optimizations include path caching (reuse path
if target moved < 1 block) and async path requests for non-combat AI. See
ADR-023 for the full A* implementation and navigation type details.

---

## Entity Tracking

The server tracks which players can see each entity and sends spawn/despawn/update
packets accordingly. In the ECS model, tracking state is represented as components
rather than a struct holding `Arc<RwLock<dyn Entity>>`.

```rust
#[derive(Component)]
pub struct TrackingRange(pub i32);        // blocks (varies by entity type)

#[derive(Component)]
pub struct UpdateInterval(pub i32);       // ticks between delta updates

#[derive(Component)]
pub struct TrackedBy(pub HashSet<Entity>); // player entities currently tracking

#[derive(Component)]
pub struct LastSentPosition(pub DVec3);

#[derive(Component)]
pub struct LastSentRotation {
    pub yaw: f32,
    pub pitch: f32,
}
```

The `entity_tracking_system` runs in the NETWORK_SEND phase
([ADR-019](../adr/adr-019-tick-loop.md)). It uses `bevy_ecs` change detection
(`Changed<Position>`) to only process entities whose position actually changed:

```rust
fn entity_tracking_system(
    query: Query<
        (&Position, &Rotation, &TrackedBy, &mut LastSentPosition, &mut LastSentRotation),
        Or<(Changed<Position>, Changed<Rotation>)>,
    >,
    // ... packet queue resources
) {
    for (pos, rot, tracked_by, mut last_pos, mut last_rot) in &query {
        // Compute delta from last_sent, serialize packets, update last_sent
    }
}
```

**Default tracking ranges (vanilla `EntityType`):**

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

## Entity Tick Order

Entity ticking executes within the **ENTITY_TICK** phase of the server tick loop
(see [ADR-019](../adr/adr-019-tick-loop.md)). Within ENTITY_TICK, systems run in
a fixed sub-phase order. `bevy_ecs` parallelizes non-conflicting systems *within*
each sub-phase; phase barriers between sub-phases guarantee deterministic ordering.

| Sub-phase | Systems | Notes |
|-----------|---------|-------|
| **Pre-tick** | Increment `TickCount`, process pending spawns/despawns | `Commands` from previous tick applied here |
| **Physics** | Gravity, velocity application, collision resolution, `OnGround`/`FallDistance` update | See [ADR-021](../adr/adr-021-physics.md) |
| **AI** | `goal_selector_system`, `brain_tick_system`, pathfinding | See [ADR-023](../adr/adr-023-ai-pathfinding.md) |
| **Entity behavior** | Type-specific logic (zombie burning, creeper fuse, breeding, projectile flight) | Queries use marker components (`With<ZombieMarker>`, etc.) |
| **Status effects** | Apply/expire potion effects, tick poison/wither/regeneration | Operates on `ActiveEffects` component |
| **Post-tick** | Update bounding boxes, chunk section tracking, trigger game events | Prepares state for network sync |

After ENTITY_TICK completes, the NETWORK_SEND phase serializes dirty
`SynchedData`, position deltas, and equipment changes into outbound packets.

Vanilla equivalent: `net.minecraft.server.level.ServerLevel.tick()` (entity
section) — the same logical ordering, but individual steps are now parallel ECS
systems instead of sequential method calls on each entity object.

---

## Related ADRs

- **[ADR-018](../adr/adr-018-entity-system.md)** — Entity System Architecture:
  the authoritative decision to adopt `bevy_ecs`; defines component-per-field
  mapping, marker components, and system scheduling phases
- **[ADR-019](../adr/adr-019-tick-loop.md)** — Tick Loop Design: defines the
  ENTITY_TICK phase (and other phases) with parallel execution and phase barriers
- **[ADR-020](../adr/adr-020-player-session.md)** — Player Session Lifecycle:
  player entities are ECS entities with additional components and a network bridge
- **[ADR-021](../adr/adr-021-physics.md)** — Physics & Collision Engine: physics
  systems (gravity, movement, collision) that operate on `Position`, `Velocity`,
  `OnGround`, `FallDistance`, `BoundingBox` components within ENTITY_TICK
- **[ADR-023](../adr/adr-023-ai-pathfinding.md)** — AI & Pathfinding: GoalSelector
  and Brain as ECS components, A* pathfinding, goal evaluation systems within
  ENTITY_TICK
- **[ADR-024](../adr/adr-024-inventory.md)** — Inventory & Container Transactions:
  player inventory as ECS components on the player entity
