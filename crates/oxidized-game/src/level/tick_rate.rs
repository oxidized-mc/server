//! Server tick rate manager — controls freeze, step, sprint, and rate.
//!
//! Mirrors `net.minecraft.server.ServerTickRateManager` and the base
//! `net.minecraft.world.TickRateManager`.

use std::time::Duration;

/// Controls the server tick rate and supports freeze/step/sprint modes.
#[derive(Debug, Clone)]
pub struct ServerTickRateManager {
    /// Target ticks per second (default 20.0).
    pub tick_rate: f32,
    /// Whether the server is frozen (no game ticks advance).
    pub frozen: bool,
    /// Remaining steps to advance while frozen.
    pub steps_remaining: u32,
    /// Whether the server is in sprint mode (ticks as fast as possible).
    pub sprinting: bool,
    /// Remaining ticks in the current sprint.
    pub sprint_ticks_remaining: u64,
    /// Frozen state saved before a sprint, restored when the sprint ends.
    previous_frozen: bool,
}

impl Default for ServerTickRateManager {
    fn default() -> Self {
        Self {
            tick_rate: 20.0,
            frozen: false,
            steps_remaining: 0,
            sprinting: false,
            sprint_ticks_remaining: 0,
            previous_frozen: false,
        }
    }
}

impl ServerTickRateManager {
    /// Returns the interval between ticks for the current rate.
    pub fn tick_interval(&self) -> Duration {
        if self.tick_rate <= 0.0 {
            return Duration::from_millis(50);
        }
        Duration::from_secs_f32(1.0 / self.tick_rate)
    }

    /// Returns `true` if the server is frozen and has no pending steps.
    pub fn is_frozen(&self) -> bool {
        self.frozen && self.steps_remaining == 0
    }

    /// Returns `true` if the server is frozen but has pending steps.
    pub fn should_step(&self) -> bool {
        self.frozen && self.steps_remaining > 0
    }

    /// Consumes one pending step. No-op if no steps remain.
    pub fn consume_step(&mut self) {
        if self.steps_remaining > 0 {
            self.steps_remaining -= 1;
        }
    }

    /// Enqueues `count` steps to advance while frozen.
    pub fn request_steps(&mut self, count: u32) {
        self.steps_remaining = self.steps_remaining.saturating_add(count);
    }

    /// Returns `true` if the current tick should execute game logic.
    ///
    /// This accounts for frozen state, stepping, and sprinting.
    pub fn should_tick(&mut self) -> bool {
        if self.sprinting {
            if self.sprint_ticks_remaining > 0 {
                self.sprint_ticks_remaining -= 1;
                return true;
            }
            // Sprint ended — restore previous frozen state.
            self.sprinting = false;
            self.frozen = self.previous_frozen;
        }

        if self.frozen {
            if self.steps_remaining > 0 {
                self.steps_remaining -= 1;
                return true;
            }
            return false;
        }

        true
    }

    /// Starts a sprint of `ticks` duration.
    ///
    /// Saves the current frozen state and unfreezes for the duration of
    /// the sprint. The frozen state is restored when the sprint ends.
    pub fn start_sprint(&mut self, ticks: u64) {
        self.previous_frozen = self.frozen;
        self.frozen = false;
        self.sprinting = true;
        self.sprint_ticks_remaining = ticks;
    }

    /// Sets the tick rate and returns `true` if it changed.
    pub fn set_rate(&mut self, rate: f32) -> bool {
        if (self.tick_rate - rate).abs() > f32::EPSILON {
            self.tick_rate = rate;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_20_tps() {
        let mgr = ServerTickRateManager::default();
        assert!((mgr.tick_rate - 20.0).abs() < f32::EPSILON);
        assert!(!mgr.frozen);
        assert!(!mgr.sprinting);
    }

    #[test]
    fn test_freeze_and_step() {
        let mut mgr = ServerTickRateManager {
            frozen: true,
            ..Default::default()
        };
        assert!(mgr.is_frozen());
        assert!(!mgr.should_step());

        mgr.request_steps(3);
        assert!(mgr.should_step());
        assert!(!mgr.is_frozen());

        mgr.consume_step();
        mgr.consume_step();
        mgr.consume_step();
        assert!(!mgr.should_step());
        assert!(mgr.is_frozen(), "still frozen after all steps consumed");
    }

    #[test]
    fn test_tick_interval_matches_rate() {
        let mgr = ServerTickRateManager {
            tick_rate: 10.0,
            ..Default::default()
        };
        let interval = mgr.tick_interval();
        assert!((interval.as_secs_f32() - 0.1).abs() < 1e-5);
    }

    #[test]
    fn test_should_tick_normal() {
        let mut mgr = ServerTickRateManager::default();
        assert!(mgr.should_tick());
    }

    #[test]
    fn test_should_tick_frozen() {
        let mut mgr = ServerTickRateManager {
            frozen: true,
            ..Default::default()
        };
        assert!(!mgr.should_tick());
    }

    #[test]
    fn test_should_tick_frozen_with_steps() {
        let mut mgr = ServerTickRateManager {
            frozen: true,
            ..Default::default()
        };
        mgr.request_steps(2);
        assert!(mgr.should_tick()); // step 1
        assert!(mgr.should_tick()); // step 2
        assert!(!mgr.should_tick()); // no more steps
    }

    #[test]
    fn test_sprint() {
        let mut mgr = ServerTickRateManager::default();
        mgr.start_sprint(3);
        assert!(mgr.sprinting);
        assert!(mgr.should_tick());
        assert!(mgr.should_tick());
        assert!(mgr.should_tick());
        // Sprint ended, but was not frozen before — should still tick normally.
        assert!(mgr.should_tick());
        assert!(!mgr.sprinting);
        assert!(!mgr.frozen, "was not frozen before sprint");
    }

    #[test]
    fn test_sprint_restores_frozen_state() {
        let mut mgr = ServerTickRateManager {
            frozen: true,
            ..Default::default()
        };
        mgr.start_sprint(2);
        // Sprint unfreezes temporarily.
        assert!(!mgr.frozen);
        assert!(mgr.should_tick());
        assert!(mgr.should_tick());
        // Sprint ended — frozen state should be restored.
        assert!(!mgr.should_tick(), "should be frozen again after sprint");
        assert!(mgr.frozen, "frozen state restored after sprint");
    }

    #[test]
    fn test_set_rate_returns_changed() {
        let mut mgr = ServerTickRateManager::default();
        assert!(mgr.set_rate(40.0));
        assert!(!mgr.set_rate(40.0));
    }

    #[test]
    fn test_default_interval_50ms() {
        let mgr = ServerTickRateManager::default();
        let interval = mgr.tick_interval();
        assert!((interval.as_millis() as f64 - 50.0).abs() < 1.0);
    }
}
