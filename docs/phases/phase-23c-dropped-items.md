# Phase 23c — Dropped Items (Item Entities)

**Status:** 📋 Planned
**Crate:** `oxidized-game`, `oxidized-server`, `oxidized-protocol`
**Reward:** Breaking blocks scatters item drops on the ground; players can drop items
with Q; items bob, merge, get picked up with an animation, and despawn after 5 minutes.

**Depends on:** Phase 15 (Entity Framework), Phase 16 (Physics), Phase 21 (Inventory),
Phase 22 (Block Interaction), Phase 23b (ECS Runtime Integration)
**Required by:** Phase 24 (Combat — death drops), Phase 25 (Hostile Mobs — mob loot),
Phase 27 (Animals — animal loot), Phase 34 (Loot Tables — full loot evaluation)

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-018: Entity System](../adr/adr-018-entity-system.md) — bevy_ecs components, markers, tick phases
- [ADR-026: Loot Tables](../adr/adr-026-loot-tables.md) — enum-based loot tree (Phase 34 builds on our spawning)
- [ADR-019: Tick Loop](../adr/adr-019-tick-loop.md) — dedicated tick thread, parallel system phases
- [ADR-029: Memory Management](../adr/adr-029-memory-management.md) — arena allocator for per-tick work

---

## Goal

Implement the full lifecycle of **item entities** (dropped items): spawning, physics,
merging, pickup, despawn, and network synchronization. This phase bridges the gap between
the inventory system (Phase 21) and block interaction (Phase 22) by making items exist as
physical entities in the world. Currently, `drop_item()` in `mining.rs` discards the
returned `ItemStack` — after this phase, every dropped item becomes a visible, collectible
entity with vanilla-accurate behavior.

This phase intentionally uses **simple block-to-item-stack mapping** (block → 1 item stack
of that block) rather than full loot tables. Phase 34 will replace the simple mapping with
the complete loot table evaluation engine (Fortune, Silk Touch, conditional drops, etc.).

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Item entity lifecycle | `ItemEntity` | `net.minecraft.world.entity.item.ItemEntity` |
| Block drops | `Block.popResource()` | `net.minecraft.world.level.block.Block` |
| Player item drop | `LivingEntity.drop()` | `net.minecraft.world.entity.LivingEntity` |
| Drop spawn helper | `Entity.spawnAtLocation()` | `net.minecraft.world.entity.Entity` |
| Pickup animation packet | `ClientboundTakeItemEntityPacket` | `net.minecraft.network.protocol.game` |
| Entity spawn packet | `ClientboundAddEntityPacket` | `net.minecraft.network.protocol.game` |
| Entity metadata sync | `SynchedEntityData` | `net.minecraft.network.syncher.SynchedEntityData` |

---

## Tasks

### 23c.1 — Item entity ECS components (`oxidized-game/src/entity/item_entity.rs`) 📋

Define the ECS components and bundle that represent a dropped item in the world. The
`ItemEntityMarker` already exists in `markers.rs` — this task adds the data components
and the spawn bundle.

Vanilla `ItemEntity` has: item stack, age (ticks alive), pickup delay, health, owner
(who can pick it up), and thrower (who dropped it). We model each as a separate ECS
component for query flexibility.

```rust
use bevy_ecs::prelude::*;

/// The item stack this entity represents on the ground.
#[derive(Component, Debug, Clone)]
pub struct DroppedItem(pub ItemStack);

/// Ticks since the item was spawned. Despawns at 6000 (5 minutes).
/// Special values: -32768 = infinite lifetime (never despawn).
#[derive(Component, Debug, Clone, Copy)]
pub struct ItemAge(pub i32);

/// Ticks remaining before this item can be picked up.
/// Special value: 32767 = never pickupable.
/// Defaults: 10 (block drops), 40 (player-thrown), 0 (immediate).
#[derive(Component, Debug, Clone, Copy)]
pub struct PickupDelay(pub i32);

/// Health points of the item entity. Default: 5.
/// Destroyed when reduced to 0 (fire, explosions, cactus, void).
#[derive(Component, Debug, Clone, Copy)]
pub struct ItemHealth(pub i16);

/// UUID of the only player allowed to pick up this item.
/// `None` = anyone can pick it up.
#[derive(Component, Debug, Clone, Copy)]
pub struct ItemOwner(pub Option<Uuid>);

/// UUID of the entity that dropped this item (for pickup priority).
#[derive(Component, Debug, Clone, Copy)]
pub struct ItemThrower(pub Option<Uuid>);

/// Bundle for spawning a complete item entity.
#[derive(Bundle)]
pub struct ItemEntityBundle {
    pub base: BaseEntityBundle,
    pub marker: ItemEntityMarker,
    pub item: DroppedItem,
    pub age: ItemAge,
    pub pickup_delay: PickupDelay,
    pub health: ItemHealth,
    pub owner: ItemOwner,
    pub thrower: ItemThrower,
}
```

