# Phase 16 — Basic Physics

**Crate:** `oxidized-game`  
**Reward:** Players and entities fall due to gravity, collide with solid blocks,
and come to rest on the ground. Fluid physics add buoyancy in water and lava.
Slow blocks (soul sand, honey, powder snow) reduce movement. Fall distance is
tracked so Phase 24 can apply fall damage.

---

## Goal

Implement the per-tick physics pipeline: gravity, air drag, AABB sweep
collision against block voxel shapes, on_ground detection, fluid buoyancy, and
the slow-block speed multipliers. Match the numerical constants from the vanilla
`LivingEntity.travel()` and `Entity.move()` methods.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Entity movement | `Entity.move(MoverType, Vec3)` | `net.minecraft.world.entity.Entity` |
| Entity travel | `LivingEntity.travel(Vec3)` | `net.minecraft.world.entity.LivingEntity` |
| Gravity constant | `LivingEntity.DEFAULT_BASE_GRAVITY` (0.08) | `net.minecraft.world.entity.LivingEntity` |
| Air drag | `LivingEntity.travel` (`0.91F`, `0.98F`) | `net.minecraft.world.entity.LivingEntity` |
| Voxel shapes | `Shapes.collide(AABB, …)` | `net.minecraft.world.phys.shapes.Shapes` |
| Voxel shape | `VoxelShape` | `net.minecraft.world.phys.shapes.VoxelShape` |
| Fluid physics | `Entity.updateFluidHeightAndDoFluidPushing` | `net.minecraft.world.entity.Entity` |
| Jump physics | `LivingEntity.jumpFromGround` | `net.minecraft.world.entity.LivingEntity` |
| Block speed factor | `Block.getSpeedFactor` | `net.minecraft.world.level.block.Block` |
| Set motion packet | `ClientboundSetEntityMotionPacket` | `net.minecraft.network.protocol.game.ClientboundSetEntityMotionPacket` |

---

## Tasks

### 16.1 — Physics constants

```rust
// crates/oxidized-game/src/physics/constants.rs

/// Gravity acceleration (blocks/tick²). Matches LivingEntity.DEFAULT_BASE_GRAVITY.
pub const GRAVITY: f64 = 0.08;

/// Vertical velocity multiplier per tick in air.
/// Applied to vy each tick: vy_new = (vy - GRAVITY) * VERTICAL_DRAG.
pub const VERTICAL_DRAG: f64 = 0.98;

/// Horizontal velocity multiplier per tick in air (base, before block friction).
/// Matches `blockFriction * 0.91F` where blockFriction=1.0 in air.
pub const HORIZONTAL_DRAG_AIR: f64 = 0.91;

/// Horizontal multiplier when on the ground (multiplied by block friction, then by 0.91).
/// Typical ground block friction is 0.6 → effective = 0.6 * 0.91 = 0.546.
pub const BLOCK_FRICTION_DEFAULT: f64 = 0.6;

// Jump velocities
/// Base jump velocity (blocks/tick).
pub const JUMP_POWER: f64 = 0.42;
/// Per-level Jump Boost effect addition.
pub const JUMP_BOOST_PER_LEVEL: f64 = 0.1;

// Fluid buoyancy
/// Upward velocity added per tick when submerged in water.
pub const WATER_BUOYANCY: f64 = 0.014;
/// Upward velocity added per tick in lava.
pub const LAVA_BUOYANCY: f64 = 0.007;
/// Fluid drag multiplier (applied to horizontal velocity in water).
pub const WATER_DRAG: f64 = 0.8;
pub const LAVA_DRAG: f64 = 0.5;

// Slow block speed multipliers
/// Soul sand horizontal speed multiplier.
pub const SOUL_SAND_SPEED: f64 = 0.4;
/// Honey block horizontal speed multiplier.
pub const HONEY_BLOCK_SPEED: f64 = 0.4;
/// Powder snow horizontal speed multiplier.
pub const POWDER_SNOW_SPEED: f64 = 0.3;

// Movement speeds (blocks/tick, before friction)
pub const WALK_SPEED: f64 = 0.1;
pub const SPRINT_SPEED: f64 = 0.13;
pub const SNEAK_SPEED: f64 = 0.065;
```

### 16.2 — Voxel shape representation

Block collision shapes are loaded from the vanilla-extracted
`collision_shapes.json`. Each block state maps to a list of AABB components.

