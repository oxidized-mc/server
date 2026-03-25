# Phase 23b — ECS Runtime Integration

**Status:** 📋 Planned
**Crate:** `oxidized-game`, `oxidized-server`
**Reward:** All entities live in a `bevy_ecs::World`; the tick loop runs a 7-phase
`Schedule`; systems replace procedural entity logic; future phases (23c+) can add
entity types by just defining components, bundles, and systems.

**Depends on:** Phase 15 (Entity Framework — scaffolding), Phase 16 (Physics),
Phase 22 (Block Interaction)
**Required by:** Phase 23c (Dropped Items), Phase 24 (Combat), Phase 25 (Hostile Mobs),
Phase 27 (Animals), Phase 34 (Loot Tables)

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-018: Entity System](../adr/adr-018-entity-system.md) — bevy_ecs adoption, component design, system scheduling
- [ADR-019: Tick Loop](../adr/adr-019-tick-loop.md) — dedicated OS thread, phase barriers
- [ADR-020: Player Session](../adr/adr-020-player-session.md) — network actor ↔ ECS entity bridge via channels

---

## Goal

Complete the transition from the monolithic `Entity` / `ServerPlayer` structs to a live
`bevy_ecs::World` with phased `Schedule` execution. Phase 15 defined the ECS scaffolding
(components, bundles, markers, tick phases) — this phase **wires it into the runtime**.

After this phase:
- A `bevy_ecs::World` holds every entity as an archetype-based ECS entity
- The tick loop runs a `Schedule` with 7 ordered phases (PreTick → NetworkSync)
- Player state is bridged: network tasks ↔ ECS entity via channel commands
- The monolithic `Entity` struct is removed; `ServerPlayer` is decomposed
- Existing functionality (movement, chunk tracking, entity visibility) is preserved
- Future phases add entity types by defining bundles + systems, nothing else

**Non-goal:** This phase does NOT add new game features. It restructures the runtime so
that features in Phase 23c–38 can be built on a proper ECS foundation.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Main tick loop | `MinecraftServer.tickServer()` | `net.minecraft.server.MinecraftServer` |
| Entity base | `Entity` | `net.minecraft.world.entity.Entity` |
| Living entity | `LivingEntity` | `net.minecraft.world.entity.LivingEntity` |
| Player state | `ServerPlayer` | `net.minecraft.server.level.ServerPlayer` |
| Entity tracking | `ChunkMap.TrackedEntity` | `net.minecraft.server.level.ChunkMap` |
| Synched data | `SynchedEntityData` | `net.minecraft.network.syncher.SynchedEntityData` |

---

## Current State (What Exists)

### Active (monolithic, in use at runtime)

| File | What it does |
|------|-------------|
| `entity/mod.rs` — `Entity` struct | 15-field monolithic struct with `SynchedEntityData`, AABB, flags |
| `player/server_player.rs` — `ServerPlayer` | ~1000 LOC struct with nested sub-structs: `PlayerMovement`, `CombatStats`, `PlayerExperience`, `SpawnInfo`, `ConnectionInfo`, `TeleportTracker`, `MiningState`, `RawPlayerNbt` |
| `player/player_list.rs` — `PlayerList` | `AHashMap<Uuid, Arc<RwLock<ServerPlayer>>>` + join order + atomic entity ID counter |
| `network/mod.rs` — `ServerContext` | Top-level shared state: `WorldContext`, `NetworkContext`, `ServerSettings` |
| `network/play/mod.rs` — `PlayContext` | Per-packet borrow struct: `&Arc<RwLock<ServerPlayer>>`, `&Arc<ServerContext>` |
| `tick.rs` — `run_tick_loop()` | Procedural: time, weather, light, autosave. No entity tick, no ECS |
| `physics/tick.rs` — `physics_tick()` | Takes `&mut Entity` monolithic struct. Not connected to tick loop |

### Scaffolding (defined, never instantiated at runtime)

| File | What it defines |
|------|----------------|
| `entity/components.rs` | 13 components: `Position`, `Velocity`, `Rotation`, `OnGround`, `FallDistance`, `EntityFlags`, `NoGravity`, `Silent`, `TickCount`, `Health`, `Equipment`, `ArmorValue`, `AbsorptionAmount`, `PlayerMarker`, `SelectedSlot`, `ExperienceData` |
| `entity/bundles.rs` | 6 bundles: `BaseEntityBundle`, `LivingEntityBundle`, `ZombieBundle`, `SkeletonBundle`, `CreeperBundle`, `CowBundle`, `PlayerBundle` |
| `entity/markers.rs` | 27 markers: 9 hostile + 10 passive + 7 misc (`ItemEntityMarker`, `ExperienceOrbMarker`, `ArrowMarker`, etc.) + `PlayerMarker` |
| `entity/phases.rs` | `TickPhase` enum: `PreTick`, `Physics`, `Ai`, `EntityBehavior`, `StatusEffects`, `PostTick`, `NetworkSync` |
| `entity/tracker.rs` | `EntityTracker` struct with `register()`, `unregister()`, `update()`, tracking range constants |
| `entity/id.rs` | `next_entity_id()` — atomic global counter |
| `entity/synched_data.rs` | `SynchedEntityData` — dirty-tracked data slot map |
| `entity/data_slots.rs` | 8 base data slot constants (`DATA_SHARED_FLAGS` through `DATA_TICKS_FROZEN`) |

