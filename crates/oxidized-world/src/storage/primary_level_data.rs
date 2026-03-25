//! Primary level data parsed from and written to `level.dat`.
//!
//! Contains world metadata: spawn position, game type, difficulty, time,
//! weather, and other server-level settings. Supports both deserialization
//! (from NBT) and serialization (to NBT) for save/load cycles.

use std::path::Path;

use oxidized_nbt::{NbtCompound, NbtTag};

use crate::anvil::AnvilError;

/// World spawn location and orientation for new players.
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnPoint {
    /// World spawn X coordinate.
    pub x: i32,
    /// World spawn Y coordinate.
    pub y: i32,
    /// World spawn Z coordinate.
    pub z: i32,
    /// Spawn angle (yaw) for new players.
    pub angle: f32,
}

/// World age and day/night cycle time.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldTime {
    /// Total world age in game ticks.
    pub game_time: i64,
    /// Time of day within the 24000-tick day cycle.
    pub day_time: i64,
}

/// Current weather conditions and countdown timers.
#[derive(Debug, Clone, PartialEq)]
pub struct WeatherState {
    /// Whether it is currently raining.
    pub is_raining: bool,
    /// Whether it is currently thundering.
    pub is_thundering: bool,
    /// Ticks until rain stops or starts.
    pub rain_time: i32,
    /// Ticks until thunder stops or starts.
    pub thunder_time: i32,
    /// Ticks of guaranteed clear weather remaining (overrides rain/thunder when > 0).
    pub clear_weather_time: i32,
}

/// Persistent world configuration and generation settings.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldSettings {
    /// World display name.
    pub level_name: String,
    /// NBT data version for schema migrations.
    pub data_version: i32,
    /// Game mode as numeric ID (0=Survival, 1=Creative, 2=Adventure, 3=Spectator).
    pub game_type: i32,
    /// Whether this is a hardcore world.
    pub is_hardcore: bool,
    /// Difficulty as numeric ID (0=Peaceful, 1=Easy, 2=Normal, 3=Hard).
    pub difficulty: i32,
    /// Whether commands are allowed.
    pub is_commands_allowed: bool,
    /// Whether the world has been initialized (initial chunks generated).
    pub is_initialized: bool,
    /// Sea level height (default 63).
    pub sea_level: i32,
    /// Whether the difficulty is locked (prevents players from changing it).
    pub is_difficulty_locked: bool,
    /// World generation seed.
    pub world_seed: i64,
}

/// World metadata parsed from the `Data` compound inside `level.dat`.
///
/// Groups related fields into sub-structs: spawn location, time tracking,
/// weather state, and persistent world settings.
#[derive(Debug, Clone, PartialEq)]
pub struct PrimaryLevelData {
    /// World spawn location and player orientation.
    pub spawn: SpawnPoint,
    /// World age and day/night cycle.
    pub time: WorldTime,
    /// Current weather conditions.
    pub weather: WeatherState,
    /// Persistent world configuration.
    pub settings: WorldSettings,
}

impl PrimaryLevelData {
    /// Parses level data from the `Data` compound inside `level.dat`.
    ///
    /// # Errors
    ///
    /// Returns [`AnvilError::MissingField`] if required fields are absent.
    pub fn from_nbt(data: &NbtCompound) -> Result<Self, AnvilError> {
        Ok(Self {
            spawn: SpawnPoint {
                x: data.get_int("SpawnX").unwrap_or(0),
                y: data.get_int("SpawnY").unwrap_or(64),
                z: data.get_int("SpawnZ").unwrap_or(0),
                angle: data.get_float("SpawnAngle").unwrap_or(0.0),
            },
            time: WorldTime {
                game_time: data.get_long("Time").unwrap_or(0),
                day_time: data.get_long("DayTime").unwrap_or(0),
            },
            weather: WeatherState {
                is_raining: data.get_byte("raining").unwrap_or(0) != 0,
                is_thundering: data.get_byte("thundering").unwrap_or(0) != 0,
                rain_time: data.get_int("rainTime").unwrap_or(0),
                thunder_time: data.get_int("thunderTime").unwrap_or(0),
                clear_weather_time: data.get_int("clearWeatherTime").unwrap_or(0),
            },
            settings: WorldSettings {
                level_name: data.get_string("LevelName").unwrap_or("Unnamed").to_owned(),
                data_version: data.get_int("DataVersion").unwrap_or(0),
                game_type: data.get_int("GameType").unwrap_or(0),
                is_hardcore: data.get_byte("hardcore").unwrap_or(0) != 0,
                difficulty: data.get_byte("Difficulty").unwrap_or(2) as i32,
                is_commands_allowed: data.get_byte("allowCommands").unwrap_or(0) != 0,
                is_initialized: data.get_byte("initialized").unwrap_or(1) != 0,
                sea_level: data.get_int("SeaLevel").unwrap_or(63),
                is_difficulty_locked: data.get_byte("DifficultyLocked").unwrap_or(0) != 0,
                world_seed: data
                    .get_compound("WorldGenSettings")
                    .and_then(|wgs| wgs.get_long("seed"))
                    .unwrap_or(0),
            },
        })
    }

