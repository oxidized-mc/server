//! Tick phase enum and schedule labels for entity system scheduling.
//!
//! Defines the strict phase order within each server tick as specified
//! by ADR-018 §System Scheduling (Entity System). Within each phase, `bevy_ecs`
//! automatically parallelizes non-conflicting systems.
//!
//! Each [`TickPhase`] maps to a [`PhaseSchedule`] label via
//! [`TickPhase::label()`], allowing registration of systems into
//! the appropriate per-tick schedule.

use bevy_ecs::prelude::SystemSet;
use bevy_ecs::schedule::ScheduleLabel;

/// Schedule label newtype for [`TickPhase`].
///
/// `bevy_ecs` requires [`ScheduleLabel`] on schedule identifiers.
/// We wrap `TickPhase` rather than deriving directly on it to keep
/// the phase enum decoupled from bevy scheduling details.
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhaseSchedule(pub TickPhase);

/// Ordering sets within the [`TickPhase::EntityBehavior`] phase.
///
/// Systems can use `.before(BehaviorOrder::Pickup)` etc. to declare
/// ordering constraints within the behavior phase.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BehaviorOrder {
    /// Decrement pickup delays, despawn timers, etc.
    Timers,
    /// Merge nearby identical entities (e.g. item stacks).
    Merge,
    /// Pickup / collection logic.
    Pickup,
}

/// Phases within a single server tick, in execution order.
///
/// Each phase groups related systems. Phases execute sequentially to
/// maintain determinism and vanilla compatibility, while systems
/// *within* a phase may run in parallel.
///
/// # Phase Order (ADR-018)
///
/// 1. [`PreTick`](Self::PreTick) — bookkeeping, spawns/despawns
/// 2. [`Physics`](Self::Physics) — gravity, velocity, collisions
/// 3. [`Ai`](Self::Ai) — goal selection, pathfinding
/// 4. [`EntityBehavior`](Self::EntityBehavior) — type-specific logic
/// 5. [`StatusEffects`](Self::StatusEffects) — potion effects
/// 6. [`PostTick`](Self::PostTick) — bounding boxes, chunk tracking
/// 7. [`NetworkSync`](Self::NetworkSync) — dirty data serialisation
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TickPhase {
    /// Increment tick counter, process pending entity spawns and despawns.
    PreTick = 0,
    /// Apply gravity, velocity, resolve collisions, update ground state
    /// and fall distance.
    Physics = 1,
    /// Run `GoalSelector` for mobs, evaluate goals, update pathfinding.
    Ai = 2,
    /// Entity-type-specific logic (zombie burning, creeper explosion
    /// timer, villager trading, breeding cooldowns, item pickup,
    /// projectile flight).
    EntityBehavior = 3,
    /// Apply and expire potion effects, tick poison/wither/regeneration.
    StatusEffects = 4,
    /// Update bounding boxes, chunk section tracking, trigger game events.
    PostTick = 5,
    /// Serialise dirty `SynchedEntityData`, position updates, equipment
    /// changes, and other entity-related packets.
    NetworkSync = 6,
}

impl TickPhase {
    /// All phases in execution order.
    pub const ALL: [TickPhase; 7] = [
        TickPhase::PreTick,
        TickPhase::Physics,
        TickPhase::Ai,
        TickPhase::EntityBehavior,
        TickPhase::StatusEffects,
        TickPhase::PostTick,
        TickPhase::NetworkSync,
    ];

    /// Returns the corresponding [`PhaseSchedule`] label for this phase.
    pub fn label(self) -> PhaseSchedule {
        PhaseSchedule(self)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_tick_phase_ordering_matches_adr018() {
        // ADR-018 specifies this strict order
        assert!(TickPhase::PreTick < TickPhase::Physics);
        assert!(TickPhase::Physics < TickPhase::Ai);
        assert!(TickPhase::Ai < TickPhase::EntityBehavior);
        assert!(TickPhase::EntityBehavior < TickPhase::StatusEffects);
        assert!(TickPhase::StatusEffects < TickPhase::PostTick);
        assert!(TickPhase::PostTick < TickPhase::NetworkSync);
    }

    #[test]
    fn test_tick_phase_all_contains_every_variant() {
        assert_eq!(TickPhase::ALL.len(), 7);
        assert_eq!(TickPhase::ALL[0], TickPhase::PreTick);
        assert_eq!(TickPhase::ALL[6], TickPhase::NetworkSync);
    }

    #[test]
    fn test_tick_phase_all_is_sorted() {
        for window in TickPhase::ALL.windows(2) {
            assert!(
                window[0] < window[1],
                "{:?} should be before {:?}",
                window[0],
                window[1]
            );
        }
    }

    #[test]
    fn test_tick_phase_discriminant_values() {
        assert_eq!(TickPhase::PreTick as u8, 0);
        assert_eq!(TickPhase::Physics as u8, 1);
        assert_eq!(TickPhase::Ai as u8, 2);
        assert_eq!(TickPhase::EntityBehavior as u8, 3);
        assert_eq!(TickPhase::StatusEffects as u8, 4);
        assert_eq!(TickPhase::PostTick as u8, 5);
        assert_eq!(TickPhase::NetworkSync as u8, 6);
    }

    #[test]
    fn test_tick_phase_debug_format() {
        assert_eq!(format!("{:?}", TickPhase::PreTick), "PreTick");
        assert_eq!(format!("{:?}", TickPhase::NetworkSync), "NetworkSync");
    }

    #[test]
    fn test_tick_phase_eq_and_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        for phase in TickPhase::ALL {
            assert!(set.insert(phase), "duplicate phase: {phase:?}");
        }
        assert_eq!(set.len(), 7);
    }

    #[test]
    fn test_phase_schedule_label() {
        let label = TickPhase::Physics.label();
        assert_eq!(label.0, TickPhase::Physics);
    }

    #[test]
    fn test_all_phases_produce_distinct_labels() {
        use std::collections::HashSet;
        let labels: HashSet<_> = TickPhase::ALL.iter().map(|p| p.label()).collect();
        assert_eq!(labels.len(), 7);
    }

    #[test]
    fn test_behavior_order_is_system_set() {
        // Verify the enum variants are distinct for use as system ordering constraints.
        assert_ne!(BehaviorOrder::Timers, BehaviorOrder::Merge);
        assert_ne!(BehaviorOrder::Merge, BehaviorOrder::Pickup);
        assert_ne!(BehaviorOrder::Timers, BehaviorOrder::Pickup);
    }
}
