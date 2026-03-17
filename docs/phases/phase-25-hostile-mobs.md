# Phase 25 — Hostile Mobs

**Crate:** `oxidized-game`  
**Reward:** Zombies, skeletons, creepers spawn, pathfind, and attack the player.

---

## Goal

Implement the full hostile-mob stack: `Mob` entity extension with goal-based AI,
`GoalSelector`, six core `PathfinderGoal` implementations, A\*-based
`PathNavigation`, and five specific hostile mobs (Zombie, Skeleton, Creeper,
Spider, Enderman). Mobs must spawn naturally during the night, navigate terrain,
and deal damage to players. All mob-specific mechanics (zombie sunburn, creeper
swell/explosion, skeleton bow shooting, enderman teleport) must work correctly.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Base mob | `Mob` | `net.minecraft.world.entity.Mob` |
| Goal interface | `Goal` | `net.minecraft.world.entity.ai.goal.Goal` |
| Goal selector | `GoalSelector` | `net.minecraft.world.entity.ai.goal.GoalSelector` |
| Path navigation | `PathNavigation` | `net.minecraft.world.entity.ai.navigation.PathNavigation` |
| Ground navigation | `GroundPathNavigation` | `net.minecraft.world.entity.ai.navigation.GroundPathNavigation` |
| Float goal | `FloatGoal` | `net.minecraft.world.entity.ai.goal.FloatGoal` |
| Melee attack goal | `MeleeAttackGoal` | `net.minecraft.world.entity.ai.goal.MeleeAttackGoal` |
| Random look around | `RandomLookAroundGoal` | `net.minecraft.world.entity.ai.goal.RandomLookAroundGoal` |
| Water-avoiding stroll | `WaterAvoidingRandomStrollGoal` | `net.minecraft.world.entity.ai.goal.WaterAvoidingRandomStrollGoal` |
| Look at player | `LookAtPlayerGoal` | `net.minecraft.world.entity.ai.goal.LookAtPlayerGoal` |
| Nearest attackable target | `NearestAttackableTargetGoal` | `net.minecraft.world.entity.ai.goal.target.NearestAttackableTargetGoal` |
| Zombie entity | `Zombie` | `net.minecraft.world.entity.monster.Zombie` |
| Skeleton entity | `Skeleton` | `net.minecraft.world.entity.monster.Skeleton` |
| Creeper entity | `Creeper` | `net.minecraft.world.entity.monster.Creeper` |
| Spider entity | `Spider` | `net.minecraft.world.entity.monster.Spider` |
| Enderman entity | `EnderMan` | `net.minecraft.world.entity.monster.EnderMan` |
| Look control | `LookControl` | `net.minecraft.world.entity.ai.control.LookControl` |
| Move control | `MoveControl` | `net.minecraft.world.entity.ai.control.MoveControl` |
| Jump control | `JumpControl` | `net.minecraft.world.entity.ai.control.JumpControl` |

---

## Tasks

### 25.1 — `GoalSelector` and `PathfinderGoal` trait

```rust
// crates/oxidized-game/src/entity/ai/goal.rs

use std::collections::BTreeMap;

/// Category flags used to prevent conflicting goals from running simultaneously.
/// Matches Goal.Flag in Java.
bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct GoalFlag: u8 {
        const MOVE   = 0b0001;
        const LOOK   = 0b0010;
        const JUMP   = 0b0100;
        const TARGET = 0b1000;
    }
}

/// Trait that every AI goal must implement.
/// Mirrors `net.minecraft.world.entity.ai.goal.Goal`.
pub trait PathfinderGoal: Send + Sync {
    /// Whether this goal can start running given the current world state.
    fn can_use(&self) -> bool;

    /// Whether a running goal may continue. Defaults to `can_use()`.
    fn can_continue_to_use(&self) -> bool {
        self.can_use()
    }

    /// Whether competing goals of the same flag can interrupt this one.
    fn is_interruptable(&self) -> bool {
        true
    }

    /// Called once when the goal begins.
    fn start(&mut self) {}

    /// Called once when the goal ends (either stopped or interrupted).
    fn stop(&mut self) {}

    /// Called every server tick while the goal is active.
    fn tick(&mut self) {}

    /// If true, `tick()` is called every game tick regardless of goal scheduling.
    fn requires_update_every_tick(&self) -> bool {
        false
    }

    /// The set of `GoalFlag`s this goal occupies.
    fn flags(&self) -> GoalFlag;
}

struct WrappedGoal {
    priority: i32,
    goal: Box<dyn PathfinderGoal>,
    running: bool,
}

/// Priority-ordered goal scheduler. Runs one active goal per `GoalFlag`.
/// Mirrors `net.minecraft.world.entity.ai.goal.GoalSelector`.
pub struct GoalSelector {
    /// Goals ordered by ascending priority (lower number = higher priority).
    goals: Vec<WrappedGoal>,
    /// Tick counter; goals are re-evaluated every `tick_interval` ticks.
    tick_count: u32,
    tick_interval: u32,
}

impl GoalSelector {
    pub fn new() -> Self {
        Self {
            goals: Vec::new(),
            tick_count: 0,
            tick_interval: 2,
        }
    }

    /// Register a goal at the given priority (lower = higher priority).
    pub fn add_goal(&mut self, priority: i32, goal: impl PathfinderGoal + 'static) {
        self.goals.push(WrappedGoal {
            priority,
            goal: Box::new(goal),
            running: false,
        });
        self.goals.sort_unstable_by_key(|g| g.priority);
    }

    /// Tick all goals: stop disabled ones, start newly eligible ones, tick active ones.
    pub fn tick(&mut self) {
        self.tick_count += 1;

        if self.tick_count % self.tick_interval == 0 {
            // Stop goals that can no longer continue.
            for g in self.goals.iter_mut() {
                if g.running && !g.goal.can_continue_to_use() {
                    g.goal.stop();
                    g.running = false;
                }
            }

            // Track which flags are already occupied by running goals.
            let mut occupied = GoalFlag::empty();
            for g in self.goals.iter() {
                if g.running {
                    occupied |= g.goal.flags();
                }
            }

            // Start new goals where flags are not occupied.
            for g in self.goals.iter_mut() {
                if g.running {
                    continue;
                }
                let flags = g.goal.flags();
                if occupied.intersects(flags) {
                    continue;
                }
                if g.goal.can_use() {
                    g.goal.start();
                    g.running = true;
                    occupied |= flags;
                }
            }
        }

        // Tick all currently running goals.
        for g in self.goals.iter_mut() {
            if g.running {
                g.goal.tick();
            }
        }
    }

    /// Force-stop all currently running goals.
    pub fn stop_all(&mut self) {
        for g in self.goals.iter_mut() {
            if g.running {
                g.goal.stop();
                g.running = false;
            }
        }
    }
}
```