---

## Tasks

### 23b.1 — Create `bevy_ecs::World` and `Schedule` in `ServerContext` (`oxidized-server/src/network/mod.rs`) 📋

Add an ECS `World` to `ServerContext` so all game entities live in a single
archetype-based store. Add a `Schedule` configured with the 7 tick phases from
`TickPhase`.

```rust
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use oxidized_game::entity::phases::TickPhase;

/// Schedule label for each tick phase.
/// Wraps `TickPhase` to implement bevy's `ScheduleLabel` trait.
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhaseLabel(pub TickPhase);

/// ECS world and schedule, owned by the tick thread.
///
/// Created on startup, passed to `run_tick_loop()`.
/// Network tasks never access this directly — they post
/// commands via `EntityCommandQueue`.
pub struct EcsContext {
    /// The single bevy_ecs world holding all entities.
    pub world: World,
    /// One schedule per tick phase, run sequentially.
    pub schedules: [Schedule; 7],
}
```

The `World` is **NOT** placed inside `ServerContext` (which is `Arc`-shared across
async tasks). Instead, `EcsContext` is created on the main thread and moved to
the tick thread, which has exclusive ownership. Network tasks communicate with it
via command channels (task 23b.5).

**Why separate from `ServerContext`?** `bevy_ecs::World` is `!Sync` — it cannot be
shared behind `Arc`. The tick thread exclusively owns and mutates it. This matches
ADR-019 (dedicated tick thread) and ADR-020 (channel bridge).

**Tests:**
- Unit: `EcsContext` creates `World` with empty entity count
- Unit: all 7 schedules correspond to `TickPhase::ALL`

---

### 23b.2 — Convert `TickPhase` to `ScheduleLabel` and configure system sets (`oxidized-game/src/entity/phases.rs`) 📋

Extend the existing `TickPhase` enum to work as bevy_ecs schedule labels. Each phase
becomes a separate `Schedule` that can have systems added to it. Systems within a
schedule run in parallel automatically; schedules run sequentially.

```rust
use bevy_ecs::schedule::ScheduleLabel;

/// Schedule label newtype for TickPhase.
///
/// bevy_ecs requires `ScheduleLabel` on schedule identifiers.
/// We wrap TickPhase rather than deriving directly on it to
/// keep oxidized-game decoupled from bevy scheduling details.
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhaseSchedule(pub TickPhase);

impl TickPhase {
    /// Returns the corresponding schedule label.
    pub fn label(self) -> PhaseSchedule {
        PhaseSchedule(self)
    }
}
```

Also define **system ordering constraints** within phases where needed. Most systems
within a phase run in parallel, but some have explicit ordering:

```rust
/// Ordering sets within the EntityBehavior phase.
/// Systems can use `.before(BehaviorOrder::Pickup)` etc.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BehaviorOrder {
    /// Decrement pickup delays, despawn timers, etc.
    Timers,
    /// Merge nearby identical entities.
    Merge,
    /// Pickup / collection logic.
    Pickup,
}
```

**Tests:**
- Unit: `TickPhase::Physics.label()` produces a valid `ScheduleLabel`
- Unit: all 7 phases produce distinct labels

---

### 23b.3 — New ECS components for player state (`oxidized-game/src/entity/components.rs`) 📋

Decompose `ServerPlayer`'s nested sub-structs into additional ECS components. The
existing scaffolding covers base entity and living entity fields — this task adds
the player-specific fields that `ServerPlayer` currently holds monolithically.

**New components to add:**

```rust
// ---------------------------------------------------------------------------
// Player identity (from ServerPlayer)
// ---------------------------------------------------------------------------

/// Network entity ID (unique per session, never recycled).
/// All entities get this — used in every packet.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkId(pub i32);

/// Entity UUID (persistent across sessions for players).
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct EntityUuid(pub Uuid);

/// Entity type (e.g., `minecraft:player`, `minecraft:zombie`).
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct EntityTypeName(pub ResourceLocation);

/// Player's authenticated game profile (name + UUID + properties).
#[derive(Component, Debug, Clone)]
pub struct Profile(pub GameProfile);

// ---------------------------------------------------------------------------
// Player game state (from ServerPlayer sub-structs)
// ---------------------------------------------------------------------------

/// Player's game mode (survival, creative, adventure, spectator).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameModeComponent {
    pub current: GameMode,
    pub previous: Option<GameMode>,
}

/// Player abilities derived from game mode.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Abilities(pub PlayerAbilities);

/// Player inventory (46 protocol slots).
#[derive(Component, Debug, Clone)]
pub struct Inventory(pub PlayerInventory);

/// Combat stats: food, saturation, score, last death location.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct CombatData {
    pub food_level: i32,
    pub food_saturation: f32,
    pub score: i32,
    pub last_death_location: Option<(ResourceLocation, i64)>,
}

/// Player spawn point and current dimension.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct SpawnData {
    pub dimension: ResourceLocation,
    pub spawn_pos: BlockPos,
    pub spawn_angle: f32,
}

/// Skin model customisation byte (visible parts bitmask).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelCustomisation(pub u8);

// ---------------------------------------------------------------------------
// Entity physics (replacing Entity struct fields)
// ---------------------------------------------------------------------------

/// Axis-aligned bounding box for collision.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox(pub Aabb);

/// Entity hitbox width and height.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Dimensions {
    pub width: f32,
    pub height: f32,
}

/// Dirty-tracked entity data slots for network sync.
#[derive(Component, Debug)]
pub struct SynchedData(pub SynchedEntityData);

/// Whether the entity has been marked for removal.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Removed;
```

