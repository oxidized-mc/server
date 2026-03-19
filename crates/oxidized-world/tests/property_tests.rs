//! Property-based tests for oxidized-world data structures.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;

use oxidized_world::chunk::level_chunk::OVERWORLD_HEIGHT;
use oxidized_world::chunk::paletted_container::Strategy;
use oxidized_world::chunk::{BitStorage, Heightmap, HeightmapType, PalettedContainer};

proptest! {
    #[test]
    fn proptest_bit_storage_roundtrip(
        bits in 1u8..=15,
        size in 1usize..=4096,
    ) {
        let max_val = (1u64 << bits) - 1;
        let mut storage = BitStorage::new(bits, size).unwrap();

        // Use a deterministic pattern based on index
        for i in 0..size {
            let value = (i as u64) % (max_val + 1);
            storage.set(i, value).unwrap();
        }

        for i in 0..size {
            let expected = (i as u64) % (max_val + 1);
            prop_assert_eq!(
                storage.get(i).unwrap(),
                expected,
            );
        }
    }

    #[test]
    fn proptest_bit_storage_from_raw_roundtrip(
        bits in 1u8..=15,
        size in 1usize..=4096,
    ) {
        let max_val = (1u64 << bits) - 1;
        let mut storage = BitStorage::new(bits, size).unwrap();

        for i in 0..size {
            let value = (i as u64) % (max_val + 1);
            storage.set(i, value).unwrap();
        }

        let raw = storage.raw().to_vec();
        let storage2 = BitStorage::from_raw(bits, size, raw).unwrap();

        let original: Vec<u64> = storage.iter().collect();
        let restored: Vec<u64> = storage2.iter().collect();
        prop_assert_eq!(original, restored);
    }

    #[test]
    fn proptest_paletted_container_single_value(value in 0u32..1000) {
        let container = PalettedContainer::new(Strategy::BlockStates, value);
        prop_assert_eq!(container.get(0, 0, 0).unwrap(), value);
        prop_assert_eq!(container.bits_per_entry(), 0);
        // All positions should have the same value
        prop_assert_eq!(container.get(15, 15, 15).unwrap(), value);
        prop_assert_eq!(container.get(8, 4, 12).unwrap(), value);
    }

    #[test]
    fn proptest_paletted_container_set_get(
        x in 0usize..16,
        y in 0usize..16,
        z in 0usize..16,
        value in 0u32..100,
    ) {
        let mut container = PalettedContainer::empty(Strategy::BlockStates);
        container.set(x, y, z, value).unwrap();
        prop_assert_eq!(container.get(x, y, z).unwrap(), value);

        // Other positions should still be 0 (unless same position)
        if x != 0 || y != 0 || z != 0 {
            prop_assert_eq!(container.get(0, 0, 0).unwrap(), 0);
        }
    }

    #[test]
    fn proptest_paletted_container_write_read(seed in 0u64..1000) {
        let mut container = PalettedContainer::empty(Strategy::BlockStates);

        // Set ~20 values deterministically from the seed
        let mut positions = Vec::new();
        for i in 0..20u64 {
            let mixed = std::num::Wrapping(seed) * std::num::Wrapping(6364136223846793005u64)
                + std::num::Wrapping(i) * std::num::Wrapping(1442695040888963407u64);
            let mixed = mixed.0;
            let x = (mixed % 16) as usize;
            let y = ((mixed >> 8) % 16) as usize;
            let z = ((mixed >> 16) % 16) as usize;
            let value = ((mixed >> 24) % 50) as u32;
            container.set(x, y, z, value).unwrap();
            positions.push((x, y, z, value));
        }

        let bytes = container.write_to_bytes();
        let mut cursor = bytes.as_slice();
        let container2 =
            PalettedContainer::read_from_bytes(Strategy::BlockStates, &mut cursor).unwrap();

        // Later writes to the same position overwrite earlier ones, so check
        // the final value for each unique position by replaying in order.
        let mut final_values = std::collections::HashMap::new();
        for &(x, y, z, value) in &positions {
            final_values.insert((x, y, z), value);
        }

        for (&(x, y, z), &value) in &final_values {
            prop_assert_eq!(
                container2.get(x, y, z).unwrap(),
                value,
            );
        }
    }

    #[test]
    fn proptest_heightmap_roundtrip(seed in 0u64..10000) {
        let mut hm =
            Heightmap::new(HeightmapType::WorldSurface, OVERWORLD_HEIGHT).unwrap();

        // Generate 256 heights deterministically from seed
        let mut heights = Vec::with_capacity(256);
        for i in 0..256u64 {
            let mixed = std::num::Wrapping(seed) * std::num::Wrapping(6364136223846793005u64)
                + std::num::Wrapping(i) * std::num::Wrapping(1442695040888963407u64);
            let height = (mixed.0 % (OVERWORLD_HEIGHT as u64 + 1)) as u32;
            heights.push(height);
        }

        // Set all 16×16 heights
        for z in 0..16usize {
            for x in 0..16usize {
                let h = heights[z * 16 + x];
                hm.set(x, z, h).unwrap();
            }
        }

        // Verify via get()
        for z in 0..16usize {
            for x in 0..16usize {
                let expected = heights[z * 16 + x];
                prop_assert_eq!(
                    hm.get(x, z).unwrap(),
                    expected,
                );
            }
        }

        // Roundtrip through raw data
        let raw = hm.raw().to_vec();
        let hm2 =
            Heightmap::from_raw(HeightmapType::WorldSurface, OVERWORLD_HEIGHT, raw).unwrap();

        for z in 0..16usize {
            for x in 0..16usize {
                prop_assert_eq!(
                    hm.get(x, z).unwrap(),
                    hm2.get(x, z).unwrap(),
                );
            }
        }
    }
}
