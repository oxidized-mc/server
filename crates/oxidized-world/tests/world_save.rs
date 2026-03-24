//! Integration tests for world saving (Phase 20).
//!
//! Tests the full save pipeline through the public API:
//! chunk creation → serialization → compression → region write → read → decompress → deserialize.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use oxidized_types::ChunkPos;
use oxidized_world::anvil::{AnvilChunkLoader, ChunkSerializer, RegionFile, compress_zlib};
use oxidized_world::chunk::LevelChunk;
use oxidized_world::registry::BlockRegistry;
use oxidized_world::storage::{DirtyChunkTracker, PrimaryLevelData};

fn test_registry() -> Arc<BlockRegistry> {
    Arc::new(BlockRegistry::load().unwrap())
}

/// Full roundtrip: create chunk → serialize → compress → write region → loader.load_chunk → compare.
#[test]
fn test_chunk_save_roundtrip_through_region_file() {
    let registry = test_registry();
    let serializer = ChunkSerializer::new(Arc::clone(&registry));

    // Build a chunk with some blocks.
    let mut chunk = LevelChunk::new(ChunkPos::new(3, 7));
    let stone_id = registry
        .get_block_def("minecraft:stone")
        .unwrap()
        .default_state as u32;
    let dirt_id = registry
        .get_block_def("minecraft:dirt")
        .unwrap()
        .default_state as u32;

    // Place blocks at various heights.
    for x in 0..16 {
        for z in 0..16 {
            chunk.set_block_state(x, 0, z, stone_id).unwrap();
        }
    }
    chunk.set_block_state(8, 64, 8, dirt_id).unwrap();
    chunk.set_block_state(0, -60, 0, stone_id).unwrap();

    // Serialize → compress.
    let nbt_bytes = serializer.serialize(&chunk).unwrap();
    let compressed = compress_zlib(&nbt_bytes).unwrap();

    // Write to a region file.
    let dir = tempfile::tempdir().unwrap();
    // Chunk (3,7) → region (0,0), so file must be named r.0.0.mca for the loader.
    let region_dir = dir.path();
    let region_path = region_dir.join("r.0.0.mca");
    let timestamp = 1_700_000_000;

    {
        let mut region = RegionFile::create(&region_path).unwrap();
        region
            .write_chunk_data(3, 7, &compressed, timestamp)
            .unwrap();
    }

    // Read back through AnvilChunkLoader (tests the full public API path).
    let mut loader = AnvilChunkLoader::new(region_dir, Arc::clone(&registry));
    let loaded = loader.load_chunk(3, 7).unwrap().unwrap();

    // Verify block states survived the roundtrip.
    assert_eq!(loaded.pos, ChunkPos::new(3, 7));

    for x in 0..16 {
        for z in 0..16 {
            assert_eq!(
                loaded.get_block_state(x, 0, z).unwrap(),
                stone_id,
                "stone floor at ({x}, 0, {z})"
            );
        }
    }
    assert_eq!(loaded.get_block_state(8, 64, 8).unwrap(), dirt_id);
    assert_eq!(loaded.get_block_state(0, -60, 0).unwrap(), stone_id);

    // Verify air is still air.
    assert_eq!(loaded.get_block_state(0, 100, 0).unwrap(), 0);

    // Verify timestamp persists.
    let region = RegionFile::open(&region_path).unwrap();
    assert_eq!(region.chunk_timestamp(3, 7), timestamp);
}