    /// Returns the world spawn position as a tuple `(x, y, z)`.
    ///
    /// Returns a tuple rather than `BlockPos` because `oxidized-world`
    /// does not depend on `oxidized-protocol`.
    pub fn spawn_pos(&self) -> (i32, i32, i32) {
        (self.spawn.x, self.spawn.y, self.spawn.z)
    }

    /// Loads level data from a `level.dat` file (GZip-compressed NBT).
    ///
    /// # Errors
    ///
    /// Returns an error on I/O failure, decompression failure, or missing
    /// required NBT fields.
    pub fn load(path: &Path) -> Result<Self, AnvilError> {
        let root = oxidized_nbt::read_file(path).map_err(AnvilError::from)?;
        let data = root
            .get_compound("Data")
            .ok_or(AnvilError::MissingField { field: "Data" })?;
        Self::from_nbt(data)
    }

    /// Serializes this level data to an NBT `Data` compound.
    ///
    /// Returns a root compound containing a `Data` child with all fields,
    /// matching the vanilla `level.dat` format.
    #[must_use]
    pub fn to_nbt(&self) -> NbtCompound {
        let mut data = NbtCompound::new();
        data.put_string("LevelName", &self.settings.level_name);
        data.put_int("DataVersion", self.settings.data_version);
        data.put_int("GameType", self.settings.game_type);
        data.put_int("SpawnX", self.spawn.x);
        data.put_int("SpawnY", self.spawn.y);
        data.put_int("SpawnZ", self.spawn.z);
        data.put_float("SpawnAngle", self.spawn.angle);
        data.put_long("Time", self.time.game_time);
        data.put_long("DayTime", self.time.day_time);
        data.put_byte("raining", i8::from(self.weather.is_raining));
        data.put_byte("thundering", i8::from(self.weather.is_thundering));
        data.put_int("rainTime", self.weather.rain_time);
        data.put_int("thunderTime", self.weather.thunder_time);
        data.put_int("clearWeatherTime", self.weather.clear_weather_time);
        data.put_byte("hardcore", i8::from(self.settings.is_hardcore));
        #[allow(clippy::cast_possible_truncation)]
        data.put_byte("Difficulty", self.settings.difficulty as i8);
        data.put_byte("allowCommands", i8::from(self.settings.is_commands_allowed));
        data.put_byte("initialized", i8::from(self.settings.is_initialized));
        data.put_int("SeaLevel", self.settings.sea_level);
        data.put_byte(
            "DifficultyLocked",
            i8::from(self.settings.is_difficulty_locked),
        );

        // WorldGenSettings/seed
        let mut wgs = NbtCompound::new();
        wgs.put_long("seed", self.settings.world_seed);
        data.put("WorldGenSettings", NbtTag::Compound(wgs));

        let mut root = NbtCompound::new();
        root.put("Data", NbtTag::Compound(data));
        root
    }

    /// Saves level data to a file using the safe backup pattern.
    ///
    /// 1. Write to `<path>_new` (temporary)
    /// 2. If `<path>` exists, rename it to `<path>_old` (backup)
    /// 3. Rename `<path>_new` to `<path>` (atomic commit)
    ///
    /// This ensures at least one valid file always exists on disk, even if
    /// the process crashes mid-write.
    ///
    /// # Errors
    ///
    /// Returns [`AnvilError::Io`] on file I/O failure.
    pub fn save(&self, path: &Path) -> Result<(), AnvilError> {
        let nbt = self.to_nbt();

        // Ensure parent directories exist before writing.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AnvilError::io(parent, e))?;
        }

        let tmp_path = path.with_extension("dat_new");
        let old_path = path.with_extension("dat_old");

        // Write to temporary file first.
        oxidized_nbt::write_file(&tmp_path, &nbt)
            .map_err(|e| AnvilError::io(&tmp_path, std::io::Error::other(e.to_string())))?;

        // Back up the existing file.
        if path.exists() {
            std::fs::rename(path, &old_path).map_err(|e| AnvilError::io(&old_path, e))?;
        }

        // Commit: rename temporary to final path.
        std::fs::rename(&tmp_path, path).map_err(|e| AnvilError::io(path, e))?;

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use oxidized_nbt::NbtCompound;

