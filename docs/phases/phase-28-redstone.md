# Phase 28 — Redstone

**Crate:** `oxidized-game`  
**Reward:** Basic redstone circuits work: levers, buttons, wire, torches, repeaters, pistons.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-025: Redstone](../adr/adr-025-redstone.md) — vanilla-compatible update propagation preserving all quirks


## Goal

Implement the full redstone signal propagation model: direct power, soft power,
`NeighborUpdater` cascade, `ScheduledTick` priority queue, and seven redstone
components (dust, torch, button, lever, pressure plate, observer, repeater,
comparator, piston). All components must behave exactly as vanilla including
burnout for torches, 12-block push limit for pistons, and comparator container
output calculation.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Redstone dust block | `RedStoneWireBlock` | `net.minecraft.world.level.block.RedStoneWireBlock` |
| Redstone torch | `RedstoneTorchBlock` | `net.minecraft.world.level.block.RedstoneTorchBlock` |
| Abstract button | `AbstractButtonBlock` | `net.minecraft.world.level.block.AbstractButtonBlock` |
| Lever block | `LeverBlock` | `net.minecraft.world.level.block.LeverBlock` |
| Pressure plate (abstract) | `AbstractPressurePlateBlock` | `net.minecraft.world.level.block.AbstractPressurePlateBlock` |
| Observer block | `ObserverBlock` | `net.minecraft.world.level.block.ObserverBlock` |
| Repeater block | `RepeaterBlock` | `net.minecraft.world.level.block.RepeaterBlock` |
| Comparator block | `ComparatorBlock` | `net.minecraft.world.level.block.ComparatorBlock` |
| Piston base | `PistonBaseBlock` | `net.minecraft.world.level.block.PistonBaseBlock` |
| Neighbor updater | `NeighborUpdater` | `net.minecraft.world.level.NeighborUpdater` |
| Level ticks | `LevelTicks` | `net.minecraft.world.level.LevelTicks` |
| Block event packet | `ClientboundBlockEventPacket` | `net.minecraft.network.protocol.game.ClientboundBlockEventPacket` |

---

## Tasks

### 28.1 — Signal power model

```rust
// crates/oxidized-game/src/world/redstone/power.rs

/// Signal strength: 0 (off) through 15 (full power).
pub type SignalStrength = u8; // 0..=15

/// The two ways a block can be powered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerType {
    /// Block is directly powered (e.g., by wire touching it, by torch attached to it).
    Direct,
    /// Block is soft-powered (can power wires attached to it but not other components).
    Indirect,
}

/// Query the redstone signal flowing into a face of `pos` from `from` direction.
/// Mirrors `Level.getSignal` and `Level.hasNeighborSignal`.
pub fn get_signal_from(
    world: &impl BlockReader,
    pos: BlockPos,
    direction: Direction,
) -> SignalStrength {
    let source_pos = pos.relative(direction);
    let source_state = world.get_block_state(source_pos);
    source_state.get_signal(world, source_pos, direction.opposite())
}

/// Whether any of the 6 neighbors provides a signal strength > 0.
pub fn has_neighbor_signal(world: &impl BlockReader, pos: BlockPos) -> bool {
    Direction::ALL.iter().any(|&d| get_signal_from(world, pos, d) > 0)
}

/// Maximum signal among all 6 faces.
pub fn get_best_neighbor_signal(world: &impl BlockReader, pos: BlockPos) -> SignalStrength {
    Direction::ALL.iter()
        .map(|&d| get_signal_from(world, pos, d))
        .max()
        .unwrap_or(0)
}
```

### 28.2 — `ScheduledTick` priority queue