### 25.2 — `PathNavigation` (A\* pathfinder)

```rust
// crates/oxidized-game/src/entity/ai/navigation.rs

use std::collections::{BinaryHeap, HashMap};
use std::cmp::Reverse;
use glam::IVec3;

/// A single waypoint in a computed path.
#[derive(Debug, Clone)]
pub struct PathPoint {
    pub pos: IVec3,
    /// Cumulative path cost from origin.
    pub g: f32,
    /// Estimated remaining cost to goal (heuristic).
    pub h: f32,
    pub parent: Option<usize>,
}

impl PathPoint {
    pub fn f(&self) -> f32 {
        self.g + self.h
    }
}

/// A computed path returned by the navigator.
#[derive(Debug, Clone)]
pub struct Path {
    pub points: Vec<IVec3>,
    pub next_index: usize,
    pub reached: bool,
}

impl Path {
    pub fn current_node(&self) -> Option<IVec3> {
        self.points.get(self.next_index).copied()
    }

    pub fn advance(&mut self) {
        self.next_index += 1;
        if self.next_index >= self.points.len() {
            self.reached = true;
        }
    }

    pub fn is_done(&self) -> bool {
        self.reached || self.points.is_empty()
    }
}

/// Ground-based A\* pathfinder. Mirrors `GroundPathNavigation` in Java.
pub struct PathNavigation {
    pub current_path: Option<Path>,
    pub speed_modifier: f64,
    /// Max nodes expanded before giving up (default 200).
    pub max_visit_nodes: usize,
}

impl PathNavigation {
    pub fn new(speed: f64) -> Self {
        Self {
            current_path: None,
            speed_modifier: speed,
            max_visit_nodes: 200,
        }
    }

    /// Compute a path from `start` to `goal` using A\*.
    /// Returns `None` if the goal is unreachable within `max_visit_nodes`.
    pub fn create_path(
        &self,
        start: IVec3,
        goal: IVec3,
        is_walkable: &dyn Fn(IVec3) -> bool,
    ) -> Option<Path> {
        // Open set: (Reverse<ordered_f32>, index)
        let mut open: BinaryHeap<(Reverse<u32>, usize)> = BinaryHeap::new();
        let mut nodes: Vec<PathPoint> = Vec::new();
        let mut visited: HashMap<IVec3, usize> = HashMap::new();

        let heuristic = |p: IVec3| -> f32 {
            let d = goal - p;
            ((d.x * d.x + d.y * d.y + d.z * d.z) as f32).sqrt()
        };

        nodes.push(PathPoint { pos: start, g: 0.0, h: heuristic(start), parent: None });
        open.push((Reverse(0), 0));
        visited.insert(start, 0);

        let mut visited_count = 0;
        while let Some((_, idx)) = open.pop() {
            if visited_count >= self.max_visit_nodes {
                break;
            }
            visited_count += 1;

            let pos = nodes[idx].pos;
            if pos == goal {
                return Some(Self::reconstruct_path(&nodes, idx));
            }

            for neighbor in Self::neighbors(pos) {
                if !is_walkable(neighbor) {
                    continue;
                }
                let new_g = nodes[idx].g + 1.0;
                if let Some(&existing) = visited.get(&neighbor) {
                    if new_g < nodes[existing].g {
                        nodes[existing].g = new_g;
                        nodes[existing].parent = Some(idx);
                    }
                    continue;
                }
                let new_idx = nodes.len();
                nodes.push(PathPoint { pos: neighbor, g: new_g, h: heuristic(neighbor), parent: Some(idx) });
                visited.insert(neighbor, new_idx);
                let f_bits = nodes[new_idx].f().to_bits();
                open.push((Reverse(f_bits), new_idx));
            }
        }
        None
    }

    fn neighbors(pos: IVec3) -> [IVec3; 4] {
        [
            pos + IVec3::new(1, 0, 0),
            pos + IVec3::new(-1, 0, 0),
            pos + IVec3::new(0, 0, 1),
            pos + IVec3::new(0, 0, -1),
        ]
    }

    fn reconstruct_path(nodes: &[PathPoint], end: usize) -> Path {
        let mut points = Vec::new();
        let mut current = Some(end);
        while let Some(idx) = current {
            points.push(nodes[idx].pos);
            current = nodes[idx].parent;
        }
        points.reverse();
        Path { points, next_index: 0, reached: false }
    }

    /// Set a new path target. Recomputes path immediately.
    pub fn move_to(
        &mut self,
        start: IVec3,
        goal: IVec3,
        speed: f64,
        is_walkable: &dyn Fn(IVec3) -> bool,
    ) {
        self.speed_modifier = speed;
        self.current_path = self.create_path(start, goal, is_walkable);
    }

    /// Call every tick to advance along the current path.
    /// Returns the next waypoint the entity should move toward, if any.
    pub fn tick_path(&mut self) -> Option<IVec3> {
        let path = self.current_path.as_mut()?;
        if path.is_done() {
            self.current_path = None;
            return None;
        }
        path.current_node()
    }

    pub fn stop_navigation(&mut self) {
        self.current_path = None;
    }

    pub fn is_in_progress(&self) -> bool {
        self.current_path.as_ref().map_or(false, |p| !p.is_done())
    }
}
```

