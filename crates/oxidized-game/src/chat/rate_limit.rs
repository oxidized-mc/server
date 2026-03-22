//! Chat rate limiter — prevents message spam.
//!
//! Implements the vanilla `TickThrottler` algorithm: each chat message
//! increments an accumulator by [`INCREMENT_STEP`]; every server tick
//! decrements it by 1. When the accumulator reaches [`THRESHOLD`] the
//! player is kicked for spamming.

/// Accumulator increment per chat message (vanilla default: 20).
pub const INCREMENT_STEP: i32 = 20;

/// Accumulator value at which the player is considered spamming (vanilla default: 200).
pub const THRESHOLD: i32 = 200;

/// Per-tick decay chat rate limiter matching vanilla's `TickThrottler`.
#[derive(Debug)]
pub struct ChatRateLimiter {
    /// Current accumulator value.
    count: i32,
}

impl ChatRateLimiter {
    /// Creates a new rate limiter with a zero accumulator.
    pub fn new() -> Self {
        Self { count: 0 }
    }

    /// Records a chat message and returns `true` if the player is still
    /// under the spam threshold, or `false` if they should be kicked.
    pub fn try_acquire(&mut self) -> bool {
        self.count += INCREMENT_STEP;
        self.count < THRESHOLD
    }

    /// Called once per server tick to decay the accumulator.
    pub fn tick(&mut self) {
        if self.count > 0 {
            self.count -= 1;
        }
    }

    /// Returns `true` if the player is currently over the spam threshold.
    pub fn is_rate_limited(&self) -> bool {
        self.count >= THRESHOLD
    }
}

impl Default for ChatRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_allows_messages_under_threshold() {
        let mut limiter = ChatRateLimiter::new();
        // 9 messages × 20 = 180, still under 200
        for i in 0..9 {
            assert!(limiter.try_acquire(), "message {i} should be allowed");
        }
    }

    #[test]
    fn test_kicks_at_threshold() {
        let mut limiter = ChatRateLimiter::new();
        // 10 messages × 20 = 200, exactly at threshold → kicked
        for _ in 0..9 {
            assert!(limiter.try_acquire());
        }
        assert!(!limiter.try_acquire(), "10th message should trigger kick");
    }

    #[test]
    fn test_tick_decay_allows_more_messages() {
        let mut limiter = ChatRateLimiter::new();
        // Send 9 messages (count = 180)
        for _ in 0..9 {
            assert!(limiter.try_acquire());
        }
        // Decay 20 ticks → count = 160
        for _ in 0..20 {
            limiter.tick();
        }
        // One more message: 160 + 20 = 180, under threshold
        assert!(limiter.try_acquire());
    }

    #[test]
    fn test_full_decay() {
        let mut limiter = ChatRateLimiter::new();
        // Send 5 messages (count = 100)
        for _ in 0..5 {
            assert!(limiter.try_acquire());
        }
        // Decay all 100 ticks
        for _ in 0..100 {
            limiter.tick();
        }
        assert_eq!(limiter.count, 0);
        assert!(!limiter.is_rate_limited());
    }

    #[test]
    fn test_tick_does_not_go_negative() {
        let mut limiter = ChatRateLimiter::new();
        limiter.tick();
        assert_eq!(limiter.count, 0);
    }

    #[test]
    fn test_is_rate_limited() {
        let mut limiter = ChatRateLimiter::new();
        assert!(!limiter.is_rate_limited());
        // Push to threshold
        for _ in 0..10 {
            limiter.try_acquire();
        }
        assert!(limiter.is_rate_limited());
    }

    #[test]
    fn test_default_impl() {
        let limiter = ChatRateLimiter::default();
        assert!(!limiter.is_rate_limited());
    }
}
