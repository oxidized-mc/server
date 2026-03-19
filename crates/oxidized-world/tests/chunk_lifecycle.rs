//! Integration tests for chunk lifecycle through the public API.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use oxidized_world::chunk::{
    Heightmap, HeightmapType, LevelChunk, LevelChunkSection, ChunkPos,
};
use oxidized_world::chunk::level_chunk::OVERWORLD_HEIGHT;

#[test]
fn test_new_chunk_is_all_air() {
    let chunk = LevelChunk::new(ChunkPos::new(0, 0));
    assert_eq!(chunk.get_block_state(0, 0, 0).unwrap(), 0);

    for i in 0..chunk.section_count() {
        assert!(
            chunk.section(i).unwrap().is_empty(),
            "section {i} should be empty"
        );
    }
}

#[test]
fn test_set_and_get_block_state() {
    let mut chunk = LevelChunk::new(ChunkPos::new(0, 0));
    let old = chunk.set_block_state(1, 64, 1, 1).unwrap();
    assert_eq!(old, 0, "previous state should be air");

    assert_eq!(chunk.get_block_state(1, 64, 1).unwrap(), 1);
    assert_eq!(
        chunk.get_block_state(0, 64, 0).unwrap(),
        0,
        "neighbor should still be air"
    );
}

#[test]
fn test_section_allocation_on_set() {
    let mut chunk = LevelChunk::new(ChunkPos::new(0, 0));
    // y=0 → section index = (0 - (-64)) / 16 = 4
    chunk.set_block_state(5, 0, 5, 42).unwrap();

    let section4 = chunk.section(4).unwrap();
    assert_eq!(section4.non_empty_block_count(), 1);

    for i in 0..chunk.section_count() {
        if i != 4 {
            assert!(
                chunk.section(i).unwrap().is_empty(),
                "section {i} should still be empty"
            );
        }
    }
}

#[test]
fn test_chunk_sections_serialization() {
    let mut chunk = LevelChunk::new(ChunkPos::new(0, 0));
    // Set blocks in different sections
    chunk.set_block_state(0, -64, 0, 1).unwrap(); // section 0
    chunk.set_block_state(0, 0, 0, 2).unwrap(); // section 4
    chunk.set_block_state(0, 319, 0, 3).unwrap(); // section 23

    let bytes = chunk.write_sections_to_bytes();
    assert!(!bytes.is_empty());
    // Each section serializes at minimum: 2 (non_empty) + 2 (fluid)
    // + states container + biomes container. 24 sections total.
    assert!(
        bytes.len() > 24 * 4,
        "serialized data should be larger than header-only: got {} bytes",
        bytes.len()
    );
}

#[test]
fn test_section_write_read_roundtrip() {
    let mut section = LevelChunkSection::new();
    section.set_block_state(0, 0, 0, 1).unwrap();
    section.set_block_state(5, 5, 5, 42).unwrap();
    section.set_block_state(15, 15, 15, 7).unwrap();
    section.set_block_state(8, 0, 8, 100).unwrap();
    section.set_biome(0, 0, 0, 3).unwrap();
    section.set_biome(3, 3, 3, 10).unwrap();

    let bytes = section.write_to_bytes();
    let mut cursor = bytes.as_slice();
    let section2 = LevelChunkSection::read_from_bytes(&mut cursor).unwrap();

    assert_eq!(section2.get_block_state(0, 0, 0).unwrap(), 1);
    assert_eq!(section2.get_block_state(5, 5, 5).unwrap(), 42);
    assert_eq!(section2.get_block_state(15, 15, 15).unwrap(), 7);
    assert_eq!(section2.get_block_state(8, 0, 8).unwrap(), 100);
    assert_eq!(section2.get_biome(0, 0, 0).unwrap(), 3);
    assert_eq!(section2.get_biome(3, 3, 3).unwrap(), 10);
    assert_eq!(
        section2.non_empty_block_count(),
        section.non_empty_block_count()
    );
}

#[test]
fn test_heightmap_set_get() {
    let mut hm = Heightmap::new(HeightmapType::WorldSurface, OVERWORLD_HEIGHT).unwrap();
    hm.set(0, 0, 100).unwrap();
    hm.set(15, 15, 200).unwrap();

    assert_eq!(hm.get(0, 0).unwrap(), 100);
    assert_eq!(hm.get(15, 15).unwrap(), 200);
}

#[test]
fn test_heightmap_raw_roundtrip() {
    let mut hm = Heightmap::new(HeightmapType::MotionBlocking, OVERWORLD_HEIGHT).unwrap();
    hm.set(0, 0, 50).unwrap();
    hm.set(7, 7, 150).unwrap();
    hm.set(15, 15, 384).unwrap();
    hm.set(3, 12, 0).unwrap();

    let raw = hm.raw().to_vec();
    let hm2 = Heightmap::from_raw(HeightmapType::MotionBlocking, OVERWORLD_HEIGHT, raw).unwrap();

    for x in 0..16 {
        for z in 0..16 {
            assert_eq!(
                hm.get(x, z).unwrap(),
                hm2.get(x, z).unwrap(),
                "mismatch at ({x}, {z})"
            );
        }
    }
}

#[test]
fn test_chunk_out_of_bounds() {
    let mut chunk = LevelChunk::new(ChunkPos::new(0, 0));

    // y out of bounds
    assert!(chunk.get_block_state(0, 320, 0).is_err());
    assert!(chunk.get_block_state(0, -65, 0).is_err());
    assert!(chunk.set_block_state(0, 320, 0, 1).is_err());
    assert!(chunk.set_block_state(0, -65, 0, 1).is_err());

    // x and z use & 15 masking internally (chunk-local), so x=16 wraps.
    // Only y is truly bounds-checked against section range.
    // Verify values just beyond the valid range are rejected:
    assert!(chunk.get_block_state(0, 500, 0).is_err());
    assert!(chunk.get_block_state(0, -100, 0).is_err());
}

#[test]
fn test_multiple_blocks_same_section() {
    let mut section = LevelChunkSection::new();
    let mut expected = Vec::new();

    for i in 0..100u32 {
        let x = (i % 16) as usize;
        let y = ((i / 16) % 16) as usize;
        let z = ((i / 256) % 16) as usize;
        let state = i + 1;
        section.set_block_state(x, y, z, state).unwrap();
        expected.push((x, y, z, state));
    }

    assert_eq!(section.non_empty_block_count(), 100);

    for (x, y, z, state) in &expected {
        assert_eq!(
            section.get_block_state(*x, *y, *z).unwrap(),
            *state,
            "mismatch at ({x}, {y}, {z})"
        );
    }
}

#[test]
fn test_section_biome_set_get() {
    let mut section = LevelChunkSection::new();

    section.set_biome(0, 0, 0, 1).unwrap();
    assert_eq!(section.get_biome(0, 0, 0).unwrap(), 1);

    section.set_biome(3, 3, 3, 7).unwrap();
    assert_eq!(section.get_biome(3, 3, 3).unwrap(), 7);

    // Other biome cells should still be 0
    assert_eq!(section.get_biome(1, 1, 1).unwrap(), 0);

    // Out of bounds for biome coordinates (4×4×4 grid)
    assert!(section.get_biome(4, 0, 0).is_err());
    assert!(section.set_biome(0, 4, 0, 1).is_err());
}
