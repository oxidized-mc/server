# ADR-025: Redstone Simulation Model

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P28 |
| Deciders | Oxidized Core Team |

## Context

Redstone is Minecraft's in-game logic circuit system. Using redstone dust, torches,
repeaters, comparators, pistons, observers, and other components, players build functional
circuits ranging from simple door toggles to full CPUs and programmable displays. The
redstone simulation is fundamentally a block update propagation system: when a redstone
component changes state (e.g., a lever is flipped), it triggers neighbor updates in
adjacent blocks, which may change their own state and trigger further updates, propagating
through the circuit until a stable state is reached or the update limit is exceeded.

Vanilla's redstone implementation has accumulated a set of behaviors over 15 years of
development that are technically bugs but are now considered features by the community.
**Quasi-connectivity** (also called "BUD powering"): pistons, dispensers, and droppers can
be activated by a power source in the block position directly above them, even though no
redstone signal path exists. This was originally a bug in how these blocks check for power,
but players now rely on it extensively for "BUD switches" and flush piston doors.
**0-tick pistons**: under specific timing conditions, a piston can receive a 1-tick pulse
and extend/retract within the same game tick, bypassing the normal 2-tick extension time.
This allows instant block transport that is fundamental to many complex contraptions.
**Update order dependence**: the order in which block neighbors receive updates is fixed
(−X, +X, −Z, +Z, −Y, +Y) and deterministic. Players exploit this for directional circuits
where the output depends on which component updates first. **Comparator timing quirks**:
comparators measuring container contents or comparing signals have specific tick delays that
enable precise timing circuits.

The Minecraft technical community has spent years documenting and building around these
behaviors. Any redstone implementation that changes even one of these quirks would break
thousands of existing contraptions, YouTube tutorials, and world downloads. Redstone
correctness is not about matching a specification — it's about matching vanilla's actual
behavior, including all unintentional emergent behaviors.

## Decision Drivers

- **Exact behavioral match**: Every redstone circuit that works on vanilla must work
  identically on Oxidized. This includes quasi-connectivity, 0-tick pistons, update
  ordering, comparator timing, and all other known quirks.
- **Update order determinism**: Block neighbor update order must be identical to vanilla
  (−X, +X, −Z, +Z, −Y, +Y). Non-deterministic ordering would break directional circuits.
- **Scheduled tick fidelity**: The `ScheduledTick` system must use the same priority
  ordering as vanilla (tick_time, then priority, then insertion order) to ensure correct
  timing.
- **Piston mechanics**: Piston extension, retraction, block pushing (up to 12 blocks),
  slime/honey block connectivity, and movable block entities must match vanilla behavior.
- **Performance**: Large redstone contraptions can trigger thousands of block updates per
  tick. The update propagation must be efficient enough to handle this without causing
  tick overruns.
- **Signal strength propagation**: Redstone signal strength (0-15) must propagate correctly
  through dust, repeaters, and comparators, following vanilla's BFS algorithm.

## Considered Options

### Option 1: Vanilla-Compatible Update Propagation

Replicate vanilla's `NeighborUpdater`, `ScheduledTick`, and per-block `neighborChanged()`
/ `onPlace()` / `onRemove()` methods exactly. Every block type implements its own update
logic, matching vanilla's Java source.

**Pros:**
- Exact behavioral match — same code, same results, same quirks.
- Community can verify correctness against known redstone test circuits.
- All documentation, tutorials, and contraption designs work unchanged.

**Cons:**
- Must replicate bugs intentionally, which is counterintuitive and requires careful
  documentation of which behaviors are "bugs we must keep."
- Some vanilla behaviors are genuinely confusing and poorly documented (e.g., the exact
  conditions for 0-tick piston behavior).
- No performance improvement over vanilla's propagation model.

**Verdict: Selected.** Correctness for redstone means "same as vanilla," period.

### Option 2: Clean-Room Redstone (Fix Bugs)

Implement redstone from the specification (redstone dust carries signal 0-15, repeaters
delay and boost, comparators compare/subtract) without replicating bugs. Quasi-connectivity
is removed. 0-tick pistons don't exist. Update order is canonical.

**Pros:**
- Cleaner, more predictable behavior.
- Easier to implement — no need to reverse-engineer bugs.
- Better for new players learning redstone.