```rust
// crates/oxidized-game/src/physics/voxel_shape.rs

/// A voxel shape is composed of one or more AABB fragments in block-local
/// coordinates (0.0–1.0 per axis).
#[derive(Debug, Clone)]
pub struct VoxelShape {
    pub boxes: Vec<ShapeBox>,
}

/// An axis-aligned box in block-local space.
#[derive(Debug, Clone, Copy)]
pub struct ShapeBox {
    pub min_x: f64, pub min_y: f64, pub min_z: f64,
    pub max_x: f64, pub max_y: f64, pub max_z: f64,
}

impl VoxelShape {
    /// A solid, full-cube shape (e.g. stone, dirt).
    pub fn full() -> Self {
        Self { boxes: vec![ShapeBox { min_x: 0.0, min_y: 0.0, min_z: 0.0,
                                      max_x: 1.0, max_y: 1.0, max_z: 1.0 }] }
    }

    /// Empty shape — no collision (e.g. air, grass).
    pub fn empty() -> Self {
        Self { boxes: Vec::new() }
    }

    pub fn is_empty(&self) -> bool { self.boxes.is_empty() }

    /// Translate the shape to world coordinates given a block origin.
    pub fn translated(&self, bx: i32, by: i32, bz: i32) -> Vec<WorldAABB> {
        self.boxes.iter().map(|b| WorldAABB {
            min_x: bx as f64 + b.min_x,
            min_y: by as f64 + b.min_y,
            min_z: bz as f64 + b.min_z,
            max_x: bx as f64 + b.max_x,
            max_y: by as f64 + b.max_y,
            max_z: bx as f64 + b.max_z,
        }).collect()
    }
}

pub type WorldAABB = AABB;
```

### 16.3 — AABB sweep collision (per-axis)

```rust
// crates/oxidized-game/src/physics/collision.rs

use super::voxel_shape::WorldAABB;

/// Perform a swept AABB collision test along a single axis.
/// Returns the maximum safe delta the entity can move before hitting `obstacle`.
///
/// Algorithm: if the entity AABB (expanded in move direction) overlaps the
/// obstacle in the other two axes, the entity stops at the face.
pub fn sweep_axis_x(
    entity: &WorldAABB, obstacle: &WorldAABB, mut dx: f64,
) -> f64 {
    // Check overlap in Y and Z first.
    if entity.max_y <= obstacle.min_y || entity.min_y >= obstacle.max_y { return dx; }
    if entity.max_z <= obstacle.min_z || entity.min_z >= obstacle.max_z { return dx; }

    if dx > 0.0 && entity.max_x <= obstacle.min_x {
        let max_move = obstacle.min_x - entity.max_x;
        if max_move < dx { dx = max_move; }
    } else if dx < 0.0 && entity.min_x >= obstacle.max_x {
        let max_move = entity.min_x - obstacle.max_x;
        if max_move < -dx { dx = -max_move; }
    }
    dx
}

pub fn sweep_axis_y(entity: &WorldAABB, obstacle: &WorldAABB, mut dy: f64) -> f64 {
    if entity.max_x <= obstacle.min_x || entity.min_x >= obstacle.max_x { return dy; }
    if entity.max_z <= obstacle.min_z || entity.min_z >= obstacle.max_z { return dy; }
    if dy > 0.0 && entity.max_y <= obstacle.min_y {
        let max_move = obstacle.min_y - entity.max_y;
        if max_move < dy { dy = max_move; }
    } else if dy < 0.0 && entity.min_y >= obstacle.max_y {
        let max_move = entity.min_y - obstacle.max_y;
        if max_move < -dy { dy = -max_move; }
    }
    dy
}

pub fn sweep_axis_z(entity: &WorldAABB, obstacle: &WorldAABB, mut dz: f64) -> f64 {
    if entity.max_x <= obstacle.min_x || entity.min_x >= obstacle.max_x { return dz; }
    if entity.max_y <= obstacle.min_y || entity.min_y >= obstacle.max_y { return dz; }
    if dz > 0.0 && entity.max_z <= obstacle.min_z {
        let max_move = obstacle.min_z - entity.max_z;
        if max_move < dz { dz = max_move; }
    } else if dz < 0.0 && entity.min_z >= obstacle.max_z {
        let max_move = entity.min_z - obstacle.max_z;
        if max_move < -dz { dz = -max_move; }
    }
    dz
}

/// Collect the world-space collision boxes for all blocks in the swept volume.
pub fn collect_obstacles(
    level: &impl BlockGetter,
    entity: &WorldAABB,
    dx: f64, dy: f64, dz: f64,
    shape_provider: &BlockShapeProvider,
) -> Vec<WorldAABB> {
    let expand_min_x = (entity.min_x + dx.min(0.0) - 1.0).floor() as i32;
    let expand_max_x = (entity.max_x + dx.max(0.0) + 1.0).ceil()  as i32;
    let expand_min_y = (entity.min_y + dy.min(0.0) - 1.0).floor() as i32;
    let expand_max_y = (entity.max_y + dy.max(0.0) + 1.0).ceil()  as i32;
    let expand_min_z = (entity.min_z + dz.min(0.0) - 1.0).floor() as i32;
    let expand_max_z = (entity.max_z + dz.max(0.0) + 1.0).ceil()  as i32;

    let mut obstacles = Vec::new();
    for bx in expand_min_x..=expand_max_x {
        for by in expand_min_y..=expand_max_y {
            for bz in expand_min_z..=expand_max_z {
                let pos = BlockPos::new(bx, by, bz);
                let state = level.get_block_state(pos);
                let shape = shape_provider.get_shape(state);
                for aabb in shape.translated(bx, by, bz) {
                    obstacles.push(aabb);
                }
            }
        }
    }
    obstacles
}
```