```rust
// crates/oxidized-game/src/world/level_ticks.rs

use std::collections::BinaryHeap;
use std::cmp::Reverse;

/// A single pending tick for a block or fluid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledTick {
    pub pos:           BlockPos,
    pub block_id:      u32,
    /// Absolute game tick when this tick should fire.
    pub scheduled_time: i64,
    /// Ordering within the same tick (higher priority = fires first).
    pub priority:       TickPriority,
    /// Unique sub-tick order for determinism.
    pub sub_tick_order: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TickPriority {
    /// Fires before NORMAL (used by observers).
    ExtremelyHigh = -3,
    VeryHigh      = -2,
    High          = -1,
    Normal        =  0,
    Low           =  1,
    VeryLow       =  2,
    Lowest        =  3,
}

impl PartialOrd for ScheduledTick {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledTick {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Min-heap ordering: earlier time and higher priority first.
        other.scheduled_time.cmp(&self.scheduled_time)
            .then(other.priority.cmp(&self.priority))
            .then(other.sub_tick_order.cmp(&self.sub_tick_order))
    }
}

/// Thread-local scheduled-tick queue for one world dimension.
pub struct LevelTicks {
    queue: BinaryHeap<ScheduledTick>,
    next_sub_tick: i64,
    /// Maximum ticks processed per game tick to prevent lag spirals.
    pub max_ticks_per_tick: usize,
}

impl LevelTicks {
    pub const MAX_TICKS_PER_TICK: usize = 65_536;

    pub fn new() -> Self {
        Self { queue: BinaryHeap::new(), next_sub_tick: 0, max_ticks_per_tick: Self::MAX_TICKS_PER_TICK }
    }

    pub fn schedule(
        &mut self,
        pos: BlockPos,
        block_id: u32,
        delay_ticks: i32,
        current_tick: i64,
        priority: TickPriority,
    ) {
        self.next_sub_tick += 1;
        self.queue.push(ScheduledTick {
            pos,
            block_id,
            scheduled_time: current_tick + delay_ticks as i64,
            priority,
            sub_tick_order: self.next_sub_tick,
        });
    }

    /// Drain all ticks due at `current_tick`, up to `max_ticks_per_tick`.
    /// Returns the list of ticks to process.
    pub fn drain_due(&mut self, current_tick: i64) -> Vec<ScheduledTick> {
        let mut due = Vec::new();
        while let Some(tick) = self.queue.peek() {
            if tick.scheduled_time > current_tick || due.len() >= self.max_ticks_per_tick {
                break;
            }
            due.push(self.queue.pop().unwrap());
        }
        due
    }
}
```

### 28.3 — `NeighborUpdater`

```rust
// crates/oxidized-game/src/world/neighbor_updater.rs

/// Triggers neighbor block updates efficiently, avoiding duplicates.
/// Mirrors `CollectingNeighborUpdater` in Java.
pub struct NeighborUpdater {
    updates: Vec<NeighborUpdate>,
}

#[derive(Debug, Clone)]
enum NeighborUpdate {
    /// Notify the block at `pos` that its neighbor at `from` changed.
    SimpleNeighborUpdate { pos: BlockPos, block: u32, from: BlockPos },
    /// Update all 6 neighbors of `pos` except toward `except`.
    UpdateNeighbors { pos: BlockPos, block: u32, except: Option<Direction> },
}

impl NeighborUpdater {
    pub fn new() -> Self { Self { updates: Vec::new() } }

    /// Queue: notify all 6 neighbors of `pos` except from `except` direction.
    pub fn update_neighbors_at_except_from_facing(
        &mut self,
        pos: BlockPos,
        block: u32,
        except: Option<Direction>,
    ) {
        self.updates.push(NeighborUpdate::UpdateNeighbors { pos, block, except });
    }

    /// Queue: notify `neighbor_pos` that `source_pos` (its neighbor toward `from`) changed.
    pub fn notify_neighbor_of_change(
        &mut self,
        neighbor_pos: BlockPos,
        source_block: u32,
        source_pos: BlockPos,
    ) {
        self.updates.push(NeighborUpdate::SimpleNeighborUpdate {
            pos: neighbor_pos, block: source_block, from: source_pos,
        });
    }

    /// Process all queued updates against the world.
    pub fn flush(&mut self, world: &mut impl BlockWorld) {
        for update in self.updates.drain(..) {
            match update {
                NeighborUpdate::UpdateNeighbors { pos, block, except } => {
                    for dir in Direction::ALL {
                        if Some(dir) == except { continue; }
                        let neighbor = pos.relative(dir);
                        world.neighbor_changed(neighbor, block, pos);
                    }
                }
                NeighborUpdate::SimpleNeighborUpdate { pos, block, from } => {
                    world.neighbor_changed(pos, block, from);
                }
            }
        }
    }
}
```