**Constants** (from vanilla `ItemEntity.java`):

```rust
pub mod item_entity_constants {
    /// Ticks before despawn (6000 = 5 minutes at 20 TPS).
    pub const LIFETIME: i32 = 6000;
    /// Age value that prevents despawn entirely.
    pub const INFINITE_LIFETIME: i32 = -32768;
    /// Pickup delay value that prevents pickup entirely.
    pub const INFINITE_PICKUP_DELAY: i32 = 32767;
    /// Default pickup delay for block drops.
    pub const DEFAULT_PICKUP_DELAY: i32 = 10;
    /// Pickup delay for player-thrown items (Q key).
    pub const PLAYER_DROP_PICKUP_DELAY: i32 = 40;
    /// Default health points for item entities.
    pub const DEFAULT_HEALTH: i16 = 5;
    /// Entity type ID for items (vanilla registry).
    pub const ITEM_ENTITY_TYPE_ID: i32 = 2;
    /// Tracking range in blocks (6 chunks).
    pub const ITEM_TRACKING_RANGE: i32 = 96;
    /// Item entity dimensions (width × height in blocks).
    pub const ITEM_WIDTH: f32 = 0.25;
    pub const ITEM_HEIGHT: f32 = 0.25;
}
```

**Tests:**
- Unit: `ItemEntityBundle` construction with defaults
- Unit: constants match vanilla values
- Unit: `ItemAge`, `PickupDelay` special-value semantics

---

### 23c.2 — Item entity synched data slot (`oxidized-game/src/entity/data_slots.rs`) 📋

Register the `DATA_ITEM` synched data slot for item entities so clients know which item
to render. Vanilla uses slot index 8 (after the 8 base entity slots 0–7) with serializer
type `ITEM_STACK`.

```rust
// Item Entity data slots (EntityDataSerializers.ITEM_STACK)
/// The ItemStack displayed by this item entity. Slot 8.
pub const DATA_ITEM_STACK: u8 = 8;
```

The item stack must be written into `SynchedEntityData` when the item entity spawns and
whenever the stack changes (e.g., after a merge). The `entity_data_sync_system` in
`NetworkSync` phase will serialize dirty item stacks into `ClientboundSetEntityDataPacket`.

**Integration:**
- Initialize `DATA_ITEM_STACK` in `SynchedEntityData::define()` during item entity spawn
- Mark dirty after merges or count changes

**Tests:**
- Unit: synched data slot index matches vanilla (8)
- Unit: dirty flag set when item stack changes
- Compliance: serialized entity data matches vanilla wire format

---

### 23c.3 — Item entity spawn function (`oxidized-game/src/entity/item_entity.rs`) 📋

Create factory functions for spawning item entities with the correct initial velocity
depending on the drop source. Vanilla has three patterns:

1. **Block drop** (`Block.popResource`): spawn at block center ± 0.25, random low velocity
2. **Player throw** (`LivingEntity.drop`): spawn at eye height − 0.3, velocity along look direction × 0.3
3. **Death scatter** (`Player.dropAll`): spawn at entity pos, random horizontal spread × 0.5 + 0.2 upward