### 16.4 — Full per-tick physics step

```rust
// crates/oxidized-game/src/physics/tick.rs

/// Apply one tick of physics to `entity`.
///
/// Order of operations matches LivingEntity.travel():
///   1. Apply gravity (reduce vy).
///   2. Move by velocity (sweep collision).
///   3. Detect on_ground from Y collision.
///   4. Apply drag.
///   5. Track fall distance.
pub fn physics_tick(
    entity: &mut Entity,
    level: &impl BlockGetter,
    shape_provider: &BlockShapeProvider,
    in_water: bool,
    in_lava: bool,
) {
    // 1. Gravity (only when not in fluid; fluid has its own buoyancy).
    if !in_water && !in_lava {
        entity.vy -= GRAVITY;
    }

    let mut dx = entity.vx;
    let mut dy = entity.vy;
    let mut dz = entity.vz;

    // Fluid modifiers.
    if in_water {
        dy += WATER_BUOYANCY;
        dx *= WATER_DRAG;
        dz *= WATER_DRAG;
    } else if in_lava {
        dy += LAVA_BUOYANCY;
        dx *= LAVA_DRAG;
        dz *= LAVA_DRAG;
    }

    // 2. Swept AABB collision: X, then Y, then Z.
    let bbox = &entity.bounding_box.clone();
    let obstacles = collect_obstacles(level, bbox, dx, dy, dz, shape_provider);

    // Sweep X.
    let actual_dx = obstacles.iter().fold(dx, |acc, obs| sweep_axis_x(bbox, obs, acc));
    let bbox_x = translate_aabb(bbox, actual_dx, 0.0, 0.0);

    // Sweep Y.
    let actual_dy = obstacles.iter().fold(dy, |acc, obs| sweep_axis_y(&bbox_x, obs, acc));
    let bbox_xy = translate_aabb(&bbox_x, 0.0, actual_dy, 0.0);

    // Sweep Z.
    let actual_dz = obstacles.iter().fold(dz, |acc, obs| sweep_axis_z(&bbox_xy, obs, acc));

    // Apply movement.
    entity.set_pos(
        entity.x + actual_dx,
        entity.y + actual_dy,
        entity.z + actual_dz,
    );

    // 3. on_ground: Y was reduced by collision.
    let was_on_ground = entity.on_ground;
    entity.on_ground = dy < 0.0 && actual_dy > dy; // floor stopped downward motion

    // 4. Zero out velocity on collision axes.
    let vx_new = if (actual_dx - dx).abs() > 1e-9 { 0.0 } else { entity.vx };
    let vy_new = if (actual_dy - dy).abs() > 1e-9 { 0.0 } else { entity.vy };
    let vz_new = if (actual_dz - dz).abs() > 1e-9 { 0.0 } else { entity.vz };

    // 5. Apply horizontal drag.
    let block_friction = get_block_friction(level, entity);
    let h_drag = if entity.on_ground {
        block_friction * HORIZONTAL_DRAG_AIR  // ground friction: blockFriction * 0.91
    } else {
        HORIZONTAL_DRAG_AIR  // air drag
    };

    entity.vx = vx_new * h_drag;
    entity.vy = vy_new * VERTICAL_DRAG;
    entity.vz = vz_new * h_drag;

    // 6. Fall distance tracking.
    if !entity.on_ground && actual_dy < 0.0 {
        entity.fall_distance -= actual_dy as f32;
    } else if entity.on_ground {
        entity.fall_distance = 0.0; // reset on landing (damage calculated before reset)
    }
}

fn translate_aabb(aabb: &AABB, dx: f64, dy: f64, dz: f64) -> AABB {
    AABB {
        min_x: aabb.min_x + dx, min_y: aabb.min_y + dy, min_z: aabb.min_z + dz,
        max_x: aabb.max_x + dx, max_y: aabb.max_y + dy, max_z: aabb.max_z + dz,
    }
}

fn get_block_friction(level: &impl BlockGetter, entity: &Entity) -> f64 {
    // Block directly below entity's feet.
    let below = BlockPos::new(
        entity.x.floor() as i32,
        (entity.y - 0.5).floor() as i32,
        entity.z.floor() as i32,
    );
    match level.get_block_state(below).block_id() {
        id if id == ICE_ID || id == PACKED_ICE_ID || id == FROSTED_ICE_ID => 0.98,
        id if id == SLIME_BLOCK_ID => 0.8,
        _ => BLOCK_FRICTION_DEFAULT,
    }
}
```