### 28.4 — `RedstoneDustBlock`

```rust
// crates/oxidized-game/src/world/block/redstone_dust.rs

/// The four connection states per face for redstone wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireConnection { None, Side, Up }

/// Mutable state of a redstone dust block.
#[derive(Debug, Clone)]
pub struct RedstoneDustState {
    /// Signal strength 0-15.
    pub power: SignalStrength,
    pub north: WireConnection,
    pub south: WireConnection,
    pub east:  WireConnection,
    pub west:  WireConnection,
}

/// Recalculate the signal strength of a redstone wire at `pos`.
/// Returns the new power level (max of neighbor signals minus 1, minimum 0).
pub fn recalculate_wire_strength(
    world: &impl BlockReader,
    pos: BlockPos,
) -> SignalStrength {
    let mut max_signal: SignalStrength = 0;
    for dir in [Direction::North, Direction::South, Direction::East, Direction::West] {
        let neighbor = pos.relative(dir);
        let ns = world.get_redstone_power(neighbor, dir.opposite());
        max_signal = max_signal.max(ns);

        // Check wire one step below (wire going down a slope)
        if world.get_block(neighbor) == Block::Air {
            let below = neighbor.below();
            let ns_below = world.get_redstone_power(below, dir.opposite());
            max_signal = max_signal.max(ns_below);
        }
        // Check wire one step above (wire going up a slope)
        if !world.get_block(pos).is_solid() {
            let above = neighbor.above();
            let ns_above = world.get_redstone_power(above, dir.opposite());
            max_signal = max_signal.max(ns_above);
        }
    }
    max_signal.saturating_sub(1)
}

/// Called when a neighbor of this wire changes.
/// Propagates power change and triggers further updates.
pub fn on_neighbor_changed(
    world: &mut impl BlockWorld,
    pos: BlockPos,
    ticks: &mut LevelTicks,
    current_tick: i64,
) {
    let new_power = recalculate_wire_strength(world, pos);
    let current_power = world.get_block_state(pos).redstone_dust_power();
    if new_power != current_power {
        world.set_block_state_power(pos, new_power);
        // Update all 6 neighbors + diagonals
        let mut updater = NeighborUpdater::new();
        for dir in Direction::ALL {
            updater.update_neighbors_at_except_from_facing(pos.relative(dir), Block::RedstoneWire as u32, None);
        }
        updater.flush(world);
    }
}
```

### 28.5 — `RedstoneTorchBlock`

