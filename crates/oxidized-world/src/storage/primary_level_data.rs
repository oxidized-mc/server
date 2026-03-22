//! Primary level data parsed from and written to `level.dat`.
//!
//! Contains world metadata: spawn position, game type, difficulty, time,
//! weather, and other server-level settings. Supports both deserialization
//! (from NBT) and serialization (to NBT) for save/load cycles.

use std::path::Path;

use oxidized_nbt::{NbtCompound, NbtTag};

use crate::anvil::AnvilError;

/// World metadata parsed from the `Data` compound inside `level.dat`.
///
/// Stores raw numeric IDs for game type and difficulty to avoid depending
/// on protocol-layer enums. Convert to `GameType`/`Difficulty` at a higher
/// layer.
#[derive(Debug, Clone, PartialEq)]
pub struct PrimaryLevelData {
    /// World display name.
    pub level_name: String,
    /// NBT data version for schema migrations.
    pub data_version: i32,
    /// Game mode as numeric ID (0=Survival, 1=Creative, 2=Adventure, 3=Spectator).
    pub game_type: i32,
    /// World spawn X coordinate.
    pub spawn_x: i32,
    /// World spawn Y coordinate.
    pub spawn_y: i32,
    /// World spawn Z coordinate.
    pub spawn_z: i32,
    /// Spawn angle (yaw) for new players.
    pub spawn_angle: f32,
    /// Total world age in game ticks.
    pub time: i64,
    /// Time of day within the 24000-tick day cycle.
    pub day_time: i64,
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
    /// Whether this is a hardcore world.
    pub hardcore: bool,
    /// Difficulty as numeric ID (0=Peaceful, 1=Easy, 2=Normal, 3=Hard).
    pub difficulty: i32,
    /// Whether commands are allowed.
    pub allow_commands: bool,
    /// Whether the world has been initialized (initial chunks generated).
    pub initialized: bool,
    /// Sea level height (default 63).
    pub sea_level: i32,
}

impl PrimaryLevelData {
    /// Parses level data from the `Data` compound inside `level.dat`.
    ///
    /// # Errors
    ///
    /// Returns [`AnvilError::MissingField`] if required fields are absent.
    pub fn from_nbt(data: &NbtCompound) -> Result<Self, AnvilError> {
        let level_name = data.get_string("LevelName").unwrap_or("Unnamed").to_owned();

        Ok(Self {
            level_name,
            data_version: data.get_int("DataVersion").unwrap_or(0),
            game_type: data.get_int("GameType").unwrap_or(0),
            spawn_x: data.get_int("SpawnX").unwrap_or(0),
            spawn_y: data.get_int("SpawnY").unwrap_or(64),
            spawn_z: data.get_int("SpawnZ").unwrap_or(0),
            spawn_angle: data.get_float("SpawnAngle").unwrap_or(0.0),
            time: data.get_long("Time").unwrap_or(0),
            day_time: data.get_long("DayTime").unwrap_or(0),
            is_raining: data.get_byte("raining").unwrap_or(0) != 0,
            is_thundering: data.get_byte("thundering").unwrap_or(0) != 0,
            rain_time: data.get_int("rainTime").unwrap_or(0),
            thunder_time: data.get_int("thunderTime").unwrap_or(0),
            clear_weather_time: data.get_int("clearWeatherTime").unwrap_or(0),
            hardcore: data.get_byte("hardcore").unwrap_or(0) != 0,
            difficulty: data.get_byte("Difficulty").unwrap_or(2) as i32,
            allow_commands: data.get_byte("allowCommands").unwrap_or(0) != 0,
            initialized: data.get_byte("initialized").unwrap_or(1) != 0,
            sea_level: data.get_int("SeaLevel").unwrap_or(63),
        })
    }

    /// Returns the world spawn position as a tuple `(x, y, z)`.
    ///
    /// Returns a tuple rather than `BlockPos` because `oxidized-world`
    /// does not depend on `oxidized-protocol`.
    pub fn spawn_pos(&self) -> (i32, i32, i32) {
        (self.spawn_x, self.spawn_y, self.spawn_z)
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
        data.put_string("LevelName", &self.level_name);
        data.put_int("DataVersion", self.data_version);
        data.put_int("GameType", self.game_type);
        data.put_int("SpawnX", self.spawn_x);
        data.put_int("SpawnY", self.spawn_y);
        data.put_int("SpawnZ", self.spawn_z);
        data.put_float("SpawnAngle", self.spawn_angle);
        data.put_long("Time", self.time);
        data.put_long("DayTime", self.day_time);
        data.put_byte("raining", i8::from(self.is_raining));
        data.put_byte("thundering", i8::from(self.is_thundering));
        data.put_int("rainTime", self.rain_time);
        data.put_int("thunderTime", self.thunder_time);
        data.put_int("clearWeatherTime", self.clear_weather_time);
        data.put_byte("hardcore", i8::from(self.hardcore));
        #[allow(clippy::cast_possible_truncation)]
        data.put_byte("Difficulty", self.difficulty as i8);
        data.put_byte("allowCommands", i8::from(self.allow_commands));
        data.put_byte("initialized", i8::from(self.initialized));
        data.put_int("SeaLevel", self.sea_level);

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
        data
    }

