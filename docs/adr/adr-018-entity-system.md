# ADR-018: Entity System Architecture

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P15, P24, P25, P27 |
| Deciders | Oxidized Core Team |

## Context

The entity system is the single largest architectural divergence from vanilla Minecraft in
the entire Oxidized project. In vanilla, entities use a deep object-oriented inheritance
hierarchy: `Entity` → `LivingEntity` → `Mob` → `Monster` → `Zombie`, often five or more
levels deep. Each class adds its own fields (health, equipment slots, AI goals, attack
damage) and overrides methods from parent classes. `Entity` alone has over 250 fields and
200 methods. `LivingEntity` adds another 100+ fields. This pattern made sense in 2009 when
Minecraft was a single-threaded Java game with a few hundred entities, but it creates
severe problems at scale.

The inheritance model causes three critical performance and design problems. First, **poor
cache locality**: each entity is a heap-allocated object with fields scattered across the
inheritance chain. When the physics system iterates over 10,000 entities to update
positions, it touches `Entity.position`, `Entity.velocity`, `Entity.onGround` — but each
access chases pointers through the entity object and potentially into sub-objects like
`Vec3` and `AABB`. The CPU cache is thrashed because adjacent entities in a list are not
adjacent in memory. Second, **impossible parallelism**: methods at every level of the
hierarchy read and write shared mutable state. `LivingEntity.tick()` calls `Entity.tick()`
via `super`, which calls `checkInsideBlocks()`, which mutates entity state, which other
entities might be reading. There is no safe way to tick two entities in parallel. Third,
**composition rigidity**: when a new cross-cutting behavior is needed (e.g., "entities that
can be leashed"), vanilla must either add it to a base class (bloating all entities) or
duplicate code across branches of the hierarchy. Interfaces help but cannot carry state.

In a server targeting 10,000+ entities per dimension with multi-threaded tick execution,
these problems are not theoretical — they are the primary bottleneck. Our entity system
design determines whether Oxidized can achieve its performance goals. This decision affects
every system that touches entities: physics, AI, networking, persistence, chunk loading,
and command execution. It is the most consequential architectural choice in the project.

## Decision Drivers

- **Cache-friendly iteration**: Systems that process thousands of entities per tick (physics,
  AI, network sync) must iterate over tightly packed data, not chase pointers through
  scattered heap objects.
- **Safe parallelism**: Independent systems (gravity doesn't conflict with AI target
  selection) must be able to run on different threads simultaneously without data races or
  locks.
- **Behavioral composition**: Adding a new behavior to a subset of entities (e.g., "tameable")
  must not require modifying base types or duplicating code across entity kinds.
- **Vanilla behavior fidelity**: Despite the architectural change, every observable entity
  behavior must match vanilla — movement physics, AI decisions, network serialization,
  damage calculation, and status effects must produce identical results.
- **Mappability to vanilla source**: Developers reading vanilla source must be able to find
  the corresponding Oxidized code. If vanilla has `Zombie.aiStep()`, there must be a clear
  system or function that implements the same logic.
- **Runtime entity modification**: Components can be added or removed at runtime (e.g., a mob
  gains `Burning` when set on fire, loses it when extinguished), matching vanilla's dynamic
  entity state.

## Considered Options

### Option 1: OOP Inheritance via Trait Objects

Mimic vanilla's class hierarchy using Rust traits. Define `Entity`, `LivingEntity`, `Mob`,
and so on as traits. Data is stored in structs that implement multiple traits. Downcasting
between trait levels uses `Any`.

```rust
trait Entity: Any {
    fn position(&self) -> Vec3;
    fn tick(&mut self, level: &mut Level);
}

trait LivingEntity: Entity {
    fn health(&self) -> f32;
    fn hurt(&mut self, source: DamageSource, amount: f32);
}

struct ZombieEntity {
    position: Vec3,
    health: f32,
    // ... 50+ fields across the hierarchy
}

impl Entity for ZombieEntity { /* ... */ }
impl LivingEntity for ZombieEntity { /* ... */ }
```

**Pros:**
- Direct 1:1 mapping to vanilla's class hierarchy — easiest to port vanilla logic.
- Familiar OOP patterns for developers coming from Java Minecraft modding.
- No external dependencies.

**Cons:**
- Data is still scattered per-entity — no cache locality improvement. Iterating over all
  entity positions requires touching each entity object individually.
- Virtual dispatch through `dyn Entity` adds indirection on every method call.
- Downcasting (`entity.downcast_ref::<ZombieEntity>()`) is fragile, not checked at compile
  time, and is a code smell in Rust.
- Parallelism is just as difficult as vanilla — `&mut self` on `tick()` means exclusive
  access to the entire entity, preventing concurrent system execution.
- Composition still requires trait explosion or manual delegation.
- This approach throws away Rust's strengths and fights the language.

**Verdict: Rejected.** Preserves all of vanilla's problems with a worse developer experience.

### Option 2: Full ECS with bevy_ecs

Use Bevy's ECS crate (`bevy_ecs`) as a standalone library. Entities are opaque IDs.
Components are plain data structs. Systems are functions that declare their data access
via queries, enabling automatic parallelism.

```rust
#[derive(Component)]
struct Position(DVec3);

#[derive(Component)]
struct Velocity(DVec3);

#[derive(Component)]
struct Health { current: f32, max: f32 }

fn gravity_system(mut query: Query<(&mut Velocity, &Position), Without<NoGravity>>) {
    for (mut vel, pos) in &mut query {
        vel.0.y -= 0.08; // gravity constant
        vel.0.y *= 0.98; // vertical drag
    }
}
```

**Pros:**
- Excellent cache locality — components of the same type are stored contiguously in memory
  via archetype-based storage. Iterating over all `(Position, Velocity)` pairs is a
  cache-friendly linear scan.
- Automatic parallelism — `bevy_ecs` analyzes system queries and runs non-conflicting
  systems on different threads. `gravity_system` (reads `Position`, writes `Velocity`) can
  run in parallel with `ai_target_system` (reads `Position`, writes `AiTarget`).
- Natural composition — adding "tameable" to wolves means adding a `Tameable` component.
  No inheritance change. Any system can query `With<Tameable>`.
- Battle-tested — `bevy_ecs` is used by thousands of Bevy game projects, has excellent
  performance benchmarks, and is actively maintained.
- Runtime dynamism — components can be added/removed at any time, matching vanilla's
  dynamic entity state (entities gain/lose effects, change modes, etc.).

**Cons:**
- Adds a significant dependency (`bevy_ecs` plus its transitive deps). Not the full Bevy
  engine, but still a meaningful addition to the dependency tree.
- ECS paradigm shift — developers must think in terms of "data and systems" rather than
  "objects and methods." Vanilla logic like `zombie.aiStep()` becomes multiple systems
  operating on components. This requires careful documentation of the mapping.
- Some vanilla patterns are awkward in ECS — deeply nested method calls like
  `LivingEntity.hurt()` → `actuallyHurt()` → `getDamageAfterArmorAbsorb()` → etc. need
  refactoring into system pipelines or event chains.
- Bevy's release cadence is fast; we must pin to a stable version and manage upgrades.
- Query syntax has a learning curve, especially for complex queries with filters, change
  detection, and optional components.

**Verdict: Selected.** The benefits massively outweigh the costs for a server targeting
high entity counts with parallel execution.

### Option 3: Custom Lightweight ECS

Build a minimal ECS tailored to Minecraft's specific needs. Entity ID → component storage
using a `TypeMap` of dense `Vec`s. Systems are functions manually scheduled by the tick
loop. No external ECS dependency.

```rust
struct World {
    entities: EntityAllocator,
    components: TypeMap<ComponentStorage>,
}

struct ComponentStorage<T> {
    dense: Vec<T>,
    sparse: Vec<Option<usize>>, // entity_id → dense_index
}
```

**Pros:**
- No external dependency — full control over the implementation.
- Can tailor storage layouts to Minecraft's specific access patterns.
- Simpler mental model (no Bevy-specific concepts like `SystemParam`, `Commands`, etc.).

**Cons:**
- Massive implementation effort — building a correct, performant ECS is a multi-month
  project. Archetype management, change detection, parallel scheduling, and command
  buffering are each complex subsystems.
- No automatic parallelism — we would need to build our own system scheduler that analyzes
  data access patterns, essentially reimplementing a core part of `bevy_ecs`.
- Likely to have more bugs and worse performance than a mature, battle-tested ECS.
- Maintenance burden falls entirely on the Oxidized team.

**Verdict: Rejected.** Reinventing the wheel with worse results.

### Option 4: Hybrid — Component Storage with Entity Type Structs

Each entity type (Zombie, Skeleton, etc.) is a plain struct with all its fields. Stored in
typed `Vec`s per entity type. Common behavior via traits. Not a full ECS but still
data-oriented.

```rust
struct ZombieStore {
    positions: Vec<DVec3>,
    velocities: Vec<DVec3>,
    healths: Vec<f32>,
    // ... all zombie fields in SoA layout
}
```

**Pros:**
- Excellent cache locality within a single entity type (struct-of-arrays layout).
- No external dependency.
- Simple to understand — each entity type is a known struct.

**Cons:**
- Cross-type queries are expensive — "iterate all entities with Health" requires iterating
  every entity type's store and checking if it has a health field.
- Adding a new component to a subset of entities is awkward — every store must be updated.
- Parallel execution requires manual synchronization per store.
- Behavioral composition is limited — adding "Burning" to any entity requires modifying
  every entity type's store.
- Code duplication across entity types for shared logic.

**Verdict: Rejected.** Good for cache locality but poor for composition and cross-cutting
queries, which are pervasive in Minecraft.

### Option 5: Actor Model

Each entity is an actor with its own task and mailbox. Inter-entity interactions are
message-based. Natural isolation — each actor owns its state exclusively.

```rust
struct ZombieActor {
    state: ZombieState,
    mailbox: mpsc::Receiver<EntityMessage>,
}
```

**Pros:**
- Perfect isolation — no shared mutable state by construction.
- Natural fit for Rust's async model.
- Easy to reason about individual entity behavior.

**Cons:**
- Catastrophic overhead for 10,000+ entities — each entity needs its own task, mailbox, and
  message serialization. Tokio can handle many tasks but the per-message overhead for simple
  operations (position update, gravity) is orders of magnitude worse than direct memory access.
- Spatial queries ("find all entities within 16 blocks") require broadcasting messages to
  all entities or maintaining a separate spatial index that duplicates state.
- Deterministic tick ordering is extremely difficult — message delivery order is
  nondeterministic, but vanilla behavior depends on entity tick order.
- Poor cache locality — each actor's state is isolated in its own heap allocation.

**Verdict: Rejected.** The overhead model is fundamentally wrong for a system with
thousands of small, frequently-interacting entities.

## Decision

**We adopt bevy_ecs as a standalone library for all entity representation and processing.**
Entities are opaque `Entity` IDs. All entity state is decomposed into components — plain
Rust structs with `#[derive(Component)]`. All entity logic is implemented as systems —
functions that declare their data access via `Query<>` parameters. The ECS `World` is
owned by each dimension (Overworld, Nether, End) and ticked by the server's main loop.

Every field in vanilla's entity class hierarchy is mapped to a named component. Vanilla's
`Entity` base class fields become: `Position(DVec3)`, `Velocity(DVec3)`, `Rotation { yaw:
f32, pitch: f32 }`, `OnGround(bool)`, `FallDistance(f32)`, `EntityFlags(u8)` (on_fire,
crouching, sprinting, swimming, invisible, glowing, fall_flying), `NoGravity`, `Silent`,
`CustomName(Option<Component>)`, `TickCount(u32)`, `BoundingBox(AABB)`, and
`EntityType(ResourceLocation)`. `LivingEntity` fields become: `Health { current: f32, max:
f32 }`, `Equipment(EquipmentSlots)`, `ActiveEffects(HashMap<MobEffect, EffectInstance>)`,
`ArmorValue(f32)`, `AbsorptionAmount(f32)`, `DeathTime(u16)`, `HurtTime(u16)`,
`LastDamageSource(Option<DamageSource>)`, `Attributes(AttributeMap)`, and
`LivingEntityMarker`. Player-specific state becomes: `PlayerInventory`, `GameMode`,
`FoodData`, `ExperienceData { level: i32, progress: f32, total: i32 }`,
`Abilities(PlayerAbilities)`, `SelectedSlot(u8)`, and `PlayerMarker`.

Entity-type-specific behavior is driven by **marker components**. A zombie entity has
components: `Position`, `Velocity`, `Health`, `Equipment`, `AiGoals`, `NavigationPath`,
`ZombieMarker`, `SynchedEntityData`, and others. Systems that implement zombie-specific
logic query `With<ZombieMarker>`:

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

The `SynchedEntityData` component deserves special attention. In vanilla, this is a
per-entity map of tracked values (indexed by `EntityDataAccessor<T>`) that flags dirty
entries for network serialization. In Oxidized, `SynchedEntityData` is a component
containing a `SmallVec` of typed data entries. Each entry stores its current value and a
dirty flag. When any system modifies a tracked value (health, pose, entity flags, custom
name, etc.), it sets the dirty flag. The `entity_data_sync_system` runs at the end of the
tick phase, iterates all entities with dirty `SynchedEntityData`, serializes the changed
entries into `ClientboundSetEntityDataPacket`, and clears the dirty flags. This preserves
vanilla's lazy network sync behavior while fitting cleanly into the ECS model.

System scheduling follows a strict phase order within each tick to maintain determinism and
vanilla compatibility. Within each phase, `bevy_ecs` automatically parallelizes
non-conflicting systems. The phases are:

1. **Pre-tick**: Increment `TickCount`, process pending entity spawns/despawns.
2. **Physics**: Apply gravity, apply velocity, resolve collisions, update `OnGround` and
   `FallDistance`. See ADR-021 for collision details.
3. **AI**: Run `GoalSelector` for mobs, evaluate goals, update pathfinding. See ADR-023.
4. **Entity behavior**: Entity-type-specific logic (zombie burning, creeper explosion
   timer, villager trading, breeding cooldowns, item pickup, projectile flight).
5. **Status effects**: Apply/expire potion effects, tick poison/wither/regeneration.
6. **Post-tick**: Update bounding boxes, chunk section tracking, trigger game events.
7. **Network sync**: Serialize dirty `SynchedEntityData`, position updates, equipment
   changes, and other entity-related packets. See ADR-020.

## Consequences

### Positive

- **10-50x improvement in entity iteration throughput**: Archetype-based storage means
  iterating `Query<(&Position, &Velocity)>` over 10,000 entities is a linear scan over two
  contiguous arrays. Benchmarks of `bevy_ecs` show ~1ns per entity for simple queries.
- **Automatic parallelism**: The gravity system, the AI targeting system, and the status
  effect system can run on different CPU cores simultaneously, with `bevy_ecs` ensuring no
  data races. On an 8-core server, entity tick time could drop by 4-6x.
- **Trivial behavior composition**: Making any entity "burnable" is adding `Burning` as a
  component. Making any entity "tameable" is adding `Tameable`. No hierarchy restructuring.
- **Clean separation of data and behavior**: Components are plain data; systems are pure
  functions (with declared side effects via `Commands`). This makes each system
  independently testable.
- **Natural fit for Rust**: Components are owned data, queries borrow them. The borrow
  checker enforces correct access patterns at compile time. No `Rc<RefCell<>>` gymnastics.

### Negative

- **Paradigm shift for vanilla reference**: Developers porting vanilla logic must mentally
  decompose `Zombie.aiStep()` (a 200-line method in a class with 5 parent classes) into
  multiple independent systems operating on shared components. This is a significant
  cognitive overhead, especially for newcomers.
- **Relationship modeling is harder**: Vanilla's "a wolf has an owner (Player)" is natural
  in OOP (`wolf.getOwner()`). In ECS, it requires an `Owner(Entity)` component and a
  system that resolves the entity reference. Bidirectional relationships (mounts, passengers)
  need careful design to avoid stale entity references.
- **Debugging is different**: Instead of stepping through `zombie.tick()`, developers must
  trace which systems run on an entity with `ZombieMarker`. Bevy's system ordering
  visualization helps but is not as intuitive as a call stack.
- **bevy_ecs dependency**: We take on a major external dependency. Version upgrades may
  require non-trivial migration. We mitigate this by depending only on `bevy_ecs` (not the
  full Bevy engine) and wrapping key APIs behind our own traits where appropriate.

### Neutral

- **Vanilla mapping documentation is mandatory**: We must maintain a living document that
  maps every vanilla class/method to its Oxidized component/system equivalent. Without this,
  porting vanilla logic becomes guesswork.
- **Entity type registration**: Each entity type's set of components must be declared in a
  spawn template (archetype bundle). This replaces vanilla's constructor chain but serves
  the same purpose — defining what components a zombie has vs. what a chicken has.

## Compliance

- **Component-per-field rule**: Every field in vanilla's `Entity`, `LivingEntity`, `Mob`,
  `PathfinderMob`, `Monster`, `Animal`, `AbstractVillager`, and `Player` classes must have
  a corresponding component in Oxidized. Verified by maintaining a checked mapping table
  in `docs/entity-mapping.md`.
- **No game logic in platform/networking code**: Systems must not access raw network
  channels. Packet generation is confined to network sync systems.
- **System scheduling test**: Integration tests verify that system execution order matches
  the defined phase order (physics before AI, AI before network sync).
- **Behavior parity tests**: For each entity type, record vanilla server behavior (entity
  positions, health changes, AI decisions over N ticks) and assert Oxidized produces
  identical results for the same initial conditions.
- **No direct World mutation**: All entity state changes go through ECS `Commands` or
  direct component mutation within systems. No `unsafe` mutation of entity state outside
  the ECS.

## Related ADRs

- **ADR-019**: Tick Loop Design — defines the phase structure that schedules entity systems
- **ADR-020**: Player Session Lifecycle — player entities are ECS entities with additional
  components and a network bridge
- **ADR-021**: Physics & Collision Engine — the physics phase systems that operate on
  entity `Position` and `Velocity` components
- **ADR-023**: AI & Pathfinding System — the AI phase systems that operate on `AiGoals`
  and `NavigationPath` components
- **ADR-024**: Inventory & Container Transactions — player inventory is a set of ECS
  components on the player entity

## References

- [Bevy ECS documentation](https://docs.rs/bevy_ecs/latest/bevy_ecs/)
- [Bevy ECS as a standalone crate](https://github.com/bevyengine/bevy/tree/main/crates/bevy_ecs)
- Vanilla source: `net.minecraft.world.entity.Entity` (base class, ~4000 lines)
- Vanilla source: `net.minecraft.world.entity.LivingEntity` (~3500 lines)
- Vanilla source: `net.minecraft.world.entity.Mob` (~1200 lines)
- Vanilla source: `net.minecraft.network.syncher.SynchedEntityData`
- [Data-Oriented Design and C++](https://www.dataorienteddesign.com/dodbook/) — foundational
  reading on why ECS outperforms OOP for entity-heavy simulations
- [Catherine West — "Using Rust for Game Development"](https://kyren.github.io/2018/09/14/rustconf-talk.html)
  — canonical talk on why ECS is the right choice for Rust game servers