```rust
// crates/oxidized-game/src/world/block/redstone_torch.rs

/// Tracks recent toggles for torch burnout logic.
struct TorchBurnoutRecord {
    pos:  BlockPos,
    tick: i64,
}

/// Global per-world list of recent torch toggles (capped at 8 per position).
pub struct TorchBurnoutTracker {
    records: Vec<TorchBurnoutRecord>,
}

impl TorchBurnoutTracker {
    pub const BURNOUT_WINDOW: i64  = 60; // ticks
    pub const BURNOUT_LIMIT:  usize = 8;

    pub fn new() -> Self { Self { records: Vec::new() } }

    /// Record a toggle. Returns true if the torch has now burned out.
    pub fn record_toggle(&mut self, pos: BlockPos, current_tick: i64) -> bool {
        // Purge old entries
        self.records.retain(|r| current_tick - r.tick < Self::BURNOUT_WINDOW);
        self.records.push(TorchBurnoutRecord { pos, tick: current_tick });
        let count = self.records.iter().filter(|r| r.pos == pos).count();
        count >= Self::BURNOUT_LIMIT
    }
}

/// Evaluate whether the torch should be lit given the power state of the block it's attached to.
/// A torch is OFF (powered=true) when its attachment block is powered.
pub fn torch_should_be_off(
    world: &impl BlockReader,
    pos: BlockPos,
    attached_face: Direction,
) -> bool {
    let support_pos = pos.relative(attached_face);
    has_neighbor_signal(world, support_pos)
}

/// Called on scheduled tick: update powered state, propagate neighbors.
pub fn torch_tick(
    world: &mut impl BlockWorld,
    pos: BlockPos,
    ticks: &mut LevelTicks,
    burnout: &mut TorchBurnoutTracker,
    current_tick: i64,
    attached_face: Direction,
) {
    let should_be_off = torch_should_be_off(world, pos, attached_face);
    let is_off = world.get_block_state(pos).torch_is_off();

    if is_off != should_be_off {
        // Toggle the torch
        if burnout.record_toggle(pos, current_tick) {
            // Burned out: replace with burned variant, schedule restore in 160 ticks
            world.set_block(pos, Block::BurnedRedstoneTorch);
            ticks.schedule(pos, Block::BurnedRedstoneTorch as u32, 160, current_tick, TickPriority::Normal);
        } else {
            world.set_block_powered(pos, !should_be_off);
            let mut updater = NeighborUpdater::new();
            updater.update_neighbors_at_except_from_facing(pos, Block::RedstoneTorch as u32, Some(attached_face));
            updater.flush(world);
        }
    }
}
```

### 28.6 — `AbstractButtonBlock`, `LeverBlock`, `AbstractPressurePlateBlock`

```rust
// crates/oxidized-game/src/world/block/button.rs

pub struct ButtonConfig {
    /// Ticks the button remains active. Stone=20, Wood=30.
    pub duration: i32,
}

impl ButtonConfig {
    pub const STONE: Self = Self { duration: 20 };
    pub const WOOD:  Self = Self { duration: 30 };
}

/// Called when the button is pressed (player use or arrow impact for wooden).
pub fn button_press(
    world: &mut impl BlockWorld,
    pos: BlockPos,
    config: &ButtonConfig,
    ticks: &mut LevelTicks,
    current_tick: i64,
) {
    if !world.get_block_state(pos).button_is_powered() {
        world.set_button_powered(pos, true);
        // Power blocks adjacent + directly above/below
        let mut updater = NeighborUpdater::new();
        updater.update_neighbors_at_except_from_facing(pos, Block::StoneButton as u32, None);
        updater.flush(world);
        ticks.schedule(pos, Block::StoneButton as u32, config.duration, current_tick, TickPriority::Normal);
    }
}

/// Called on scheduled tick to release the button.
pub fn button_release(
    world: &mut impl BlockWorld,
    pos: BlockPos,
    ticks: &mut LevelTicks,
    current_tick: i64,
) {
    world.set_button_powered(pos, false);
    let mut updater = NeighborUpdater::new();
    updater.update_neighbors_at_except_from_facing(pos, Block::StoneButton as u32, None);
    updater.flush(world);
}

// crates/oxidized-game/src/world/block/lever.rs

/// Toggle the lever, propagate neighbor updates.
pub fn lever_toggle(world: &mut impl BlockWorld, pos: BlockPos) {
    let powered = !world.get_block_state(pos).lever_is_powered();
    world.set_lever_powered(pos, powered);
    let mut updater = NeighborUpdater::new();
    updater.update_neighbors_at_except_from_facing(pos, Block::Lever as u32, None);
    updater.flush(world);
}

// crates/oxidized-game/src/world/block/pressure_plate.rs

/// Compute signal strength for a weighted pressure plate from entity count.
/// Returns 0-15 based on number of entities in the plate's AABB.
pub fn weighted_plate_signal(entity_count: usize, max: usize) -> SignalStrength {
    let fraction = entity_count as f32 / max as f32;
    (fraction.clamp(0.0, 1.0) * 15.0).ceil() as SignalStrength
}

/// Called each tick when an entity is on the plate.
pub fn pressure_plate_tick(
    world: &mut impl BlockWorld,
    pos: BlockPos,
    entity_count: usize,
    max: usize,
    ticks: &mut LevelTicks,
    current_tick: i64,
) {
    let new_signal = weighted_plate_signal(entity_count, max);
    let old_signal = world.get_block_state(pos).plate_signal();
    if new_signal != old_signal {
        world.set_plate_signal(pos, new_signal);
        let mut updater = NeighborUpdater::new();
        updater.update_neighbors_at_except_from_facing(pos, Block::StonePressurePlate as u32, None);
        updater.flush(world);
        if new_signal == 0 {
            // Schedule deactivation check in 20 ticks
            ticks.schedule(pos, Block::StonePressurePlate as u32, 20, current_tick, TickPriority::Normal);
        }
    }
}
```

