//! [`ChunkPos`] — an immutable chunk position (x, z) in the world.
//!
//! A chunk is a 16×16 column of blocks. The chunk position is derived
//! from block coordinates by right-shifting by 4.

use std::fmt;

use bytes::{Bytes, BytesMut};

use crate::codec::types::{self, TypeError};

use super::block_pos::BlockPos;

/// A chunk position (x, z) in the world.
///
/// # Wire format
///
/// A single big-endian `i64` (8 bytes):
/// `x` in the lower 32 bits, `z` in the upper 32 bits.
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
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    /// Returns the chunk containing the given [`BlockPos`].
    pub const fn from_block_pos(pos: &BlockPos) -> Self {
        Self {
            x: pos.x >> 4,
            z: pos.z >> 4,
        }
    }

    /// Returns the chunk containing the block at `(block_x, block_z)`.
    pub const fn from_block_coords(block_x: i32, block_z: i32) -> Self {
        Self {
            x: block_x >> 4,
            z: block_z >> 4,
        }
    }

    /// Packs this chunk position into a 64-bit integer.
    ///
    /// Layout: `x` in the lower 32 bits, `z` in the upper 32 bits.
    pub const fn as_long(&self) -> i64 {
        (self.x as i64 & 0xFFFF_FFFF) | ((self.z as i64 & 0xFFFF_FFFF) << 32)
    }

    /// Unpacks a chunk position from a 64-bit integer.
    pub const fn from_long(packed: i64) -> Self {
        Self {
            x: packed as i32,
            z: (packed >> 32) as i32,
        }
    }

    /// Returns the smallest block X coordinate in this chunk.
    pub const fn min_block_x(&self) -> i32 {
        self.x << 4
    }

    /// Returns the smallest block Z coordinate in this chunk.
    pub const fn min_block_z(&self) -> i32 {
        self.z << 4
    }

    /// Returns the largest block X coordinate in this chunk.
    pub const fn max_block_x(&self) -> i32 {
        self.min_block_x() + 15
    }

    /// Returns the largest block Z coordinate in this chunk.
    pub const fn max_block_z(&self) -> i32 {
        self.min_block_z() + 15
    }

    /// Returns the middle block X coordinate in this chunk.
    pub const fn middle_block_x(&self) -> i32 {
        self.min_block_x() + 7
    }

    /// Returns the middle block Z coordinate in this chunk.
    pub const fn middle_block_z(&self) -> i32 {
        self.min_block_z() + 7
    }

    /// Returns the region file X coordinate containing this chunk.
    pub const fn region_x(&self) -> i32 {
        self.x >> 5
    }

    /// Returns the region file Z coordinate containing this chunk.
    pub const fn region_z(&self) -> i32 {
        self.z >> 5
    }

    /// Returns the local X offset within the region file (0–31).
    pub const fn region_local_x(&self) -> i32 {
        self.x & 31
    }

    /// Returns the local Z offset within the region file (0–31).
    pub const fn region_local_z(&self) -> i32 {
        self.z & 31
    }

    /// Returns the Chebyshev (chessboard / L∞) distance to `other`.
    ///
    /// Uses `i64` arithmetic internally to avoid overflow with extreme coordinates.
    pub fn chessboard_distance(&self, other: &ChunkPos) -> i64 {
        let dx = (i64::from(self.x) - i64::from(other.x)).abs();
        let dz = (i64::from(self.z) - i64::from(other.z)).abs();
        dx.max(dz)
    }

    /// Reads a [`ChunkPos`] from a wire buffer (packed `i64`).
    ///
    /// # Errors
    ///
    /// Returns [`TypeError`] if fewer than 8 bytes remain.
    pub fn read(buf: &mut Bytes) -> Result<Self, TypeError> {
        let packed = types::read_i64(buf)?;
        Ok(Self::from_long(packed))
    }

    /// Writes this [`ChunkPos`] to a wire buffer (packed `i64`).
    pub fn write(&self, buf: &mut BytesMut) {
        types::write_i64(buf, self.as_long());
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

    // ── Construction ─────────────────────────────────────────────────

    #[test]
    fn test_chunk_pos_new() {
        let pos = ChunkPos::new(3, 7);
        assert_eq!(pos.x, 3);
        assert_eq!(pos.z, 7);
    }

    #[test]
    fn test_chunk_pos_zero() {
        assert_eq!(ChunkPos::ZERO, ChunkPos::new(0, 0));
    }

    // ── from_block_pos ──────────────────────────────────────────────

    #[test]
    fn test_chunk_pos_from_block_pos_positive() {
        let block = BlockPos::new(32, 64, 48);
        assert_eq!(ChunkPos::from_block_pos(&block), ChunkPos::new(2, 3));
    }

    #[test]
    fn test_chunk_pos_from_block_pos_negative() {
        // -1 >> 4 = -1 in Rust (arithmetic shift)
        let block = BlockPos::new(-1, 0, -1);
        assert_eq!(ChunkPos::from_block_pos(&block), ChunkPos::new(-1, -1));
    }

    #[test]
    fn test_chunk_pos_from_block_pos_boundary_zero() {
        let block = BlockPos::new(0, 0, 0);
        assert_eq!(ChunkPos::from_block_pos(&block), ChunkPos::new(0, 0));
    }

    #[test]
    fn test_chunk_pos_from_block_pos_boundary_15() {
        let block = BlockPos::new(15, 0, 15);
        assert_eq!(ChunkPos::from_block_pos(&block), ChunkPos::new(0, 0));
    }

    #[test]
    fn test_chunk_pos_from_block_pos_boundary_16() {
        let block = BlockPos::new(16, 0, 16);
        assert_eq!(ChunkPos::from_block_pos(&block), ChunkPos::new(1, 1));
    }

    #[test]
    fn test_chunk_pos_from_block_coords() {
        assert_eq!(
            ChunkPos::from_block_coords(32, 48),
            ChunkPos::new(2, 3)
        );
    }

    // ── Pack / unpack roundtrip ─────────────────────────────────────

    #[test]
    fn test_chunk_pos_pack_unpack_zero() {
        let pos = ChunkPos::ZERO;
        assert_eq!(ChunkPos::from_long(pos.as_long()), pos);
    }

    #[test]
    fn test_chunk_pos_pack_unpack_positive() {
        let pos = ChunkPos::new(100, 200);
        assert_eq!(ChunkPos::from_long(pos.as_long()), pos);
    }

    #[test]
    fn test_chunk_pos_pack_unpack_negative() {
        let pos = ChunkPos::new(-100, -200);
        assert_eq!(ChunkPos::from_long(pos.as_long()), pos);
    }

    #[test]
    fn test_chunk_pos_pack_unpack_mixed() {
        let pos = ChunkPos::new(-50, 50);
        assert_eq!(ChunkPos::from_long(pos.as_long()), pos);
    }

    // ── Block ranges ────────────────────────────────────────────────

    #[test]
    fn test_chunk_pos_block_ranges_positive() {
        let pos = ChunkPos::new(2, 3);
        assert_eq!(pos.min_block_x(), 32);
        assert_eq!(pos.min_block_z(), 48);
        assert_eq!(pos.max_block_x(), 47);
        assert_eq!(pos.max_block_z(), 63);
        assert_eq!(pos.middle_block_x(), 39);
        assert_eq!(pos.middle_block_z(), 55);
    }

    #[test]
    fn test_chunk_pos_block_ranges_negative() {
        let pos = ChunkPos::new(-1, -1);
        assert_eq!(pos.min_block_x(), -16);
        assert_eq!(pos.min_block_z(), -16);
        assert_eq!(pos.max_block_x(), -1);
        assert_eq!(pos.max_block_z(), -1);
    }

    // ── Region coordinates ──────────────────────────────────────────

    #[test]
    fn test_chunk_pos_region_coords() {
        let pos = ChunkPos::new(33, 65);
        assert_eq!(pos.region_x(), 1); // 33 >> 5 = 1
        assert_eq!(pos.region_z(), 2); // 65 >> 5 = 2
        assert_eq!(pos.region_local_x(), 1); // 33 & 31 = 1
        assert_eq!(pos.region_local_z(), 1); // 65 & 31 = 1
    }

    #[test]
    fn test_chunk_pos_region_coords_zero() {
        let pos = ChunkPos::ZERO;
        assert_eq!(pos.region_x(), 0);
        assert_eq!(pos.region_z(), 0);
        assert_eq!(pos.region_local_x(), 0);
        assert_eq!(pos.region_local_z(), 0);
    }

    // ── Chessboard distance ─────────────────────────────────────────

    #[test]
    fn test_chunk_pos_chessboard_distance() {
        let a = ChunkPos::new(0, 0);
        let b = ChunkPos::new(3, 7);
        assert_eq!(a.chessboard_distance(&b), 7);
    }

    #[test]
    fn test_chunk_pos_chessboard_distance_same() {
        let pos = ChunkPos::new(5, 5);
        assert_eq!(pos.chessboard_distance(&pos), 0);
    }

    #[test]
    fn test_chunk_pos_chessboard_distance_negative() {
        let a = ChunkPos::new(-3, 2);
        let b = ChunkPos::new(4, -1);
        assert_eq!(a.chessboard_distance(&b), 7); // max(7, 3)
    }

    // ── Display ─────────────────────────────────────────────────────

    #[test]
    fn test_chunk_pos_display() {
        let pos = ChunkPos::new(3, -7);
        assert_eq!(format!("{pos}"), "[3, -7]");
    }

    // ── Wire roundtrip ──────────────────────────────────────────────

    #[test]
    fn test_chunk_pos_wire_roundtrip() {
        let pos = ChunkPos::new(100, -200);
        let mut buf = BytesMut::new();
        pos.write(&mut buf);
        assert_eq!(buf.len(), 8);
        let mut data = buf.freeze();
        let decoded = ChunkPos::read(&mut data).unwrap();
        assert_eq!(decoded, pos);
    }

    #[test]
    fn test_chunk_pos_wire_roundtrip_zero() {
        let pos = ChunkPos::ZERO;
        let mut buf = BytesMut::new();
        pos.write(&mut buf);
        let mut data = buf.freeze();
        let decoded = ChunkPos::read(&mut data).unwrap();
        assert_eq!(decoded, pos);
    }
}