    fn sample_data_compound() -> NbtCompound {
        let mut data = NbtCompound::new();
        data.put_string("LevelName", "TestWorld");
        data.put_int("DataVersion", 4782);
        data.put_int("GameType", 1);
        data.put_int("SpawnX", 100);
        data.put_int("SpawnY", 72);
        data.put_int("SpawnZ", -200);
        data.put_float("SpawnAngle", 90.0);
        data.put_long("Time", 24000);
        data.put_long("DayTime", 6000);
        data.put_byte("raining", 1);
        data.put_byte("thundering", 0);
        data.put_int("rainTime", 5000);
        data.put_int("thunderTime", 0);
        data.put_int("clearWeatherTime", 0);
        data.put_byte("hardcore", 0);
        data.put_byte("Difficulty", 2);
        data.put_byte("allowCommands", 1);
        data.put_byte("initialized", 1);
        data.put_byte("DifficultyLocked", 1);

        let mut wgs = NbtCompound::new();
        wgs.put_long("seed", 123_456_789);
        data.put("WorldGenSettings", NbtTag::Compound(wgs));

        data
    }

    #[test]
    fn test_from_nbt_all_fields() {
        let data = sample_data_compound();
        let level = PrimaryLevelData::from_nbt(&data).unwrap();

        assert_eq!(level.settings.level_name, "TestWorld");
        assert_eq!(level.settings.data_version, 4782);
        assert_eq!(level.settings.game_type, 1);
        assert_eq!(level.spawn.x, 100);
        assert_eq!(level.spawn.y, 72);
        assert_eq!(level.spawn.z, -200);
        assert!((level.spawn.angle - 90.0).abs() < f32::EPSILON);
        assert_eq!(level.time.game_time, 24000);
        assert_eq!(level.time.day_time, 6000);
        assert!(level.weather.is_raining);
        assert!(!level.weather.is_thundering);
        assert_eq!(level.weather.rain_time, 5000);
        assert_eq!(level.weather.thunder_time, 0);
        assert_eq!(level.weather.clear_weather_time, 0);
        assert!(!level.settings.is_hardcore);
        assert_eq!(level.settings.difficulty, 2);
        assert!(level.settings.is_commands_allowed);
        assert!(level.settings.is_initialized);
        assert!(level.settings.is_difficulty_locked);
        assert_eq!(level.settings.world_seed, 123_456_789);
    }

    #[test]
    fn test_from_nbt_defaults() {
        let data = NbtCompound::new();
        let level = PrimaryLevelData::from_nbt(&data).unwrap();

        assert_eq!(level.settings.level_name, "Unnamed");
        assert_eq!(level.settings.data_version, 0);
        assert_eq!(level.settings.game_type, 0);
        assert_eq!(level.spawn.x, 0);
        assert_eq!(level.spawn.y, 64);
        assert_eq!(level.spawn.z, 0);
        assert!(!level.weather.is_raining);
        assert!(!level.settings.is_hardcore);
        assert_eq!(level.settings.difficulty, 2); // Normal default
        assert!(!level.settings.is_commands_allowed);
        assert!(level.settings.is_initialized); // Default true
        assert!(!level.settings.is_difficulty_locked);
        assert_eq!(level.settings.world_seed, 0);
    }

