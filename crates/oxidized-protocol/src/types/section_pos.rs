//! [`SectionPos`] — an immutable chunk section position (x, y, z).
//!
//! Each component is in section space, i.e. `block_coord >> 4`.
//! The packed 64-bit layout uses 22 bits for X, 22 bits for Z, and 20 bits for Y.

use std::fmt;

use bytes::{Bytes, BytesMut};

use crate::codec::types::{self, TypeError};

use super::block_pos::BlockPos;
use super::chunk_pos::ChunkPos;
use super::vec3i::Vec3i;

/// Number of bits per axis within a section (log2(16) = 4).
pub const SECTION_BITS: u32 = 4;

/// Number of blocks along one axis of a section.
pub const SECTION_SIZE: i32 = 16;

/// Bitmask for converting a block coordinate to a section-local offset.
pub const SECTION_MASK: i32 = 15;

/// Half the section size (used for center calculations).
pub const SECTION_HALF_SIZE: i32 = 8;

/// Number of bits used for the packed Y coordinate.
const PACKED_Y_LENGTH: u32 = 20;

/// Number of bits used for the packed X and Z coordinates.
const PACKED_XZ_LENGTH: u32 = 22;

/// Bit mask for a packed Y coordinate.
const PACKED_Y_MASK: i64 = (1_i64 << PACKED_Y_LENGTH) - 1;

/// Bit mask for a packed X or Z coordinate.
const PACKED_XZ_MASK: i64 = (1_i64 << PACKED_XZ_LENGTH) - 1;

/// Bit offset of the Z coordinate in the packed representation.
const Z_OFFSET: u32 = PACKED_Y_LENGTH;

/// Bit offset of the X coordinate in the packed representation.
const X_OFFSET: u32 = Z_OFFSET + PACKED_XZ_LENGTH;

/// A chunk section position (x, y, z) in section space.
///
/// Section coordinates are derived from block coordinates by right-shifting
/// by 4 (dividing by 16). Each section is a 16×16×16 cube of blocks.
///
/// # Wire format
///
/// A single big-endian `i64` (8 bytes):
///
/// | Field | Bits | Width |
/// |-------|------|-------|
/// | X | 42–63 | 22 |
/// | Z | 20–41 | 22 |
/// | Y | 0–19 | 20 |
///
/// # Examples
///
/// ```
/// use oxidized_protocol::types::{SectionPos, BlockPos};
///
/// // Create from section coordinates
/// let section = SectionPos::new(2, -4, 3);
/// assert_eq!(section.min_block_y(), -64);
///
/// // Derive from a block position
/// let block = BlockPos::new(32, -64, 48);
/// let section = SectionPos::of_block_pos(&block);
/// assert_eq!(section, SectionPos::new(2, -4, 3));
///
/// // Pack/unpack roundtrip
/// let packed = section.as_long();
/// assert_eq!(SectionPos::from_long(packed), section);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SectionPos {
    /// The section X coordinate.
    pub x: i32,
    /// The section Y coordinate.
    pub y: i32,
    /// The section Z coordinate.
    pub z: i32,
}

impl SectionPos {
    /// Creates a new [`SectionPos`].
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Returns the section containing the given [`BlockPos`].
    pub const fn of_block_pos(pos: &BlockPos) -> Self {
        Self {
            x: pos.x >> 4,
            y: pos.y >> 4,
            z: pos.z >> 4,
        }
    }

    /// Returns the section at the given Y level within the given chunk.
    pub const fn of_chunk(chunk: &ChunkPos, section_y: i32) -> Self {
        Self {
            x: chunk.x,
            y: section_y,
            z: chunk.z,
        }
    }

    /// Packs this section position into a 64-bit integer.
    pub const fn as_long(&self) -> i64 {
        ((self.x as i64 & PACKED_XZ_MASK) << X_OFFSET)
            | ((self.z as i64 & PACKED_XZ_MASK) << Z_OFFSET)
            | (self.y as i64 & PACKED_Y_MASK)
    }

    /// Unpacks a section position from a 64-bit integer.
    ///
    /// Sign extension is applied to recover negative coordinates.
    pub const fn from_long(packed: i64) -> Self {
        let x = (packed >> X_OFFSET) as i32;
        let z =
            ((packed << PACKED_XZ_LENGTH as i64) >> (PACKED_XZ_LENGTH + Z_OFFSET) as i64) as i32;
        let y = ((packed << (64 - PACKED_Y_LENGTH) as i64) >> (64 - PACKED_Y_LENGTH) as i64) as i32;
        Self { x, y, z }
    }

