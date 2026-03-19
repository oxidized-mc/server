//! Entity tracking — determines which players see each entity.
//!
//! Mirrors `ChunkMap.TrackedEntity` in vanilla. When the set of players
//! within tracking range changes, the tracker returns lists of players
//! that need spawn/despawn packets.

use std::collections::{HashMap, HashSet};

/// Default tracking ranges (in blocks) per entity type category.
///
/// These match the `clientTrackingRange()` × 16 values from
/// `EntityType.java` registrations.
pub const TRACKING_RANGE_PLAYER: i32 = 160; // 10 chunks
/// Tracking range for animals (cows, pigs, etc.).
pub const TRACKING_RANGE_ANIMAL: i32 = 128; // 8 chunks
/// Tracking range for hostile mobs (zombies, skeletons, etc.).
pub const TRACKING_RANGE_MONSTER: i32 = 128; // 8 chunks
/// Tracking range for miscellaneous entities (items, XP orbs).
pub const TRACKING_RANGE_MISC: i32 = 80; // 5 chunks (default)
/// Tracking range for projectiles (arrows, fireballs).
pub const TRACKING_RANGE_PROJECTILE: i32 = 64; // 4 chunks

/// Tracks which players are watching each entity.
///
/// Call [`update()`](Self::update) each tick with the current set of
/// players in range. It returns the players that need spawn packets
/// (newly in range) and despawn packets (just left range).
pub struct EntityTracker {
    /// entity_id → set of player UUIDs currently watching it.
    watching: HashMap<i32, HashSet<uuid::Uuid>>,
    /// Tracking range per entity (in blocks).
    range: HashMap<i32, i32>,
}

impl EntityTracker {
    /// Creates an empty tracker.
    pub fn new() -> Self {
        Self {
            watching: HashMap::new(),
            range: HashMap::new(),
        }
    }

    /// Registers an entity for tracking with the given range in blocks.
    pub fn register(&mut self, entity_id: i32, tracking_range: i32) {
        self.watching.insert(entity_id, HashSet::new());
        self.range.insert(entity_id, tracking_range);
    }

    /// Unregisters an entity, returning the set of players that were
    /// watching it (all need despawn packets).
    pub fn unregister(&mut self, entity_id: i32) -> HashSet<uuid::Uuid> {
        self.range.remove(&entity_id);
        self.watching.remove(&entity_id).unwrap_or_default()
    }

    /// Updates the watching set for an entity.
    ///
    /// Returns `(to_add, to_remove)`:
    /// - `to_add`: players that just entered range (need spawn packets).
    /// - `to_remove`: players that just left range (need despawn packets).
    pub fn update(
        &mut self,
        entity_id: i32,
        now_watching: HashSet<uuid::Uuid>,
    ) -> (Vec<uuid::Uuid>, Vec<uuid::Uuid>) {
        let current = self.watching.entry(entity_id).or_default();
        let to_add: Vec<_> = now_watching.difference(current).copied().collect();
        let to_remove: Vec<_> =
            current.difference(&now_watching).copied().collect();
        *current = now_watching;
        (to_add, to_remove)
    }

    /// Returns `true` if `player_uuid` is currently tracking `entity_id`.
    pub fn is_tracking(
        &self,
        entity_id: i32,
        player_uuid: &uuid::Uuid,
    ) -> bool {
        self.watching
            .get(&entity_id)
            .is_some_and(|s| s.contains(player_uuid))
    }

    /// Returns the tracking range for the given entity, or `None` if
    /// the entity is not registered.
    pub fn tracking_range(&self, entity_id: i32) -> Option<i32> {
        self.range.get(&entity_id).copied()
    }

    /// Returns the number of tracked entities.
    pub fn len(&self) -> usize {
        self.watching.len()
    }

    /// Returns `true` if no entities are being tracked.
    pub fn is_empty(&self) -> bool {
        self.watching.is_empty()
    }

    /// Returns the number of players watching a specific entity.
    pub fn watcher_count(&self, entity_id: i32) -> usize {
        self.watching
            .get(&entity_id)
            .map_or(0, HashSet::len)
    }
}