**Cons:**
- **Breaks virtually every complex redstone contraption ever built.** BUD switches, flush
  piston doors, 0-tick farms, instant wire alternatives — all broken.
- The technical Minecraft community would reject Oxidized outright.
- Contraptions that work on Oxidized would not work on vanilla (and vice versa), creating
  an ecosystem split.

**Verdict: Rejected.** This would be a project-killing decision.

### Option 3: Configurable — Vanilla Mode vs. Fixed Mode

Offer both: a "vanilla" mode that replicates all quirks and a "fixed" mode with clean
behavior. Server operators choose via config.

**Pros:**
- Appeals to both purists and those who want cleaner redstone.
- Future-proof if Mojang ever fixes these behaviors.

**Cons:**
- Doubles the testing surface — every redstone test must pass in both modes.
- Creates a community split (contraptions marked "vanilla mode only" vs "fixed mode only").
- Implementation and maintenance burden is nearly 2x.
- Confusing for players who don't understand the distinction.

**Verdict: Rejected.** The maintenance cost is not justified for a niche use case.

### Option 4: LLVM-JIT Compiled Circuits

Detect stable redstone circuits at rest, compile them to native code via LLVM JIT, and
execute the compiled circuit when an input changes. Decompile back to block updates for
output.

**Pros:**
- Theoretically massive speedup for large, stable circuits (e.g., a 16-bit CPU).
- Could reduce complex circuit evaluation from thousands of block updates to a single
  function call.

**Cons:**
- Enormous implementation complexity — circuit detection, compilation, decompilation.
- Only helps for circuits that are stable (no continuous redstone clocks, which are common).
- LLVM dependency is massive.
- Very difficult to verify that the compiled circuit matches vanilla's update-by-update
  behavior, especially for circuits that depend on update ordering.
- Timing-sensitive circuits (comparator clocks, observer chains) may behave differently
  when evaluated atomically vs. tick-by-tick.

**Verdict: Rejected.** Fascinating research project, terrible engineering decision.

## Decision

**We replicate vanilla's redstone simulation exactly, including all known quirks.** The
implementation consists of three core systems: (1) neighbor block updates via
`NeighborUpdater`, (2) scheduled ticks via `ScheduledTickQueue`, and (3) per-block
redstone logic methods. Every block type that participates in redstone (dust, torch,
repeater, comparator, piston, observer, lever, button, pressure plate, tripwire, target,
sculk sensor, etc.) implements its redstone behavior as methods called during block
updates.

### Block Neighbor Update System

When a block changes state (placed, broken, or state update), it notifies its neighbors:

```rust
struct NeighborUpdater {
    /// Maximum updates per chain to prevent infinite loops.
    max_chain_depth: u32, // vanilla default: 1,000,000
    /// Current chain depth counter.
    current_depth: u32,
}

impl NeighborUpdater {
    /// Notify all 6 neighbors that the block at `pos` changed.
    fn update_neighbors_at(
        &mut self,
        level: &mut Level,
        pos: BlockPos,
        source_block: BlockState,
    ) {
        // Fixed neighbor order: -X, +X, -Z, +Z, -Y, +Y
        const NEIGHBOR_OFFSETS: [(i32, i32, i32); 6] = [
            (-1, 0, 0), (1, 0, 0),   // west, east
            (0, 0, -1), (0, 0, 1),   // north, south
            (0, -1, 0), (0, 1, 0),   // down, up
        ];

        for (dx, dy, dz) in NEIGHBOR_OFFSETS {
            let neighbor_pos = pos.offset(dx, dy, dz);
            let neighbor_state = level.get_block_state(neighbor_pos);
            self.current_depth += 1;

            if self.current_depth > self.max_chain_depth {
                warn!("Neighbor update chain exceeded limit at {:?}", pos);
                return;
            }

            // Call the neighbor's update handler
            neighbor_state.block().neighbor_changed(
                level, neighbor_pos, neighbor_state, source_block, pos,
            );
        }
    }
}
```

### Signal Strength Propagation

Redstone dust propagates signal strength using a BFS (breadth-first search) algorithm:

