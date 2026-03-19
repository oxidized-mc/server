//! Server-wide player roster.
//!
//! Tracks all connected players by UUID with join-order iteration,
//! entity ID assignment, and capacity enforcement.
//!
//! Mirrors `net.minecraft.server.players.PlayerList`.

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

use ahash::AHashMap;
use parking_lot::RwLock;
use uuid::Uuid;

use super::server_player::ServerPlayer;

/// Server-wide player roster.
///
/// Thread-safe via `Arc<RwLock<ServerPlayer>>` entries. The `PlayerList`
/// itself should be wrapped in its own lock or held by a single owner
/// (the tick loop).
///
/// # Examples
///
/// ```
/// use oxidized_game::player::PlayerList;
///
/// let list = PlayerList::new(20);
/// assert_eq!(list.player_count(), 0);
/// assert_eq!(list.max_players(), 20);
///
/// let id1 = list.next_entity_id();
/// let id2 = list.next_entity_id();
/// assert_ne!(id1, id2); // monotonically increasing
/// ```
#[derive(Debug)]
pub struct PlayerList {
    /// Players indexed by UUID.
    players: AHashMap<Uuid, Arc<RwLock<ServerPlayer>>>,
    /// UUIDs in join order for deterministic iteration.
    join_order: Vec<Uuid>,
    /// Maximum allowed players (for display and capacity checks).
    max_players: usize,
    /// Monotonically increasing entity ID counter.
    entity_id_counter: AtomicI32,
}

impl PlayerList {
    /// Creates an empty player list with the given maximum capacity.
    pub fn new(max_players: usize) -> Self {
        Self {
            players: AHashMap::new(),
            join_order: Vec::new(),
            max_players,
            entity_id_counter: AtomicI32::new(1),
        }
    }

    /// Returns the next unique entity ID.
    pub fn next_entity_id(&self) -> i32 {
        self.entity_id_counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Returns the number of connected players.
    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    /// Returns the maximum number of players allowed.
    pub fn max_players(&self) -> usize {
        self.max_players
    }

    /// Returns `true` if the server is at capacity.
    pub fn is_full(&self) -> bool {
        self.player_count() >= self.max_players
    }

    /// Returns `true` if no players are connected.
    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
    }

    /// Adds a player to the list and returns a shared reference.
    ///
    /// If a player with the same UUID already exists, the old entry is
    /// replaced (matching vanilla duplicate-login behavior).
    pub fn add(&mut self, player: ServerPlayer) -> Arc<RwLock<ServerPlayer>> {
        let uuid = player.uuid;
        let arc = Arc::new(RwLock::new(player));
        self.players.insert(uuid, Arc::clone(&arc));
        if !self.join_order.contains(&uuid) {
            self.join_order.push(uuid);
        }
        arc
    }

    /// Removes a player by UUID. Returns the player if found.
    pub fn remove(&mut self, uuid: &Uuid) -> Option<Arc<RwLock<ServerPlayer>>> {
        let player = self.players.remove(uuid)?;
        self.join_order.retain(|u| u != uuid);
        Some(player)
    }

    /// Returns a shared reference to the player with the given UUID.
    pub fn get(&self, uuid: &Uuid) -> Option<&Arc<RwLock<ServerPlayer>>> {
        self.players.get(uuid)
    }