    #[test]
    fn test_load_from_file() {
        use oxidized_nbt::NbtTag;

        let dir = std::env::temp_dir();
        let path = dir.join("oxidized_test_level.dat");

        let mut root = NbtCompound::new();
        root.put("Data", NbtTag::Compound(sample_data_compound()));

        oxidized_nbt::write_file(&path, &root).unwrap();

        let level = PrimaryLevelData::load(&path).unwrap();
        assert_eq!(level.settings.level_name, "TestWorld");
        assert_eq!(level.spawn.x, 100);
        assert_eq!(level.settings.game_type, 1);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_load_missing_data_compound() {
        let dir = std::env::temp_dir();
        let path = dir.join("oxidized_test_level_no_data.dat");

        let root = NbtCompound::new();
        oxidized_nbt::write_file(&path, &root).unwrap();

        let result = PrimaryLevelData::load(&path);
        assert!(result.is_err());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_to_nbt_roundtrip() {
        let data = sample_data_compound();
        let level = PrimaryLevelData::from_nbt(&data).unwrap();

        // Serialize back to NBT
        let root_nbt = level.to_nbt();
        let data_nbt = root_nbt.get_compound("Data").unwrap();

        // Deserialize again
        let level2 = PrimaryLevelData::from_nbt(data_nbt).unwrap();

        assert_eq!(level2.settings.level_name, level.settings.level_name);
        assert_eq!(level2.settings.data_version, level.settings.data_version);
        assert_eq!(level2.settings.game_type, level.settings.game_type);
        assert_eq!(level2.spawn.x, level.spawn.x);
        assert_eq!(level2.spawn.y, level.spawn.y);
        assert_eq!(level2.spawn.z, level.spawn.z);
        assert!((level2.spawn.angle - level.spawn.angle).abs() < f32::EPSILON);
        assert_eq!(level2.time.game_time, level.time.game_time);
        assert_eq!(level2.time.day_time, level.time.day_time);
        assert_eq!(level2.weather.is_raining, level.weather.is_raining);
        assert_eq!(level2.weather.is_thundering, level.weather.is_thundering);
        assert_eq!(level2.weather.rain_time, level.weather.rain_time);
        assert_eq!(level2.weather.thunder_time, level.weather.thunder_time);
        assert_eq!(
            level2.weather.clear_weather_time,
            level.weather.clear_weather_time
        );
        assert_eq!(level2.settings.is_hardcore, level.settings.is_hardcore);
        assert_eq!(level2.settings.difficulty, level.settings.difficulty);
        assert_eq!(
            level2.settings.is_commands_allowed,
            level.settings.is_commands_allowed
        );
        assert_eq!(
            level2.settings.is_initialized,
            level.settings.is_initialized
        );
        assert_eq!(level2.settings.sea_level, level.settings.sea_level);
        assert_eq!(
            level2.settings.is_difficulty_locked,
            level.settings.is_difficulty_locked
        );
        assert_eq!(level2.settings.world_seed, level.settings.world_seed);
    }

    #[test]
    fn test_to_nbt_defaults_roundtrip() {
        let level = PrimaryLevelData::from_nbt(&NbtCompound::new()).unwrap();
        let root_nbt = level.to_nbt();
        let data_nbt = root_nbt.get_compound("Data").unwrap();
        let level2 = PrimaryLevelData::from_nbt(data_nbt).unwrap();

        assert_eq!(level2.settings.level_name, "Unnamed");
        assert_eq!(level2.spawn.y, 64);
        assert_eq!(level2.settings.difficulty, 2);
        assert!(level2.settings.is_initialized);
    }

    #[test]
    fn test_save_creates_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("oxidized_test_save_create.dat");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("dat_old"));
        let _ = std::fs::remove_file(path.with_extension("dat_new"));

        let level = PrimaryLevelData::from_nbt(&sample_data_compound()).unwrap();
        level.save(&path).unwrap();

        assert!(path.exists(), "level.dat should exist after save");
        // No backup when there was no previous file
        assert!(!path.with_extension("dat_old").exists());

        // Verify we can load it back
        let loaded = PrimaryLevelData::load(&path).unwrap();
        assert_eq!(loaded.settings.level_name, "TestWorld");
        assert_eq!(loaded.time.game_time, 24000);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_save_creates_backup() {
        let dir = std::env::temp_dir();
        let path = dir.join("oxidized_test_save_backup.dat");
        let old_path = path.with_extension("dat_old");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&old_path);

        // Initial save
        let mut level = PrimaryLevelData::from_nbt(&sample_data_compound()).unwrap();
        level.save(&path).unwrap();

        // Modify and save again — should create backup
        level.time.game_time = 48000;
        level.save(&path).unwrap();

        assert!(path.exists(), "level.dat should exist");
        assert!(old_path.exists(), "level.dat_old backup should exist");

        // Backup should contain original data
        let backup = PrimaryLevelData::load(&old_path).unwrap();
        assert_eq!(
            backup.time.game_time, 24000,
            "backup should have original time"
        );

        // New file should contain updated data
        let current = PrimaryLevelData::load(&path).unwrap();
        assert_eq!(
            current.time.game_time, 48000,
            "current should have updated time"
        );

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&old_path);
    }

    #[test]
    fn test_save_load_full_roundtrip() {
        let dir = std::env::temp_dir();
        let path = dir.join("oxidized_test_full_roundtrip.dat");
        let _ = std::fs::remove_file(&path);

        let data = sample_data_compound();
        let level = PrimaryLevelData::from_nbt(&data).unwrap();
        level.save(&path).unwrap();
        let loaded = PrimaryLevelData::load(&path).unwrap();

        assert_eq!(loaded.settings.level_name, level.settings.level_name);
        assert_eq!(loaded.time.game_time, level.time.game_time);
        assert_eq!(loaded.time.day_time, level.time.day_time);
        assert_eq!(loaded.weather.is_raining, level.weather.is_raining);
        assert_eq!(loaded.spawn.x, level.spawn.x);
        assert_eq!(loaded.spawn.y, level.spawn.y);
        assert_eq!(loaded.spawn.z, level.spawn.z);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("dat_old"));
    }
}