```rust
fn update_redstone_signal(
    level: &mut Level,
    source_pos: BlockPos,
    source_power: u8, // 0-15
) {
    let mut queue: VecDeque<(BlockPos, u8)> = VecDeque::new();
    let mut visited: HashSet<BlockPos> = HashSet::new();

    queue.push_back((source_pos, source_power));
    visited.insert(source_pos);

    while let Some((pos, power)) = queue.pop_front() {
        let current_state = level.get_block_state(pos);

        if current_state.is_redstone_dust() {
            let current_power = current_state.get_power();
            let new_power = calculate_target_strength(level, pos);

            if new_power != current_power {
                level.set_block_state(pos, current_state.with_power(new_power));

                // Propagate to connected dust
                for neighbor_pos in get_redstone_connections(level, pos) {
                    if !visited.contains(&neighbor_pos) {
                        visited.insert(neighbor_pos);
                        queue.push_back((neighbor_pos, new_power.saturating_sub(1)));
                    }
                }
            }
        }
    }
}

/// Calculate the signal strength a redstone dust should have based on its neighbors.
fn calculate_target_strength(level: &Level, pos: BlockPos) -> u8 {
    let mut max_signal: u8 = 0;

    // Check all neighbors for power sources
    for dir in Direction::ALL {
        let neighbor_pos = pos.offset(dir);
        let neighbor_state = level.get_block_state(neighbor_pos);

        // Direct power sources (redstone block, lever, etc.)
        if neighbor_state.is_signal_source() {
            max_signal = max_signal.max(neighbor_state.get_signal(level, neighbor_pos, dir.opposite()));
        }

        // Adjacent redstone dust (signal - 1)
        if neighbor_state.is_redstone_dust() {
            let dust_power = neighbor_state.get_power();
            max_signal = max_signal.max(dust_power.saturating_sub(1));
        }
    }

    max_signal
}
```

### Scheduled Tick System

Many redstone components use delayed updates via the `ScheduledTick` system:

```rust
#[derive(Clone, Debug)]
struct ScheduledTick {
    /// Block position.
    pos: BlockPos,
    /// Block type (for validation — if the block changed, the tick is stale).
    block: BlockState,
    /// Game tick when this scheduled tick should execute.
    trigger_tick: u64,
    /// Priority for ordering ticks at the same game tick.
    priority: TickPriority,
    /// Sub-tick insertion order (for deterministic ordering within same priority).
    sub_tick_order: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum TickPriority {
    ExtremelyHigh = -3,
    VeryHigh = -2,
    High = -1,
    Normal = 0,
    Low = 1,
    VeryLow = 2,
    ExtremelyLow = 3,
}

struct ScheduledTickQueue {
    ticks: BTreeSet<ScheduledTick>,
    next_sub_tick: u64,
}

impl ScheduledTickQueue {
    fn schedule(&mut self, pos: BlockPos, block: BlockState, delay: u32, priority: TickPriority, current_tick: u64) {
        let tick = ScheduledTick {
            pos,
            block,
            trigger_tick: current_tick + delay as u64,
            priority,
            sub_tick_order: self.next_sub_tick,
        };
        self.next_sub_tick += 1;
        self.ticks.insert(tick);
    }

    fn drain_ready(&mut self, current_tick: u64) -> Vec<ScheduledTick> {
        // Drain all ticks where trigger_tick <= current_tick,
        // ordered by (trigger_tick, priority, sub_tick_order)
        let mut ready = Vec::new();
        while let Some(tick) = self.ticks.iter().next() {
            if tick.trigger_tick > current_tick { break; }
            let tick = self.ticks.take(tick).unwrap();
            ready.push(tick);
        }
        ready
    }
}
```

Component-specific tick delays:
- **Repeater**: 1-4 ticks (based on slider position)
- **Comparator**: 1 tick (2 redstone ticks = 1 game tick for comparators)
- **Observer**: 2 ticks (1 tick to detect change, 1 tick pulse)
- **Piston**: 0 ticks to start extending (but 2 ticks to complete animation)
- **Redstone torch**: 1 tick to toggle

### Quasi-Connectivity (BUD Powering)

Pistons, dispensers, and droppers check for power in an unusual way:

```rust
fn is_receiving_power_bud(level: &Level, pos: BlockPos) -> bool {
    // Standard power check at the block's own position
    if level.has_neighbor_signal(pos) {
        return true;
    }

    // BUD: also check the position one block above
    // This is the quasi-connectivity "bug" that we must preserve
    if level.has_neighbor_signal(pos.above()) {
        return true;
    }

    false
}
```

This means a redstone signal adjacent to the block ABOVE a piston can power the piston
itself. Players use this extensively for BUD switches (Block Update Detectors) where
a piston activates when ANY neighbor of the block above it changes.

### 0-Tick Pistons

Under specific conditions, a piston can extend and retract within the same game tick:

1. Piston receives a 1-game-tick pulse (power on, then power off in the same tick via
   scheduled tick ordering).
2. The piston starts extending (scheduled tick for extension).
3. Before the extension animation completes, the piston receives the "power off" update.
4. The piston starts retracting immediately.
5. The block being pushed is teleported instantly (no animation).

We replicate this by ensuring scheduled tick processing order matches vanilla exactly.

### Piston Block Movement

When a piston extends, it pushes blocks:

```rust
fn resolve_push_list(
    level: &Level,
    piston_pos: BlockPos,
    direction: Direction,
) -> Result<PushList, PistonError> {
    let mut to_push: Vec<BlockPos> = Vec::new();
    let mut to_destroy: Vec<BlockPos> = Vec::new();

    // BFS from the block in front of the piston
    let mut check_pos = piston_pos.offset(direction);

    loop {
        let state = level.get_block_state(check_pos);

        if state.is_air() {
            break; // empty space — push succeeds
        }

        if !state.is_pushable() {
            // Obsidian, bedrock, etc. — push fails
            return Err(PistonError::ImmovableBlock);
        }

        if state.is_destroyed_by_piston() {
            // Glass, flowers, etc. — destroyed
            to_destroy.push(check_pos);
            break;
        }

        to_push.push(check_pos);

        // Check 12-block push limit
        if to_push.len() > 12 {
            return Err(PistonError::TooManyBlocks);
        }

        // Handle slime/honey block connectivity
        if state.is_sticky() {
            // Check all 5 non-push-direction faces for connected blocks
            for face_dir in Direction::ALL {
                if face_dir == direction.opposite() { continue; }
                let adjacent = check_pos.offset(face_dir);
                let adj_state = level.get_block_state(adjacent);
                if adj_state.is_pushable() && should_stick(state, adj_state) {
                    // Add to push list (recursive, with 12-block total limit)
                    to_push.push(adjacent);
                }
            }
        }

        check_pos = check_pos.offset(direction);
    }

    Ok(PushList { to_push, to_destroy })
}
```

### Redstone Dust Connectivity

Redstone dust visually and electrically connects to adjacent redstone components. The
connectivity state (north, south, east, west: none/side/up) is a block state property:

```rust
#[derive(Clone, Copy, Debug)]
enum WireConnection {
    None,  // no connection in this direction
    Side,  // connects to adjacent block at same level
    Up,    // connects to block one level up (dust on adjacent wall)
}

fn calculate_dust_connections(
    level: &Level,
    pos: BlockPos,
) -> [WireConnection; 4] {
    let mut connections = [WireConnection::None; 4];

    for (i, dir) in [Direction::North, Direction::South, Direction::East, Direction::West].iter().enumerate() {
        let neighbor = pos.offset(*dir);
        let neighbor_state = level.get_block_state(neighbor);

        // Connect to redstone components at same level
        if connects_to(neighbor_state, *dir) {
            connections[i] = WireConnection::Side;
            continue;
        }

        // Connect to dust on block above (if no solid block overhead)
        let above_neighbor = neighbor.above();
        if !level.get_block_state(pos.above()).is_solid()
            && level.get_block_state(above_neighbor).is_redstone_dust() {
            connections[i] = WireConnection::Up;
            continue;
        }

        // Connect to dust on block below (if neighbor is not solid)
        let below_neighbor = neighbor.below();
        if !neighbor_state.is_solid()
            && level.get_block_state(below_neighbor).is_redstone_dust() {
            connections[i] = WireConnection::Side;
        }
    }

    connections
}
```

### Observer Self-Clocking

Observers detect block state changes in the block they're facing. When they detect a
change, they emit a 1-tick pulse. Two observers facing each other create a clock — each
one's pulse triggers the other. This must be handled without infinite loops:

1. Observer A detects change → schedules pulse after 2 ticks.
2. Observer A emits pulse → changes its own state (powered).
3. Observer B detects A's state change → schedules pulse after 2 ticks.
4. Observer B emits pulse → changes its own state.
5. Observer A detects B's state change → cycle continues.

The scheduled tick system naturally handles this: each observer schedules its pulse, and
the scheduled tick queue processes them in order, interleaved with other game ticks.

## Consequences

### Positive

- **Full contraption compatibility**: Every redstone circuit, farm, and mechanism that
  works on vanilla works identically on Oxidized. This is a hard requirement for
  community adoption.
- **Deterministic behavior**: Fixed update ordering and scheduled tick ordering ensure
  circuits produce the same result every time, matching vanilla.
- **Community trust**: Explicitly preserving quirks like quasi-connectivity and 0-tick
  pistons signals that Oxidized takes technical Minecraft seriously.

### Negative

- **Must replicate bugs**: Quasi-connectivity, 0-tick behavior, and update order
  dependence are all technically bugs. Intentionally replicating them requires careful
  documentation of WHY each behavior exists and WHERE it's implemented, so future
  developers don't "fix" them.
- **Performance matches vanilla**: We don't improve redstone performance because we use
  the same propagation model. Large redstone machines (thousands of components) can still
  cause lag spikes.
- **Testing complexity**: Verifying redstone correctness requires testing specific timing,
  ordering, and quirk interactions. Standard unit tests are insufficient — we need
  integration tests with known circuits.

### Neutral

- **Scheduled tick persistence**: Scheduled ticks must be saved to disk (in chunk data)
  and restored on load. If a tick was scheduled for game tick 1000 and the server restarts
  at tick 999, the tick must still fire at tick 1000 after restart.
- **Redstone dust is the most complex single block**: Dust has 1,296 possible block states
  (4 directions × 3 connection types each + 16 signal levels). State computation and
  rendering are the most complex of any block.

## Compliance

- **Quasi-connectivity test**: Build a BUD switch (piston powered by block above) and
  verify it activates on both vanilla and Oxidized.
- **0-tick piston test**: Build a 0-tick piston circuit and verify instant block transport
  works identically.
- **Update order test**: Build a circuit where output depends on neighbor update order and
  verify identical results.
- **Comparator timing test**: Build a comparator clock and verify timing matches vanilla
  within ±0 ticks (exact match required).
- **12-block push limit test**: Verify pistons fail to push exactly when the 13th block is
  encountered.
- **Signal strength test**: Place redstone dust in a line and verify signal decreases by
  1 per block from the source, reaching 0 at 15 blocks.
- **Observer clock test**: Build two facing observers and verify the clock frequency
  matches vanilla exactly.
- **Known circuit regression suite**: Maintain a collection of 50+ known redstone circuits
  (from community designs) that must produce identical behavior on every Oxidized build.

## Related ADRs

- **ADR-019**: Tick Loop Design — scheduled ticks are processed in the BLOCK_TICK phase
- **ADR-021**: Physics & Collision Engine — piston block movement changes collision shapes

## References

- Vanilla source: `net.minecraft.world.level.redstone.NeighborUpdater`
- Vanilla source: `net.minecraft.world.level.block.RedStoneWireBlock`
- Vanilla source: `net.minecraft.world.level.block.piston.PistonBaseBlock`
- Vanilla source: `net.minecraft.world.level.block.RedstoneTorchBlock`
- Vanilla source: `net.minecraft.world.level.block.DiodeBlock` (repeater/comparator)
- Vanilla source: `net.minecraft.world.level.block.ObserverBlock`
- Vanilla source: `net.minecraft.world.ticks.LevelTicks` (scheduled ticks)
- [Minecraft Wiki — Redstone Dust](https://minecraft.wiki/w/Redstone_Dust)
- [Minecraft Wiki — Quasi-Connectivity](https://minecraft.wiki/w/Tutorials/Quasi-connectivity)
- [Minecraft Wiki — Piston — Technical](https://minecraft.wiki/w/Piston#Technical_information)
- [Minecraft Technical Community — SciCraft](https://scicraft.net/) — redstone behavior documentation