```rust
/// Spawn an item entity from a broken block.
/// Position: block center ± 0.25 random offset.
/// Velocity: small random (±0.1 horizontal, 0.2 upward).
/// Pickup delay: 10 ticks.
pub fn spawn_block_drop(
    commands: &mut Commands,
    pos: BlockPos,
    item: ItemStack,
    rng: &mut impl Rng,
) -> Entity { todo!() }

/// Spawn an item entity thrown by a player (Q key).
/// Position: player eye height − 0.3.
/// Velocity: 0.3 × look direction + small random deviation.
/// Pickup delay: 40 ticks. Thrower set to player UUID.
pub fn spawn_player_drop(
    commands: &mut Commands,
    player_pos: DVec3,
    player_eye_height: f64,
    yaw: f32,
    pitch: f32,
    item: ItemStack,
    player_uuid: Uuid,
    rng: &mut impl Rng,
) -> Entity { todo!() }

/// Spawn an item entity from a dying entity (death loot scatter).
/// Position: entity position + 0.5 upward.
/// Velocity: random horizontal spread (0.5 magnitude), 0.2 upward.
/// Pickup delay: 10 ticks (default).
pub fn spawn_death_drop(
    commands: &mut Commands,
    entity_pos: DVec3,
    item: ItemStack,
    rng: &mut impl Rng,
) -> Entity { todo!() }
```

Each function allocates an entity ID via `next_entity_id()`, generates a UUID, constructs
the `ItemEntityBundle` with appropriate `Velocity`, `Position`, `PickupDelay`, and inserts
the ECS entity via `commands.spawn(bundle)`.

**Vanilla velocity formulas:**
- **Block drop**: `vx = rand(-0.1, 0.1)`, `vy = 0.2`, `vz = rand(-0.1, 0.1)`
- **Player throw**: `vx = -sin(yaw) * cos(pitch) * 0.3 + rand * 0.02`,
  `vy = -sin(pitch) * 0.3 + 0.1 + rand * 0.02`,
  `vz = cos(yaw) * cos(pitch) * 0.3 + rand * 0.02`
- **Death scatter**: `vx = -sin(rand_angle) * rand(0, 0.5)`, `vy = 0.2`,
  `vz = cos(rand_angle) * rand(0, 0.5)`

**Tests:**
- Unit: block drop position is within ±0.25 of block center
- Unit: player throw velocity direction matches look direction
- Unit: death scatter velocity always has positive Y component
- Property: all spawn functions produce valid entity bundles

---

### 23c.4 — Item entity physics system (`oxidized-game/src/entity/item_entity.rs`) 📋

Add an ECS system that runs in the **Physics** tick phase to apply gravity, friction,
collision, and bounce to all item entities. This extends the existing physics engine
(`oxidized-game/src/physics/`) with item-entity-specific behavior.

Vanilla item entity physics (from `ItemEntity.tick()`):

| Property | Value | Notes |
|----------|-------|-------|
| Gravity | 0.04 blocks/tick² | Standard entity gravity |
| Horizontal drag | × 0.98 per tick | Applied after ground friction |
| Vertical drag | × 0.98 per tick | Air resistance |
| Ground friction | × block friction | From block below (ice = 0.989, default = 0.6) |
| Bounce | vy × −0.5 | When hitting ground with downward velocity |
| Water buoyancy | vy += 0.014 per tick | Float upward in water |
| Water drag | × 0.99 all axes | Slower in water |
| Lava drag | × 0.949 all axes | Very slow in lava |

```rust
/// ECS system: applies physics to all item entities.
/// Runs in the Physics tick phase.
pub fn item_entity_physics_system(
    mut query: Query<
        (&mut Position, &mut Velocity, &mut OnGround),
        With<ItemEntityMarker>,
    >,
    level: Res<ServerLevel>,
) {
    for (mut pos, mut vel, mut on_ground) in query.iter_mut() {
        // 1. Apply gravity: vel.y -= 0.04
        // 2. Move with AABB collision (reuse physics::collision)
        // 3. Bounce on ground hit: vel.y *= -0.5
        // 4. Apply friction: vel.x *= 0.98, vel.z *= 0.98
        // 5. Apply drag: vel *= 0.98 (or 0.99 in water, 0.949 in lava)
        // 6. Clamp near-zero velocity to zero
        // 7. Update on_ground state
        todo!()
    }
}
```

