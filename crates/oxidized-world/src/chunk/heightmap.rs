//! Heightmap storage for chunk columns.
//!
//! Stores the highest Y value for each (x, z) column in a compact bit-packed
//! format. Sent as NBT long arrays in the chunk packet.

use super::bit_storage::{BitStorage, BitStorageError};

/// The different heightmap types tracked by the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeightmapType {
    /// Highest non-air block (world generation variant).
    WorldSurfaceWg,
    /// Highest non-air block.
    WorldSurface,
    /// Highest solid block (world generation variant).
    OceanFloorWg,
    /// Highest solid block (ocean floor).
    OceanFloor,
    /// Highest motion-blocking block (solids + fluids).
    MotionBlocking,
    /// Like `MotionBlocking` but excludes leaves.
    MotionBlockingNoLeaves,
}

impl HeightmapType {
    /// Returns the string key used in NBT serialization.
    #[must_use]
    pub const fn nbt_key(self) -> &'static str {
        match self {
            Self::WorldSurfaceWg => "WORLD_SURFACE_WG",
            Self::WorldSurface => "WORLD_SURFACE",
            Self::OceanFloorWg => "OCEAN_FLOOR_WG",
            Self::OceanFloor => "OCEAN_FLOOR",
            Self::MotionBlocking => "MOTION_BLOCKING",
            Self::MotionBlockingNoLeaves => "MOTION_BLOCKING_NO_LEAVES",
        }
    }

    /// The types that must be sent to the client (`Usage.CLIENT` in Java).
    pub const CLIENT_TYPES: &[HeightmapType] = &[
        HeightmapType::MotionBlocking,
        HeightmapType::WorldSurface,
        HeightmapType::MotionBlockingNoLeaves,
    ];
}

/// Heightmap storing the highest Y for each (x, z) column in a chunk.
///
/// Uses compact bit-packed storage with `ceil(log2(world_height + 1))` bits
/// per entry. For the overworld (height 384), this is 9 bits × 256 entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heightmap {
    data: BitStorage,
    heightmap_type: HeightmapType,
}

impl Heightmap {
    /// Creates a new heightmap for the given world height.
    ///
    /// `world_height` is the total number of Y levels (e.g. 384 for overworld).
    ///
    /// # Errors
    ///
    /// Returns an error if the bit storage cannot be allocated.
    pub fn new(heightmap_type: HeightmapType, world_height: u32) -> Result<Self, BitStorageError> {
        let bits = bits_for_height(world_height);
        let data = BitStorage::new(bits, 256)?; // 16×16 columns
        Ok(Self {
            data,
            heightmap_type,
        })
    }

    /// Creates a heightmap from raw long data (from NBT).
    ///
    /// # Errors
    ///
    /// Returns an error if the data length is invalid.
    pub fn from_raw(
        heightmap_type: HeightmapType,
        world_height: u32,
        raw: Vec<u64>,
    ) -> Result<Self, BitStorageError> {
        let bits = bits_for_height(world_height);
        let data = BitStorage::from_raw(bits, 256, raw)?;
        Ok(Self {
            data,
            heightmap_type,
        })
    }

    /// Returns the height at column `(x, z)` (0–15 each).
    ///
    /// # Errors
    ///
    /// Returns an error if coordinates are out of bounds.
    pub fn get(&self, x: usize, z: usize) -> Result<u32, BitStorageError> {
        let index = x + z * 16;
        #[allow(clippy::cast_possible_truncation)]
        Ok(self.data.get(index)? as u32)
    }

    /// Sets the height at column `(x, z)`.
    ///
    /// # Errors
    ///
    /// Returns an error if coordinates are out of bounds.
    pub fn set(&mut self, x: usize, z: usize, height: u32) -> Result<(), BitStorageError> {
        let index = x + z * 16;
        self.data.set(index, u64::from(height))
    }

    /// Returns the heightmap type.
    #[must_use]
    pub fn heightmap_type(&self) -> HeightmapType {
        self.heightmap_type
    }

    /// Returns the raw packed data as a slice of longs (for NBT serialization).
    #[must_use]
    pub fn raw(&self) -> &[u64] {
        self.data.raw()
    }

    /// Returns the raw packed data as `i64` values for NBT long arrays.
    #[must_use]
    pub fn to_nbt_longs(&self) -> Vec<i64> {
        self.data.raw().iter().map(|&v| v as i64).collect()
    }
}

/// Returns the number of bits needed to store a height value for the given
/// world height.
fn bits_for_height(world_height: u32) -> u8 {
    let max_height = world_height + 1;
    #[allow(clippy::cast_possible_truncation)]
    let bits = (u32::BITS - max_height.leading_zeros()) as u8;
    bits.max(1)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_bits_for_height() {
        // Overworld: 384 height → needs 9 bits (max 385)
        assert_eq!(bits_for_height(384), 9);
        // Nether: 128 height → needs 8 bits (max 129)
        assert_eq!(bits_for_height(128), 8);
        // Old style: 256 height → needs 9 bits (max 257)
        assert_eq!(bits_for_height(256), 9);
    }

    #[test]
    fn test_new_all_zeros() {
        let hm = Heightmap::new(HeightmapType::WorldSurface, 384).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                assert_eq!(hm.get(x, z).unwrap(), 0);
            }
        }
    }

    #[test]
    fn test_set_and_get() {
        let mut hm = Heightmap::new(HeightmapType::MotionBlocking, 384).unwrap();
        hm.set(0, 0, 64).unwrap();
        hm.set(15, 15, 320).unwrap();
        assert_eq!(hm.get(0, 0).unwrap(), 64);
        assert_eq!(hm.get(15, 15).unwrap(), 320);
    }

    #[test]
    fn test_nbt_longs_roundtrip() {
        let mut hm = Heightmap::new(HeightmapType::WorldSurface, 384).unwrap();
        hm.set(5, 10, 200).unwrap();

        let longs = hm.to_nbt_longs();
        let raw: Vec<u64> = longs.iter().map(|&v| v as u64).collect();

        let hm2 = Heightmap::from_raw(HeightmapType::WorldSurface, 384, raw).unwrap();
        assert_eq!(hm2.get(5, 10).unwrap(), 200);
    }

    #[test]
    fn test_client_types() {
        assert_eq!(HeightmapType::CLIENT_TYPES.len(), 3);
    }
}
