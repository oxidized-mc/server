//! Property-based tests for Phase 13 chunk sending logic.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashSet;

use proptest::prelude::*;

use oxidized_game::chunk::chunk_tracker::PlayerChunkTracker;
use oxidized_game::chunk::view_distance::{chunks_to_load, chunks_to_unload, spiral_chunks};
use oxidized_game::net::light_serializer::build_light_data;
use oxidized_world::chunk::level_chunk::ChunkPos;
use oxidized_world::chunk::DataLayer;

proptest! {
    /// Spiral chunk count is always (2r+1)² for any center and radius.
    #[test]
    fn proptest_spiral_chunks_count_invariant(
        cx in -1_000_000i32..1_000_000,
        cz in -1_000_000i32..1_000_000,
        radius in 0i32..=16,
    ) {
        let center = ChunkPos::new(cx, cz);
        let chunks: Vec<_> = spiral_chunks(center, radius).collect();
        let expected = ((2 * radius + 1) * (2 * radius + 1)) as usize;
        prop_assert_eq!(chunks.len(), expected);
    }

    /// Spiral always yields center as the first position.
    #[test]
    fn proptest_spiral_chunks_center_first(
        cx in -1_000_000i32..1_000_000,
        cz in -1_000_000i32..1_000_000,
        radius in 0i32..=16,
    ) {
        let center = ChunkPos::new(cx, cz);
        let first = spiral_chunks(center, radius).next().unwrap();
        prop_assert_eq!(first, center);
    }

    /// Spiral never yields duplicate positions.
    #[test]
    fn proptest_spiral_chunks_no_duplicates(
        cx in -100i32..100,
        cz in -100i32..100,
        radius in 0i32..=8,
    ) {
        let center = ChunkPos::new(cx, cz);
        let chunks: Vec<_> = spiral_chunks(center, radius).collect();
        let unique: HashSet<_> = chunks.iter().copied().collect();
        prop_assert_eq!(unique.len(), chunks.len(), "spiral has duplicates");
    }

    /// chunks_to_load and chunks_to_unload are always disjoint.
    #[test]
    fn proptest_load_unload_disjoint(
        ox in -100i32..100,
        oz in -100i32..100,
        nx in -100i32..100,
        nz in -100i32..100,
        radius in 1i32..=8,
    ) {
        let old = ChunkPos::new(ox, oz);
        let new = ChunkPos::new(nx, nz);
        let to_load: HashSet<_> = chunks_to_load(old, new, radius).into_iter().collect();
        let to_unload: HashSet<_> = chunks_to_unload(old, new, radius).into_iter().collect();
        prop_assert!(
            to_load.is_disjoint(&to_unload),
            "load and unload sets must never overlap"
        );
    }

    /// load ∪ unload ∪ (old ∩ new) = old ∪ new (complete coverage).
    #[test]
    fn proptest_load_unload_coverage(
        ox in -50i32..50,
        oz in -50i32..50,
        nx in -50i32..50,
        nz in -50i32..50,
        radius in 1i32..=6,
    ) {
        let old = ChunkPos::new(ox, oz);
        let new = ChunkPos::new(nx, nz);

        let old_view: HashSet<_> = spiral_chunks(old, radius).collect();
        let new_view: HashSet<_> = spiral_chunks(new, radius).collect();

        let to_load: HashSet<_> = chunks_to_load(old, new, radius).into_iter().collect();
        let to_unload: HashSet<_> = chunks_to_unload(old, new, radius).into_iter().collect();

        // to_load should be exactly new_view \ old_view
        let expected_load: HashSet<_> = new_view.difference(&old_view).copied().collect();
        prop_assert_eq!(&to_load, &expected_load, "load set mismatch");

        // to_unload should be exactly old_view \ new_view
        let expected_unload: HashSet<_> = old_view.difference(&new_view).copied().collect();
        prop_assert_eq!(&to_unload, &expected_unload, "unload set mismatch");
    }

    /// PlayerChunkTracker loaded count is always (2r+1)² after any update.
    #[test]
    fn proptest_chunk_tracker_loaded_count_invariant(
        cx in -100i32..100,
        cz in -100i32..100,
        nx in -100i32..100,
        nz in -100i32..100,
        radius in 1i32..=8,
    ) {
        let center = ChunkPos::new(cx, cz);
        let mut tracker = PlayerChunkTracker::new(center, radius);
        let expected = ((2 * radius + 1) * (2 * radius + 1)) as usize;
        prop_assert_eq!(tracker.loaded_count(), expected);

        let new = ChunkPos::new(nx, nz);
        tracker.update_center(new);
        prop_assert_eq!(tracker.loaded_count(), expected);
    }

    /// Light data: sky_y_mask and empty_sky_y_mask are disjoint bit sets.
    #[test]
    fn proptest_light_masks_disjoint(
        filled_indices in prop::collection::vec(0usize..26, 0..10),
    ) {
        let mut sky: Vec<Option<DataLayer>> = vec![None; 26];
        for &idx in &filled_indices {
            sky[idx] = Some(DataLayer::filled(15));
        }
        let block: Vec<Option<DataLayer>> = vec![None; 26];
        let data = build_light_data(&sky, &block);

        // If both masks exist, their bits must be disjoint.
        if !data.sky_y_mask.is_empty() && !data.empty_sky_y_mask.is_empty() {
            prop_assert_eq!(
                data.sky_y_mask[0] & data.empty_sky_y_mask[0],
                0,
                "sky_y_mask and empty_sky_y_mask must be disjoint"
            );
        }
    }

    /// Light data: number of sky_updates matches popcount of sky_y_mask.
    #[test]
    fn proptest_light_updates_count_matches_mask(
        filled_indices in prop::collection::vec(0usize..26, 0..26),
    ) {
        let mut sky: Vec<Option<DataLayer>> = vec![None; 26];
        let unique: HashSet<_> = filled_indices.into_iter().collect();
        for idx in &unique {
            sky[*idx] = Some(DataLayer::filled(15));
        }
        let block: Vec<Option<DataLayer>> = vec![None; 26];
        let data = build_light_data(&sky, &block);

        let mask_bits = if data.sky_y_mask.is_empty() {
            0u32
        } else {
            (data.sky_y_mask[0] as u64).count_ones()
        };
        prop_assert_eq!(
            data.sky_updates.len() as u32,
            mask_bits,
            "sky_updates count must match sky_y_mask popcount"
        );
    }
}
