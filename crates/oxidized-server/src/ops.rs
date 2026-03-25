//! Per-player operator permission management (`ops.json`).
//!
//! Provides [`OpsStore`] for loading, querying, and persisting operator
//! entries in vanilla-compatible JSON format. Each entry records a player's
//! UUID, name, permission level (0–4), and whether they bypass the player
//! limit.
//!
//! # File Format
//!
//! ```json
//! [
//!   {
//!     "uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5",
//!     "name": "Notch",
//!     "level": 4,
//!     "bypassesPlayerLimit": false
//!   }
//! ]
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

/// A single operator entry, matching vanilla's `ops.json` schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OpEntry {
    /// The player's Mojang UUID.
    pub uuid: Uuid,
    /// The player's display name (informational, not authoritative).
    pub name: String,
    /// Permission level (0–4). Vanilla levels:
    /// - 0 = default (no special permissions)
    /// - 1 = moderator (bypass spawn protection)
    /// - 2 = gamemaster (use cheat commands, command blocks)
    /// - 3 = admin (use management commands: /ban, /op, /deop)
    /// - 4 = owner (use all commands including /stop, /save-all)
    pub level: i32,
    /// Whether the player can join even when the server is full.
    pub bypasses_player_limit: bool,
}

/// Thread-safe operator store backed by a JSON file.
///
/// All lookups are O(1) via [`DashMap`]. Mutations automatically persist
/// to disk. If the file does not exist on load, an empty store is created.
#[derive(Debug)]
pub struct OpsStore {
    /// In-memory operator entries indexed by UUID.
    entries: DashMap<Uuid, OpEntry>,
    /// Path to the `ops.json` file for persistence.
    path: PathBuf,
    /// Default permission level for new ops (from server config).
    default_level: i32,
}

impl OpsStore {
    /// Loads the operator store from `ops.json` at the given path.
    ///
    /// If the file does not exist, returns an empty store. If the file
    /// exists but is malformed, logs a warning and returns an empty store.
    pub fn load(path: impl Into<PathBuf>, default_level: i32) -> Self {
        let path = path.into();
        let entries = DashMap::new();

        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<Vec<OpEntry>>(&content) {
                    Ok(ops) => {
                        let count = ops.len();
                        for op in ops {
                            entries.insert(op.uuid, op);
                        }
                        info!(count, path = %path.display(), "Loaded ops.json");
                    }
                    Err(e) => {
                        warn!(
                            error = %e,
                            path = %path.display(),
                            "Failed to parse ops.json; starting with empty ops list"
                        );
                    }
                },
                Err(e) => {
                    warn!(
                        error = %e,
                        path = %path.display(),
                        "Failed to read ops.json; starting with empty ops list"
                    );
                }
            }
        } else {
            info!(path = %path.display(), "No ops.json found; starting with empty ops list");
        }

        Self {
            entries,
            path,
            default_level: default_level.clamp(1, 4),
        }
    }

    /// Returns `true` if the given UUID has an operator entry.
    pub fn is_op(&self, uuid: &Uuid) -> bool {
        self.entries.contains_key(uuid)
    }

    /// Returns the permission level for a player.
    ///
    /// Returns 0 if the player is not an operator.
    pub fn get_permission_level(&self, uuid: &Uuid) -> i32 {
        self.entries
            .get(uuid)
            .map(|e| e.level)
            .unwrap_or(0)
    }

    /// Returns a clone of the operator entry for the given UUID, if any.
    pub fn get(&self, uuid: &Uuid) -> Option<OpEntry> {
        self.entries.get(uuid).map(|e| e.clone())
    }

    /// Adds or updates an operator entry and persists to disk.
    ///
    /// Uses the store's `default_level` if no explicit level is provided.
    pub fn add(&self, uuid: Uuid, name: String, level: Option<i32>, bypasses_player_limit: bool) {
        let level = level.unwrap_or(self.default_level).clamp(0, 4);
        let entry = OpEntry {
            uuid,
            name,
            level,
            bypasses_player_limit,
        };
        self.entries.insert(uuid, entry);
        self.save();
    }

    /// Removes an operator entry by UUID and persists to disk.
    ///
    /// Returns `true` if the entry was present and removed.
    pub fn remove(&self, uuid: &Uuid) -> bool {
        let removed = self.entries.remove(uuid).is_some();
        if removed {
            self.save();
        }
        removed
    }

    /// Returns `true` if the player can bypass the server player limit.
    pub fn bypasses_player_limit(&self, uuid: &Uuid) -> bool {
        self.entries
            .get(uuid)
            .map(|e| e.bypasses_player_limit)
            .unwrap_or(false)
    }

    /// Returns `true` if no operators are configured.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the number of operator entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns the names of all operators (for tab-completion).
    pub fn op_names(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.name.clone()).collect()
    }

    /// Returns the file path for this ops store.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Persists the current entries to `ops.json`.
    fn save(&self) {
        let entries: Vec<OpEntry> = self.entries.iter().map(|e| e.clone()).collect();
        match serde_json::to_string_pretty(&entries) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&self.path, json) {
                    warn!(
                        error = %e,
                        path = %self.path.display(),
                        "Failed to write ops.json"
                    );
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to serialize ops.json");
            }
        }
    }
}

