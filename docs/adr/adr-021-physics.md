# ADR-021: Physics & Collision Engine

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P16 |
| Deciders | Oxidized Core Team |

## Context

Minecraft's physics system governs how entities move through the world: gravity pulls
entities downward, velocity is applied each tick, movement is checked against block
collision shapes, and various environmental effects (water buoyancy, lava drag, soul sand
slowdown, honey blocks, slime bouncing) modify the result. The physics simulation runs
every tick for every entity in every loaded chunk. On a server with 10,000 loaded entities,
the physics system is one of the most CPU-intensive per-tick operations.

Vanilla's physics implementation centers on `Entity.move(MoverType, Vec3)`, which performs
a per-axis sweep test. The entity's proposed movement vector is decomposed into X, Y, and Z
components. For each axis, the entity's AABB (axis-aligned bounding box) is expanded along
that axis by the movement amount, all block collision shapes within the expanded AABB are
collected, and the movement is clamped to the nearest collision. This per-axis approach is
simple and correct for axis-aligned geometries — it handles stairs, slabs, fences, walls,
and all of Minecraft's complex VoxelShapes without needing a general-purpose physics engine.
The collision shapes themselves are precomputed per block state via `BlockState.getCollisionShape()`,
which returns a `VoxelShape` — a union of AABBs that precisely models each block's geometry
(a stair block has multiple boxes, a slab is a half-height box, a fence has a center post
plus arm extensions).

This physics model is not merely a server-side implementation detail — it is a core part of
the multiplayer protocol contract. The Minecraft client runs the same physics simulation
locally for movement prediction. When a player presses W to walk forward, the client
predicts the resulting position using the same gravity constant (0.08), the same drag values
(0.91 horizontal in air), the same step-up height (0.6 blocks), and the same collision
algorithm. If the server's physics produces a different result, the client visually
"rubberbands" — the player snaps to the server's position. This means we cannot innovate on
the physics model without causing visible desync for all players. The physics must match
vanilla exactly, including edge cases and floating-point behavior.

## Decision Drivers

- **Exact vanilla parity**: The physics output (final position, velocity, onGround flag,
  fall distance) must match vanilla for any given initial state and movement input. This is
  not optional — client prediction depends on it.
- **Performance at scale**: Physics runs for every entity every tick. With 10,000 entities,
  the physics phase budget is ~5ms (10% of a 50ms tick). This requires efficient collision
  shape lookup and minimal allocation.
- **Correct VoxelShape handling**: Minecraft has hundreds of unique collision shapes (full
  blocks, slabs, stairs, walls, fences, trapdoors, beds, chests, brewing stands, etc.).
  Each must be represented correctly.
- **Environmental effect accuracy**: Water buoyancy, lava drag, soul sand, honey, powder
  snow, cobwebs, bubble columns — each modifier must apply in the correct order with the
  correct constants.
- **Step-up compatibility**: Vanilla's step-up algorithm (entities walk up blocks ≤ 0.6
  blocks tall without jumping) has specific behavior that players rely on, especially for
  stair navigation and farm designs.

## Considered Options

### Option 1: Per-Axis Sweep Like Vanilla

Replicate vanilla's exact algorithm: decompose movement into axes, sweep each axis
independently, clamp on collision.

**Pros:**
- Exact vanilla behavior by construction — same algorithm, same results.
- Simple to implement and verify (direct translation from Java source).
- Well-understood performance characteristics.

**Cons:**
- O(n) block checks per axis, where n is the number of blocks in the expanded AABB.
  Typically 3-27 blocks per move, so this is fast in practice.
- No broad-phase optimization — every move checks block shapes directly.
- Floating-point precision must match Java's `double` semantics. Rust's `f64` is identical
  (IEEE 754 double precision), so this is not actually a problem.

**Verdict: Selected (as the base algorithm).**

### Option 2: GJK/EPA Continuous Collision Detection

Use the GJK (Gilbert-Johnson-Keerthi) algorithm for continuous collision detection between
convex shapes. EPA (Expanding Polytope Algorithm) for penetration depth.

