//! Scheduled tick system — priority-ordered deferred block/fluid ticks.
//!
//! Mirrors `net.minecraft.world.ticks.LevelTicks`. Ticks are scheduled at a
//! future `game_time` with a priority and deduplicated by `(pos, item)`.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};
use std::hash::Hash;

use oxidized_protocol::types::BlockPos;

/// Priority level for a scheduled tick. Lower numeric value = higher priority.
///
/// Mirrors `net.minecraft.world.ticks.TickPriority`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i8)]
pub enum TickPriority {
    /// Highest priority (e.g., instant repeaters).
    ExtremelyHigh = -3,
    /// Very high priority.
    VeryHigh = -2,
    /// High priority (e.g., redstone torches).
    High = -1,
    /// Default priority.
    Normal = 0,
    /// Low priority.
    Low = 1,
    /// Very low priority.
    VeryLow = 2,
    /// Lowest priority.
    ExtremelyLow = 3,
}

impl PartialOrd for TickPriority {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TickPriority {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (*self as i8).cmp(&(*other as i8))
    }
}

/// A single scheduled tick for a block or fluid.
#[derive(Debug, Clone)]
pub struct ScheduledTick<T> {
    /// Block position of the scheduled tick.
    pub pos: BlockPos,
    /// The block/fluid type identifier.
    pub item: T,
    /// Game time when this tick should fire.
    pub trigger_time: i64,
    /// Priority (lower value = fires first among same trigger_time).
    pub priority: TickPriority,
    /// Sub-tick ordering for determinism among same time+priority.
    pub sub_tick: i64,
}

impl<T: PartialEq> PartialEq for ScheduledTick<T> {
    fn eq(&self, other: &Self) -> bool {
        self.trigger_time == other.trigger_time
            && self.priority == other.priority
            && self.sub_tick == other.sub_tick
            && self.pos == other.pos
            && self.item == other.item
    }
}

impl<T: Eq> Eq for ScheduledTick<T> {}

impl<T: PartialEq> PartialOrd for ScheduledTick<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp_order(other))
    }
}

impl<T: Eq> Ord for ScheduledTick<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.cmp_order(other)
    }
}

impl<T: PartialEq> ScheduledTick<T> {
    fn cmp_order(&self, other: &Self) -> std::cmp::Ordering {
        self.trigger_time
            .cmp(&other.trigger_time)
            .then(self.priority.cmp(&other.priority))
            .then(self.sub_tick.cmp(&other.sub_tick))
    }
}

/// A priority queue of scheduled ticks with deduplication.
///
/// Ticks are ordered by `(trigger_time, priority, sub_tick)`.
/// Duplicate `(pos, item)` pairs are ignored when scheduling.
#[derive(Debug)]
pub struct LevelTicks<T: Eq + Hash + Clone> {
    /// Min-heap of scheduled ticks (using Reverse for min ordering).
    queue: BinaryHeap<Reverse<ScheduledTick<T>>>,
    /// Set of `(pos, item)` pairs currently in the queue for deduplication.
    scheduled: HashSet<(BlockPos, T)>,
    /// Current game time (updated each tick).
    current_time: i64,
    /// Monotonic counter for sub-tick ordering.
    next_sub_tick: i64,
}

impl<T: Eq + Hash + Clone> Default for LevelTicks<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Eq + Hash + Clone> LevelTicks<T> {
    /// Creates an empty tick queue.
    pub fn new() -> Self {
        Self {
            queue: BinaryHeap::new(),
            scheduled: HashSet::new(),
            current_time: 0,
            next_sub_tick: 0,
        }
    }

    /// Schedules a tick at `current_time + delay` with the given priority.
    ///
    /// Duplicate `(pos, item)` pairs are silently ignored.
    pub fn schedule(&mut self, pos: BlockPos, item: T, delay: i64, priority: TickPriority) {
        let key = (pos, item.clone());
        if self.scheduled.contains(&key) {
            return;
        }
        self.scheduled.insert(key);
        let sub_tick = self.next_sub_tick;
        self.next_sub_tick += 1;
        self.queue.push(Reverse(ScheduledTick {
            pos,
            item,
            trigger_time: self.current_time + delay,
            priority,
            sub_tick,
        }));
    }