Item entities use a small bounding box (0.25 × 0.25 blocks) and the existing AABB
sweep collision from `physics/collision.rs`. The `noPhysics` flag is set when an item
spawns inside a block (prevents getting stuck in walls after block breaks).

**Optimization:** stationary items (velocity < 0.01) skip full physics and only check
if their supporting block still exists.

**Tests:**
- Unit: gravity reduces Y velocity by 0.04 per tick
- Unit: item on ground has zero Y velocity (no sinking)
- Unit: bounce reverses and halves Y velocity
- Unit: friction reduces horizontal velocity each tick
- Unit: water buoyancy pushes items upward
- Property: item always comes to rest after finite ticks (energy decay)
- Integration: item dropped from height lands on ground correctly

---

### 23c.5 — Item age and despawn system (`oxidized-game/src/entity/item_entity.rs`) 📋

Add an ECS system that runs in the **EntityBehavior** tick phase to increment item age
and despawn items that exceed their lifetime.

```rust
/// ECS system: increments item age and despawns expired items.
/// Runs in the EntityBehavior tick phase.
pub fn item_age_despawn_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut ItemAge), With<ItemEntityMarker>>,
) {
    for (entity, mut age) in query.iter_mut() {
        // Skip infinite lifetime items
        if age.0 == INFINITE_LIFETIME {
            continue;
        }
        age.0 += 1;
        if age.0 >= LIFETIME {
            commands.entity(entity).despawn();
        }
    }
}
```

**Despawn lifecycle:**
- Age starts at 0, incremented every tick
- At age 6000 (5 minutes), entity is despawned
- Age −32768 = infinite (never despawn, used by commands / creative)
- `ClientboundRemoveEntitiesPacket` sent to all tracking players on despawn

**Tests:**
- Unit: age increments by 1 per tick
- Unit: entity despawns at exactly age 6000
- Unit: infinite lifetime items never despawn
- Unit: removal packet sent on despawn

---

### 23c.6 — Item pickup system (`oxidized-game/src/entity/item_entity.rs`) 📋

Add an ECS system that runs in the **EntityBehavior** tick phase (after age/despawn)
to detect when a player is close enough to pick up an item entity, transfer the item
to the player's inventory, and send the pickup animation packet.

Vanilla pickup rules (from `ItemEntity.playerTouch()`):
- Pickup delay must be 0 (not counting down, not infinite)
- Item owner must be `None` or match the player's UUID
- Player inventory must have room for the item stack
- Distance: player bounding box overlaps item bounding box (effectively ~1 block)

```rust
/// ECS system: detects player–item proximity and performs pickup.
/// Runs in the EntityBehavior tick phase.
pub fn item_pickup_system(
    mut commands: Commands,
    item_query: Query<
        (Entity, &Position, &DroppedItem, &PickupDelay, &ItemOwner),
        With<ItemEntityMarker>,
    >,
    player_query: Query<(Entity, &Position), With<PlayerMarker>>,
    // Access to player inventories and network for packets
) {
    for (item_entity, item_pos, dropped, delay, owner) in item_query.iter() {
        // Skip items still on pickup delay
        if delay.0 > 0 {
            continue;
        }

        for (player_entity, player_pos) in player_query.iter() {
            // Check owner restriction
            // Check distance (bounding box overlap, ~1 block)
            // Try to insert into player inventory
            // If successful:
            //   - Send ClientboundTakeItemEntityPacket
            //   - Update or despawn item entity
            //   - Award ITEM_PICKED_UP stat
            todo!()
        }
    }
}

/// ECS system: decrements pickup delay each tick.
pub fn pickup_delay_tick_system(
    mut query: Query<&mut PickupDelay, With<ItemEntityMarker>>,
) {
    for mut delay in query.iter_mut() {
        if delay.0 > 0 && delay.0 != INFINITE_PICKUP_DELAY {
            delay.0 -= 1;
        }
    }
}
```

**`ClientboundTakeItemEntityPacket`** (new packet):

```rust
/// Sent when a player picks up an item entity.
/// Client plays the "fly into player" animation and sound.
pub struct ClientboundTakeItemEntityPacket {
    /// Entity ID of the item being picked up.
    pub item_entity_id: VarInt,
    /// Entity ID of the player collecting it.
    pub collector_entity_id: VarInt,
    /// Number of items collected.
    pub amount: VarInt,
}
```