### 25.3 — Control types: `LookControl`, `MoveControl`, `JumpControl`

```rust
// crates/oxidized-game/src/entity/ai/controls.rs

use glam::Vec3;

/// Smoothly interpolates the entity's head yaw/pitch toward a target.
pub struct LookControl {
    pub yaw: f32,
    pub pitch: f32,
    pub target: Option<Vec3>,
    pub y_rot_speed: f32,
    pub x_rot_speed: f32,
}

impl LookControl {
    pub fn new(y_speed: f32, x_speed: f32) -> Self {
        Self { yaw: 0.0, pitch: 0.0, target: None, y_rot_speed: y_speed, x_rot_speed: x_speed }
    }

    pub fn set_look_at(&mut self, target: Vec3) {
        self.target = Some(target);
    }

    /// Called each tick; rotates yaw/pitch toward target by at most `y_rot_speed`/`x_rot_speed` degrees.
    pub fn tick(&mut self, entity_pos: Vec3) {
        if let Some(target) = self.target {
            let dx = target.x - entity_pos.x;
            let dy = target.y - entity_pos.y;
            let dz = target.z - entity_pos.z;
            let desired_yaw = dz.atan2(dx).to_degrees() - 90.0;
            let horiz = (dx * dx + dz * dz).sqrt();
            let desired_pitch = -(dy.atan2(horiz).to_degrees());

            self.yaw = Self::rotate_toward(self.yaw, desired_yaw, self.y_rot_speed);
            self.pitch = Self::rotate_toward(self.pitch, desired_pitch, self.x_rot_speed);
        }
    }

    fn rotate_toward(current: f32, target: f32, max_delta: f32) -> f32 {
        let mut delta = target - current;
        while delta > 180.0  { delta -= 360.0; }
        while delta < -180.0 { delta += 360.0; }
        current + delta.clamp(-max_delta, max_delta)
    }
}

/// Drives the entity's movement toward a waypoint at a desired speed.
pub struct MoveControl {
    pub desired_x: f64,
    pub desired_z: f64,
    pub desired_speed: f64,
    pub wants_to_move: bool,
}

impl MoveControl {
    pub fn new() -> Self {
        Self { desired_x: 0.0, desired_z: 0.0, desired_speed: 0.0, wants_to_move: false }
    }

    pub fn set_wanted_position(&mut self, x: f64, z: f64, speed: f64) {
        self.desired_x = x;
        self.desired_z = z;
        self.desired_speed = speed;
        self.wants_to_move = true;
    }
}

/// Triggers a jump when requested.
pub struct JumpControl {
    pub wants_jump: bool,
}

impl JumpControl {
    pub fn new() -> Self { Self { wants_jump: false } }

    pub fn jump_if_possible(&mut self, on_ground: bool) -> bool {
        if self.wants_jump && on_ground {
            self.wants_jump = false;
            return true;
        }
        false
    }
}
```

### 25.4 — Core AI goals

