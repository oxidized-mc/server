//! World folder layout resolver.
//!
//! Locates the world folder and provides paths to per-dimension subdirectories,
//! `level.dat`, player data, and other world files.

use std::path::{Path, PathBuf};

use super::dimension::Dimension;

/// Locates the world folder and resolves paths to per-dimension data.
#[derive(Debug, Clone)]
pub struct LevelStorageSource {
    /// Root world folder (e.g. `./world`).
    world_dir: PathBuf,
}

impl LevelStorageSource {
    /// Creates a new storage source for the given world directory.
    pub fn new(world_dir: impl Into<PathBuf>) -> Self {
        Self {
            world_dir: world_dir.into(),
        }
    }

    /// Returns the root world directory.
    #[must_use]
    pub fn world_dir(&self) -> &Path {
        &self.world_dir
    }

    /// Returns the path to `level.dat`.
    #[must_use]
    pub fn level_dat_path(&self) -> PathBuf {
        self.world_dir.join("level.dat")
    }

    /// Returns the path to `level.dat_old` (backup).
    #[must_use]
    pub fn level_dat_old_path(&self) -> PathBuf {
        self.world_dir.join("level.dat_old")
    }

    /// Returns the region directory for a dimension.
    ///
    /// - Overworld: `<world>/region`
    /// - Nether: `<world>/DIM-1/region`
    /// - End: `<world>/DIM1/region`
    #[must_use]
    pub fn region_dir(&self, dimension: Dimension) -> PathBuf {
        match dimension.folder_name() {
            None => self.world_dir.join("region"),
            Some(folder) => self.world_dir.join(folder).join("region"),
        }
    }

    /// Returns the player data directory.
    #[must_use]
    pub fn player_data_dir(&self) -> PathBuf {
        self.world_dir.join("playerdata")
    }

    /// Returns the data directory (advancements, stats, etc).
    #[must_use]
    pub fn data_dir(&self) -> PathBuf {
        self.world_dir.join("data")
    }

    /// Returns the entities directory for a dimension.
    #[must_use]
    pub fn entities_dir(&self, dimension: Dimension) -> PathBuf {
        match dimension.folder_name() {
            None => self.world_dir.join("entities"),
            Some(folder) => self.world_dir.join(folder).join("entities"),
        }
    }

    /// Returns `true` if the world directory exists.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.world_dir.exists()
    }

    /// Returns `true` if `level.dat` exists.
    #[must_use]
    pub fn has_level_dat(&self) -> bool {
        self.level_dat_path().exists()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_level_dat_path() {
        let storage = LevelStorageSource::new("/tmp/world");
        assert_eq!(
            storage.level_dat_path(),
            PathBuf::from("/tmp/world/level.dat")
        );
    }

    #[test]
    fn test_region_dir_overworld() {
        let storage = LevelStorageSource::new("/tmp/world");
        assert_eq!(
            storage.region_dir(Dimension::Overworld),
            PathBuf::from("/tmp/world/region")
        );
    }

    #[test]
    fn test_region_dir_nether() {
        let storage = LevelStorageSource::new("/tmp/world");
        assert_eq!(
            storage.region_dir(Dimension::Nether),
            PathBuf::from("/tmp/world/DIM-1/region")
        );
    }

    #[test]
    fn test_region_dir_end() {
        let storage = LevelStorageSource::new("/tmp/world");
        assert_eq!(
            storage.region_dir(Dimension::End),
            PathBuf::from("/tmp/world/DIM1/region")
        );
    }

    #[test]
    fn test_player_data_dir() {
        let storage = LevelStorageSource::new("/tmp/world");
        assert_eq!(
            storage.player_data_dir(),
            PathBuf::from("/tmp/world/playerdata")
        );
    }
}