**Component mapping table** (ServerPlayer field → ECS component):

| ServerPlayer field | ECS Component |
|---|---|
| `entity_id: i32` | `NetworkId(i32)` |
| `uuid: Uuid` | `EntityUuid(Uuid)` |
| `name: String` | `Profile(GameProfile)` — profile contains name |
| `movement.pos` | `Position(DVec3)` |
| `movement.yaw/pitch` | `Rotation { yaw, pitch }` |
| `movement.is_on_ground` | `OnGround(bool)` |
| `movement.is_sneaking/sprinting/fall_flying` | `EntityFlags(u8)` |
| `game_mode` | `GameModeComponent { current, previous }` |
| `abilities` | `Abilities(PlayerAbilities)` |
| `inventory` | `Inventory(PlayerInventory)` |
| `combat.health/max_health` | `Health { current, max }` |
| `combat.food_level/saturation/...` | `CombatData { ... }` |
| `combat.absorption_amount` | `AbsorptionAmount(f32)` |
| `experience.*` | `ExperienceData { level, progress, total }` |
| `spawn.*` | `SpawnData { dimension, spawn_pos, spawn_angle }` |
| `connection.model_customisation` | `ModelCustomisation(u8)` |

**Tests:**
- Unit: all new components can be inserted and queried on a `World`
- Unit: `PlayerBundle` updated to include new components spawns correctly
- Unit: component defaults match `ServerPlayer::new()` defaults

---

### 23b.4 — Update `PlayerBundle` to include all player components (`oxidized-game/src/entity/bundles.rs`) 📋

Extend `PlayerBundle` with the new components from task 23b.3 so that spawning a
player entity creates a complete representation with every field `ServerPlayer` had.

```rust
/// Complete spawn bundle for player entities.
///
/// Contains every component a connected player needs. After spawning,
/// the entity's `NetworkId` and `EntityUuid` are used by the network
/// bridge to route packets.
#[derive(Bundle)]
pub struct PlayerBundle {
    // Base entity
    pub network_id: NetworkId,
    pub uuid: EntityUuid,
    pub entity_type: EntityTypeName,
    pub position: Position,
    pub velocity: Velocity,
    pub rotation: Rotation,
    pub on_ground: OnGround,
    pub fall_distance: FallDistance,
    pub flags: EntityFlags,
    pub tick_count: TickCount,
    pub bounding_box: BoundingBox,
    pub dimensions: Dimensions,
    pub synched_data: SynchedData,

    // Living entity
    pub health: Health,
    pub armor: ArmorValue,
    pub absorption: AbsorptionAmount,
    pub equipment: Equipment,

    // Player-specific
    pub marker: PlayerMarker,
    pub profile: Profile,
    pub game_mode: GameModeComponent,
    pub abilities: Abilities,
    pub inventory: Inventory,
    pub selected_slot: SelectedSlot,
    pub experience: ExperienceData,
    pub combat: CombatData,
    pub spawn_data: SpawnData,
    pub model_customisation: ModelCustomisation,
}

impl PlayerBundle {
    /// Create a PlayerBundle from an existing ServerPlayer's data.
    ///
    /// Used during the migration: load player from NBT into the legacy
    /// struct, then convert to a bundle for ECS spawning.
    pub fn from_server_player(sp: &ServerPlayer) -> Self {
        todo!()
    }
}
```

Similarly update `BaseEntityBundle` to include `NetworkId`, `EntityUuid`,
`EntityTypeName`, `BoundingBox`, `Dimensions`, and `SynchedData`.

**Tests:**
- Unit: `PlayerBundle::from_server_player()` roundtrips all fields
- Unit: spawned player entity has all expected components
- Unit: `query_filtered::<Entity, With<PlayerMarker>>` finds player entities

---

### 23b.5 — Entity command channel (network ↔ tick bridge) (`oxidized-game/src/entity/commands.rs`) 📋

Create a bounded MPSC channel that network tasks use to send entity mutations to
the tick thread. This is the **core bridge** between the async network layer and
the single-threaded ECS world (ADR-020).

Network handlers **never** access `bevy_ecs::World` directly. Instead they enqueue
`EntityCommand` values which the tick thread drains at the start of each tick.