```rust
// crates/oxidized-game/src/entity/ai/goals/float.rs

/// Makes the mob swim upward when submerged in water (vy += 0.2 if touching water).
pub struct FloatGoal {
    mob_in_water: bool,
}
impl PathfinderGoal for FloatGoal {
    fn flags(&self) -> GoalFlag { GoalFlag::JUMP }
    fn can_use(&self) -> bool { self.mob_in_water }
    fn requires_update_every_tick(&self) -> bool { true }
    fn tick(&mut self) { /* apply upward velocity 0.2 */ }
}

// crates/oxidized-game/src/entity/ai/goals/melee_attack.rs

/// Moves the mob toward its target and attacks when within melee reach.
/// Attack cooldown is 20 ticks (1 second). Reach = 1.5 + target_width.
pub struct MeleeAttackGoal {
    pub speed_modifier: f64,
    /// Ticks remaining until next attack is allowed.
    pub attack_cooldown: i32,
    /// Ticks the goal has been running without a valid target.
    pub ticks_without_sight: i32,
    pub path_recalc_interval: u32,
    ticks_since_path: u32,
}

impl MeleeAttackGoal {
    const ATTACK_COOLDOWN_TICKS: i32 = 20;

    fn is_in_attack_range(&self, target_width: f32, distance_sq: f64) -> bool {
        let reach = (1.5 + target_width) as f64;
        distance_sq <= reach * reach
    }
}

impl PathfinderGoal for MeleeAttackGoal {
    fn flags(&self) -> GoalFlag { GoalFlag::MOVE }
    fn can_use(&self) -> bool { /* check mob has target */ true }
    fn start(&mut self) { self.attack_cooldown = 0; self.ticks_without_sight = 0; }
    fn stop(&mut self) { /* clear path */ }
    fn tick(&mut self) {
        self.attack_cooldown -= 1;
        self.ticks_since_path += 1;
        // Recalculate path every path_recalc_interval ticks.
        // If in range and cooldown elapsed, call mob.do_hurt_target()
        // Increment ticks_without_sight if no line of sight; stop after 200.
    }
    fn requires_update_every_tick(&self) -> bool { true }
}

// crates/oxidized-game/src/entity/ai/goals/random_look_around.rs

/// Randomly rotates the mob's yaw every 20–60 ticks.
pub struct RandomLookAroundGoal {
    angle_y: f32,
    countdown: i32,
}
impl PathfinderGoal for RandomLookAroundGoal {
    fn flags(&self) -> GoalFlag { GoalFlag::LOOK }
    fn can_use(&self) -> bool { true }
    fn tick(&mut self) {
        self.countdown -= 1;
        if self.countdown <= 0 {
            // pick random yaw offset in [-180, 180]
            self.countdown = 20 + (rand::random::<i32>().abs() % 41);
        }
        // apply look_control.set_look_at(...)
    }
}

// crates/oxidized-game/src/entity/ai/goals/water_avoiding_random_stroll.rs

/// Random-walk goal that steers away from water blocks.
/// Picks a random position within 10 blocks; re-picks if the target is water.
pub struct WaterAvoidingRandomStrollGoal {
    pub speed: f64,
    pub interval: u32,
    pub probability: f32,
    wander_target: Option<glam::Vec3>,
}
impl PathfinderGoal for WaterAvoidingRandomStrollGoal {
    fn flags(&self) -> GoalFlag { GoalFlag::MOVE }
    fn can_use(&self) -> bool { /* pick random pos, reject if water */ true }
    fn start(&mut self) { /* begin navigation to wander_target */ }
    fn tick(&mut self) { /* check path done; if so, stop */ }
}

// crates/oxidized-game/src/entity/ai/goals/look_at_player.rs

/// Faces the nearest player within `look_distance` blocks.
pub struct LookAtPlayerGoal {
    pub look_distance: f32,
    pub probability: f32,
    pub look_time: i32,
}
impl PathfinderGoal for LookAtPlayerGoal {
    fn flags(&self) -> GoalFlag { GoalFlag::LOOK }
    fn can_use(&self) -> bool {
        // Find nearest player within look_distance; rand < probability
        true
    }
    fn tick(&mut self) {
        self.look_time -= 1;
        // update look_control toward the tracked player
    }
}

// crates/oxidized-game/src/entity/ai/goals/nearest_attackable_target.rs

/// Acquires the nearest living entity of type T within `follow_range` as the mob's target.
pub struct NearestAttackableTargetGoal {
    pub follow_range: f32,
    pub randomize_interval: u32,
    pub must_see: bool,
    pub must_reach: bool,
}
impl PathfinderGoal for NearestAttackableTargetGoal {
    fn flags(&self) -> GoalFlag { GoalFlag::TARGET }
    fn can_use(&self) -> bool {
        // Scan entities within follow_range, pick closest player not behind wall
        true
    }
    fn start(&mut self) { /* lock on to target */ }
}
```

### 25.5 — `Mob` base struct