**Tests:**
- Unit: item with pickup delay > 0 is not picked up
- Unit: item with owner restricts pickup to that player
- Unit: item with `INFINITE_PICKUP_DELAY` is never picked up
- Unit: pickup transfers item to player inventory
- Unit: pickup despawns item if fully collected
- Unit: partial pickup reduces item count (stack > inventory space)
- Unit: pickup delay decrements each tick
- Integration: `ClientboundTakeItemEntityPacket` sent to all tracking players

---

### 23c.7 — Item merge system (`oxidized-game/src/entity/item_entity.rs`) 📋

Add an ECS system that runs in the **EntityBehavior** tick phase to merge nearby
identical item entities into larger stacks, reducing entity count on the ground.

Vanilla merge rules (from `ItemEntity.mergeWithNeighbours()`):
- **Search radius:** 0.5 blocks horizontal (X/Z), no vertical limit check — but
  vanilla uses AABB getEntitiesOfClass which defaults to the item's bounding box expanded
  by 0.5 on each axis
- **Check frequency:** every 2 ticks while moving, every 40 ticks while stationary
- **Merge conditions** (both items must satisfy):
  - Entity is alive
  - `pickup_delay != 32767` (not permanently prevented)
  - `age != -32768` (not infinite lifetime)
  - `age < 6000` (not about to despawn)
  - `count < max_stack_size` (not already a full stack)
  - Same item type, same data components
  - Same owner/target

```rust
/// ECS system: merges nearby identical item entities.
/// Runs in the EntityBehavior tick phase.
pub fn item_merge_system(
    mut commands: Commands,
    mut query: Query<
        (Entity, &Position, &mut DroppedItem, &mut ItemAge, &mut PickupDelay,
         &Velocity, &TickCount, &ItemOwner),
        With<ItemEntityMarker>,
    >,
) {
    // 1. For each item entity, check tick frequency (moving: 2, still: 40)
    // 2. Find nearby items within merge radius
    // 3. Validate merge conditions on both items
    // 4. Transfer from smaller stack → larger stack
    // 5. Update age to min(both ages) — older item survives longer
    // 6. Update pickup delay to max(both delays) — safety
    // 7. Despawn empty source entity
    // 8. Mark destination item's synched data as dirty
    todo!()
}

/// Check if two item stacks can merge.
fn can_merge_items(a: &ItemStack, b: &ItemStack) -> bool {
    a.item_id() == b.item_id()
        && ItemStack::same_components(a, b)
        && a.count() + b.count() <= a.max_stack_size()
}
```

**Merge algorithm (vanilla-accurate):**
1. Smaller stack merges INTO larger stack (preserves the older entity)
2. Destination count = min(src.count + dst.count, max_stack_size)
3. Source count -= transferred amount
4. If source is empty → despawn
5. Destination age = min(dst.age, src.age) — keeps older item's timer
6. Destination pickup_delay = max(dst.delay, src.delay)

**Performance consideration:** Use a spatial index or chunk-local entity list to avoid
O(n²) all-pairs checks. Items only merge within a small radius, so bucketing by chunk
section is sufficient.

**Tests:**
- Unit: 32 + 30 of same item → 62 (merged) + despawned
- Unit: 64 + 10 of same item → no merge (destination full)
- Unit: same item, different components → no merge
- Unit: different owners → no merge
- Unit: infinite pickup delay items → no merge
- Unit: age set to min of both after merge
- Unit: stationary items check merge every 40 ticks, moving every 2
- Property: merge never creates stacks exceeding max_stack_size

---

### 23c.8 — Block break item drops (`oxidized-server/src/network/play/mining.rs`) 📋

Hook into the existing block-breaking code to spawn item entities when a block is
destroyed. Currently `mining.rs` handles the full break sequence but does not create
item drops.

For this phase, use a **simple 1:1 block-to-item mapping**: breaking a block drops
one item stack of that block type. Phase 34 (Loot Tables) will replace this with full
loot table evaluation (Fortune, Silk Touch, conditional drops, multi-item drops, etc.).