**Pros:**
- Handles arbitrary convex shapes, not just AABBs.
- Continuous (not discrete) — detects collisions between frames.

**Cons:**
- Massive overkill for axis-aligned blocks. Minecraft's collision shapes are unions of AABBs.
- Significantly more expensive per-check than simple AABB intersection.
- Does not match vanilla's algorithm — would produce different results for edge cases.
- Adds complex dependencies (e.g., `parry3d` or custom GJK implementation).

**Verdict: Rejected.** Over-engineered for Minecraft's axis-aligned geometry and would not
match vanilla behavior.

### Option 3: Spatial Hash Grid for Broad Phase + Vanilla Narrow Phase

Add a spatial hash grid that buckets block shapes by grid cell. Before checking collision
for a movement, query the grid to get candidate blocks, then run vanilla's narrow-phase
sweep on only those candidates.

**Pros:**
- Reduces candidate set for large movements (e.g., falling entities at terminal velocity).
- Amortizes grid construction across many entity moves in the same region.

**Cons:**
- For typical entity movements (< 1 block per tick), the candidate set from vanilla's
  AABB expansion is already 3-27 blocks — a spatial grid doesn't reduce this meaningfully.
- Grid maintenance adds overhead (must be updated when blocks change).
- The hot path (short movements for ground entities) is not improved.

**Verdict: Rejected for now.** May revisit if profiling shows large-movement entities
(ender pearls, TNT) are a bottleneck.

### Option 4: SIMD-Optimized AABB Checks

Use SIMD instructions (AVX2, NEON) to perform multiple AABB intersection checks in
parallel. Pack collision shapes into SIMD-friendly layouts and test 4-8 shapes at once.

**Pros:**
- Could provide 4-8x speedup on the innermost collision check loop.
- No algorithmic change — same results, faster execution.

**Cons:**
- AABB intersection is already very fast (a few nanoseconds per check). The bottleneck is
  more likely to be cache misses from loading block state data than the arithmetic itself.
- SIMD layout requires careful data structure design that complicates the code.
- Portability concerns across architectures (x86 vs ARM).

**Verdict: Deferred.** Interesting optimization but premature. Profile first, SIMD later.

### Option 5: Chunk-Local Collision Shape Cache

Cache the resolved collision shapes for all block states in a dense array indexed by block
state ID. When a collision check needs the shape for a block at position (x,y,z), look up
the block state ID in the chunk section, then index into the cache array to get the shape.
No virtual dispatch, no VoxelShape construction — just a direct array index.

**Pros:**
- Eliminates per-access VoxelShape construction. In vanilla, `BlockState.getCollisionShape()`
  involves a method call chain through the block class hierarchy. Caching makes this O(1).
- Dense array is cache-friendly — block state IDs are small integers, so the array fits in
  L2/L3 cache.
- Combines cleanly with Option 1 (vanilla algorithm + fast shape lookup).

**Cons:**
- Some shapes are context-dependent: fences connect to neighbors, redstone wire shape
  depends on connections, stairs shape depends on adjacent stairs. These need per-position
  computation, not per-state caching.
- Must be invalidated when blocks change (but only the changed position's shape needs
  recomputation).

**Verdict: Selected (as an optimization layer on top of Option 1).**

## Decision

**We implement vanilla's per-axis sweep collision algorithm exactly, with a collision shape
cache for performance.** The algorithm is a direct translation of vanilla's
`Entity.move(MoverType, Vec3)` and `Entity.collide(Vec3)` methods, preserving the same
mathematical operations, the same axis ordering (Y, X, Z), and the same step-up logic.
Performance is improved via a dense collision shape cache: `Vec<CollisionShape>` indexed by
block state ID, where `CollisionShape` is a `SmallVec<[AABB; 2]>` (most shapes are 1-2
boxes, avoiding heap allocation).

### Core Constants

These values are hardcoded in vanilla and must be exactly replicated:

```rust
/// Gravity acceleration, applied every tick to non-flying entities.
const GRAVITY: f64 = 0.08;  // blocks/tick²

/// Horizontal drag multiplier in air (applied after movement).
const AIR_DRAG_HORIZONTAL: f64 = 0.91;

/// Vertical drag multiplier (applied after gravity).
const VERTICAL_DRAG: f64 = 0.98;

/// Jump initial velocity.
const JUMP_VELOCITY: f64 = 0.42;

/// Maximum step-up height (entities walk up blocks this tall without jumping).
const STEP_HEIGHT: f64 = 0.6;

/// Water drag multiplier.
const WATER_DRAG: f64 = 0.8;

/// Water buoyancy upward acceleration per tick.
const WATER_BUOYANCY: f64 = 0.014;

/// Lava drag multiplier.
const LAVA_DRAG: f64 = 0.5;

/// Maximum vertical velocity (terminal velocity ≈ 3.92 blocks/tick).
/// Not explicitly capped — emerges from gravity and drag equilibrium.

/// Slow block speed multipliers
const SOUL_SAND_SPEED_FACTOR: f64 = 0.4;
const HONEY_BLOCK_SPEED_FACTOR: f64 = 0.4;
const POWDER_SNOW_SPEED_FACTOR: f64 = 0.9;

/// Cobweb velocity multiplier (applied per tick while inside).
const COBWEB_VELOCITY_FACTOR: f64 = 0.25;
```

### Movement Algorithm

The movement algorithm, implemented as an ECS system:

```rust
fn entity_movement_system(
    mut query: Query<(
        &mut Position, &mut Velocity, &mut OnGround, &mut FallDistance,
        &BoundingBox, &EntityType, Option<&NoGravity>, Option<&Flying>,
    )>,
    level: Res<Level>,
    shape_cache: Res<CollisionShapeCache>,
) {
    for (mut pos, mut vel, mut on_ground, mut fall_dist, bbox, etype, no_grav, flying) in &mut query {
        // 1. Apply gravity
        if no_grav.is_none() && flying.is_none() {
            vel.0.y -= GRAVITY;
        }

        // 2. Attempt movement with collision resolution
        let movement = vel.0;
        let resolved = collide(&pos, bbox, movement, &level, &shape_cache);

        // 3. Update position
        pos.0 += resolved;

        // 4. Detect ground contact
        let was_on_ground = on_ground.0;
        on_ground.0 = movement.y != resolved.y && movement.y < 0.0;

        // 5. Fall distance tracking
        if on_ground.0 {
            if fall_dist.0 > 0.0 {
                // Trigger fall damage event
            }
            fall_dist.0 = 0.0;
        } else if resolved.y < 0.0 {
            fall_dist.0 -= resolved.y as f32;
        }

        // 6. Apply drag
        vel.0.x *= if on_ground.0 { get_block_friction(&level, &pos) * AIR_DRAG_HORIZONTAL } else { AIR_DRAG_HORIZONTAL };
        vel.0.y *= VERTICAL_DRAG;
        vel.0.z *= if on_ground.0 { get_block_friction(&level, &pos) * AIR_DRAG_HORIZONTAL } else { AIR_DRAG_HORIZONTAL };
    }
}
```

### Collision Resolution (Per-Axis Sweep)

```rust
fn collide(
    pos: &Position, bbox: &BoundingBox, movement: DVec3,
    level: &Level, cache: &CollisionShapeCache,
) -> DVec3 {
    if movement == DVec3::ZERO {
        return DVec3::ZERO;
    }

    let entity_aabb = bbox.at(pos.0);
    let expanded = entity_aabb.expand_towards(movement);

    // Collect all block shapes in the expanded AABB
    let block_shapes = collect_block_shapes(&expanded, level, cache);

    // Axis order: Y first (gravity), then X, then Z
    let mut dy = movement.y;
    for shape in &block_shapes {
        dy = shape.clip_y(&entity_aabb, dy);
    }
    let entity_aabb = entity_aabb.move_by(0.0, dy, 0.0);

    let mut dx = movement.x;
    for shape in &block_shapes {
        dx = shape.clip_x(&entity_aabb, dx);
    }
    let entity_aabb = entity_aabb.move_by(dx, 0.0, 0.0);

    let mut dz = movement.z;
    for shape in &block_shapes {
        dz = shape.clip_z(&entity_aabb, dz);
    }

    DVec3::new(dx, dy, dz)
}
```

