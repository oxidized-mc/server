//! World generation framework.
//!
//! Provides the [`ChunkGenerator`] trait, generation status types, and
//! scheduling infrastructure for parallel chunk generation per ADR-016.
//!
//! - [`flat`] — flat world generator (uniform layer-based terrain)
//! - [`priority`] — generation priority levels
//! - [`scheduler`] — dependency-aware scheduler with Rayon thread pool
//! - [`status_requirements`] — vanilla neighbor requirement table

pub mod flat;
pub mod priority;
pub mod scheduler;
pub mod status_requirements;

use oxidized_world::chunk::{ChunkPos, LevelChunk};

pub use priority::ChunkGenPriority;
pub use scheduler::{ChunkGenTask, WorldgenScheduler};
pub use status_requirements::StatusRequirement;

/// Total number of generation statuses in the pipeline.
pub const CHUNK_STATUS_COUNT: usize = 12;

/// Generation status of a chunk, matching vanilla's pipeline.
///
/// Chunks progress through these statuses during generation. Each status
/// has neighbor requirements defined in [`status_requirements::requirements`].
/// For flat worlds, most intermediate statuses are skipped since the terrain
/// is trivially computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ChunkStatus {
    /// No data generated yet.
    Empty = 0,
    /// Structure start positions determined.
    StructureStarts = 1,
    /// Structure references propagated to neighboring chunks.
    StructureReferences = 2,
    /// Biomes assigned.
    Biomes = 3,
    /// Terrain shape (density/noise) computed.
    Noise = 4,
    /// Surface blocks applied (grass, sand, etc.).
    Surface = 5,
    /// Caves and ravines carved.
    Carvers = 6,
    /// Features (trees, ores, structures) placed.
    Features = 7,
    /// Light engine initialized.
    InitializeLight = 8,
    /// Sky and block light fully propagated.
    Light = 9,
    /// Mob spawning positions calculated.
    Spawn = 10,
    /// Chunk is fully generated and ready for use.
    Full = 11,
}

impl ChunkStatus {
    /// All statuses in pipeline order.
    pub const ALL: [Self; CHUNK_STATUS_COUNT] = [
        Self::Empty,
        Self::StructureStarts,
        Self::StructureReferences,
        Self::Biomes,
        Self::Noise,
        Self::Surface,
        Self::Carvers,
        Self::Features,
        Self::InitializeLight,
        Self::Light,
        Self::Spawn,
        Self::Full,
    ];

    /// Returns the vanilla resource key for this status.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Empty => "minecraft:empty",
            Self::StructureStarts => "minecraft:structure_starts",
            Self::StructureReferences => "minecraft:structure_references",
            Self::Biomes => "minecraft:biomes",
            Self::Noise => "minecraft:noise",
            Self::Surface => "minecraft:surface",
            Self::Carvers => "minecraft:carvers",
            Self::Features => "minecraft:features",
            Self::InitializeLight => "minecraft:initialize_light",
            Self::Light => "minecraft:light",
            Self::Spawn => "minecraft:spawn",
            Self::Full => "minecraft:full",
        }
    }

    /// Creates a `ChunkStatus` from its `u8` discriminant.
    ///
    /// Returns `None` if the value does not correspond to a valid status.
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Empty),
            1 => Some(Self::StructureStarts),
            2 => Some(Self::StructureReferences),
            3 => Some(Self::Biomes),
            4 => Some(Self::Noise),
            5 => Some(Self::Surface),
            6 => Some(Self::Carvers),
            7 => Some(Self::Features),
            8 => Some(Self::InitializeLight),
            9 => Some(Self::Light),
            10 => Some(Self::Spawn),
            11 => Some(Self::Full),
            _ => None,
        }
    }

    /// Returns `true` if this status is at or past the given status.
    #[must_use]
    pub const fn is_or_after(self, other: Self) -> bool {
        (self as u8) >= (other as u8)
    }
}

/// Trait for chunk generators.
///
/// Implementations produce fully populated [`LevelChunk`] instances from
/// chunk coordinates. The generator owns its configuration (seed, layers,
/// biome source, etc.) and must be safe to share across threads.
pub trait ChunkGenerator: Send + Sync + std::fmt::Debug {
    /// Generates a complete chunk at the given position.
    ///
    /// The returned chunk must have status [`ChunkStatus::Full`] with
    /// heightmaps computed and all blocks placed.
    fn generate_chunk(&self, pos: ChunkPos) -> LevelChunk;

    /// Returns the Y coordinate where players should spawn.
    ///
    /// For flat worlds this is one block above the topmost layer.
    /// For noise worlds this scans the heightmap at the origin.
    fn find_spawn_y(&self) -> i32;

    /// Returns the generator type identifier (e.g. `"minecraft:flat"`).
    fn generator_type(&self) -> &'static str;

    /// Returns the sea level for this generator's dimension.
    ///
    /// Flat worlds return −63, noise-based overworld returns 63.
    fn sea_level(&self) -> i32;

    /// Returns the minimum Y coordinate for the generated dimension.
    fn min_y(&self) -> i32;

    /// Returns the total world height in blocks.
    fn world_height(&self) -> u32;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn chunk_status_ordering_matches_vanilla_pipeline() {
        assert!(ChunkStatus::Full > ChunkStatus::Empty);
        assert!(ChunkStatus::Noise > ChunkStatus::Biomes);
        assert!(ChunkStatus::Full.is_or_after(ChunkStatus::Full));
        assert!(ChunkStatus::Full.is_or_after(ChunkStatus::Empty));
        assert!(!ChunkStatus::Empty.is_or_after(ChunkStatus::Full));

        // Verify total ordering across all statuses.
        for (i, &a) in ChunkStatus::ALL.iter().enumerate() {
            for (j, &b) in ChunkStatus::ALL.iter().enumerate() {
                if i < j {
                    assert!(a < b, "{a:?} should be less than {b:?}");
                } else if i > j {
                    assert!(a > b, "{a:?} should be greater than {b:?}");
                } else {
                    assert_eq!(a, b);
                }
            }
        }
    }

    #[test]
    fn chunk_status_names() {
        assert_eq!(ChunkStatus::Empty.name(), "minecraft:empty");
        assert_eq!(ChunkStatus::Full.name(), "minecraft:full");
        assert_eq!(ChunkStatus::Noise.name(), "minecraft:noise");
        assert_eq!(
            ChunkStatus::StructureReferences.name(),
            "minecraft:structure_references"
        );
    }

    #[test]
    fn chunk_status_from_u8_roundtrip() {
        for &status in &ChunkStatus::ALL {
            assert_eq!(ChunkStatus::from_u8(status as u8), Some(status));
        }
        assert_eq!(ChunkStatus::from_u8(12), None);
        assert_eq!(ChunkStatus::from_u8(255), None);
    }

    #[test]
    fn chunk_status_all_is_complete() {
        assert_eq!(ChunkStatus::ALL.len(), CHUNK_STATUS_COUNT);
        // Verify ALL is in ascending order.
        for pair in ChunkStatus::ALL.windows(2) {
            assert!(pair[0] < pair[1]);
        }
    }
}