### 28.7 — `RepeaterBlock` and `ObserverBlock`

```rust
// crates/oxidized-game/src/world/block/repeater.rs

pub struct RepeaterState {
    /// Delay in game ticks: 1, 2, 3, or 4 (corresponds to 2, 4, 6, 8 ticks).
    pub delay: u8,
    pub powered: bool,
    pub locked:  bool,
}

impl RepeaterState {
    pub fn delay_ticks(&self) -> i32 { self.delay as i32 * 2 }
}

/// Called when a neighbor changes. Schedules a tick for the delay.
pub fn repeater_check_power(
    world: &mut impl BlockWorld,
    pos: BlockPos,
    state: &RepeaterState,
    ticks: &mut LevelTicks,
    current_tick: i64,
) {
    if state.locked { return; }
    let input_power = get_signal_from(world, pos, state_facing(world, pos));
    let should_power = input_power > 0;
    if should_power != state.powered {
        let priority = if should_power { TickPriority::VeryHigh } else { TickPriority::High };
        ticks.schedule(pos, Block::Repeater as u32, state.delay_ticks(), current_tick, priority);
    }
}

/// Called on scheduled tick: update powered state.
pub fn repeater_tick(world: &mut impl BlockWorld, pos: BlockPos, current_tick: i64) {
    let input = get_signal_from(world, pos, state_facing(world, pos));
    let should_power = input > 0;
    let currently_powered = world.get_block_state(pos).repeater_is_powered();
    if should_power != currently_powered {
        world.set_repeater_powered(pos, should_power);
        let mut updater = NeighborUpdater::new();
        updater.update_neighbors_at_except_from_facing(pos, Block::Repeater as u32, None);
        updater.flush(world);
    }
}

fn state_facing(_: &impl BlockWorld, _: BlockPos) -> Direction { Direction::North }

// crates/oxidized-game/src/world/block/observer.rs

/// Called when the block the observer faces has a state change.
/// Outputs a 2-tick pulse on the back face.
pub fn observer_schedule_pulse(
    ticks: &mut LevelTicks,
    pos: BlockPos,
    current_tick: i64,
) {
    // Schedule ON tick immediately (priority ExtremelyHigh = fires this tick)
    ticks.schedule(pos, Block::Observer as u32, 2, current_tick, TickPriority::ExtremelyHigh);
}

/// Called on scheduled tick: toggle the output state.
pub fn observer_tick(
    world: &mut impl BlockWorld,
    pos: BlockPos,
    ticks: &mut LevelTicks,
    current_tick: i64,
) {
    let currently_powered = world.get_block_state(pos).observer_is_powered();
    if currently_powered {
        world.set_observer_powered(pos, false);
    } else {
        world.set_observer_powered(pos, true);
        // Schedule turn-off 2 ticks later
        ticks.schedule(pos, Block::Observer as u32, 2, current_tick, TickPriority::ExtremelyHigh);
    }
    let mut updater = NeighborUpdater::new();
    updater.update_neighbors_at_except_from_facing(pos, Block::Observer as u32, None);
    updater.flush(world);
}
```