### Step-Up Algorithm

When a ground entity collides horizontally but a step-up would clear the obstacle:

1. Try the original movement. If horizontal component was reduced (collision):
2. Try the same movement but shifted up by `STEP_HEIGHT` (0.6 blocks).
3. If the shifted movement travels farther horizontally, use it and drop the entity back
   down to the ground.
4. Compare total horizontal distance: if step-up result is farther, use it.

This is how players walk up stairs and slabs without jumping. The algorithm is slightly
complex because it must compare two collision results and pick the better one.

### Collision Shape Cache

```rust
struct CollisionShapeCache {
    /// Index: block state ID (0..~26000 in vanilla 1.21)
    /// Value: collision shape as a list of AABBs
    shapes: Vec<SmallVec<[AABB; 2]>>,
}

impl CollisionShapeCache {
    fn get(&self, state_id: u16) -> &[AABB] {
        &self.shapes[state_id as usize]
    }
}
```

The cache is built at startup by iterating all block states and computing their collision
shapes. For context-dependent shapes (fences, walls, stairs with neighbor connections), the
cache stores the shape for the isolated state; the actual shape is computed per-position
by the `collect_block_shapes` function, which checks neighbor connectivity.

### Water Physics

Entities in water experience:
- Buoyancy: upward force of `WATER_BUOYANCY` (0.014) per tick per submerged tick.
- Drag: velocity multiplied by `WATER_DRAG` (0.8) each tick.
- Swimming: players holding space get additional upward velocity (0.04 per tick).
- Water current: flowing water applies directional force based on flow direction.

```rust
fn water_physics_system(
    mut query: Query<(&mut Velocity, &Position, &BoundingBox), With<InWater>>,
    level: Res<Level>,
) {
    for (mut vel, pos, bbox) in &mut query {
        // Apply water drag
        vel.0 *= WATER_DRAG;

        // Apply buoyancy
        vel.0.y += WATER_BUOYANCY;

        // Apply water flow forces
        let flow = level.get_water_flow(pos.0, bbox);
        vel.0 += flow;
    }
}
```

### Slow Block Modifiers

Certain blocks modify entity speed when standing on or inside them:

| Block | Effect | Multiplier | Application |
|-------|--------|------------|-------------|
| Soul Sand | Slows movement | 0.4 | Multiplied to horizontal velocity |
| Honey Block | Slows movement | 0.4 | Multiplied to horizontal velocity |
| Powder Snow | Slows movement | 0.9 | Multiplied to horizontal velocity |
| Cobweb | Drastically slows | 0.25 | Multiplied to all velocity axes |
| Sweet Berry Bush | Slows movement | 0.75 | Multiplied to horizontal velocity |
| Bubble Column | Upward/downward force | ±0.06-0.11 | Added to vertical velocity |
| Slime Block | Bounce | -velocity.y | Vertical velocity negated on landing |

These are applied by dedicated systems that check the block at and below the entity's feet.

### Why We Don't Change the Physics Model

It is tempting to "fix" or "improve" vanilla physics — add continuous collision detection,
smooth out the step-up, use better integration than Euler. **We must not.** The client runs
the same physics simulation for movement prediction. If our server produces positions that
differ from the client's prediction by even 0.001 blocks, the player sees rubberband
corrections. The physics model is part of the wire protocol contract, not a server-side
implementation detail.