```rust
// crates/oxidized-game/src/entity/mob.rs

use super::living_entity::LivingEntity;
use crate::entity::ai::goal::GoalSelector;
use crate::entity::ai::navigation::PathNavigation;
use crate::entity::ai::controls::{LookControl, MoveControl, JumpControl};
use uuid::Uuid;

pub struct Mob {
    pub base: LivingEntity,

    /// AI goal scheduler (movement, attack, look).
    pub goal_selector: GoalSelector,
    /// Target-selection goal scheduler.
    pub target_selector: GoalSelector,

    pub navigation: PathNavigation,
    pub look_control: LookControl,
    pub move_control: MoveControl,
    pub jump_control: JumpControl,

    /// Current attack target entity UUID.
    pub target: Option<Uuid>,

    /// Whether AI processing is disabled for this mob.
    pub ai_disabled: bool,

    /// Whether the mob can pick up items from the ground.
    pub can_pick_up_loot: bool,

    /// Whether this mob was spawned naturally (versus by a spawner/command).
    pub persistent_required: bool,

    /// No despawn flag (set via name tag).
    pub no_ai: bool,
}

impl Mob {
    /// Server tick: run goals, navigation, movement.
    pub fn server_tick(&mut self) {
        if !self.ai_disabled {
            self.target_selector.tick();
            self.goal_selector.tick();
            self.navigation.tick_path();
            self.look_control.tick(self.base.pos.as_vec3());
        }
        self.base.server_tick();
    }

    /// Apply knockback from an attack source.
    /// Sends `ClientboundSetEntityMotionPacket` to nearby clients.
    pub fn knockback(&mut self, strength: f64, dx: f64, dz: f64) {
        let horizontal = (dx * dx + dz * dz).sqrt().max(0.0001);
        self.base.velocity.x /= 2.0;
        self.base.velocity.y /= 2.0;
        self.base.velocity.z /= 2.0;
        self.base.velocity.x -= dx / horizontal * strength;
        self.base.velocity.y += strength;
        self.base.velocity.z -= dz / horizontal * strength;
        if self.base.velocity.y > 0.4 {
            self.base.velocity.y = 0.4;
        }
    }
}
```

### 25.6 — Zombie

```rust
// crates/oxidized-game/src/entity/mob/zombie.rs

use super::super::mob::Mob;

/// Natural spawn weight: 100.
/// Spawn rules: any solid block at light level 0, Y -64 to 256, overworld.
pub struct Zombie {
    pub base: Mob,
    /// Baby variant (5% spawn chance). Baby zombies move 30% faster.
    pub is_baby: bool,
    /// Ticks underwater; converts to Drowned after 300 ticks.
    pub ticks_underwater: i32,
}

impl Zombie {
    pub const MAX_HEALTH: f32 = 20.0;
    pub const ATTACK_DAMAGE: f32 = 4.0; // 3 on easy, 4 on normal, 6 on hard
    pub const FOLLOW_RANGE: f32 = 35.0;
    pub const MOVEMENT_SPEED: f64 = 0.23;
    pub const ARMOR: f32 = 2.0;

    pub fn register_goals(mob: &mut Mob) {
        // Priority 0: FloatGoal (JUMP)
        // Priority 2: ZombieAttackGoal / MeleeAttackGoal (MOVE)
        // Priority 6: MoveThroughVillageGoal (MOVE)
        // Priority 7: WaterAvoidingRandomStrollGoal speed=1.0 (MOVE)
        // Priority 8: LookAtPlayerGoal distance=8.0 (LOOK)
        // Priority 8: RandomLookAroundGoal (LOOK)
        // Target priority 1: HurtByTargetGoal (TARGET)
        // Target priority 2: NearestAttackableTargetGoal<Player> (TARGET)
        // Target priority 3: NearestAttackableTargetGoal<IronGolem> (TARGET)
    }

    /// Called each tick while in daylight to handle sunburn.
    /// Sets entity on fire if exposed to sky and not wearing a helmet.
    pub fn do_daylight_burn(&mut self) {
        if self.base.base.level_is_daytime()
            && !self.base.base.is_on_fire()
            && self.is_wearing_helmet() == false
            && self.base.base.is_in_daylight()
        {
            self.base.base.set_on_fire(8);
        }
    }

    fn is_wearing_helmet(&self) -> bool {
        // check equipment slot HELMET != air
        false
    }

    /// On being hurt, 10% chance per nearby player to summon a zombie reinforcement.
    pub fn hurt(&mut self, damage: f32, source_x: f64, source_z: f64) {
        self.base.base.hurt(damage);
        // 10% chance: spawn Zombie within 5 blocks if mob cap allows
        // Send ClientboundSetEntityMotionPacket for knockback
    }

    /// Each tick underwater: increment counter; at 300 begin conversion to Drowned.
    pub fn server_tick(&mut self) {
        if self.base.base.is_underwater() {
            self.ticks_underwater += 1;
            if self.ticks_underwater >= 300 {
                self.start_drowned_conversion();
            }
        } else {
            self.ticks_underwater = -1;
        }
        self.do_daylight_burn();
        self.base.server_tick();
    }

    fn start_drowned_conversion(&mut self) {
        // Replace with Drowned entity preserving position/health/equipment
    }
}
```

### 25.7 — Skeleton