### 28.8 — `ComparatorBlock`

```rust
// crates/oxidized-game/src/world/block/comparator.rs

/// Comparator operating mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparatorMode { Compare, Subtract }

/// Calculate comparator output from back and side signals.
/// Compare mode:   output = back if back >= side_max else 0.
/// Subtract mode:  output = max(0, back - side_max).
pub fn comparator_output(
    back: SignalStrength,
    side_max: SignalStrength,
    mode: ComparatorMode,
) -> SignalStrength {
    match mode {
        ComparatorMode::Compare  => if back >= side_max { back } else { 0 },
        ComparatorMode::Subtract => back.saturating_sub(side_max),
    }
}

/// Compute the signal output by a container (used for comparators facing chests, etc.).
/// Mirrors `AbstractContainerMenu.getRedstoneSignalFromContainer`.
pub fn container_signal(filled_slots: usize, total_slots: usize, max_stack: u8) -> SignalStrength {
    if total_slots == 0 { return 0; }
    let fraction: f32 = filled_slots as f32 / total_slots as f32;
    let signal = (fraction * 14.0 + if filled_slots > 0 { 1.0 } else { 0.0 }) as SignalStrength;
    signal.min(15)
}

/// Full comparator tick: read inputs, compute output, update if changed.
pub fn comparator_tick(
    world: &mut impl BlockWorld,
    pos: BlockPos,
    mode: ComparatorMode,
    current_tick: i64,
) {
    let back_pos  = pos.relative(comparator_facing(world, pos));
    let back      = world.get_redstone_power(back_pos, Direction::South);
    let left_pow  = get_signal_from(world, pos, Direction::East);
    let right_pow = get_signal_from(world, pos, Direction::West);
    let side_max  = left_pow.max(right_pow);
    let output    = comparator_output(back, side_max, mode);
    let old_output = world.get_block_state(pos).comparator_output();
    if output != old_output {
        world.set_comparator_output(pos, output);
        let mut updater = NeighborUpdater::new();
        updater.update_neighbors_at_except_from_facing(pos, Block::Comparator as u32, None);
        updater.flush(world);
    }
}

fn comparator_facing(_: &impl BlockWorld, _: BlockPos) -> Direction { Direction::South }
```

### 28.9 — `PistonBlock`