Blocks that should NOT drop items in the simple mapping:
- Air, cave air, void air
- Fire, soul fire
- Bedrock (in survival)
- End portal, nether portal blocks
- Spawners (in vanilla, only with Silk Touch — but we have no enchantments yet)
- Water, lava (fluid blocks)

```rust
/// Spawn item drop(s) for a broken block.
/// Simple 1:1 mapping — Phase 34 replaces with loot tables.
pub fn drop_block_items(
    commands: &mut Commands,
    block_pos: BlockPos,
    block_state: BlockStateId,
    _breaker: Option<Uuid>,
    _tool: Option<&ItemStack>,
    rng: &mut impl Rng,
) {
    let block_name = block_state.block_name();
    // Skip non-droppable blocks
    if is_non_droppable_block(block_name) {
        return;
    }
    // Map block → item (most blocks share the name, e.g. "minecraft:stone")
    let item_name = block_to_item_name(block_name);
    if let Some(item_id) = ItemRegistry::item_name_to_id(item_name) {
        let stack = ItemStack::new(item_id, 1);
        spawn_block_drop(commands, block_pos, stack, rng);
    }
}
```

**Integration points:**
- `mining.rs` `handle_player_action` → `FinishBreaking` branch: call `drop_block_items()`
- `cmd_setblock.rs` → destroy mode: call `drop_block_items()` (resolves existing TODO)
- Creative mode: do NOT drop items (vanilla behavior)
- Game rule: respect `doTileDrops` (default true) when implemented

**Tests:**
- Unit: breaking stone spawns 1 stone item entity
- Unit: breaking air spawns nothing
- Unit: breaking bedrock in creative spawns nothing
- Unit: item spawns at block center ± 0.25 offset
- Unit: drop has 10-tick pickup delay (default)
- Integration: breaking block visible to other players as item entity

---

### 23c.9 — Player drop actions (`oxidized-server/src/network/play/mining.rs`) 📋

Complete the existing `PlayerAction::DropItem` and `PlayerAction::DropAllItems` handlers
in `mining.rs` that currently ignore the returned `ItemStack`. Wire them to spawn item
entities with the player-throw velocity formula.

Currently (lines 201–221 in `mining.rs`):
```rust
PlayerAction::DropItem => {
    let _dropped = play_ctx.player.write().inventory.drop_item();
    // ⚠️ _dropped is IGNORED
}
```

After this task:
```rust
PlayerAction::DropItem => {
    let dropped = play_ctx.player.write().inventory.drop_item();
    if let Some(stack) = dropped {
        let player = play_ctx.player.read();
        spawn_player_drop(
            &mut commands,
            player.movement.pos.into(),
            player.eye_height(),
            player.movement.yaw,
            player.movement.pitch,
            stack,
            player.uuid,
            &mut rng,
        );
    }
}

PlayerAction::DropAllItems => {
    let dropped_stacks = play_ctx.player.write().inventory.drop_all_items();
    let player = play_ctx.player.read();
    for stack in dropped_stacks {
        spawn_death_drop(&mut commands, player.movement.pos.into(), stack, &mut rng);
    }
}
```

**Also handle:** Creative mode inventory clicks with slot −999 (drop outside window),
which also drops items.

**Tests:**
- Unit: Q-key drop removes item from held slot and spawns entity
- Unit: dropped item has 40-tick pickup delay
- Unit: dropped item velocity follows look direction
- Unit: Ctrl+Q drops entire stack
- Unit: drop with empty hand does nothing
- Integration: other players see the dropped item entity spawn

---

### 23c.10 — Network: spawn, sync, and pickup packets (`oxidized-server/src/network/play/entity_tracking.rs`) 📋

Extend the entity tracking and network sync systems to handle item entities:

**Spawn:** When a player enters tracking range of an item entity, send:
1. `ClientboundAddEntityPacket` with `entity_type = ITEM_ENTITY_TYPE_ID` (2)
2. `ClientboundSetEntityDataPacket` with `DATA_ITEM_STACK` slot containing the item
3. `ClientboundSetEntityMotionPacket` with current velocity

**Movement sync:** Item entities use the existing delta-encoding movement system
(`EntityMoveKind::Delta` for small moves, `::Sync` for teleports > 8 blocks).
Stationary items stop sending movement packets.