/// Wraps `OpsStore` in an `Arc` for thread-safe shared access.
pub type SharedOpsStore = Arc<OpsStore>;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn sample_ops_json() -> String {
        serde_json::to_string_pretty(&vec![
            OpEntry {
                uuid: Uuid::parse_str("069a79f4-44e9-4726-a5be-fca90e38aaf5").unwrap(),
                name: "Notch".to_string(),
                level: 4,
                bypasses_player_limit: false,
            },
            OpEntry {
                uuid: Uuid::parse_str("61699b2e-d327-4a01-9f1e-0ea8c3f06bc6").unwrap(),
                name: "Dinnerbone".to_string(),
                level: 3,
                bypasses_player_limit: true,
            },
        ])
        .unwrap()
    }

    #[test]
    fn test_load_parses_valid_ops_json() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", sample_ops_json()).unwrap();

        let store = OpsStore::load(file.path(), 4);
        assert_eq!(store.len(), 2);
        assert!(store.is_op(
            &Uuid::parse_str("069a79f4-44e9-4726-a5be-fca90e38aaf5").unwrap()
        ));
        assert_eq!(
            store.get_permission_level(
                &Uuid::parse_str("069a79f4-44e9-4726-a5be-fca90e38aaf5").unwrap()
            ),
            4
        );
        assert_eq!(
            store.get_permission_level(
                &Uuid::parse_str("61699b2e-d327-4a01-9f1e-0ea8c3f06bc6").unwrap()
            ),
            3
        );
    }

    #[test]
    fn test_load_returns_empty_when_file_missing() {
        let store = OpsStore::load("/tmp/nonexistent_ops_test.json", 4);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_load_returns_empty_when_file_malformed() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{{not valid json array}}").unwrap();

        let store = OpsStore::load(file.path(), 4);
        assert!(store.is_empty());
    }

    #[test]
    fn test_add_persists_and_is_queryable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ops.json");
        let store = OpsStore::load(&path, 4);

        let uuid = Uuid::new_v4();
        store.add(uuid, "TestPlayer".to_string(), None, false);

        assert!(store.is_op(&uuid));
        assert_eq!(store.get_permission_level(&uuid), 4);
        assert!(!store.bypasses_player_limit(&uuid));

        // Verify file was written
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: Vec<OpEntry> = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].uuid, uuid);
    }

    #[test]
    fn test_add_with_explicit_level() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ops.json");
        let store = OpsStore::load(&path, 4);

        let uuid = Uuid::new_v4();
        store.add(uuid, "ModPlayer".to_string(), Some(2), true);

        assert_eq!(store.get_permission_level(&uuid), 2);
        assert!(store.bypasses_player_limit(&uuid));
    }

    #[test]
    fn test_add_clamps_level_to_valid_range() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ops.json");
        let store = OpsStore::load(&path, 4);

        let uuid1 = Uuid::new_v4();
        store.add(uuid1, "TooHigh".to_string(), Some(10), false);
        assert_eq!(store.get_permission_level(&uuid1), 4);

        let uuid2 = Uuid::new_v4();
        store.add(uuid2, "TooLow".to_string(), Some(-5), false);
        assert_eq!(store.get_permission_level(&uuid2), 0);
    }

    #[test]
    fn test_remove_deletes_entry_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ops.json");
        let store = OpsStore::load(&path, 4);

        let uuid = Uuid::new_v4();
        store.add(uuid, "RemoveMe".to_string(), None, false);
        assert!(store.is_op(&uuid));

        assert!(store.remove(&uuid));
        assert!(!store.is_op(&uuid));
        assert_eq!(store.get_permission_level(&uuid), 0);

        // Verify file was updated
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: Vec<OpEntry> = serde_json::from_str(&content).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_remove_returns_false_for_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ops.json");
        let store = OpsStore::load(&path, 4);
        assert!(!store.remove(&Uuid::new_v4()));
    }

    #[test]
    fn test_roundtrip_load_save_preserves_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ops.json");

        // Create and populate a store
        let store1 = OpsStore::load(&path, 4);
        let uuid = Uuid::parse_str("069a79f4-44e9-4726-a5be-fca90e38aaf5").unwrap();
        store1.add(uuid, "Notch".to_string(), Some(4), false);
        drop(store1);

        // Load again and verify
        let store2 = OpsStore::load(&path, 4);
        assert_eq!(store2.len(), 1);
        let entry = store2.get(&uuid).unwrap();
        assert_eq!(entry.name, "Notch");
        assert_eq!(entry.level, 4);
        assert!(!entry.bypasses_player_limit);
    }

    #[test]
    fn test_op_names_returns_all_names() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ops.json");
        let store = OpsStore::load(&path, 4);

        store.add(Uuid::new_v4(), "Alice".to_string(), None, false);
        store.add(Uuid::new_v4(), "Bob".to_string(), None, false);

        let mut names = store.op_names();
        names.sort();
        assert_eq!(names, vec!["Alice", "Bob"]);
    }

    #[test]
    fn test_non_op_returns_zero_permission() {
        let store = OpsStore::load("/tmp/nonexistent_ops.json", 4);
        assert_eq!(store.get_permission_level(&Uuid::new_v4()), 0);
    }

    #[test]
    fn test_serde_camel_case_field_names() {
        let entry = OpEntry {
            uuid: Uuid::nil(),
            name: "Test".to_string(),
            level: 4,
            bypasses_player_limit: true,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("bypassesPlayerLimit"));
        assert!(!json.contains("bypasses_player_limit"));
    }
}