```rust
// crates/oxidized-game/src/entity/mob/skeleton.rs

/// Skeleton with bow-based ranged attack.
pub struct Skeleton {
    pub base: Mob,
    /// Ticks since last arrow was fired.
    pub arrow_cooldown: i32,
}

impl Skeleton {
    pub const MAX_HEALTH: f32 = 20.0;
    pub const ATTACK_DAMAGE: f32 = 4.0; // base arrow damage
    pub const MOVEMENT_SPEED: f64 = 0.25;
    pub const FOLLOW_RANGE: f32 = 35.0;

    pub fn register_goals(mob: &mut Mob) {
        // Priority 1: RangedBowAttackGoal (MOVE + LOOK)
        // Priority 2: WaterAvoidingRandomStrollGoal speed=1.0 (MOVE)
        // Priority 6: LookAtPlayerGoal distance=8.0 (LOOK)
        // Priority 6: RandomLookAroundGoal (LOOK)
        // Target priority 1: HurtByTargetGoal (TARGET)
        // Target priority 2: NearestAttackableTargetGoal<Player> (TARGET)
    }

    pub fn server_tick(&mut self) {
        // Convert to SkeletonHorse rider during thunderstorm with very low chance
        self.base.server_tick();
    }
}

/// Shoots an arrow at the target every 20-60 ticks (varies with difficulty).
/// Mimics `RangedBowAttackGoal` in Java.
pub struct RangedBowAttackGoal {
    pub speed_modifier: f64,
    pub attack_interval_min: i32, // 20 ticks
    pub attack_interval_max: i32, // 60 ticks
    pub attack_radius_sq: f32,    // 15.0^2
    attack_time: i32,
    strafing_time: i32,
    strafing_clockwise: bool,
    strafing_backwards: bool,
}

impl PathfinderGoal for RangedBowAttackGoal {
    fn flags(&self) -> GoalFlag { GoalFlag::MOVE | GoalFlag::LOOK }
    fn can_use(&self) -> bool {
        // mob has a target AND is holding a bow
        true
    }
    fn tick(&mut self) {
        self.attack_time -= 1;
        if self.attack_time <= 0 {
            // performRangedAttack: spawn ArrowEntity aimed at target
            // add inaccuracy based on difficulty
            let next = 20 + (rand::random::<i32>().abs() % 41);
            self.attack_time = next;
        }
        // Strafe perpendicular to target every 20 ticks
        self.strafing_time += 1;
        if self.strafing_time >= 20 {
            self.strafing_time = 0;
            self.strafing_clockwise = rand::random();
            self.strafing_backwards = rand::random();
        }
    }
}
```

### 25.8 — Creeper

```rust
// crates/oxidized-game/src/entity/mob/creeper.rs

/// Creeper: silent approach → swell → explode.
pub struct Creeper {
    pub base: Mob,
    /// Current swell progress: -1 (not swelling), 0 (start), 30 (explode).
    pub swell: i32,
    /// Previous swell for interpolation.
    pub old_swell: i32,
    /// Maximum swell before explosion (30 ticks = 1.5 s).
    pub max_swell: i32,
    /// Whether powered by a lightning strike (doubled explosion radius).
    pub is_powered: bool,
    /// Whether the fuse has been lit (approaching player).
    pub is_ignited: bool,
    /// Explosion radius (3.0 normal, 6.0 charged).
    pub explosion_radius: f32,
}

impl Creeper {
    pub const MAX_HEALTH: f32 = 20.0;
    pub const MOVEMENT_SPEED: f64 = 0.25;
    pub const FOLLOW_RANGE: f32 = 16.0;
    /// Ticks from swell start to explosion.
    pub const MAX_SWELL: i32 = 30;

    pub fn new(is_powered: bool) -> Self {
        Self {
            base: Mob::default(),
            swell: -1,
            old_swell: -1,
            max_swell: Self::MAX_SWELL,
            is_powered,
            is_ignited: false,
            explosion_radius: if is_powered { 6.0 } else { 3.0 },
        }
    }

    pub fn server_tick(&mut self) {
        self.old_swell = self.swell;
        if self.should_swell() {
            if self.swell == -1 {
                self.swell = 0;
                // Send CREEPER_SWELL EntityEvent (id=9) to clients
                // Send ClientboundGameEventPacket type 17 for swell audio
            }
            self.swell += 1;
            if self.swell >= self.max_swell {
                self.explode();
            }
        } else if self.swell > 0 {
            self.swell -= 1;
        }
        self.base.server_tick();
    }

    fn should_swell(&self) -> bool {
        // true if target player is within 3 blocks
        false
    }

    fn explode(&mut self) {
        // Create Explosion at entity pos, radius self.explosion_radius, power 3.0
        // destroy blocks in sphere, hurt entities, send ClientboundExplosionPacket
        // remove entity from world
    }
}

/// Swells the creeper when a player is within 3 blocks.
pub struct SwellGoal {
    pub swell_distance: f32, // 3.0
}
impl PathfinderGoal for SwellGoal {
    fn flags(&self) -> GoalFlag { GoalFlag::MOVE }
    fn can_use(&self) -> bool { /* target != null && dist < swell_distance */ true }
    fn tick(&mut self) { /* handled in Creeper::server_tick */ }
}
```

