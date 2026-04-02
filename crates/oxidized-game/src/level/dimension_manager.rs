//! [`DimensionManager`] — manages multiple dimensions (overworld, nether, end).
//!
//! Each dimension is a [`ServerLevel`] accessed via its
//! [`ResourceLocation`] key.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use oxidized_mc_types::ResourceLocation;

use super::server_level::ServerLevel;

/// Manages all loaded dimensions on the server.
///
/// Each dimension is identified by its [`ResourceLocation`] key
/// (e.g., `minecraft:overworld`) and wrapped in `Arc<RwLock<ServerLevel>>`
/// for shared async access.
pub struct DimensionManager {
    levels: HashMap<ResourceLocation, Arc<RwLock<ServerLevel>>>,
}

impl DimensionManager {
    /// Creates a new empty dimension manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            levels: HashMap::new(),
        }
    }

    /// Registers a dimension.
    pub fn register(&mut self, id: ResourceLocation, level: ServerLevel) {
        self.levels.insert(id, Arc::new(RwLock::new(level)));
    }

    /// Returns the level for the given dimension, if registered.
    #[must_use]
    pub fn get(&self, id: &ResourceLocation) -> Option<Arc<RwLock<ServerLevel>>> {
        self.levels.get(id).map(Arc::clone)
    }

    /// Returns the overworld level.
    ///
    /// # Panics
    ///
    /// Panics if the overworld has not been registered.
    #[must_use]
    #[allow(clippy::expect_used)]
    pub fn overworld(&self) -> Arc<RwLock<ServerLevel>> {
        self.get(&ResourceLocation::minecraft("overworld"))
            .expect("overworld not registered")
    }

    /// Returns an iterator over all registered dimensions.
    pub fn iter(&self) -> impl Iterator<Item = (&ResourceLocation, &Arc<RwLock<ServerLevel>>)> {
        self.levels.iter()
    }

    /// Returns the number of registered dimensions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.levels.len()
    }

    /// Returns `true` if no dimensions are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }
}

impl Default for DimensionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::path::Path;

    use oxidized_registry::BlockRegistry;
    use oxidized_world::anvil::{AnvilChunkLoader, AsyncChunkLoader};
    use oxidized_world::storage::PrimaryLevelData;

    use crate::level::dimension::DimensionType;

    fn test_level(dim: DimensionType) -> ServerLevel {
        let registry = Arc::new(BlockRegistry::load().unwrap());
        let loader = AnvilChunkLoader::new(Path::new("/tmp/oxidized_test_nonexistent"), registry);
        let async_loader = AsyncChunkLoader::new(loader);
        let level_data = PrimaryLevelData::from_nbt(&oxidized_nbt::NbtCompound::new()).unwrap();
        ServerLevel::new(
            dim,
            Arc::new(tokio::sync::RwLock::new(level_data)),
            async_loader,
            64,
        )
    }

    #[test]
    fn register_and_get() {
        let mut mgr = DimensionManager::new();
        mgr.register(
            ResourceLocation::minecraft("overworld"),
            test_level(DimensionType::overworld()),
        );

        assert_eq!(mgr.len(), 1);
        assert!(mgr.get(&ResourceLocation::minecraft("overworld")).is_some());
        assert!(
            mgr.get(&ResourceLocation::minecraft("the_nether"))
                .is_none()
        );
    }

    #[test]
    fn overworld_shorthand() {
        let mut mgr = DimensionManager::new();
        mgr.register(
            ResourceLocation::minecraft("overworld"),
            test_level(DimensionType::overworld()),
        );

        let _ow = mgr.overworld(); // should not panic
    }

    #[test]
    #[should_panic(expected = "overworld not registered")]
    fn overworld_panics_if_missing() {
        let mgr = DimensionManager::new();
        let _ = mgr.overworld();
    }

    #[test]
    fn multiple_dimensions() {
        let mut mgr = DimensionManager::new();
        mgr.register(
            ResourceLocation::minecraft("overworld"),
            test_level(DimensionType::overworld()),
        );
        mgr.register(
            ResourceLocation::minecraft("the_nether"),
            test_level(DimensionType::nether()),
        );
        mgr.register(
            ResourceLocation::minecraft("the_end"),
            test_level(DimensionType::the_end()),
        );

        assert_eq!(mgr.len(), 3);
        assert!(!mgr.is_empty());
    }

    #[test]
    fn iter_dimensions() {
        let mut mgr = DimensionManager::new();
        mgr.register(
            ResourceLocation::minecraft("overworld"),
            test_level(DimensionType::overworld()),
        );
        mgr.register(
            ResourceLocation::minecraft("the_nether"),
            test_level(DimensionType::nether()),
        );

        let keys: Vec<_> = mgr.iter().map(|(k, _)| k.to_string()).collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"minecraft:overworld".to_string()));
        assert!(keys.contains(&"minecraft:the_nether".to_string()));
    }

    #[test]
    fn default_is_empty() {
        let mgr = DimensionManager::default();
        assert!(mgr.is_empty());
    }
}