    /// Iterates over players in join order.
    pub fn iter(&self) -> impl Iterator<Item = &Arc<RwLock<ServerPlayer>>> {
        self.join_order
            .iter()
            .filter_map(move |uuid| self.players.get(uuid))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use oxidized_protocol::auth::GameProfile;
    use oxidized_protocol::types::ResourceLocation;

    use super::*;
    use crate::player::game_mode::GameMode;

    fn make_player(name: &str) -> ServerPlayer {
        let uuid = Uuid::new_v4();
        let profile = GameProfile::new(uuid, name.into());
        ServerPlayer::new(
            1, // entity_id doesn't matter for these tests
            profile,
            ResourceLocation::minecraft("overworld"),
            GameMode::Survival,
        )
    }

    fn make_player_with_id(list: &PlayerList, name: &str) -> ServerPlayer {
        let uuid = Uuid::new_v4();
        let profile = GameProfile::new(uuid, name.into());
        ServerPlayer::new(
            list.next_entity_id(),
            profile,
            ResourceLocation::minecraft("overworld"),
            GameMode::Survival,
        )
    }

    #[test]
    fn test_add_and_get() {
        let mut list = PlayerList::new(20);
        let uuid = Uuid::new_v4();
        let profile = GameProfile::new(uuid, "Steve".into());
        let player = ServerPlayer::new(
            list.next_entity_id(),
            profile,
            ResourceLocation::minecraft("overworld"),
            GameMode::Survival,
        );

        let arc = list.add(player);
        assert_eq!(list.player_count(), 1);
        assert_eq!(arc.read().name, "Steve");

        let got = list.get(&uuid).unwrap();
        assert_eq!(got.read().name, "Steve");
    }

    #[test]
    fn test_is_full() {
        let mut list = PlayerList::new(2);
        let p1 = make_player_with_id(&list, "Alice");
        let p2 = make_player_with_id(&list, "Bob");

        assert!(!list.is_full());
        list.add(p1);
        assert!(!list.is_full());
        list.add(p2);
        assert!(list.is_full());
    }

    #[test]
    fn test_remove() {
        let mut list = PlayerList::new(20);
        let uuid = Uuid::new_v4();
        let profile = GameProfile::new(uuid, "Steve".into());
        let player = ServerPlayer::new(
            list.next_entity_id(),
            profile,
            ResourceLocation::minecraft("overworld"),
            GameMode::Survival,
        );

        list.add(player);
        let removed = list.remove(&uuid);
        assert!(removed.is_some());
        assert_eq!(list.player_count(), 0);
        assert!(list.is_empty());
    }

    #[test]
    fn test_remove_nonexistent() {
        let mut list = PlayerList::new(20);
        assert!(list.remove(&Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_iter_join_order() {
        let mut list = PlayerList::new(20);
        let p1 = make_player("Alice");
        let p2 = make_player("Bob");
        let p3 = make_player("Charlie");

        list.add(p1);
        list.add(p2);
        list.add(p3);

        let names: Vec<String> = list.iter().map(|p| p.read().name.clone()).collect();
        assert_eq!(names, vec!["Alice", "Bob", "Charlie"]);
    }

    #[test]
    fn test_iter_after_remove() {
        let mut list = PlayerList::new(20);
        let p1 = make_player("Alice");
        let uuid2 = Uuid::new_v4();
        let profile2 = GameProfile::new(uuid2, "Bob".into());
        let p2 = ServerPlayer::new(
            list.next_entity_id(),
            profile2,
            ResourceLocation::minecraft("overworld"),
            GameMode::Survival,
        );
        let p3 = make_player("Charlie");

        list.add(p1);
        list.add(p2);
        list.add(p3);

        list.remove(&uuid2);

        let names: Vec<String> = list.iter().map(|p| p.read().name.clone()).collect();
        assert_eq!(names, vec!["Alice", "Charlie"]);
    }

    #[test]
    fn test_entity_ids_unique() {
        let list = PlayerList::new(20);
        let id1 = list.next_entity_id();
        let id2 = list.next_entity_id();
        let id3 = list.next_entity_id();
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[test]
    fn test_empty_list() {
        let list = PlayerList::new(20);
        assert!(list.is_empty());
        assert_eq!(list.player_count(), 0);
        assert_eq!(list.max_players(), 20);
        assert!(!list.is_full());
    }

    #[test]
    fn test_player_count_and_max() {
        let mut list = PlayerList::new(2);
        assert_eq!(list.player_count(), 0);
        assert_eq!(list.max_players(), 2);

        list.add(make_player("Alice"));
        list.add(make_player("Bob"));
        assert_eq!(list.player_count(), 2);
        assert!(list.is_full());
    }
}