### 25.9 — Spider and Enderman

```rust
// crates/oxidized-game/src/entity/mob/spider.rs

pub struct Spider {
    pub base: Mob,
    pub is_climbing: bool,
}

impl Spider {
    pub const MAX_HEALTH: f32 = 16.0;
    pub const ATTACK_DAMAGE: f32 = 2.0;
    pub const MOVEMENT_SPEED: f64 = 0.3;

    pub fn server_tick(&mut self) {
        // Check if horizontally adjacent to a wall → set is_climbing
        // Apply poison effect (3s on Normal, 7s on Hard) when attacking
        // Passive during day if not provoked (no target)
        self.base.server_tick();
    }

    /// Walk up vertical surfaces.
    pub fn check_climbing(&mut self, adjacent_to_wall: bool) {
        self.is_climbing = adjacent_to_wall;
        // Set EntityDataAccessor CLIMBING flag
    }
}

// crates/oxidized-game/src/entity/mob/enderman.rs

use glam::DVec3;
use uuid::Uuid;

pub struct Enderman {
    pub base: Mob,
    /// Block the enderman is currently carrying (if any).
    pub carried_block: Option<u32>, // block state id
    /// Ticks until the enderman can teleport again (cooldown).
    pub teleport_cooldown: i32,
    /// Whether the player is currently staring at this enderman.
    pub is_being_stared_at: bool,
    /// Whether the enderman is screaming (player stared at it).
    pub is_screaming: bool,
}

impl Enderman {
    pub const MAX_HEALTH: f32 = 40.0;
    pub const ATTACK_DAMAGE: f32 = 7.0;
    pub const MOVEMENT_SPEED: f64 = 0.3;
    /// Hitbox: 0.6 wide × 2.9 tall (3×3 blocks visible height).
    pub const HITBOX_WIDTH: f32 = 0.6;
    pub const HITBOX_HEIGHT: f32 = 2.9;

    /// Teleport to a random position within 32 blocks.
    pub fn random_teleport(&mut self) -> bool {
        if self.teleport_cooldown > 0 {
            return false;
        }
        let dx = (rand::random::<f64>() - 0.5) * 64.0;
        let dy = (rand::random::<f64>() - 0.5) * 64.0;
        let dz = (rand::random::<f64>() - 0.5) * 64.0;
        // Attempt teleport; verify destination is solid below and air at feet/head
        self.teleport_cooldown = 20;
        true
    }

    /// Teleport away from a projectile.
    pub fn teleport_away_from(&mut self, source: DVec3) -> bool {
        let angle = (source - self.base.base.pos).normalize();
        // Try 16 random positions behind the entity
        self.random_teleport()
    }

    pub fn server_tick(&mut self) {
        if self.teleport_cooldown > 0 { self.teleport_cooldown -= 1; }
        // Take 1 damage per tick in rain or water
        if self.base.base.is_in_water() || self.base.base.is_in_rain() {
            self.base.base.hurt(1.0);
        }
        // If player is staring (eye vector intersects head bbox within 64 blocks):
        //   set is_screaming=true, play scream sound, set target to that player
        self.base.server_tick();
    }
}
```

### 25.10 — Mob spawning basics

```rust
// crates/oxidized-game/src/world/spawner.rs

/// Natural spawn categories and their caps (per 17×17 chunk area around players).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MobCategory {
    Monster,       // cap: 70
    Creature,      // cap: 10
    Ambient,       // cap: 15
    WaterCreature, // cap: 5
    WaterAmbient,  // cap: 20
    Misc,          // no cap
}

impl MobCategory {
    pub fn max_instances_per_chunk(self) -> i32 {
        match self {
            Self::Monster       => 70,
            Self::Creature      => 10,
            Self::Ambient       => 15,
            Self::WaterCreature => 5,
            Self::WaterAmbient  => 20,
            Self::Misc          => i32::MAX,
        }
    }

    pub fn is_friendly(self) -> bool {
        !matches!(self, Self::Monster)
    }
}

/// Per-tick natural spawn check. Called once per game tick for each loaded chunk.
/// Mirrors `NaturalSpawner::spawnForChunk`.
pub struct NaturalSpawner;

impl NaturalSpawner {
    /// Attempt to spawn mobs in `chunk`. Returns number of mobs spawned.
    pub fn spawn_for_chunk(
        chunk_x: i32,
        chunk_z: i32,
        category: MobCategory,
        current_count: i32,
        loaded_chunk_count: i32,
    ) -> i32 {
        let cap = category.max_instances_per_chunk()
            * loaded_chunk_count / 289; // 17×17 = 289 chunks
        if current_count >= cap {
            return 0;
        }
        // Pick random block in chunk, check:
        //   1. is_full_block(below), is_air(at), is_air(above)
        //   2. light level <= 0 for monsters
        //   3. not within 24 blocks of any player
        //   4. valid spawn biome for the mob type
        // Spawn up to pack_size mobs in the same area
        0
    }
}
```