```rust
use tokio::sync::mpsc;

/// Commands sent from network tasks to the tick thread's ECS world.
#[derive(Debug)]
pub enum EntityCommand {
    /// A player connected — spawn their ECS entity.
    SpawnPlayer {
        network_id: i32,
        uuid: Uuid,
        profile: GameProfile,
        position: DVec3,
        rotation: (f32, f32),
        game_mode: GameMode,
        inventory: PlayerInventory,
        health: f32,
        food_level: i32,
        experience: ExperienceData,
        spawn_data: SpawnData,
    },
    /// A player disconnected — despawn their ECS entity.
    DespawnPlayer { uuid: Uuid },
    /// Player moved (from ServerboundMovePlayerPacket).
    PlayerMoved {
        uuid: Uuid,
        position: DVec3,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    /// Player changed game state (sneak, sprint, etc.).
    PlayerAction {
        uuid: Uuid,
        action: PlayerActionKind,
    },
    /// Player changed selected hotbar slot.
    SlotChanged { uuid: Uuid, slot: u8 },
    /// Player inventory mutation (item added/removed/moved).
    InventoryUpdate {
        uuid: Uuid,
        update: InventoryMutation,
    },
    /// Generic entity spawn (non-player).
    SpawnEntity {
        bundle: Box<dyn DynBundle + Send>,
    },
    /// Remove a non-player entity by network ID.
    DespawnEntity { network_id: i32 },
}

/// Sender half given to network tasks.
pub type EntityCommandSender = mpsc::Sender<EntityCommand>;

/// Receiver half owned by the tick thread.
pub type EntityCommandReceiver = mpsc::Receiver<EntityCommand>;

/// Creates a new command channel with the given capacity.
pub fn entity_command_channel(capacity: usize) -> (EntityCommandSender, EntityCommandReceiver) {
    mpsc::channel(capacity)
}
```

The `EntityCommandSender` is cloned into each connection's play loop. The
`EntityCommandReceiver` is owned by the tick thread and drained in the `PreTick`
phase.