/// Multiple chunks written to the same region file.
#[test]
fn test_multiple_chunks_in_region() {
    let registry = test_registry();
    let serializer = ChunkSerializer::new(Arc::clone(&registry));

    let dir = tempfile::tempdir().unwrap();
    let region_path = dir.path().join("r.0.0.mca");

    let stone_id = registry
        .get_block_def("minecraft:stone")
        .unwrap()
        .default_state as u32;

    let coords: [(i32, i32); 4] = [(0, 0), (1, 0), (0, 1), (15, 15)];

    {
        let mut region = RegionFile::create(&region_path).unwrap();
        for (cx, cz) in coords {
            let mut chunk = LevelChunk::new(ChunkPos::new(cx, cz));
            // Place a marker block at a unique position within the chunk.
            chunk.set_block_state(cx, 0, cz, stone_id).unwrap();

            let nbt_bytes = serializer.serialize(&chunk).unwrap();
            let compressed = compress_zlib(&nbt_bytes).unwrap();
            region.write_chunk_data(cx, cz, &compressed, 0).unwrap();
        }
    }

    // Read back all chunks through the loader.
    let mut loader = AnvilChunkLoader::new(dir.path(), Arc::clone(&registry));

    for (cx, cz) in coords {
        let chunk = loader.load_chunk(cx, cz).unwrap().unwrap();
        assert_eq!(chunk.pos, ChunkPos::new(cx, cz));
        assert_eq!(
            chunk.get_block_state(cx, 0, cz).unwrap(),
            stone_id,
            "marker block at chunk ({cx}, {cz})"
        );
    }
}

/// DirtyChunkTracker integration: mark, drain, verify empty.
#[test]
fn test_dirty_tracker_save_workflow() {
    let mut tracker = DirtyChunkTracker::new();

    // Simulate modifying some chunks.
    tracker.mark_dirty(ChunkPos::new(0, 0));
    tracker.mark_dirty(ChunkPos::new(1, 1));
    tracker.mark_dirty(ChunkPos::new(0, 0)); // duplicate

    assert_eq!(tracker.dirty_count(), 2);

    // Drain and "save" them.
    let dirty: Vec<ChunkPos> = tracker.drain_dirty().collect();
    assert_eq!(dirty.len(), 2);
    assert_eq!(tracker.dirty_count(), 0);
}

/// level.dat save and reload roundtrip.
#[test]
fn test_level_dat_save_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let level_dat_path = dir.path().join("level.dat");

    // Create level data with non-default values.
    let mut nbt = oxidized_nbt::NbtCompound::new();
    nbt.put_string("LevelName", "IntegrationTest");
    nbt.put_int("SpawnX", 100);
    nbt.put_int("SpawnY", 64);
    nbt.put_int("SpawnZ", -200);
    nbt.put_long("DayTime", 12_000);
    nbt.put_long("Time", 24_000);
    nbt.put_byte("raining", 1);
    nbt.put_int("rainTime", 5000);
    nbt.put_byte("thundering", 0);
    nbt.put_int("thunderTime", 10_000);

    let original = PrimaryLevelData::from_nbt(&nbt).unwrap();

    // Save.
    original.save(&level_dat_path).unwrap();
    assert!(level_dat_path.exists());

    // Reload.
    let loaded_nbt = oxidized_nbt::read_file(&level_dat_path).unwrap();
    // The file is wrapped in a root compound with "Data" key.
    let data_nbt = loaded_nbt.get_compound("Data").unwrap();
    let loaded = PrimaryLevelData::from_nbt(data_nbt).unwrap();

    assert_eq!(loaded.level_name, "IntegrationTest");
    assert_eq!(loaded.spawn_x, 100);
    assert_eq!(loaded.spawn_y, 64);
    assert_eq!(loaded.spawn_z, -200);
    assert_eq!(loaded.day_time, 12_000);
    assert_eq!(loaded.time, 24_000);
    assert!(loaded.is_raining);
    assert_eq!(loaded.rain_time, 5000);
}

/// Backup pattern: saving twice creates level.dat_old.
#[test]
fn test_level_dat_backup_pattern() {
    let dir = tempfile::tempdir().unwrap();
    let level_dat_path = dir.path().join("level.dat");

    let nbt = oxidized_nbt::NbtCompound::new();
    let data = PrimaryLevelData::from_nbt(&nbt).unwrap();

    // First save.
    data.save(&level_dat_path).unwrap();
    assert!(level_dat_path.exists());
    assert!(!dir.path().join("level.dat_old").exists());

    // Second save should create backup.
    data.save(&level_dat_path).unwrap();
    assert!(level_dat_path.exists());
    assert!(
        dir.path().join("level.dat_old").exists(),
        "backup should exist after second save"
    );
}
