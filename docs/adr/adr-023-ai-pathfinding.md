# ADR-023: AI & Pathfinding System

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P25, P27 |
| Deciders | Oxidized Core Team |

## Context

Mob artificial intelligence in Minecraft is built on two complementary systems. The
**GoalSelector** is a priority-based goal list used by most mobs (zombies, skeletons,
creepers, animals, iron golems, etc.). Each mob has an ordered list of goals. Every tick,
the selector evaluates which goals can start (`canUse()`), which active goals should
continue (`canContinueToUse()`), and runs the highest-priority active goals. Goals control
high-level behavior: "attack nearest player," "wander randomly," "flee from danger," "breed
with nearby animal." The **Brain** system, introduced later, is a behavior-tree-like
structure used by advanced mobs (villagers, piglins, axolotls, frogs, wardens). Brains use
activities (IDLE, WORK, REST, FIGHT), sensors (detect nearby entities/blocks), and memories
(store perceived state). Both systems rely heavily on pathfinding.

**Pathfinding** uses A* search on a block-level grid. Each block position is a potential
node. Edges connect adjacent blocks (including diagonals for some navigation types). Edge
costs reflect traversal difficulty: open air is cost 1.0, water is higher for ground mobs,
lava is impassable for most, doors are passable if the mob can open doors. The path from a
mob to its target is computed by expanding the cheapest-cost node until the target is
reached or a node limit is exceeded. Vanilla's pathfinder has a maximum expansion limit
(default 400 nodes) and a maximum path length (configurable per mob, typically 16-48
blocks). Pathfinding is one of the most expensive per-mob operations: A* on a block grid
can evaluate hundreds of nodes per path request, and many mobs request new paths every 4-10
ticks.

On a server with 2,000 AI-enabled mobs (a common scenario with mob farms, villager
breeders, and natural spawning), AI and pathfinding can consume 10-20ms per tick — a
substantial fraction of the 50ms budget. Any optimization that reduces this cost directly
translates to higher player capacity. However, we must be careful: changing AI behavior
changes mob farm rates, villager trading mechanics, and raid difficulty, which players are
extremely sensitive to.

## Decision Drivers

- **Behavioral fidelity**: Mob behavior must match vanilla exactly. A zombie must target
  the nearest player, pathfind toward them, attack at melee range, burn in sunlight (unless
  wearing a helmet), and convert villagers on hard difficulty. Players build farms and
  contraptions that depend on precise mob behavior.
- **Pathfinding performance**: A* is the dominant cost. Optimizations must reduce node
  evaluations without changing the resulting path (or changing it only in cases where
  vanilla's path is non-deterministic anyway).
- **GoalSelector compatibility**: The goal priority system, mutual exclusion via flag bits,
  and tick-by-tick evaluation must match vanilla's `GoalSelector` semantics.
- **Brain system compatibility**: Villager schedules, piglin bartering, warden detection —
  all brain-based behaviors must work correctly.
- **Spawn cap integration**: Mob AI is closely tied to mob spawning — spawn cap calculations,
  despawn distance checks, and biome filters must use the same entity queries as AI systems.
- **Scalability**: The AI system must degrade gracefully under load — if there are too many
  mobs, non-critical AI (random strolling) should be deprioritized before critical AI
  (attack targeting).

## Considered Options

### Option 1: GoalSelector + A* Like Vanilla

Directly replicate vanilla's `GoalSelector`, `Brain`, and `PathFinder` systems as ECS
systems operating on components.

**Pros:**
- Exact behavioral match by construction.
- Straightforward to port and verify against vanilla source.
- Well-understood performance characteristics.

**Cons:**
- Inherits vanilla's performance problems — no path caching, no async pathfinding,
  every mob recalculates every tick.
- A* on a block grid is inherently expensive for long paths.

**Verdict: Selected (as the base), with targeted optimizations.**

### Option 2: Behavior Trees with a Proper BT Library

Replace GoalSelector and Brain with a unified behavior tree framework (e.g., using a
library like `bonsai`).

**Pros:**
- Unified model for all mob AI (no GoalSelector vs. Brain split).
- Behavior trees are well-studied and have good tooling.
- Easier to debug and visualize.

**Cons:**
- Different execution model than vanilla — behavior tree tick semantics don't match
  GoalSelector's priority-based preemption exactly.
- Mapping vanilla's specific goal interactions (mutual exclusion flags, same-priority
  replacement rules) to BT nodes is non-trivial.
