//! [`ChunkPos`] — an immutable chunk position (x, z) in the world.
//!
//! A chunk is a 16×16 column of blocks. The chunk position is derived
//! from block coordinates by right-shifting by 4.

use std::fmt;

/// A chunk position (x, z) in the world.
///
/// # Examples
///
/// ```
/// use oxidized_types::ChunkPos;
///
/// // Create from chunk coordinates
/// let chunk = ChunkPos::new(2, 3);
/// assert_eq!(chunk.min_block_x(), 32);
/// assert_eq!(chunk.min_block_z(), 48);
///
/// // Derive from block coordinates
/// let chunk = ChunkPos::from_block_coords(100, 200);
/// assert_eq!(chunk, ChunkPos::new(6, 12));
///
/// // Pack/unpack roundtrip
/// let packed = chunk.as_long();
/// assert_eq!(ChunkPos::from_long(packed), chunk);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkPos {
    /// The chunk X coordinate.
    pub x: i32,
    /// The chunk Z coordinate.
    pub z: i32,
}

impl ChunkPos {
    /// The origin chunk `(0, 0)`.
    pub const ZERO: ChunkPos = ChunkPos { x: 0, z: 0 };

    /// Number of chunks per region file axis.
    pub const REGION_SIZE: i32 = 32;

    /// Creates a new [`ChunkPos`].
    #[must_use]
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    /// Returns the chunk containing the block at `(block_x, block_z)`.
    #[must_use]
    pub const fn from_block_coords(block_x: i32, block_z: i32) -> Self {
        Self {
            x: block_x >> 4,
            z: block_z >> 4,
        }
    }

    /// Packs this chunk position into a 64-bit integer.
    ///
    /// Layout: `x` in the lower 32 bits, `z` in the upper 32 bits.
    #[must_use]
    pub const fn as_long(&self) -> i64 {
        (self.x as i64 & 0xFFFF_FFFF) | ((self.z as i64 & 0xFFFF_FFFF) << 32)
    }

    /// Unpacks a chunk position from a 64-bit integer.
    #[must_use]
    pub const fn from_long(packed: i64) -> Self {
        Self {
            x: packed as i32,
            z: (packed >> 32) as i32,
        }
    }

    /// Returns the smallest block X coordinate in this chunk.
    #[must_use]
    pub const fn min_block_x(&self) -> i32 {
        self.x << 4
    }

    /// Returns the smallest block Z coordinate in this chunk.
    #[must_use]
    pub const fn min_block_z(&self) -> i32 {
        self.z << 4
    }

    /// Returns the largest block X coordinate in this chunk.
    #[must_use]
    pub const fn max_block_x(&self) -> i32 {
        self.min_block_x() + 15
    }

    /// Returns the largest block Z coordinate in this chunk.
    #[must_use]
    pub const fn max_block_z(&self) -> i32 {
        self.min_block_z() + 15
    }

    /// Returns the middle block X coordinate in this chunk.
    #[must_use]
    pub const fn middle_block_x(&self) -> i32 {
        self.min_block_x() + 7
    }

    /// Returns the middle block Z coordinate in this chunk.
    #[must_use]
    pub const fn middle_block_z(&self) -> i32 {
        self.min_block_z() + 7
    }

    /// Returns the region file X coordinate containing this chunk.
    #[must_use]
    pub const fn region_x(&self) -> i32 {
        self.x >> 5
    }

    /// Returns the region file Z coordinate containing this chunk.
    #[must_use]
    pub const fn region_z(&self) -> i32 {
        self.z >> 5
    }

    /// Returns the local X offset within the region file (0–31).
    #[must_use]
    pub const fn region_local_x(&self) -> i32 {
        self.x & 31
    }

    /// Returns the local Z offset within the region file (0–31).
    #[must_use]
    pub const fn region_local_z(&self) -> i32 {
        self.z & 31
    }

    /// Returns the Chebyshev (chessboard / L∞) distance to `other`.
    ///
    /// Uses `i64` arithmetic internally to avoid overflow with extreme coordinates.
    #[must_use]
    pub fn chessboard_distance(&self, other: &ChunkPos) -> i64 {
        let dx = (i64::from(self.x) - i64::from(other.x)).abs();
        let dz = (i64::from(self.z) - i64::from(other.z)).abs();
        dx.max(dz)
    }
}

impl fmt::Display for ChunkPos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {}]", self.x, self.z)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let pos = ChunkPos::new(3, 7);
        assert_eq!(pos.x, 3);
        assert_eq!(pos.z, 7);
    }

    #[test]
    fn test_zero() {
        assert_eq!(ChunkPos::ZERO, ChunkPos::new(0, 0));
    }

    #[test]
    fn test_from_block_coords_positive() {
        assert_eq!(ChunkPos::from_block_coords(32, 48), ChunkPos::new(2, 3));
    }

    #[test]
    fn test_from_block_coords_negative() {
        assert_eq!(ChunkPos::from_block_coords(-1, -1), ChunkPos::new(-1, -1));
    }

    #[test]
    fn test_pack_unpack_roundtrip() {
        for (x, z) in [(0, 0), (100, 200), (-100, -200), (-50, 50)] {
            let pos = ChunkPos::new(x, z);
            assert_eq!(ChunkPos::from_long(pos.as_long()), pos);
        }
    }

    #[test]
    fn test_block_ranges() {
        let pos = ChunkPos::new(2, 3);
        assert_eq!(pos.min_block_x(), 32);
        assert_eq!(pos.min_block_z(), 48);
        assert_eq!(pos.max_block_x(), 47);
        assert_eq!(pos.max_block_z(), 63);
        assert_eq!(pos.middle_block_x(), 39);
        assert_eq!(pos.middle_block_z(), 55);
    }

    #[test]
    fn test_region_coords() {
        let pos = ChunkPos::new(33, 65);
        assert_eq!(pos.region_x(), 1);
        assert_eq!(pos.region_z(), 2);
        assert_eq!(pos.region_local_x(), 1);
        assert_eq!(pos.region_local_z(), 1);
    }

    #[test]
    fn test_chessboard_distance() {
        let a = ChunkPos::new(0, 0);
        let b = ChunkPos::new(3, 7);
        assert_eq!(a.chessboard_distance(&b), 7);
        assert_eq!(a.chessboard_distance(&a), 0);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", ChunkPos::new(3, -7)), "[3, -7]");
    }
}
