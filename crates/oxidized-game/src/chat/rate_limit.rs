//! Chat rate limiter — prevents message spam.
//!
//! Implements a sliding-window rate limiter that rejects messages once a
//! player exceeds `MAX_MESSAGES` within `WINDOW_DURATION`.

use std::time::{Duration, Instant};

/// Maximum number of messages allowed within the sliding window.
pub const MAX_MESSAGES: usize = 200;

/// Duration of the sliding window.
pub const WINDOW_DURATION: Duration = Duration::from_secs(60);

/// Sliding-window rate limiter for chat messages.
#[derive(Debug)]
pub struct ChatRateLimiter {
    /// Circular buffer of timestamps for recent messages.
    timestamps: Vec<Instant>,
    /// Head position in the circular buffer.
    head: usize,
    /// Number of valid entries.
    count: usize,
}

impl ChatRateLimiter {
    /// Creates a new rate limiter.
    pub fn new() -> Self {
        Self {
            timestamps: Vec::with_capacity(MAX_MESSAGES),
            head: 0,
            count: 0,
        }
    }

    /// Attempts to record a message at the given time.
    ///
    /// Returns `true` if the message is allowed, `false` if rate-limited.
    pub fn try_acquire(&mut self, now: Instant) -> bool {
        self.expire(now);

        if self.count >= MAX_MESSAGES {
            return false;
        }

        if self.timestamps.len() < MAX_MESSAGES {
            self.timestamps.push(now);
        } else {
            let idx = (self.head + self.count) % MAX_MESSAGES;
            self.timestamps[idx] = now;
        }
        self.count += 1;
        true
    }

    /// Expire timestamps outside the sliding window.
    fn expire(&mut self, now: Instant) {
        while self.count > 0 {
            if now.duration_since(self.timestamps[self.head]) > WINDOW_DURATION {
                self.head = (self.head + 1) % MAX_MESSAGES;
                self.count -= 1;
            } else {
                break;
            }
        }
    }

    /// Returns `true` if the player is currently rate-limited.
    pub fn is_rate_limited(&self, now: Instant) -> bool {
        let mut temp_count = self.count;
        let mut temp_head = self.head;
        while temp_count > 0 {
            if now.duration_since(self.timestamps[temp_head]) > WINDOW_DURATION {
                temp_head = (temp_head + 1) % MAX_MESSAGES;
                temp_count -= 1;
            } else {
                break;
            }
        }
        temp_count >= MAX_MESSAGES
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
    fn test_allows_messages_under_limit() {
        let mut limiter = ChatRateLimiter::new();
        let now = Instant::now();
        for i in 0..10 {
            assert!(
                limiter.try_acquire(now + Duration::from_millis(i)),
                "message {i} should be allowed"
            );
        }
    }

    #[test]
    fn test_allows_exactly_max_messages() {
        let mut limiter = ChatRateLimiter::new();
        let now = Instant::now();
        for i in 0..MAX_MESSAGES {
            assert!(
                limiter.try_acquire(now + Duration::from_millis(i as u64)),
                "message {i} should be allowed"
            );
        }
    }

    #[test]
    fn test_blocks_over_max() {
        let mut limiter = ChatRateLimiter::new();
        let now = Instant::now();
        for i in 0..MAX_MESSAGES {
            assert!(limiter.try_acquire(now + Duration::from_millis(i as u64)));
        }
        // The 201st message should be blocked
        assert!(!limiter.try_acquire(now + Duration::from_millis(MAX_MESSAGES as u64)));
    }

    #[test]
    fn test_allows_after_window_expires() {
        let mut limiter = ChatRateLimiter::new();
        let now = Instant::now();
        for i in 0..MAX_MESSAGES {
            assert!(limiter.try_acquire(now + Duration::from_millis(i as u64)));
        }
        // After the window expires, messages should be allowed again
        let later = now + WINDOW_DURATION + Duration::from_secs(1);
        assert!(limiter.try_acquire(later));
    }

    #[test]
    fn test_is_rate_limited() {
        let mut limiter = ChatRateLimiter::new();
        let now = Instant::now();
        assert!(!limiter.is_rate_limited(now));
        for i in 0..MAX_MESSAGES {
            limiter.try_acquire(now + Duration::from_millis(i as u64));
        }
        assert!(limiter.is_rate_limited(now + Duration::from_millis(MAX_MESSAGES as u64)));
        assert!(!limiter.is_rate_limited(now + WINDOW_DURATION + Duration::from_secs(1)));
    }

    #[test]
    fn test_sliding_window() {
        let mut limiter = ChatRateLimiter::new();
        let now = Instant::now();

        // Fill half the window
        for i in 0..100 {
            assert!(limiter.try_acquire(now + Duration::from_millis(i)));
        }

        // Fill the other half at a later time
        let half_window = now + Duration::from_secs(30);
        for i in 0..100 {
            assert!(limiter.try_acquire(half_window + Duration::from_millis(i)));
        }

        // Now at MAX_MESSAGES, next should be blocked
        assert!(!limiter.try_acquire(half_window + Duration::from_millis(100)));

        // After first batch expires (60s from now), we should have room
        let after_first_expire = now + WINDOW_DURATION + Duration::from_secs(1);
        assert!(limiter.try_acquire(after_first_expire));
    }

    #[test]
    fn test_default_impl() {
        let limiter = ChatRateLimiter::default();
        assert!(!limiter.is_rate_limited(Instant::now()));
    }
}