**Channel capacity:** Default 1024 per server (not per-player). Back-pressure
means a slow tick thread will pause network handlers from sending more commands,
which is the correct behavior (don't lose mutations).

**Tests:**
- Unit: send `SpawnPlayer` → receive on other end
- Unit: channel full → sender awaits (back-pressure works)
- Unit: all `EntityCommand` variants are `Send`
- Integration: spawn command creates entity in `World`

---

### 23b.6 — Drain command queue in PreTick system (`oxidized-server/src/tick.rs`) 📋

Add a system that runs in the `PreTick` phase to drain the `EntityCommandReceiver`
and apply each command to the `World`. This is where player spawns/despawns,
movement updates, and inventory changes materialize in the ECS.

```rust
/// Resource: holds the receiver end of the entity command channel.
#[derive(Resource)]
pub struct CommandQueue(pub EntityCommandReceiver);

/// Resource: maps player UUID → bevy Entity for fast lookup.
#[derive(Resource, Default)]
pub struct PlayerEntityMap(pub AHashMap<Uuid, bevy_ecs::entity::Entity>);

/// PreTick system: drains the command queue and applies mutations.
fn drain_entity_commands(
    mut commands: Commands,
    mut queue: ResMut<CommandQueue>,
    mut player_map: ResMut<PlayerEntityMap>,
    mut positions: Query<&mut Position>,
    mut rotations: Query<&mut Rotation>,
    mut on_grounds: Query<&mut OnGround>,
    mut flags: Query<&mut EntityFlags>,
) {
    while let Ok(cmd) = queue.0.try_recv() {
        match cmd {
            EntityCommand::SpawnPlayer { uuid, profile, position, .. } => {
                let entity = commands.spawn(PlayerBundle::new(/* ... */)).id();
                player_map.0.insert(uuid, entity);
            }
            EntityCommand::DespawnPlayer { uuid } => {
                if let Some(entity) = player_map.0.remove(&uuid) {
                    commands.entity(entity).despawn();
                }
            }
            EntityCommand::PlayerMoved { uuid, position, yaw, pitch, on_ground } => {
                if let Some(&entity) = player_map.0.get(&uuid) {
                    if let Ok(mut pos) = positions.get_mut(entity) {
                        pos.0 = position;
                    }
                    // ... update rotation, on_ground
                }
            }
            // ... handle all variants
        }
    }
}
```

**Registration in tick loop:**
```rust
schedules[TickPhase::PreTick as usize].add_systems(drain_entity_commands);
```

**Tests:**
- Integration: `SpawnPlayer` command → entity exists in `World` with correct components
- Integration: `DespawnPlayer` → entity removed from `World`
- Integration: `PlayerMoved` → `Position` component updated
- Unit: empty queue → no-op (fast path)

---

### 23b.7 — Rewrite tick loop to run ECS schedules (`oxidized-server/src/tick.rs`) 📋

Rewrite `run_tick_loop()` to execute the 7-phase `Schedule` each tick. The existing
procedural logic (time advancement, weather, light processing, autosave) becomes
systems registered in the appropriate phases.

**Before (current):**
```rust
pub fn run_tick_loop(ctx: &ServerContext, shutdown: &AtomicBool) {
    loop {
        // Procedural: advance time, weather, broadcast, light, autosave
    }
}
```

**After:**
```rust
pub fn run_tick_loop(
    ctx: &ServerContext,
    mut ecs: EcsContext,
    shutdown: &AtomicBool,
) {
    // Insert shared resources into the World
    ecs.world.insert_resource(WorldTimeResource { .. });
    ecs.world.insert_resource(WeatherResource { .. });
    ecs.world.insert_resource(CommandQueue(cmd_rx));
    ecs.world.insert_resource(PlayerEntityMap::default());

    // Register systems in their phases
    ecs.schedules[PreTick].add_systems(drain_entity_commands);
    ecs.schedules[PreTick].add_systems(tick_count_system);
    ecs.schedules[Physics].add_systems(gravity_system);
    ecs.schedules[Physics].add_systems(collision_system);
    ecs.schedules[PostTick].add_systems(bounding_box_update_system);
    ecs.schedules[NetworkSync].add_systems(entity_data_sync_system);
    ecs.schedules[NetworkSync].add_systems(position_sync_system);

    let tick_duration = Duration::from_millis(50); // 20 TPS
    loop {
        if shutdown.load(Ordering::Relaxed) { break; }
        let tick_start = Instant::now();

        // --- Existing procedural logic (stays for now) ---
        advance_time(ctx, tick_count);
        advance_weather(ctx, &mut weather, &mut rng, tick_count);
        process_light_updates(ctx);

        // --- NEW: Run all 7 ECS phases sequentially ---
        for phase in TickPhase::ALL {
            ecs.schedules[phase as usize].run(&mut ecs.world);
        }

        // --- Existing: autosave, sleep ---
        maybe_autosave(ctx, tick_count, &mut rng);
        sleep_until_next_tick(tick_start, tick_duration);
        tick_count += 1;
    }
}
```

**Migration strategy:** Existing procedural code (time, weather, light, autosave)
stays as-is for now — it runs *before* the ECS schedule. Future phases can migrate
these into systems incrementally. The key deliverable is the schedule loop running
and processing entity commands.

**Tests:**
- Integration: tick loop runs all 7 phases in order
- Integration: `PreTick` drains commands before `Physics` runs
- Unit: tick timing maintains 20 TPS under normal load

---

### 23b.8 — Player spawn: ECS entity creation on join (`oxidized-server/src/network/play/join.rs`) 📋

Refactor player join to send a `SpawnPlayer` command via the entity command channel
instead of (or in addition to) creating `Arc<RwLock<ServerPlayer>>`.

**Transition strategy:** During this phase, **both** representations exist:
1. `Arc<RwLock<ServerPlayer>>` — still used by network packet handlers (unchanged)
2. ECS entity — created in parallel, used by tick-thread systems

This dual-representation is temporary. Future phases incrementally move packet
handlers to read from ECS (via response channels or shared snapshots) and eventually
remove `ServerPlayer`.

```rust
// In join.rs, after creating ServerPlayer:
let entity_id = server_ctx.network.player_list.read().next_entity_id();
let mut player = ServerPlayer::new(entity_id, profile.clone(), dimension, game_mode);
// ... load from NBT ...

// NEW: Send spawn command to tick thread
let _ = entity_cmd_tx.send(EntityCommand::SpawnPlayer {
    network_id: entity_id,
    uuid: profile.id,
    profile: profile.clone(),
    position: DVec3::new(player.movement.pos.x, player.movement.pos.y, player.movement.pos.z),
    rotation: (player.movement.yaw, player.movement.pitch),
    game_mode: player.game_mode,
    inventory: player.inventory.clone(),
    health: player.combat.health,
    food_level: player.combat.food_level,
    experience: ExperienceData {
        level: player.experience.xp_level,
        progress: player.experience.xp_progress,
        total: player.experience.xp_total,
    },
    spawn_data: SpawnData {
        dimension: player.spawn.dimension.clone(),
        spawn_pos: player.spawn.spawn_pos,
        spawn_angle: player.spawn.spawn_angle,
    },
}).await;
```

Similarly, on disconnect, send `DespawnPlayer { uuid }`.

**What changes in `PlayContext`:**
- Add `entity_cmd_tx: EntityCommandSender` field
- Packet handlers that mutate player state also send the corresponding `EntityCommand`

**Tests:**
- Integration: player join creates ECS entity with matching `NetworkId` and `Position`
- Integration: player disconnect despawns ECS entity
- Unit: dual representations have consistent initial state

---

### 23b.9 — Movement sync: forward to ECS (`oxidized-server/src/network/play/movement.rs`) 📋

When the server receives `ServerboundMovePlayerPacket`, forward the new position to
the ECS world via the command channel, in addition to updating `ServerPlayer`.

```rust
// In handle_move_player():
// Existing: update ServerPlayer (stays for now)
play_ctx.player.write().movement.pos = Vec3 { x, y, z };
play_ctx.player.write().movement.yaw = yaw;
play_ctx.player.write().movement.pitch = pitch;

// NEW: forward to ECS
let _ = play_ctx.entity_cmd_tx.try_send(EntityCommand::PlayerMoved {
    uuid: play_ctx.player_uuid,
    position: DVec3::new(x, y, z),
    yaw,
    pitch,
    on_ground,
});
```

**Why `try_send` (non-blocking)?** Network handlers are async and must not block.
If the channel is full (tick thread is behind), we drop the movement update —
the next one will carry the correct position. Movement updates are idempotent
(latest-wins), so dropping intermediate ones is safe.

**Tests:**
- Unit: movement packet updates both `ServerPlayer` and sends `EntityCommand`
- Unit: `try_send` on full channel does not panic

---

### 23b.10 — Core tick systems: tick count, gravity, bounding box (`oxidized-game/src/entity/systems.rs`) 📋

Implement the first set of ECS systems that replace procedural entity logic. These
are the minimum needed to prove the schedule works end-to-end.

```rust
/// PreTick: increment tick count for all entities.
pub fn tick_count_system(mut query: Query<&mut TickCount>) {
    for mut tc in &mut query {
        tc.0 = tc.0.wrapping_add(1);
    }
}

/// Physics: apply gravity to all entities without NoGravity marker.
pub fn gravity_system(
    mut query: Query<&mut Velocity, Without<NoGravity>>,
) {
    for mut vel in &mut query {
        vel.0.y -= 0.08; // vanilla gravity constant
    }
}

/// Physics: apply velocity to position (simplified — full collision in P23c+).
pub fn velocity_apply_system(
    mut query: Query<(&mut Position, &Velocity, &mut OnGround)>,
) {
    for (mut pos, vel, mut on_ground) in &mut query {
        pos.0 += vel.0;
        // Simplified ground check — will be replaced with full AABB sweep
        if pos.0.y < 0.0 {
            pos.0.y = 0.0;
            on_ground.0 = true;
        }
    }
}

/// PostTick: recalculate bounding box from position + dimensions.
pub fn bounding_box_update_system(
    mut query: Query<(&Position, &Dimensions, &mut BoundingBox), Changed<Position>>,
) {
    for (pos, dims, mut bbox) in &mut query {
        bbox.0 = Aabb::from_center(
            pos.0.x, pos.0.y, pos.0.z,
            f64::from(dims.width),
            f64::from(dims.height),
        );
    }
}
```

**Note:** Player entities skip gravity (players are server-authoritative — position
comes from client packets, not physics simulation). The gravity system applies to
non-player entities (items, mobs). A `PlayerMarker` filter exclusion will be added
when needed, but for now players just don't move from gravity since their position
is overwritten by `PlayerMoved` commands.

**Tests:**
- Unit: `tick_count_system` increments all entities' tick count
- Unit: `gravity_system` reduces Y velocity by 0.08
- Unit: `gravity_system` skips entities with `NoGravity`
- Unit: `bounding_box_update_system` only runs on entities with changed `Position`
- Integration: full PreTick → Physics → PostTick cycle updates all correctly

---

### 23b.11 — Entity data sync system (`oxidized-game/src/entity/systems.rs`) 📋

Implement the `NetworkSync` phase system that detects dirty `SynchedEntityData` and
produces outbound packets. This replaces the manual per-handler dirty-checking that
currently happens in `entity_tracking.rs`.

```rust
/// Resource: outbound packet buffer, drained by the network layer.
#[derive(Resource, Default)]
pub struct OutboundEntityPackets(pub Vec<(i32, Packet)>);

/// NetworkSync: serialize dirty entity data into packets.
pub fn entity_data_sync_system(
    mut query: Query<(&NetworkId, &mut SynchedData)>,
    mut outbound: ResMut<OutboundEntityPackets>,
) {
    for (net_id, mut synched) in &mut query {
        if synched.0.is_dirty() {
            let entries = synched.0.pack_dirty();
            if !entries.is_empty() {
                let packet = ClientboundSetEntityDataPacket::new(net_id.0, entries);
                outbound.0.push((net_id.0, packet.into()));
            }
        }
    }
}

/// NetworkSync: detect position changes and produce move packets.
pub fn position_sync_system(
    query: Query<(&NetworkId, &Position, &Rotation), Changed<Position>>,
    mut outbound: ResMut<OutboundEntityPackets>,
) {
    for (net_id, pos, rot) in &query {
        // Produce ClientboundMoveEntityPacket or ClientboundTeleportEntityPacket
        // depending on delta magnitude
        todo!()
    }
}
```

After the schedule runs, the tick loop drains `OutboundEntityPackets` and broadcasts
them via the existing `broadcast_tx` channel.

**Tests:**
- Unit: dirty synched data produces `SetEntityData` packet
- Unit: clean synched data produces nothing
- Unit: changed position produces movement packet
- Unit: outbound buffer cleared each tick

---

### 23b.12 — Entity tracking integration (`oxidized-server/src/network/play/entity_tracking.rs`) 📋

Wire the existing `EntityTracker` into the ECS as a `Resource`, and create a
`PostTick` system that updates tracking sets based on entity positions.

```rust
/// Resource: wraps the existing EntityTracker.
#[derive(Resource)]
pub struct TrackerResource(pub EntityTracker);

/// PostTick: update entity tracking based on current positions.
pub fn entity_tracking_system(
    entities: Query<(&NetworkId, &Position, &EntityTypeName)>,
    players: Query<(&NetworkId, &Position, &EntityUuid), With<PlayerMarker>>,
    mut tracker: ResMut<TrackerResource>,
    mut outbound: ResMut<OutboundEntityPackets>,
) {
    // For each tracked entity, compute which players are in range
    // Compare against previous tick's watching set
    // New watchers: queue spawn packet (AddEntity + SetEntityData)
    // Lost watchers: queue despawn packet (RemoveEntities)
    todo!()
}
```

**Tracking range selection** by entity type (from `tracker.rs` constants):

| Marker | Range |
|--------|-------|
| `PlayerMarker` | 512 blocks (32 chunks) |
| `ItemEntityMarker` | 96 blocks (6 chunks) |
| `ExperienceOrbMarker` | 96 blocks (6 chunks) |
| `ZombieMarker` / hostile mobs | 128 blocks (8 chunks) |
| `CowMarker` / passive mobs | 160 blocks (10 chunks) |
| `ArrowMarker` / projectiles | 64 blocks (4 chunks) |

Entity registration in `EntityTracker` happens when the `SpawnPlayer` (or
`SpawnEntity`) command is processed in `PreTick`.

**Tests:**
- Integration: player entering range of entity → spawn packet queued
- Integration: player leaving range → despawn packet queued
- Unit: tracking ranges match constants per entity type

---

### 23b.13 — Remove monolithic `Entity` struct (`oxidized-game/src/entity/mod.rs`) 📋

Remove the monolithic `Entity` struct and migrate all remaining callers to use
ECS components. The struct's fields have been decomposed into components (tasks
23b.3–23b.4), and its methods (`set_pos`, `get_flag`, `set_flag`, etc.) become
free functions or component methods.