Specific things we must NOT change:
- Gravity constant (even though 0.08 blocks/tick² ≈ 32 m/s² is not 9.8 m/s²)
- Drag values (even though they don't model real air resistance)
- Axis order in collision resolution (Y, X, Z — changing to X, Y, Z produces different
  results on corners)
- Euler integration (even though Verlet or RK4 would be more accurate)
- Float precision (f64 everywhere, matching Java's `double`)

## Consequences

### Positive

- **Exact client prediction matching**: Players will never experience rubberbanding from
  physics disagreements. This is the most important outcome.
- **Simple, auditable implementation**: The per-axis sweep is straightforward to implement,
  test, and verify against vanilla. Each component (gravity, drag, collision, step-up) is
  isolated and testable.
- **Collision shape cache provides significant speedup**: Avoiding per-access VoxelShape
  computation eliminates a major source of overhead in vanilla. Expected 2-5x improvement
  in collision check throughput.
- **ECS parallelism**: The physics system operates on independent entities. Multiple
  entities can have their physics resolved in parallel (entities don't collide with each
  other in vanilla, only with blocks).

### Negative

- **Context-dependent shapes add complexity**: Fences, walls, stairs, and other
  connectivity-dependent shapes cannot be fully cached by state ID alone. These require
  per-position neighbor checks, which partially negates the cache benefit.
- **Step-up algorithm is complex**: The "try both, compare, pick better" step-up logic is
  tricky to get right, especially when combined with sneaking (players don't step up
  while sneaking) and entity-specific step heights (horses have a 1.0 step height).
- **No room for improvement**: We cannot optimize the physics model itself (better
  integration, continuous collision) because the client assumes the vanilla model. Our
  optimizations are limited to implementation efficiency (caching, parallelism), not
  algorithmic improvement.

### Neutral

- **Entity-entity collision is minimal**: Vanilla entities mostly don't collide with each
  other (except boats, minecarts, and mob pushing). Entity-entity interactions are handled
  separately from block physics and are relatively cheap.
- **Fall damage is downstream**: Fall distance tracking feeds into the damage system (not
  covered by this ADR). Physics only tracks the distance; damage calculation happens in the
  entity behavior phase.

## Compliance

- **Vanilla parity test suite**: Record vanilla server entity physics for 100+ test cases
  (fall from height, walk on stairs, swim in water, cobweb traversal, soul sand, etc.) and
  assert Oxidized produces bit-identical position/velocity/onGround results.
- **Step-up regression tests**: Test step-up on every block height variant (slab, snow
  layers 1-8, daylight detector, carpet, etc.) and verify behavior matches vanilla.
- **No physics model modifications**: Code review rule — any PR that modifies a physics
  constant or the collision resolution algorithm must include a vanilla verification test
  proving the change is correct.
- **Performance benchmark**: Physics system must process 10,000 entities in < 2ms (on a
  modern CPU with collision cache warm). Measured via criterion benchmark.

## Related ADRs

- **ADR-018**: Entity System Architecture — physics operates on `Position`, `Velocity`,
  `OnGround`, `FallDistance`, `BoundingBox` components
- **ADR-019**: Tick Loop Design — physics runs in the ENTITY_TICK phase, before AI
- **ADR-025**: Redstone Simulation — block updates from redstone may change collision
  shapes (pistons moving blocks)

## References

- Vanilla source: `net.minecraft.world.entity.Entity.move(MoverType, Vec3)` — main movement method
- Vanilla source: `net.minecraft.world.entity.Entity.collide(Vec3)` — collision resolution
- Vanilla source: `net.minecraft.world.phys.shapes.VoxelShape` — block collision shape
- Vanilla source: `net.minecraft.world.phys.AABB` — axis-aligned bounding box
- Vanilla source: `net.minecraft.world.level.block.state.BlockBehaviour.getCollisionShape()`
- [Minecraft Wiki — Entity Physics](https://minecraft.wiki/w/Entity#Motion)
- [Minecraft Wiki — Jumping](https://minecraft.wiki/w/Jumping)
- [Game Physics — Swept AABB Collision Detection](https://www.gamedev.net/tutorials/programming/general-and-gameplay-programming/swept-aabb-collision-detection-and-response-r3084/)
