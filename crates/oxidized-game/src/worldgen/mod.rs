//! World generation framework.
//!
//! Provides the [`ChunkGenerator`] trait and generation status types.
//! The [`flat`] module implements a flat world generator that produces
//! uniform layer-based terrain.

pub mod flat;

use oxidized_world::chunk::{ChunkPos, LevelChunk};

/// Generation status of a chunk, matching vanilla's pipeline.
///
/// Chunks progress through these statuses during generation. For flat
/// worlds, most intermediate statuses are skipped since the terrain is
/// trivially computed.
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

    /// Returns true if this status is at or past the given status.
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
pub trait ChunkGenerator: Send + Sync {
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
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn chunk_status_ordering() {
        assert!(ChunkStatus::Full > ChunkStatus::Empty);
        assert!(ChunkStatus::Noise > ChunkStatus::Biomes);
        assert!(ChunkStatus::Full.is_or_after(ChunkStatus::Full));
        assert!(ChunkStatus::Full.is_or_after(ChunkStatus::Empty));
        assert!(!ChunkStatus::Empty.is_or_after(ChunkStatus::Full));
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
}
