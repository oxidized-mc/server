//! Global entity ID allocation.
//!
//! Every entity in the server (players, mobs, items, projectiles, etc.)
//! receives a unique network-visible ID from a single global counter.
//! IDs are monotonically increasing and never recycled within a session.

use std::sync::atomic::{AtomicI32, Ordering};

/// Global entity ID counter, starting at 1 (ID 0 is reserved).
static NEXT_ENTITY_ID: AtomicI32 = AtomicI32::new(1);

/// Allocates a globally unique entity ID.
///
/// Thread-safe and lock-free. IDs are sequential starting from 1.
///
/// # Examples
///
/// ```
/// use oxidized_game::entity::id::next_entity_id;
///
/// let id = next_entity_id();
/// assert!(id >= 1);
/// ```
pub fn next_entity_id() -> i32 {
    NEXT_ENTITY_ID.fetch_add(1, Ordering::Relaxed)
}

/// Resets the entity ID counter (test-only).
///
/// # Safety
///
/// Must only be called in single-threaded test contexts.
#[cfg(test)]
pub fn reset_counter(value: i32) {
    NEXT_ENTITY_ID.store(value, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use std::collections::HashSet;

    #[test]
    fn entity_id_uniqueness() {
        reset_counter(1);
        let ids: Vec<i32> = (0..100).map(|_| next_entity_id()).collect();
        let unique: HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), 100, "IDs must be unique");
        assert!(
            ids.windows(2).all(|w| w[1] == w[0] + 1),
            "IDs must be sequential"
        );
    }

    #[test]
    fn entity_id_starts_at_one() {
        reset_counter(1);
        assert_eq!(next_entity_id(), 1);
    }

    #[test]
    fn entity_id_never_zero() {
        reset_counter(1);
        let ids: Vec<i32> = (0..10).map(|_| next_entity_id()).collect();
        assert!(!ids.contains(&0));
    }
}