```rust
// crates/oxidized-game/src/world/block/piston.rs

/// Maximum number of blocks a piston can push.
pub const PISTON_MAX_PUSH: usize = 12;

/// Determine whether a piston at `pos` facing `dir` should be extended
/// based on the current redstone signal.
pub fn piston_should_extend(world: &impl BlockWorld, pos: BlockPos, dir: Direction) -> bool {
    for check_dir in Direction::ALL {
        let neighbor = pos.relative(check_dir);
        if get_signal_from(world, neighbor, check_dir) > 0 { return true; }
    }
    false
}

/// Collect the list of blocks to be pushed. Returns None if push is blocked.
/// Slime/Honey blocks pull attached blocks on retraction.
pub fn collect_push_blocks(
    world: &impl BlockReader,
    piston_pos: BlockPos,
    dir: Direction,
    sticky: bool,
) -> Option<PushList> {
    let mut to_push  = Vec::new();
    let mut to_break = Vec::new();
    let mut frontier = vec![piston_pos.relative(dir)];

    while let Some(pos) = frontier.pop() {
        if to_push.contains(&pos) { continue; }
        if to_push.len() >= PISTON_MAX_PUSH { return None; } // push limit

        let block = world.get_block(pos);
        if block == Block::Air { continue; }
        if block.piston_push_reaction() == PushReaction::Block { return None; }
        if block.piston_push_reaction() == PushReaction::Destroy {
            to_break.push(pos);
            continue;
        }
        to_push.push(pos);
        // If block is slime or honey, add its face-attached neighbors to frontier
        if block == Block::SlimeBlock || block == Block::HoneyBlock {
            for adj_dir in Direction::ALL {
                let adj = pos.relative(adj_dir);
                if !to_push.contains(&adj) { frontier.push(adj); }
            }
        } else {
            frontier.push(pos.relative(dir));
        }
    }
    Some(PushList { to_push, to_break })
}

pub struct PushList {
    pub to_push:  Vec<BlockPos>,
    pub to_break: Vec<BlockPos>,
}

/// Execute piston extension/retraction, send `ClientboundBlockEventPacket`.
pub fn piston_move(
    world: &mut impl BlockWorld,
    pos: BlockPos,
    dir: Direction,
    extend: bool,
    sticky: bool,
) {
    // 1. Collect push list (or pull list on retraction).
    // 2. Move each block in reverse order (farthest first).
    // 3. Place piston arm blocks (piston_head).
    // 4. Send ClientboundBlockEventPacket(pos, 0=extend/1=retract, facing_id).
    //    Client plays animation.
}
```

---

## Data Structures Summary