**Metadata sync:** When `DroppedItem` changes (after merge), mark `DATA_ITEM_STACK`
dirty → `entity_data_sync_system` sends `ClientboundSetEntityDataPacket`.

**Pickup animation:** New packet `ClientboundTakeItemEntityPacket`:
```rust
/// Packet ID: 0x77 (play, clientbound)
/// Sent to all players tracking the item when it is picked up.
pub struct ClientboundTakeItemEntityPacket {
    pub item_entity_id: VarInt,
    pub collector_entity_id: VarInt,
    pub amount: VarInt,
}

impl ClientboundPacket for ClientboundTakeItemEntityPacket {
    const PACKET_ID: i32 = 0x77;

    fn write(&self, buf: &mut BytesMut) -> Result<()> {
        VarInt(self.item_entity_id).write(buf)?;
        VarInt(self.collector_entity_id).write(buf)?;
        VarInt(self.amount).write(buf)?;
        Ok(())
    }
}
```

**Despawn:** `ClientboundRemoveEntitiesPacket` (already exists, ID 0x47) sent when:
- Item is picked up completely
- Item age reaches 6000 (despawn)
- Item health reaches 0 (destroyed)
- Player leaves tracking range

**Entity tracker registration:** item entities register with `TRACKING_RANGE_MISC` (96
blocks / 6 chunks). Extend `EntityTracker::register()` to handle item entities.

**Tests:**
- Compliance: `ClientboundTakeItemEntityPacket` wire format matches vanilla
- Unit: spawn sends AddEntity + SetEntityData + SetEntityMotion
- Unit: pickup sends TakeItemEntity + RemoveEntities
- Unit: item entity tracking range is 96 blocks
- Integration: item visible to player entering range, invisible when leaving

---

### 23c.11 — Tick loop integration (`oxidized-server/src/tick.rs`) 📋

Register all item entity ECS systems in the tick loop at the correct phases:

```rust
// Physics phase — runs in parallel with other physics systems
schedule.add_system(item_entity_physics_system.in_set(TickPhase::Physics));

// EntityBehavior phase — sequential within phase
schedule.add_system(
    pickup_delay_tick_system
        .in_set(TickPhase::EntityBehavior)
);
schedule.add_system(
    item_age_despawn_system
        .in_set(TickPhase::EntityBehavior)
        .after(pickup_delay_tick_system)
);
schedule.add_system(
    item_merge_system
        .in_set(TickPhase::EntityBehavior)
        .after(item_age_despawn_system)
);
schedule.add_system(
    item_pickup_system
        .in_set(TickPhase::EntityBehavior)
        .after(item_merge_system)
);

// NetworkSync phase — dirty data broadcast
// (existing entity_data_sync_system handles item entities automatically)
```

**System execution order within EntityBehavior:**
1. `pickup_delay_tick_system` — decrement delays
2. `item_age_despawn_system` — age + despawn expired
3. `item_merge_system` — merge nearby identical items
4. `item_pickup_system` — player collects eligible items

This order ensures: delays tick down before pickup checks, dead items don't merge,
and merges happen before pickup (so players pick up the merged stack, not two separate
items in the same tick).

**Tests:**
- Integration: full tick cycle processes item spawn → physics → age → merge → pickup
- Unit: system ordering enforced (pickup after merge after despawn after delay)

---

### 23c.12 — Block-to-item mapping data (`oxidized-game/src/entity/block_drops.rs`) 📋

Create the simple block-to-item mapping used by task 23c.8. Most blocks drop themselves
(stone → stone, dirt → dirt), but several have special mappings:

| Block | Drops | Notes |
|-------|-------|-------|
| `minecraft:stone` | `minecraft:cobblestone` | Without Silk Touch |
| `minecraft:grass_block` | `minecraft:dirt` | Without Silk Touch |
| `minecraft:tall_grass` | nothing | Seeds require loot tables |
| `minecraft:glass` | nothing | Without Silk Touch |
| `minecraft:ice` | nothing | Without Silk Touch |
| `minecraft:infested_*` | nothing | Silverfish blocks |
| `minecraft:spawner` | nothing | Without Silk Touch |
| `minecraft:farmland` | `minecraft:dirt` | Always |
| `minecraft:dirt_path` | `minecraft:dirt` | Always |
| Ores | Raw form | e.g., diamond_ore → diamond |
| Double slabs | 2× slab item | Half-slab × 2 |
| Beds | `minecraft:*_bed` | Color-matched |
| Doors | `minecraft:*_door` | Only bottom half drops |