**Migration map:**

| `Entity` method/field | Replacement |
|---|---|
| `Entity::new(type, w, h)` | `commands.spawn(BaseEntityBundle::new(...))` |
| `entity.id` | `Query<&NetworkId>` |
| `entity.uuid` | `Query<&EntityUuid>` |
| `entity.pos` | `Query<&Position>` |
| `entity.set_pos(x, y, z)` | `pos.0 = DVec3::new(x, y, z)` |
| `entity.velocity` | `Query<&Velocity>` |
| `entity.rotation` | `Query<&Rotation>` |
| `entity.is_on_ground` | `Query<&OnGround>` |
| `entity.bounding_box` | `Query<&BoundingBox>` |
| `entity.synched_data` | `Query<&SynchedData>` |
| `entity.dimensions` | `Query<&Dimensions>` |
| `entity.fall_distance` | `Query<&FallDistance>` |
| `entity.get_flag(bit)` | `EntityFlags::get(bit)` helper method |
| `entity.set_flag(bit, v)` | `EntityFlags::set(bit, v)` helper method |
| `entity.is_on_fire()` | `flags.0 & (1 << FLAG_ON_FIRE) != 0` |

**Keep in `mod.rs`:** `EntityRotation`, `EntityDimensions` as plain value types
(they're useful as non-ECS utility types).

**`physics/tick.rs` signature changes:**
```rust
// Before:
pub fn physics_tick(entity: &mut Entity, ...) { }

// After:
pub fn physics_tick(
    pos: &mut Position,
    vel: &mut Velocity,
    on_ground: &mut OnGround,
    fall_dist: &mut FallDistance,
    bbox: &BoundingBox,
    dims: &Dimensions,
    level: &impl BlockGetter,
    shape_provider: &impl BlockShapeProvider,
    in_water: bool,
    in_lava: bool,
) { }
```

**Tests:**
- Compilation: no remaining references to `entity::Entity` in non-test code
- All existing entity tests still pass (adapted to component API)
- `grep -r "entity::Entity" --include="*.rs"` returns zero non-test hits

---

### 23b.14 — Retain `ServerPlayer` as a thin network-side cache (`oxidized-game/src/player/server_player.rs`) 📋

`ServerPlayer` is used in ~30 packet handlers via `PlayContext`. Removing it entirely
in this phase would be too disruptive. Instead, **thin it down** to a network-side
cache that mirrors a subset of the ECS entity's state.

**Strategy:**
1. Remove fields that are now exclusively in ECS (fall_distance, absorption, armor,
   equipment — only used by tick-thread systems)
2. Keep fields that packet handlers need for immediate response (position, inventory,
   game mode, health, food — needed to respond without querying the ECS world)
3. Add a `ecs_entity: Option<bevy_ecs::entity::Entity>` field for future cross-reference
4. Mark `ServerPlayer` with a doc comment: "Network-side cache. Source of truth is
   the ECS entity. Will be fully removed when packet handlers migrate to ECS queries."

**Fields to keep (network-side cache):**
- `entity_id`, `uuid`, `name`, `profile`
- `movement` (position, rotation, flags — needed for immediate packet responses)
- `game_mode`, `abilities` (needed for permission checks in handlers)
- `inventory` (needed for creative-mode slot setting, equipment broadcasting)
- `combat.health`, `combat.food_level` (needed for respawn, food packets)
- `experience` (needed for XP bar packets)
- `spawn`, `connection`, `teleport`, `mining`, `raw_nbt`

**Fields to remove:**
- None in this phase (keep all for compatibility). Mark with `// TODO(23b): migrate to ECS-only`

**Tests:**
- All existing packet handler tests still pass
- `ServerPlayer` and ECS entity have consistent state after join

---

### 23b.15 — Update `PlayContext` with command sender (`oxidized-server/src/network/play/mod.rs`) 📋

Add the `EntityCommandSender` to `PlayContext` so packet handlers can send entity
mutations to the tick thread.

```rust
pub struct PlayContext<'a> {
    pub conn_handle: &'a ConnectionHandle,
    pub player: &'a Arc<RwLock<ServerPlayer>>,
    pub server_ctx: &'a Arc<ServerContext>,
    pub player_name: &'a str,
    pub player_uuid: uuid::Uuid,
    pub addr: SocketAddr,
    pub chunk_tracker: &'a mut PlayerChunkTracker,
    pub rate_limiter: &'a mut ChatRateLimiter,
    /// NEW: Channel to send entity commands to the tick thread.
    pub entity_cmd_tx: &'a EntityCommandSender,
}
```

Update `handle_play_split()` to accept an `EntityCommandSender` parameter and
pass it into every `PlayContext` construction.

**Tests:**
- Compilation: all packet handler call sites updated
- Integration: `PlayContext` can send commands to tick thread

---

## Performance Targets

| Scenario | Target | Notes |
|----------|--------|-------|
| Schedule run (empty world, 7 phases) | < 10 µs | Overhead of phase iteration |
| PreTick drain (100 commands) | < 50 µs | Channel drain + entity spawn/update |
| Gravity + velocity (1000 entities) | < 200 µs | Parallel within Physics phase |
| Entity data sync (100 dirty entities) | < 100 µs | Pack + serialize |
| Entity tracking update (50 players × 200 entities) | < 500 µs | Distance checks |
| Full tick (100 players, 500 entities) | < 5 ms | Leaves 45 ms budget for world tick |

---

## Dependencies

- **Requires:**
  - Phase 15 (Entity Framework) — component definitions, bundles, markers, phases, tracker
  - Phase 16 (Physics) — AABB collision, gravity constants
  - Phase 22 (Block Interaction) — context for block-aware physics
- **Required by:**
  - Phase 23c (Dropped Items) — item entity systems added to schedule
  - Phase 24 (Combat) — damage/health systems
  - Phase 25 (Hostile Mobs) — mob AI systems
  - Phase 27 (Animals) — animal behavior systems
  - Phase 34 (Loot Tables) — loot spawn integration
- **Crate deps:** `bevy_ecs = "0.18"` (already in workspace)

---

## Migration Strategy

This phase uses a **dual-representation approach**:

```
┌──────────────────┐           ┌──────────────────┐
│  Network Tasks   │           │   Tick Thread     │
│  (Tokio async)   │           │  (OS thread)      │
│                  │           │                   │
│  PlayContext     │──cmds──→  │  EcsContext        │
│    .player       │           │    .world          │
│    (ServerPlayer)│           │    (bevy World)    │
│    .entity_cmd_tx│           │    .schedules      │
│                  │           │                   │
│  ← Arc<RwLock>   │           │  ← exclusive own   │
└──────────────────┘           └──────────────────┘
```

1. **Phase 23b (this):** Both exist. Commands flow network → tick. ECS world is
   source of truth for entity state. `ServerPlayer` is a network-side cache.
2. **Phase 24+:** Packet handlers incrementally stop mutating `ServerPlayer` and
   instead send `EntityCommand` values. `ServerPlayer` fields are removed one by one.
3. **Future:** `ServerPlayer` is fully replaced by ECS queries via a response channel
   (handler sends query request, tick thread responds with snapshot).

---

## Completion Criteria

- [ ] `bevy_ecs::World` created and owned by tick thread
- [ ] 7-phase `Schedule` runs every tick in correct order
- [ ] `EntityCommand` channel bridges network ↔ tick thread
- [ ] Player join creates ECS entity via `SpawnPlayer` command
- [ ] Player disconnect despawns ECS entity
- [ ] Player movement updates `Position` component in ECS
- [ ] `tick_count_system` increments all entity tick counters
- [ ] `gravity_system` applies to non-player, non-NoGravity entities
- [ ] `entity_data_sync_system` serializes dirty data into packets
- [ ] `EntityTracker` integrated as ECS Resource
- [ ] Monolithic `Entity` struct removed from `entity/mod.rs`
- [ ] `PlayerBundle` contains all fields from `ServerPlayer`
- [ ] Existing tests still pass (movement, chat, commands, inventory)
- [ ] Existing client-visible behavior is unchanged (regression-free)
- [ ] Performance: full tick < 5 ms with 100 players, 500 entities
- [ ] All tests pass: unit, integration, property-based