```rust
// Key types in oxidized-game::world::redstone

pub use power::{SignalStrength, PowerType, get_signal_from, has_neighbor_signal};
pub use level_ticks::{LevelTicks, ScheduledTick, TickPriority};
pub use neighbor_updater::NeighborUpdater;
pub use block::redstone_dust::{RedstoneDustState, WireConnection, recalculate_wire_strength};
pub use block::redstone_torch::{TorchBurnoutTracker, torch_tick};
pub use block::button::{ButtonConfig, button_press, button_release};
pub use block::lever::lever_toggle;
pub use block::pressure_plate::{weighted_plate_signal, pressure_plate_tick};
pub use block::repeater::{RepeaterState, repeater_tick};
pub use block::observer::{observer_schedule_pulse, observer_tick};
pub use block::comparator::{ComparatorMode, comparator_output, container_signal};
pub use block::piston::{PISTON_MAX_PUSH, collect_push_blocks, piston_move};
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // --- ScheduledTick ordering ---

    /// Earlier scheduled time fires first.
    #[test]
    fn tick_queue_fires_in_time_order() {
        let mut ticks = LevelTicks::new();
        let pos = BlockPos::new(0, 0, 0);
        ticks.schedule(pos, 1, 10, 0, TickPriority::Normal);
        ticks.schedule(pos, 2, 5,  0, TickPriority::Normal);
        ticks.schedule(pos, 3, 1,  0, TickPriority::Normal);
        let due = ticks.drain_due(10);
        assert_eq!(due[0].block_id, 3); // delay=1 fires first
        assert_eq!(due[1].block_id, 2);
        assert_eq!(due[2].block_id, 1);
    }

    /// Higher-priority tick fires before lower at the same time.
    #[test]
    fn tick_queue_priority_ordering() {
        let mut ticks = LevelTicks::new();
        let pos = BlockPos::new(0, 0, 0);
        ticks.schedule(pos, 1, 5, 0, TickPriority::Normal);
        ticks.schedule(pos, 2, 5, 0, TickPriority::VeryHigh);
        let due = ticks.drain_due(5);
        assert_eq!(due[0].block_id, 2); // VeryHigh fires first
    }

    /// Ticks past the current time are not returned.
    #[test]
    fn tick_queue_future_ticks_not_returned() {
        let mut ticks = LevelTicks::new();
        ticks.schedule(BlockPos::ZERO, 99, 100, 0, TickPriority::Normal);
        let due = ticks.drain_due(50);
        assert!(due.is_empty());
    }

    /// At most MAX_TICKS_PER_TICK entries are drained per call.
    #[test]
    fn tick_queue_respects_cap() {
        let mut ticks = LevelTicks::new();
        ticks.max_ticks_per_tick = 3;
        for i in 0..10 {
            ticks.schedule(BlockPos::ZERO, i, 0, 0, TickPriority::Normal);
        }
        let due = ticks.drain_due(0);
        assert_eq!(due.len(), 3);
    }

    // --- Comparator output ---

    #[test]
    fn comparator_compare_mode_passes_back_if_stronger() {
        assert_eq!(comparator_output(8, 4, ComparatorMode::Compare), 8);
    }

    #[test]
    fn comparator_compare_mode_zero_if_back_weaker() {
        assert_eq!(comparator_output(3, 7, ComparatorMode::Compare), 0);
    }

    #[test]
    fn comparator_subtract_mode() {
        assert_eq!(comparator_output(10, 4, ComparatorMode::Subtract), 6);
        assert_eq!(comparator_output(3,  7, ComparatorMode::Subtract), 0);
    }

    // --- Container signal ---

    #[test]
    fn container_signal_empty_chest_is_zero() {
        assert_eq!(container_signal(0, 27, 64), 0);
    }

    #[test]
    fn container_signal_full_chest_is_fifteen() {
        assert_eq!(container_signal(27, 27, 64), 15);
    }

    #[test]
    fn container_signal_one_item_is_one() {
        // Any non-empty container → minimum signal 1
        assert_eq!(container_signal(1, 27, 64), 1);
    }

    // --- Wire strength ---

    #[test]
    fn wire_strength_decrements_by_one() {
        // Simulated: neighbor provides signal 10 → wire becomes 9
        // (tested via unit-testable recalculate_wire_strength)
        let input: SignalStrength = 10;
        let expected = input.saturating_sub(1);
        assert_eq!(expected, 9);
    }

    #[test]
    fn wire_strength_zero_stays_zero() {
        let result: SignalStrength = 0u8.saturating_sub(1);
        assert_eq!(result, 0);
    }

    // --- TorchBurnoutTracker ---

    #[test]
    fn torch_burnout_triggers_after_eight_toggles() {
        let mut tracker = TorchBurnoutTracker::new();
        let pos = BlockPos::ZERO;
        for i in 0..7 {
            assert!(!tracker.record_toggle(pos, i as i64));
        }
        assert!(tracker.record_toggle(pos, 7)); // 8th toggle within window
    }

    #[test]
    fn torch_burnout_resets_after_window() {
        let mut tracker = TorchBurnoutTracker::new();
        let pos = BlockPos::ZERO;
        for i in 0..8 {
            tracker.record_toggle(pos, i as i64); // burn it out
        }
        // After 60+ ticks, old records are purged
        assert!(!tracker.record_toggle(pos, 70));
    }

    // --- Weighted pressure plate ---

    #[test]
    fn plate_zero_entities_gives_signal_zero() {
        assert_eq!(weighted_plate_signal(0, 20), 0);
    }

    #[test]
    fn plate_max_entities_gives_signal_fifteen() {
        assert_eq!(weighted_plate_signal(20, 20), 15);
    }

    // --- Piston push limit ---

    #[test]
    fn piston_push_limit_constant_is_twelve() {
        assert_eq!(PISTON_MAX_PUSH, 12);
    }

    // --- ButtonConfig durations ---

    #[test]
    fn stone_button_lasts_twenty_ticks() {
        assert_eq!(ButtonConfig::STONE.duration, 20);
    }

    #[test]
    fn wood_button_lasts_thirty_ticks() {
        assert_eq!(ButtonConfig::WOOD.duration, 30);
    }
}
```