    /// Processes all ticks due at or before `game_time`, calling `callback`
    /// for each.
    pub fn tick(&mut self, game_time: i64, mut callback: impl FnMut(ScheduledTick<T>)) {
        self.current_time = game_time;
        while let Some(Reverse(tick)) = self.queue.peek() {
            if tick.trigger_time > game_time {
                break;
            }
            #[allow(clippy::unwrap_used)]
            let Reverse(tick) = self.queue.pop().unwrap();
            self.scheduled.remove(&(tick.pos, tick.item.clone()));
            callback(tick);
        }
    }

    /// Returns `true` if there are no pending ticks.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Returns the number of pending ticks.
    pub fn len(&self) -> usize {
        self.queue.len()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_fires_at_correct_time() {
        let mut ticks: LevelTicks<u32> = LevelTicks::new();
        let pos = BlockPos::new(0, 64, 0);
        ticks.schedule(pos, 42, 5, TickPriority::Normal);

        let mut fired = vec![];
        ticks.tick(4, |t| fired.push(t.item));
        assert!(fired.is_empty(), "should not fire at time 4");

        ticks.tick(5, |t| fired.push(t.item));
        assert_eq!(fired, vec![42]);
    }

    #[test]
    fn test_deduplicates_same_pos_and_item() {
        let mut ticks: LevelTicks<u32> = LevelTicks::new();
        let pos = BlockPos::new(1, 64, 1);
        ticks.schedule(pos, 1, 5, TickPriority::Normal);
        ticks.schedule(pos, 1, 5, TickPriority::Normal); // duplicate

        let mut count = 0;
        ticks.tick(10, |_| count += 1);
        assert_eq!(count, 1, "duplicate tick should be deduplicated");
    }

    #[test]
    fn test_different_items_not_deduplicated() {
        let mut ticks: LevelTicks<u32> = LevelTicks::new();
        let pos = BlockPos::new(1, 64, 1);
        ticks.schedule(pos, 1, 5, TickPriority::Normal);
        ticks.schedule(pos, 2, 5, TickPriority::Normal);

        let mut count = 0;
        ticks.tick(10, |_| count += 1);
        assert_eq!(count, 2, "different items at same pos should both fire");
    }

    #[test]
    fn test_respects_priority_ordering() {
        let mut ticks: LevelTicks<u32> = LevelTicks::new();
        let pos1 = BlockPos::new(0, 0, 0);
        let pos2 = BlockPos::new(1, 0, 0);
        ticks.schedule(pos1, 1, 1, TickPriority::Low);
        ticks.schedule(pos2, 2, 1, TickPriority::High);

        let mut order = vec![];
        ticks.tick(1, |t| order.push(t.item));
        assert_eq!(order[0], 2, "High priority tick must fire first");
        assert_eq!(order[1], 1, "Low priority tick fires second");
    }

    #[test]
    fn test_sub_tick_ordering_preserves_insertion_order() {
        let mut ticks: LevelTicks<u32> = LevelTicks::new();
        for i in 0..5 {
            ticks.schedule(BlockPos::new(i, 0, 0), i as u32, 1, TickPriority::Normal);
        }

        let mut order = vec![];
        ticks.tick(1, |t| order.push(t.item));
        assert_eq!(order, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_reschedule_after_fire() {
        let mut ticks: LevelTicks<u32> = LevelTicks::new();
        let pos = BlockPos::new(0, 0, 0);
        ticks.schedule(pos, 1, 5, TickPriority::Normal);

        let mut fired = false;
        ticks.tick(5, |_| fired = true);
        assert!(fired);

        // After firing, the same (pos, item) can be rescheduled
        ticks.schedule(pos, 1, 3, TickPriority::Normal);
        let mut fired2 = false;
        ticks.tick(8, |_| fired2 = true);
        assert!(fired2);
    }

    #[test]
    fn test_is_empty_and_len() {
        let mut ticks: LevelTicks<u32> = LevelTicks::new();
        assert!(ticks.is_empty());
        assert_eq!(ticks.len(), 0);

        ticks.schedule(BlockPos::new(0, 0, 0), 1, 5, TickPriority::Normal);
        assert!(!ticks.is_empty());
        assert_eq!(ticks.len(), 1);

        ticks.tick(10, |_| {});
        assert!(ticks.is_empty());
    }
}