- Introduces a behavioral divergence risk that's hard to test exhaustively.

**Verdict: Rejected.** Behavioral fidelity risk is too high.

### Option 3: GOAP (Goal-Oriented Action Planning)

Use GOAP where mobs plan action sequences to achieve goals, evaluating world state to find
optimal action chains.

**Pros:**
- More flexible than goal selectors — mobs can reason about multi-step plans.
- Used successfully in games like F.E.A.R.

**Cons:**
- Fundamentally different from vanilla's reactive goal model.
- Planning overhead is much higher than goal evaluation.
- Overkill for Minecraft's relatively simple mob behaviors.
- Would produce different behavior than vanilla.

**Verdict: Rejected.** Wrong model for Minecraft's tick-by-tick reactive AI.

### Option 4: Hierarchical Pathfinding (HPA*)

Use hierarchical pathfinding (HPA*) where the world is divided into regions with
precomputed inter-region paths. Long-distance navigation uses the region graph; local
navigation uses A* within a region.

**Pros:**
- Dramatically faster for long paths (100+ blocks) — order of magnitude fewer node
  evaluations.
- Amortizes work across many path requests in the same area.

**Cons:**
- Region graph must be updated when blocks change (door opened, block broken, etc.),
  adding maintenance overhead.
- May produce slightly different paths than vanilla's flat A* (different node expansion
  order → different tie-breaking → different path when multiple equal-cost paths exist).
- Significant implementation complexity.
- Most mob paths are short (16-48 blocks); the payoff for HPA* is lower for short paths.

**Verdict: Deferred.** Interesting optimization for long-range navigation (e.g., villager
"walk to work" paths) but not needed initially. May revisit in later phases.

### Option 5: Flow Fields for Group Movement

Precompute a flow field (direction grid) from a target position. All mobs targeting the
same location look up their movement direction from the field. Efficient for many mobs
converging on one point (e.g., zombie siege).

**Pros:**
- O(1) per mob for direction lookup after field computation.
- Handles large groups efficiently.

**Cons:**
- Flow fields are expensive to compute (flood fill from target).
- Only useful when many mobs share a target — most Minecraft scenarios have mobs with
  individual targets (each zombie targets the nearest player independently).