### 16.5 — Slow-block speed modifier

```rust
// crates/oxidized-game/src/physics/slow_blocks.rs

/// Speed multiplier for the block the entity is standing in.
/// Called before computing horizontal input movement.
pub fn block_speed_factor(
    level: &impl BlockGetter,
    entity: &Entity,
) -> f64 {
    // Block at entity feet (Y).
    let feet = BlockPos::new(
        entity.x.floor() as i32,
        entity.y.floor() as i32,
        entity.z.floor() as i32,
    );
    let state = level.get_block_state(feet);
    match state.block_id() {
        id if id == SOUL_SAND_ID  => SOUL_SAND_SPEED,
        id if id == HONEY_BLOCK_ID => HONEY_BLOCK_SPEED,
        id if id == POWDER_SNOW_ID => POWDER_SNOW_SPEED,
        _ => 1.0,
    }
}
```

### 16.6 — Jump physics

```rust
// crates/oxidized-game/src/physics/jump.rs

/// Apply jump impulse. jump_boost_level = 0 for no effect.
pub fn apply_jump(entity: &mut Entity, jump_boost_level: u8) {
    entity.vy = JUMP_POWER + jump_boost_level as f64 * JUMP_BOOST_PER_LEVEL;
    if entity.synched_data.get::<bool>(DATA_SPRINTING_FLAG) {
        // Sprint-jump gives a horizontal boost in the facing direction.
        let yaw_rad = entity.yaw.to_radians() as f64;
        entity.vx -= yaw_rad.sin() * 0.2;
        entity.vz += yaw_rad.cos() * 0.2;
    }
}
```

### 16.7 — Velocity change notification

When server-side physics changes an entity's velocity significantly (e.g. from
knockback), send `ClientboundSetEntityMotionPacket` to watching players.

```rust
// crates/oxidized-game/src/physics/tick.rs (continued)

pub fn velocity_changed_significantly(old: (f64,f64,f64), new: (f64,f64,f64)) -> bool {
    let d = |a: f64, b: f64| (a - b).abs();
    d(old.0, new.0) > 0.01 || d(old.1, new.1) > 0.01 || d(old.2, new.2) > 0.01
}

pub struct ClientboundSetEntityMotionPacket {
    pub entity_id: i32,
    /// Velocity components × 8000, clamped to i16 range.
    pub vx: i16,
    pub vy: i16,
    pub vz: i16,
}

impl ClientboundSetEntityMotionPacket {
    pub fn from_entity(entity: &Entity) -> Self {
        Self {
            entity_id: entity.id,
            vx: encode_velocity(entity.vx),
            vy: encode_velocity(entity.vy),
            vz: encode_velocity(entity.vz),
        }
    }
}
```

---

## Data Structures Summary