    #[test]
    fn test_from_nbt_all_fields() {
        let data = sample_data_compound();
        let level = PrimaryLevelData::from_nbt(&data).unwrap();

        assert_eq!(level.level_name, "TestWorld");
        assert_eq!(level.data_version, 4782);
        assert_eq!(level.game_type, 1);
        assert_eq!(level.spawn_x, 100);
        assert_eq!(level.spawn_y, 72);
        assert_eq!(level.spawn_z, -200);
        assert!((level.spawn_angle - 90.0).abs() < f32::EPSILON);
        assert_eq!(level.time, 24000);
        assert_eq!(level.day_time, 6000);
        assert!(level.is_raining);
        assert!(!level.is_thundering);
        assert_eq!(level.rain_time, 5000);
        assert_eq!(level.thunder_time, 0);
        assert_eq!(level.clear_weather_time, 0);
        assert!(!level.hardcore);
        assert_eq!(level.difficulty, 2);
        assert!(level.allow_commands);
        assert!(level.initialized);
    }

    #[test]
    fn test_from_nbt_defaults() {
        let data = NbtCompound::new();
        let level = PrimaryLevelData::from_nbt(&data).unwrap();

        assert_eq!(level.level_name, "Unnamed");
        assert_eq!(level.data_version, 0);
        assert_eq!(level.game_type, 0);
        assert_eq!(level.spawn_x, 0);
        assert_eq!(level.spawn_y, 64);
        assert_eq!(level.spawn_z, 0);
        assert!(!level.is_raining);
        assert!(!level.hardcore);
        assert_eq!(level.difficulty, 2); // Normal default
        assert!(!level.allow_commands);
        assert!(level.initialized); // Default true
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
        assert_eq!(level.level_name, "TestWorld");
        assert_eq!(level.spawn_x, 100);
        assert_eq!(level.game_type, 1);

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

        assert_eq!(level2.level_name, level.level_name);
        assert_eq!(level2.data_version, level.data_version);
        assert_eq!(level2.game_type, level.game_type);
        assert_eq!(level2.spawn_x, level.spawn_x);
        assert_eq!(level2.spawn_y, level.spawn_y);
        assert_eq!(level2.spawn_z, level.spawn_z);
        assert!((level2.spawn_angle - level.spawn_angle).abs() < f32::EPSILON);
        assert_eq!(level2.time, level.time);
        assert_eq!(level2.day_time, level.day_time);
        assert_eq!(level2.is_raining, level.is_raining);
        assert_eq!(level2.is_thundering, level.is_thundering);
        assert_eq!(level2.rain_time, level.rain_time);
        assert_eq!(level2.thunder_time, level.thunder_time);
        assert_eq!(level2.clear_weather_time, level.clear_weather_time);
        assert_eq!(level2.hardcore, level.hardcore);
        assert_eq!(level2.difficulty, level.difficulty);
        assert_eq!(level2.allow_commands, level.allow_commands);
        assert_eq!(level2.initialized, level.initialized);
        assert_eq!(level2.sea_level, level.sea_level);
    }

    #[test]
    fn test_to_nbt_defaults_roundtrip() {
        let level = PrimaryLevelData::from_nbt(&NbtCompound::new()).unwrap();
        let root_nbt = level.to_nbt();
        let data_nbt = root_nbt.get_compound("Data").unwrap();
        let level2 = PrimaryLevelData::from_nbt(data_nbt).unwrap();

        assert_eq!(level2.level_name, "Unnamed");
        assert_eq!(level2.spawn_y, 64);
        assert_eq!(level2.difficulty, 2);
        assert!(level2.initialized);
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
        assert_eq!(loaded.level_name, "TestWorld");
        assert_eq!(loaded.time, 24000);

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
        level.time = 48000;
        level.save(&path).unwrap();

        assert!(path.exists(), "level.dat should exist");
        assert!(old_path.exists(), "level.dat_old backup should exist");

        // Backup should contain original data
        let backup = PrimaryLevelData::load(&old_path).unwrap();
        assert_eq!(backup.time, 24000, "backup should have original time");

        // New file should contain updated data
        let current = PrimaryLevelData::load(&path).unwrap();
        assert_eq!(current.time, 48000, "current should have updated time");

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

        assert_eq!(loaded.level_name, level.level_name);
        assert_eq!(loaded.time, level.time);
        assert_eq!(loaded.day_time, level.day_time);
        assert_eq!(loaded.is_raining, level.is_raining);
        assert_eq!(loaded.spawn_x, level.spawn_x);
        assert_eq!(loaded.spawn_y, level.spawn_y);
        assert_eq!(loaded.spawn_z, level.spawn_z);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("dat_old"));
    }
}