- Does not handle mob-specific constraints (some mobs can open doors, some can't).

**Verdict: Rejected.** Niche benefit doesn't justify complexity for general mob AI.

## Decision

**We implement a GoalSelector-compatible goal system with vanilla A* pathfinding, enhanced
by targeted optimizations: path caching, lazy neighbor evaluation, and optional async path
requests.** The goal system and pathfinding are implemented as ECS systems operating on
`AiGoals`, `NavigationPath`, and related components. The Brain system is implemented as an
alternative AI driver component (`BrainAi`) used by villagers, piglins, and other
brain-based mobs.

### GoalSelector as an ECS Component

```rust
#[derive(Component)]
struct AiGoals {
    goals: Vec<PrioritizedGoal>,
    active_goals: SmallVec<[usize; 4]>,
    /// Flags used for mutual exclusion between goals.
    disabled_flags: GoalFlags,
}

struct PrioritizedGoal {
    priority: i32,
    goal: Box<dyn Goal>,
    flags: GoalFlags,
    running: bool,
}

bitflags! {
    struct GoalFlags: u8 {
        const MOVE = 0x01;
        const LOOK = 0x02;
        const JUMP = 0x04;
        const TARGET = 0x08;
    }
}

trait Goal: Send + Sync {
    /// Can this goal start? Evaluated every tick for inactive goals.
    fn can_use(&self, entity: Entity, world: &World) -> bool;

    /// Should this goal continue? Evaluated every tick for active goals.
    fn can_continue_to_use(&self, entity: Entity, world: &World) -> bool { true }

    /// Called when the goal starts.
    fn start(&mut self, entity: Entity, world: &mut World) {}

    /// Called every tick while the goal is active.
    fn tick(&mut self, entity: Entity, world: &mut World) {}

    /// Called when the goal stops (preempted or finished).
    fn stop(&mut self, entity: Entity, world: &mut World) {}

    /// Flags that this goal uses (for mutual exclusion).
    fn flags(&self) -> GoalFlags { GoalFlags::empty() }

    /// Whether this goal requires continuous updates (every tick vs. every few ticks).
    fn requires_update_every_tick(&self) -> bool { false }
}
```

### Goal Evaluation System

The `goal_selector_system` runs every tick during the AI phase:

```rust
fn goal_selector_system(
    mut query: Query<(Entity, &mut AiGoals)>,
    world: &World,
) {
    for (entity, mut goals) in &mut query {
        // 1. Stop goals that can no longer continue
        for idx in goals.active_goals.clone() {
            let goal = &goals.goals[idx];
            if !goal.goal.can_continue_to_use(entity, world) {
                goal.goal.stop(entity, world);
                goals.goals[idx].running = false;
                goals.active_goals.retain(|&i| i != idx);
            }
        }

        // 2. Evaluate inactive goals (can any start?)
        for (idx, goal) in goals.goals.iter_mut().enumerate() {
            if goal.running { continue; }
            if !goal.goal.can_use(entity, world) { continue; }

            // Check flag conflicts with higher-priority active goals
            if can_activate(&goals, idx) {
                // Stop conflicting lower-priority goals
                stop_conflicting(&mut goals, idx);
                goal.goal.start(entity, world);
                goal.running = true;
                goals.active_goals.push(idx);
            }
        }

        // 3. Tick all active goals
        for &idx in &goals.active_goals {
            goals.goals[idx].goal.tick(entity, world);
        }
    }
}
```

### Common Goal Implementations

| Vanilla Goal | Oxidized Implementation | Flags |
|-------------|------------------------|-------|
| `MeleeAttackGoal` | `MeleeAttackGoal { speed: f64, attack_interval: i32 }` | MOVE, LOOK |
| `RangedAttackGoal` | `RangedAttackGoal { speed: f64, attack_interval: i32, range: f32 }` | MOVE, LOOK |
| `RandomStrollGoal` | `RandomStrollGoal { speed: f64, interval: i32 }` | MOVE |
| `LookAtPlayerGoal` | `LookAtPlayerGoal { range: f32, probability: f32 }` | LOOK |
| `FloatGoal` | `FloatGoal` | JUMP |
| `PanicGoal` | `PanicGoal { speed: f64 }` | MOVE |
| `BreedGoal` | `BreedGoal { speed: f64, partner_class: EntityType }` | MOVE |
| `FollowParentGoal` | `FollowParentGoal { speed: f64 }` | MOVE |
| `NearestAttackableTargetGoal` | `NearestAttackableTargetGoal { target_type: EntityType, range: f64 }` | TARGET |
| `HurtByTargetGoal` | `HurtByTargetGoal { alert_others: bool }` | TARGET |
| `TemptGoal` | `TemptGoal { speed: f64, items: Vec<Item> }` | MOVE, LOOK |
| `WaterAvoidingRandomStrollGoal` | `WaterAvoidRandomStrollGoal { speed: f64 }` | MOVE |
| `OpenDoorGoal` | `OpenDoorGoal { close_after: bool }` | (none) |
| `AvoidEntityGoal` | `AvoidEntityGoal { avoid_type: EntityType, max_dist: f64, speed: f64 }` | MOVE |

### Pathfinding

Pathfinding is implemented as a subsystem called by goals when they need navigation.

```rust
#[derive(Component)]
struct NavigationPath {
    path: Option<Path>,
    navigation_type: NavigationType,
    max_distance: f32,
    can_open_doors: bool,
    can_pass_doors: bool,
    can_float: bool,
}

enum NavigationType {
    Ground,
    Flying,
    Water,
    Climbing, // spiders
}

struct Path {
    nodes: Vec<BlockPos>,
    current_index: usize,
    target: BlockPos,
    reached: bool,
}
```

#### A* Implementation

```rust
fn find_path(
    start: BlockPos,
    target: BlockPos,
    nav: &NavigationPath,
    level: &Level,
    max_nodes: usize, // default: 400
) -> Option<Path> {
    let mut open_set = BinaryHeap::new();
    let mut came_from: HashMap<BlockPos, BlockPos> = HashMap::new();
    let mut g_score: HashMap<BlockPos, f32> = HashMap::new();

    let start_node = PathNode { pos: start, f_score: heuristic(start, target) };
    open_set.push(start_node);
    g_score.insert(start, 0.0);

    let mut evaluated = 0;

    while let Some(current) = open_set.pop() {
        if current.pos == target {
            return Some(reconstruct_path(came_from, current.pos));
        }

        evaluated += 1;
        if evaluated > max_nodes {
            // Return best partial path (closest to target)
            return Some(reconstruct_partial_path(came_from, &g_score, target));
        }

        for neighbor in get_neighbors(current.pos, nav, level) {
            let tentative_g = g_score[&current.pos] + neighbor.cost;
            if tentative_g < *g_score.get(&neighbor.pos).unwrap_or(&f32::INFINITY) {
                came_from.insert(neighbor.pos, current.pos);
                g_score.insert(neighbor.pos, tentative_g);
                open_set.push(PathNode {
                    pos: neighbor.pos,
                    f_score: tentative_g + heuristic(neighbor.pos, target),
                });
            }
        }
    }

    None // no path found
}
```

#### Navigation Types

| Type | Traversable | Cost Modifiers |
|------|------------|----------------|
| **Ground** | Solid blocks below, air/passable above | Water: +8.0, Lava: impassable, Fence gate (closed): impassable |
| **Flying** | Any non-solid block | No gravity penalty, straight-line preference |
| **Water** | Water blocks | Air: impassable (for fish), current: reduced cost in flow direction |
| **Climbing** | Ground + solid walls (for spiders) | Wall-adjacent: +0.0 (free climbing) |

#### Node Evaluation

Each candidate neighbor is evaluated for traversability and cost:

```rust
fn evaluate_node(
    pos: BlockPos,
    nav: &NavigationPath,
    level: &Level,
) -> Option<NodeEvaluation> {
    let block = level.get_block_state(pos);
    let below = level.get_block_state(pos.below());

    match nav.navigation_type {
        NavigationType::Ground => {
            // Must have solid block below and passable block at pos and pos.above()
            if !below.is_solid() { return None; }
            if block.is_solid() { return None; }
            if level.get_block_state(pos.above()).is_solid() { return None; } // headroom

            let mut cost = 1.0;
            // Water penalty
            if block.is_water() { cost += 8.0; }
            // Lava is impassable
            if block.is_lava() { return None; }
            // Door handling
            if block.is_door() && !nav.can_open_doors { return None; }
            // Soul sand slowdown
            if below.is_soul_sand() { cost += 2.0; }

            Some(NodeEvaluation { pos, cost })
        }
        // ... other navigation types
    }
}
```

### Optimizations

#### Path Caching

If a mob's target has moved less than 1 block since the last path was computed, and the
mob is still making progress along the path, reuse the existing path. This avoids
recomputing paths when the target is stationary or barely moving.

```rust
fn should_recompute_path(nav: &NavigationPath, new_target: BlockPos) -> bool {
    match &nav.path {
        None => true,
        Some(path) => {
            let distance_moved = path.target.distance_squared(new_target);
            distance_moved > 1.0 || path.is_stuck()
        }
    }
}
```

#### Lazy Neighbor Evaluation

Only evaluate neighbor traversability when the neighbor is actually popped from the open
set (not when it's first discovered). This avoids evaluating nodes that are never expanded
because a better path was found first.

#### Async Path Requests (Non-Urgent)

For non-combat AI (random strolling, looking at player), path requests are queued and
processed on a background thread. The mob continues its current behavior until the path
result arrives (typically next tick). This moves pathfinding work off the main tick thread.

```rust
#[derive(Component)]
struct PendingPathRequest {
    start: BlockPos,
    target: BlockPos,
    result: Arc<OnceLock<Option<Path>>>,
}
```

Combat AI (melee attack, flee) always uses synchronous pathfinding for responsiveness.

### Mob Spawning Integration

Mob spawning is a separate system but closely related to AI:

```rust
fn mob_spawn_system(
    mut commands: Commands,
    query: Query<&Position, With<PlayerMarker>>,
    level: Res<Level>,
    mob_counts: Res<MobCategoryCount>,
) {
    // For each loaded chunk within spawn range of a player:
    // 1. Check mob cap per category (hostile: 70, passive: 10, water: 5, etc.)
    // 2. Pick random position within chunk
    // 3. Check biome spawn list for valid mob types
    // 4. Check light level, block type, height constraints
    // 5. Spawn mob with appropriate components + AI goals
}
```

Despawning rules:
- Hostile mobs > 128 blocks from any player: instant despawn.
- Hostile mobs > 32 blocks from any player: random chance to despawn each tick.
- Persistent mobs (named, tamed, carrying items): never despawn.

## Consequences

### Positive

- **Exact mob behavior match**: GoalSelector priority semantics, goal flag mutual
  exclusion, and A* pathfinding all match vanilla's model, ensuring mob farms, raid
  mechanics, and villager behavior work as expected.
- **Path caching reduces CPU cost by ~30-50%**: Most mobs target slowly-moving or
  stationary targets. Caching avoids redundant A* runs.
- **Async pathfinding for non-combat AI**: Random strolling paths computed off-thread
  reduces main-thread pathfinding by ~60% (most path requests are non-combat).
- **ECS parallelism**: Independent mob AI evaluations run in parallel. Two zombies in
  different chunks have no data dependencies.

### Negative

- **A* is still expensive for long paths**: A mob pathfinding 48 blocks may evaluate 200+
  nodes. With 500 pathfinding mobs, this is 100,000+ node evaluations per tick. The
  optimizations help but don't eliminate the fundamental cost.
- **Brain system complexity**: Villager brains with schedules, memories, and sensors are
  significantly more complex than GoalSelector. This is substantial implementation work.
- **Async path results arrive late**: A mob requesting a path asynchronously doesn't
  receive it until the next tick. For 1-tick responsiveness (combat), synchronous
  pathfinding is still required.

### Neutral

- **Goal implementations are entity-type-specific**: Each mob type has a unique combination
  of goals (zombie: float, zombie attack, nearest target, hurt-by-target, random stroll,
  look-at-player; skeleton: float, ranged attack, avoid sunlight, nearest target, random
  stroll, look-at-player). This is a large but straightforward implementation task.
- **Navigation mesh is implicit**: Unlike dedicated pathfinding libraries, we don't build
  an explicit navigation mesh. The block grid IS the navigation mesh. This is correct for
  Minecraft but differs from typical game AI approaches.

## Compliance

- **Mob behavior parity tests**: For each mob type, record vanilla AI decisions over 100
  ticks (target selection, path chosen, goal transitions) and assert Oxidized produces
  identical results given the same world state.
- **Pathfinding correctness tests**: For known start/end positions with known block layouts,
  assert A* produces the same path as vanilla (within tie-breaking tolerance).
- **Spawn rate tests**: Run a mob farm design for 10,000 ticks and verify spawn rates match
  vanilla within ±5%.
- **Performance benchmark**: AI phase for 2,000 mobs must complete in < 10ms (on a modern
  CPU). Pathfinding for a single 32-block path must complete in < 50μs.

## Related ADRs

- **ADR-018**: Entity System Architecture — AI goals and navigation are ECS components on
  mob entities
- **ADR-019**: Tick Loop Design — AI runs in the ENTITY_TICK phase after physics
- **ADR-021**: Physics & Collision Engine — pathfinding uses collision shapes to determine
  block traversability

## References

- Vanilla source: `net.minecraft.world.entity.ai.goal.GoalSelector`
- Vanilla source: `net.minecraft.world.entity.ai.goal.Goal`
- Vanilla source: `net.minecraft.world.entity.ai.Brain`
- Vanilla source: `net.minecraft.world.level.pathfinder.PathFinder`
- Vanilla source: `net.minecraft.world.level.pathfinder.WalkNodeEvaluator`
- Vanilla source: `net.minecraft.world.entity.ai.navigation.GroundPathNavigation`
- Vanilla source: `net.minecraft.world.level.NaturalSpawner`
- [A* Pathfinding — Red Blob Games](https://www.redblobgames.com/pathfinding/a-star/introduction.html)
- [Minecraft Wiki — Mob Spawning](https://minecraft.wiki/w/Mob_spawning)
- [Minecraft Wiki — Villager — Behavior](https://minecraft.wiki/w/Villager#Behavior)