impl Default for EntityTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns `true` if a player at `(px, pz)` is within squared tracking
/// range of an entity at `(ex, ez)`.
///
/// Uses XZ-plane distance only (ignoring Y), matching vanilla's
/// `ChunkMap.TrackedEntity.updatePlayer()`.
///
/// # Examples
///
/// ```
/// use oxidized_game::entity::tracker::is_in_tracking_range;
///
/// assert!(is_in_tracking_range(0.0, 0.0, 50.0, 0.0, 64));
/// assert!(!is_in_tracking_range(0.0, 0.0, 100.0, 0.0, 64));
/// ```
pub fn is_in_tracking_range(
    entity_x: f64,
    entity_z: f64,
    player_x: f64,
    player_z: f64,
    range: i32,
) -> bool {
    let dx = player_x - entity_x;
    let dz = player_z - entity_z;
    let range_sq = (range as f64) * (range as f64);
    (dx * dx + dz * dz) <= range_sq
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_register_and_unregister() {
        let mut tracker = EntityTracker::new();
        tracker.register(42, 64);
        assert_eq!(tracker.len(), 1);
        assert_eq!(tracker.tracking_range(42), Some(64));

        let watchers = tracker.unregister(42);
        assert!(watchers.is_empty());
        assert_eq!(tracker.len(), 0);
        assert_eq!(tracker.tracking_range(42), None);
    }

    #[test]
    fn test_update_adds_new_watchers() {
        let mut tracker = EntityTracker::new();
        tracker.register(42, 64);

        let p1 = uuid::Uuid::new_v4();
        let (add, remove) =
            tracker.update(42, [p1].into_iter().collect());
        assert_eq!(add.len(), 1);
        assert_eq!(add[0], p1);
        assert!(remove.is_empty());
        assert!(tracker.is_tracking(42, &p1));
    }

    #[test]
    fn test_update_removes_departed_watchers() {
        let mut tracker = EntityTracker::new();
        tracker.register(42, 64);

        let p1 = uuid::Uuid::new_v4();
        let p2 = uuid::Uuid::new_v4();

        // First: p1 and p2 watching
        tracker.update(42, [p1, p2].into_iter().collect());

        // Second: only p2 watching → p1 removed
        let (add, remove) =
            tracker.update(42, [p2].into_iter().collect());
        assert!(add.is_empty());
        assert_eq!(remove.len(), 1);
        assert_eq!(remove[0], p1);
        assert!(!tracker.is_tracking(42, &p1));
        assert!(tracker.is_tracking(42, &p2));
    }

    #[test]
    fn test_update_simultaneous_add_remove() {
        let mut tracker = EntityTracker::new();
        tracker.register(42, 64);

        let p1 = uuid::Uuid::new_v4();
        let p2 = uuid::Uuid::new_v4();

        tracker.update(42, [p1].into_iter().collect());
        let (add, remove) =
            tracker.update(42, [p2].into_iter().collect());
        assert_eq!(add.len(), 1);
        assert_eq!(add[0], p2);
        assert_eq!(remove.len(), 1);
        assert_eq!(remove[0], p1);
    }

    #[test]
    fn test_unregister_returns_watchers() {
        let mut tracker = EntityTracker::new();
        tracker.register(42, 64);

        let p1 = uuid::Uuid::new_v4();
        tracker.update(42, [p1].into_iter().collect());

        let watchers = tracker.unregister(42);
        assert!(watchers.contains(&p1));
    }

    #[test]
    fn test_watcher_count() {
        let mut tracker = EntityTracker::new();
        tracker.register(42, 64);
        assert_eq!(tracker.watcher_count(42), 0);

        let p1 = uuid::Uuid::new_v4();
        let p2 = uuid::Uuid::new_v4();
        tracker.update(42, [p1, p2].into_iter().collect());
        assert_eq!(tracker.watcher_count(42), 2);
    }

    #[test]
    fn test_is_in_tracking_range() {
        // Within range
        assert!(is_in_tracking_range(0.0, 0.0, 50.0, 0.0, 64));
        // Exactly at range boundary
        assert!(is_in_tracking_range(0.0, 0.0, 64.0, 0.0, 64));
        // Out of range
        assert!(!is_in_tracking_range(0.0, 0.0, 65.0, 0.0, 64));
        // Diagonal
        assert!(is_in_tracking_range(
            0.0, 0.0, 45.0, 45.0, 64
        ));
        assert!(!is_in_tracking_range(
            0.0, 0.0, 46.0, 46.0, 64
        ));
    }

    #[test]
    fn test_is_empty() {
        let tracker = EntityTracker::new();
        assert!(tracker.is_empty());
    }
}