---

## Data Structures Summary

```rust
// Key types in oxidized-game::entity

pub use mob::Mob;
pub use mob::zombie::Zombie;
pub use mob::skeleton::Skeleton;
pub use mob::creeper::Creeper;
pub use mob::spider::Spider;
pub use mob::enderman::Enderman;
pub use ai::goal::{GoalSelector, PathfinderGoal, GoalFlag};
pub use ai::navigation::{PathNavigation, Path, PathPoint};
pub use ai::controls::{LookControl, MoveControl, JumpControl};
pub use world::spawner::{NaturalSpawner, MobCategory};
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::ai::goal::{GoalFlag, GoalSelector};
    use crate::entity::ai::navigation::PathNavigation;
    use glam::IVec3;

    // --- GoalSelector ---

    struct AlwaysGoal { ran: bool, stopped: bool }
    impl PathfinderGoal for AlwaysGoal {
        fn flags(&self) -> GoalFlag { GoalFlag::MOVE }
        fn can_use(&self) -> bool { true }
        fn tick(&mut self) { self.ran = true; }
        fn stop(&mut self) { self.stopped = true; }
    }

    struct NeverGoal;
    impl PathfinderGoal for NeverGoal {
        fn flags(&self) -> GoalFlag { GoalFlag::MOVE }
        fn can_use(&self) -> bool { false }
    }

    /// Only one goal per flag runs at a time; lower priority wins.
    #[test]
    fn goal_selector_one_per_flag() {
        let mut sel = GoalSelector::new();
        sel.add_goal(1, AlwaysGoal { ran: false, stopped: false });
        sel.add_goal(2, AlwaysGoal { ran: false, stopped: false });
        // Two ticks so the 2-tick interval fires.
        sel.tick(); sel.tick();
        // Only the priority-1 goal should be running.
        assert_eq!(sel.goals.iter().filter(|g| g.running).count(), 1);
        assert_eq!(sel.goals[0].priority, 1);
    }

    /// NeverGoal does not start.
    #[test]
    fn goal_selector_never_goal_stays_idle() {
        let mut sel = GoalSelector::new();
        sel.add_goal(1, NeverGoal);
        sel.tick(); sel.tick();
        assert!(!sel.goals[0].running);
    }

    // --- PathNavigation A* ---

    /// Straight-line path on an open flat plane.
    #[test]
    fn navigation_straight_path() {
        let nav = PathNavigation::new(1.0);
        let start = IVec3::new(0, 0, 0);
        let goal  = IVec3::new(4, 0, 0);
        let path = nav.create_path(start, goal, &|_| true).unwrap();
        assert_eq!(*path.points.first().unwrap(), start);
        assert_eq!(*path.points.last().unwrap(), goal);
    }

    /// Unreachable goal returns None when all cells are impassable.
    #[test]
    fn navigation_unreachable_returns_none() {
        let nav = PathNavigation::new(1.0);
        let result = nav.create_path(
            IVec3::ZERO,
            IVec3::new(10, 0, 0),
            &|_| false,
        );
        assert!(result.is_none());
    }

    /// Path navigates around a wall.
    #[test]
    fn navigation_path_around_wall() {
        let nav = PathNavigation::new(1.0);
        // Wall at x=2, z=0..=2
        let wall: std::collections::HashSet<IVec3> = [
            IVec3::new(2, 0, 0),
            IVec3::new(2, 0, 1),
            IVec3::new(2, 0, 2),
        ].iter().cloned().collect();
        let path = nav.create_path(
            IVec3::ZERO,
            IVec3::new(4, 0, 0),
            &|p| !wall.contains(&p),
        ).unwrap();
        // Path must not pass through any wall block.
        for point in &path.points {
            assert!(!wall.contains(point));
        }
        assert_eq!(*path.points.last().unwrap(), IVec3::new(4, 0, 0));
    }

    // --- LookControl ---

    #[test]
    fn look_control_converges_on_target() {
        let mut lc = LookControl::new(10.0, 10.0);
        lc.yaw = 0.0;
        lc.set_look_at(glam::Vec3::new(0.0, 0.0, 5.0));
        lc.tick(glam::Vec3::ZERO);
        // After one tick yaw should have moved toward 0° (north = -z in MC, south = +z → 180°)
        assert!(lc.yaw.abs() <= 10.0 + f32::EPSILON);
    }

    // --- MobCategory ---

    #[test]
    fn monster_cap_scales_with_chunk_count() {
        let cap = MobCategory::Monster.max_instances_per_chunk();
        assert_eq!(cap, 70);
        assert!(!MobCategory::Monster.is_friendly());
        assert!(MobCategory::Creature.is_friendly());
    }

    // --- Creeper swell ---

    #[test]
    fn creeper_charged_has_doubled_explosion_radius() {
        let normal  = Creeper::new(false);
        let charged = Creeper::new(true);
        assert!((charged.explosion_radius - normal.explosion_radius * 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn creeper_swell_starts_at_minus_one() {
        let c = Creeper::new(false);
        assert_eq!(c.swell, -1);
    }
}
```