    /// Converts a block coordinate to a section coordinate.
    pub const fn block_to_section_coord(block_coord: i32) -> i32 {
        block_coord >> 4
    }

    /// Converts a section coordinate to the minimum block coordinate.
    pub const fn section_to_block_coord(section_coord: i32) -> i32 {
        section_coord << 4
    }

    /// Returns the section-local offset (0–15) for a block coordinate.
    pub const fn section_relative(block_coord: i32) -> i32 {
        block_coord & SECTION_MASK
    }

    /// Returns the smallest block X coordinate in this section.
    pub const fn min_block_x(&self) -> i32 {
        Self::section_to_block_coord(self.x)
    }

    /// Returns the smallest block Y coordinate in this section.
    pub const fn min_block_y(&self) -> i32 {
        Self::section_to_block_coord(self.y)
    }

    /// Returns the smallest block Z coordinate in this section.
    pub const fn min_block_z(&self) -> i32 {
        Self::section_to_block_coord(self.z)
    }

    /// Returns the largest block X coordinate in this section.
    pub const fn max_block_x(&self) -> i32 {
        self.min_block_x() + 15
    }

    /// Returns the largest block Y coordinate in this section.
    pub const fn max_block_y(&self) -> i32 {
        self.min_block_y() + 15
    }

    /// Returns the largest block Z coordinate in this section.
    pub const fn max_block_z(&self) -> i32 {
        self.min_block_z() + 15
    }

    /// Returns the center block position of this section.
    pub const fn center(&self) -> BlockPos {
        BlockPos::new(
            self.min_block_x() + SECTION_HALF_SIZE,
            self.min_block_y() + SECTION_HALF_SIZE,
            self.min_block_z() + SECTION_HALF_SIZE,
        )
    }

    /// Returns the origin (minimum corner) block position of this section.
    pub const fn origin(&self) -> BlockPos {
        BlockPos::new(self.min_block_x(), self.min_block_y(), self.min_block_z())
    }

    /// Returns the chunk position for this section (discards the Y component).
    pub const fn chunk(&self) -> ChunkPos {
        ChunkPos::new(self.x, self.z)
    }

    /// Converts this section position to a [`Vec3i`].
    pub const fn as_vec3i(&self) -> Vec3i {
        Vec3i::new(self.x, self.y, self.z)
    }

    /// Reads a [`SectionPos`] from a wire buffer (packed `i64`).
    ///
    /// # Errors
    ///
    /// Returns [`TypeError`] if fewer than 8 bytes remain.
    pub fn read(buf: &mut Bytes) -> Result<Self, TypeError> {
        let packed = types::read_i64(buf)?;
        Ok(Self::from_long(packed))
    }

    /// Writes this [`SectionPos`] to a wire buffer (packed `i64`).
    pub fn write(&self, buf: &mut BytesMut) {
        types::write_i64(buf, self.as_long());
    }
}