```rust
/// Map a block name to its drop item name (simple 1:1 mapping).
/// Returns `None` for blocks that drop nothing.
/// Phase 34 (Loot Tables) replaces this with full loot evaluation.
pub fn block_to_item_name(block_name: &str) -> Option<&str> {
    // 1. Check explicit overrides (stone → cobblestone, etc.)
    // 2. Check non-droppable set (air, fire, portals, etc.)
    // 3. Default: block name == item name (most blocks)
    todo!()
}

/// Blocks that never drop items regardless of tool.
pub fn is_non_droppable_block(block_name: &str) -> bool {
    matches!(block_name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
        | "minecraft:fire" | "minecraft:soul_fire"
        | "minecraft:water" | "minecraft:lava"
        | "minecraft:nether_portal" | "minecraft:end_portal"
        | "minecraft:end_gateway" | "minecraft:frosted_ice"
        | "minecraft:piston_head" | "minecraft:moving_piston"
        | "minecraft:budding_amethyst"
    )
}
```

Consider generating this mapping from vanilla data (similar to how block/item registries
are generated in `build.rs`). The `mc-server-ref/` decompiled source contains the
loot table JSON files that define exact drop behavior.

**Tests:**
- Unit: stone → cobblestone
- Unit: dirt → dirt (identity)
- Unit: air → None
- Unit: glass → None
- Unit: all 1506+ items have a valid mapping or explicit exclusion
- Property: `block_to_item_name` never returns an unknown item name

---

## Performance Targets

| Scenario | Target | Notes |
|----------|--------|-------|
| 1000 item entities physics tick | < 500 µs | Parallel within Physics phase |
| Item merge scan (100 items in chunk) | < 100 µs | Spatial bucketing by chunk section |
| Pickup check (50 players × 200 items) | < 200 µs | Early-exit on distance check |
| Spawn packet serialization | < 5 µs | Per entity, 3 packets |
| Steady-state (no moving items) | < 50 µs | Stationary items skip full physics |

---

## Dependencies

- **Requires:**
  - Phase 15 (Entity Framework) — ECS components, `BaseEntityBundle`, `next_entity_id()`
  - Phase 16 (Physics) — AABB collision, gravity, friction infrastructure
  - Phase 21 (Inventory) — `PlayerInventory`, `add_item()`, `drop_item()`
  - Phase 22 (Block Interaction) — block break handling in `mining.rs`
- **Required by:**
  - Phase 24 (Combat) — death drops use `spawn_death_drop()`
  - Phase 25 (Hostile Mobs) — mob death loot drops
  - Phase 27 (Animals) — animal death loot drops
  - Phase 34 (Loot Tables) — replaces simple block-to-item mapping with full loot evaluation
- **Crate deps:** No new external dependencies. Uses existing `bevy_ecs`, `uuid`, `rand`.

---

## Completion Criteria

- [ ] Breaking a block spawns a visible item entity at the block position
- [ ] Item entities fall with gravity and come to rest on ground
- [ ] Item entities bob/spin visually on the client (via synched data)
- [ ] Walking over an item picks it up with fly-to-player animation
- [ ] Picked-up items appear in the correct inventory slot
- [ ] Q-key drops the held item as an entity with directional velocity
- [ ] Ctrl+Q drops the entire held stack
- [ ] Identical items on the ground merge into a single entity
- [ ] Items despawn after 5 minutes (6000 ticks)
- [ ] Item pickup respects delay timers (10 ticks for block, 40 for thrown)
- [ ] Item entities are visible/hidden based on tracking range (96 blocks)
- [ ] Multiple players see the same item entities and pickup animations
- [ ] Creative mode block breaking does NOT drop items
- [ ] All tests pass: unit, integration, property-based, compliance
- [ ] Performance targets met (benchmarked with 1000 item entities)