```
oxidized-game::physics
  ├── constants::*              — GRAVITY, drags, jump power, speeds
  ├── VoxelShape / ShapeBox     — block collision geometry
  ├── sweep_axis_{x,y,z}        — single-axis AABB sweep
  ├── collect_obstacles         — gather nearby block shapes
  ├── physics_tick(entity, …)   — full per-tick physics update
  ├── block_speed_factor        — slow-block multiplier
  └── apply_jump                — jump impulse with sprint boost
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Entity in free fall accelerates by GRAVITY each tick.
    #[test]
    fn gravity_acceleration() {
        let mut entity = make_floating_entity();
        entity.vy = 0.0;

        // Tick 1: vy_after_gravity = 0 - 0.08 = -0.08; drag: -0.08 * 0.98 = -0.0784
        physics_tick(&mut entity, &EmptyLevel, &EmptyShapeProvider, false, false);
        assert!((entity.vy + 0.0784).abs() < 0.0001,
            "After tick 1: vy = {} (expected ≈ -0.0784)", entity.vy);

        // Tick 2: vy_start = -0.0784; after gravity = -0.1584; after drag ≈ -0.15523
        physics_tick(&mut entity, &EmptyLevel, &EmptyShapeProvider, false, false);
        assert!(entity.vy < -0.15, "After tick 2: vy should be more negative");
    }

    /// Entity falls onto a full-cube block and stops (on_ground = true, vy = 0).
    #[test]
    fn collision_stops_downward_movement() {
        let mut entity = make_entity_above_ground(1.0); // y=1.0 above the ground block
        entity.vy = -5.0; // large downward velocity

        let level = SingleBlockLevel { block: STONE, pos: BlockPos::new(0, 0, 0) };
        physics_tick(&mut entity, &level, &FullCubeShapeProvider, false, false);

        assert!(entity.on_ground, "Entity should be on ground");
        assert!((entity.vy).abs() < 0.001, "vy should be zeroed on ground collision");
        assert!(entity.y >= 1.0 - 0.001, "Entity should not have passed through block");
    }

    /// Swimming in water: vy increases by WATER_BUOYANCY and horizontal is damped.
    #[test]
    fn water_buoyancy() {
        let mut entity = make_floating_entity();
        entity.vy = 0.0;
        entity.vx = 1.0;
        physics_tick(&mut entity, &EmptyLevel, &EmptyShapeProvider, true, false);
        // In water: vy = (0 + WATER_BUOYANCY) * VERTICAL_DRAG (approx)
        assert!(entity.vy > 0.0, "Water buoyancy should push upward");
        // Horizontal drag in water: vx *= WATER_DRAG
        assert!(entity.vx < 1.0, "Water should reduce horizontal velocity");
        assert!((entity.vx - WATER_DRAG).abs() < 0.001,
            "vx after 1 tick in water: {} (expected {})", entity.vx, WATER_DRAG);
    }

    /// Jump power is applied correctly with and without Jump Boost.
    #[test]
    fn jump_velocity_no_boost() {
        let mut entity = make_entity_on_ground();
        apply_jump(&mut entity, 0);
        assert!((entity.vy - JUMP_POWER).abs() < 0.0001,
            "Jump vy {} ≠ {}", entity.vy, JUMP_POWER);
    }

    #[test]
    fn jump_velocity_with_boost() {
        let mut entity = make_entity_on_ground();
        apply_jump(&mut entity, 2); // Jump Boost II
        let expected = JUMP_POWER + 2.0 * JUMP_BOOST_PER_LEVEL;
        assert!((entity.vy - expected).abs() < 0.0001,
            "Jump Boost II vy {} ≠ {}", entity.vy, expected);
    }

    /// Sweep collision: entity moving +X is stopped at block face.
    #[test]
    fn sweep_x_collision() {
        let entity_box = AABB { min_x: 0.0, min_y: 0.0, min_z: 0.0,
                                 max_x: 0.6, max_y: 1.8, max_z: 0.6 };
        let obstacle  = AABB { min_x: 1.0, min_y: 0.0, min_z: 0.0,
                                max_x: 2.0, max_y: 1.0, max_z: 1.0 };
        let limited = sweep_axis_x(&entity_box, &obstacle, 2.0);
        assert!((limited - 0.4).abs() < 1e-9,
            "Expected dx=0.4 (stop at face), got {limited}");
    }

    fn make_floating_entity() -> Entity {
        let mut e = Entity::new(
            ResourceLocation::minecraft("minecraft:pig"), 0.9, 1.4
        );
        e.set_pos(8.0, 100.0, 8.0);
        e
    }

    fn make_entity_above_ground(height_above: f64) -> Entity {
        let mut e = Entity::new(
            ResourceLocation::minecraft("minecraft:pig"), 0.9, 1.4
        );
        // Ground block at y=0 (full cube: 0.0–1.0)
        // Entity feet at y = 1.0 + height_above
        e.set_pos(0.5, 1.0 + height_above, 0.5);
        e
    }

    fn make_entity_on_ground() -> Entity {
        let mut e = make_entity_above_ground(0.0);
        e.on_ground = true;
        e
    }
}
```