impl fmt::Display for SectionPos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ── Construction ─────────────────────────────────────────────────

    #[test]
    fn test_section_pos_new() {
        let pos = SectionPos::new(1, 2, 3);
        assert_eq!(pos.x, 1);
        assert_eq!(pos.y, 2);
        assert_eq!(pos.z, 3);
    }

    #[test]
    fn test_section_pos_of_block_pos() {
        let block = BlockPos::new(32, -64, 48);
        let section = SectionPos::of_block_pos(&block);
        assert_eq!(section, SectionPos::new(2, -4, 3));
    }

    #[test]
    fn test_section_pos_of_block_pos_negative() {
        // -1 >> 4 = -1 (arithmetic shift)
        let block = BlockPos::new(-1, -1, -1);
        let section = SectionPos::of_block_pos(&block);
        assert_eq!(section, SectionPos::new(-1, -1, -1));
    }

    #[test]
    fn test_section_pos_of_chunk() {
        let chunk = ChunkPos::new(5, 10);
        let section = SectionPos::of_chunk(&chunk, -4);
        assert_eq!(section, SectionPos::new(5, -4, 10));
    }

    // ── Pack / unpack roundtrip ─────────────────────────────────────

    #[test]
    fn test_section_pos_pack_unpack_zero() {
        let pos = SectionPos::new(0, 0, 0);
        assert_eq!(SectionPos::from_long(pos.as_long()), pos);
    }

    #[test]
    fn test_section_pos_pack_unpack_positive() {
        let pos = SectionPos::new(100, 10, 200);
        assert_eq!(SectionPos::from_long(pos.as_long()), pos);
    }

    #[test]
    fn test_section_pos_pack_unpack_negative() {
        let pos = SectionPos::new(-100, -4, -200);
        assert_eq!(SectionPos::from_long(pos.as_long()), pos);
    }

    #[test]
    fn test_section_pos_pack_unpack_mixed() {
        let pos = SectionPos::new(-50, 19, 50);
        assert_eq!(SectionPos::from_long(pos.as_long()), pos);
    }

    // ── Coordinate conversions ──────────────────────────────────────

    #[test]
    fn test_block_to_section_coord() {
        assert_eq!(SectionPos::block_to_section_coord(0), 0);
        assert_eq!(SectionPos::block_to_section_coord(15), 0);
        assert_eq!(SectionPos::block_to_section_coord(16), 1);
        assert_eq!(SectionPos::block_to_section_coord(-1), -1);
        assert_eq!(SectionPos::block_to_section_coord(-16), -1);
        assert_eq!(SectionPos::block_to_section_coord(-17), -2);
    }

    #[test]
    fn test_section_to_block_coord() {
        assert_eq!(SectionPos::section_to_block_coord(0), 0);
        assert_eq!(SectionPos::section_to_block_coord(1), 16);
        assert_eq!(SectionPos::section_to_block_coord(-1), -16);
    }

    #[test]
    fn test_block_to_section_roundtrip() {
        // section_to_block(block_to_section(x)) gives the min block of that section
        for block in [-17, -16, -1, 0, 15, 16, 31, 32] {
            let section = SectionPos::block_to_section_coord(block);
            let min = SectionPos::section_to_block_coord(section);
            assert!(min <= block);
            assert!(block < min + SECTION_SIZE);
        }
    }

    #[test]
    fn test_section_relative() {
        assert_eq!(SectionPos::section_relative(0), 0);
        assert_eq!(SectionPos::section_relative(7), 7);
        assert_eq!(SectionPos::section_relative(15), 15);
        assert_eq!(SectionPos::section_relative(16), 0);
        assert_eq!(SectionPos::section_relative(17), 1);
    }

    // ── Block ranges ────────────────────────────────────────────────

    #[test]
    fn test_section_pos_block_ranges() {
        let pos = SectionPos::new(2, -4, 3);
        assert_eq!(pos.min_block_x(), 32);
        assert_eq!(pos.min_block_y(), -64);
        assert_eq!(pos.min_block_z(), 48);
        assert_eq!(pos.max_block_x(), 47);
        assert_eq!(pos.max_block_y(), -49);
        assert_eq!(pos.max_block_z(), 63);
    }

    // ── Center / origin ─────────────────────────────────────────────

    #[test]
    fn test_section_pos_center() {
        let pos = SectionPos::new(0, 0, 0);
        assert_eq!(pos.center(), BlockPos::new(8, 8, 8));
    }

    #[test]
    fn test_section_pos_center_negative() {
        let pos = SectionPos::new(-1, -1, -1);
        assert_eq!(pos.center(), BlockPos::new(-8, -8, -8));
    }

    #[test]
    fn test_section_pos_origin() {
        let pos = SectionPos::new(2, -4, 3);
        assert_eq!(pos.origin(), BlockPos::new(32, -64, 48));
    }

    // ── Chunk conversion ────────────────────────────────────────────

    #[test]
    fn test_section_pos_chunk() {
        let section = SectionPos::new(5, -4, 10);
        assert_eq!(section.chunk(), ChunkPos::new(5, 10));
    }

    // ── as_vec3i ────────────────────────────────────────────────────

    #[test]
    fn test_section_pos_as_vec3i() {
        let pos = SectionPos::new(1, 2, 3);
        assert_eq!(pos.as_vec3i(), Vec3i::new(1, 2, 3));
    }

    // ── Display ─────────────────────────────────────────────────────

    #[test]
    fn test_section_pos_display() {
        let pos = SectionPos::new(1, -2, 3);
        assert_eq!(format!("{pos}"), "(1, -2, 3)");
    }

    // ── Wire roundtrip ──────────────────────────────────────────────

    #[test]
    fn test_section_pos_wire_roundtrip() {
        let pos = SectionPos::new(100, -4, -200);
        let mut buf = BytesMut::new();
        pos.write(&mut buf);
        assert_eq!(buf.len(), 8);
        let mut data = buf.freeze();
        let decoded = SectionPos::read(&mut data).unwrap();
        assert_eq!(decoded, pos);
    }

    #[test]
    fn test_section_pos_wire_roundtrip_zero() {
        let pos = SectionPos::new(0, 0, 0);
        let mut buf = BytesMut::new();
        pos.write(&mut buf);
        let mut data = buf.freeze();
        let decoded = SectionPos::read(&mut data).unwrap();
        assert_eq!(decoded, pos);
    }
}
