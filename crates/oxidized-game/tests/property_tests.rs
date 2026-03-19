//! Property-based tests for chunk sending and movement logic.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashSet;

use proptest::prelude::*;

use oxidized_game::chunk::chunk_tracker::PlayerChunkTracker;
use oxidized_game::chunk::view_distance::{chunks_to_load, chunks_to_unload, spiral_chunks};
use oxidized_game::net::light_serializer::build_light_data;
use oxidized_world::chunk::DataLayer;
use oxidized_world::chunk::level_chunk::ChunkPos;

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

// ---------------------------------------------------------------------------
// Movement logic property tests
// ---------------------------------------------------------------------------

use oxidized_game::net::entity_movement::{
    DELTA_SCALE, EntityMoveKind, classify_move, encode_delta, pack_degrees, unpack_degrees,
};
use oxidized_game::player::movement::{MAX_COORDINATE, MAX_MOVEMENT_PER_TICK, validate_movement};
use oxidized_protocol::types::Vec3;

proptest! {
    /// encode_delta roundtrip: for small deltas, decoding the i16 recovers the
    /// original position within 1/4096 precision.
    #[test]
    fn proptest_encode_delta_small_values(
        old in -1000.0f64..1000.0,
        offset in -7.0f64..7.0,
    ) {
        let new = old + offset;
        let delta = encode_delta(old, new);
        prop_assert!(delta.is_some(), "delta within ±7 blocks should fit i16");
        let d = delta.unwrap();
        // Reconstruct: (old * 4096 + d) / 4096
        let reconstructed = ((old * DELTA_SCALE) as i64 + i64::from(d)) as f64 / DELTA_SCALE;
        let error = (reconstructed - new).abs();
        prop_assert!(error < 1.0 / DELTA_SCALE + f64::EPSILON,
            "delta encoding error {error} exceeds 1/4096 for old={old}, new={new}");
    }

    /// encode_delta returns None for deltas exceeding ~8 blocks.
    #[test]
    fn proptest_encode_delta_large_values(
        old in -1000.0f64..1000.0,
        offset in 8.1f64..100.0,
    ) {
        // Positive large offset
        prop_assert!(encode_delta(old, old + offset).is_none(),
            "delta > 8 blocks should be None");
        // Negative large offset
        prop_assert!(encode_delta(old, old - offset).is_none(),
            "delta < -8 blocks should be None");
    }

    /// pack_degrees → unpack_degrees roundtrip within 1.41° tolerance.
    #[test]
    fn proptest_degrees_pack_unpack_roundtrip(angle in 0.0f32..360.0) {
        let packed = pack_degrees(angle);
        let unpacked = unpack_degrees(packed);
        // Maximum error is 360/256 = 1.40625°
        let error = (unpacked - angle).abs();
        prop_assert!(error < 1.41, "degree roundtrip error {error}° for {angle}°");
    }

    /// unpack_degrees always produces values in [0, 360).
    #[test]
    fn proptest_unpack_degrees_range(byte: u8) {
        let degrees = unpack_degrees(byte);
        prop_assert!((0.0..360.0).contains(&degrees),
            "unpack_degrees({byte}) = {degrees} out of [0, 360)");
    }

    /// classify_move returns Delta for small moves, Sync for large moves.
    #[test]
    fn proptest_classify_move_consistency(
        old_x in -1000.0f64..1000.0,
        old_y in -64.0f64..320.0,
        old_z in -1000.0f64..1000.0,
        dx in -7.0f64..7.0,
        dy in -7.0f64..7.0,
        dz in -7.0f64..7.0,
    ) {
        let kind = classify_move(old_x, old_y, old_z,
                                  old_x + dx, old_y + dy, old_z + dz);
        match kind {
            EntityMoveKind::Delta { .. } => {
                // All three deltas must have fit
                prop_assert!(encode_delta(old_x, old_x + dx).is_some());
                prop_assert!(encode_delta(old_y, old_y + dy).is_some());
                prop_assert!(encode_delta(old_z, old_z + dz).is_some());
            },
            EntityMoveKind::Sync { x, y, z } => {
                prop_assert_eq!(x.to_bits(), (old_x + dx).to_bits());
                prop_assert_eq!(y.to_bits(), (old_y + dy).to_bits());
                prop_assert_eq!(z.to_bits(), (old_z + dz).to_bits());
            },
        }
    }

    /// validate_movement: needs_correction ↔ distance² > MAX².
    #[test]
    fn proptest_validate_movement_correction_invariant(
        dx in -150.0f64..150.0,
        dy in -150.0f64..150.0,
        dz in -150.0f64..150.0,
    ) {
        let origin = Vec3::new(0.0, 64.0, 0.0);
        let result = validate_movement(
            origin, 0.0, 0.0,
            Some(dx), Some(64.0 + dy), Some(dz),
            None, None,
        );
        let dist_sq = dx * dx + dy * dy + dz * dz;
        let expected_correction = dist_sq > MAX_MOVEMENT_PER_TICK * MAX_MOVEMENT_PER_TICK;
        let max_sq = MAX_MOVEMENT_PER_TICK * MAX_MOVEMENT_PER_TICK;
        prop_assert_eq!(result.needs_correction, expected_correction,
            "dist²={}, MAX²={}", dist_sq, max_sq);
        prop_assert_eq!(result.accepted, !expected_correction);
    }

    /// validate_movement: pitch is always clamped to ±90°.
    #[test]
    fn proptest_validate_movement_pitch_clamp(pitch in -180.0f32..180.0) {
        let result = validate_movement(
            Vec3::ZERO, 0.0, 0.0,
            None, None, None, None, Some(pitch),
        );
        prop_assert!(result.new_pitch >= -90.0 && result.new_pitch <= 90.0,
            "pitch {pitch} should be clamped to ±90, got {}", result.new_pitch);
    }

    /// validate_movement: x/z coordinates clamped to ±MAX_COORDINATE.
    #[test]
    fn proptest_validate_movement_coordinate_clamp(
        x in -5.0e7f64..5.0e7,
        z in -5.0e7f64..5.0e7,
    ) {
        let result = validate_movement(
            Vec3::ZERO, 0.0, 0.0,
            Some(x), Some(0.0), Some(z),
            None, None,
        );
        prop_assert!(result.new_pos.x >= -MAX_COORDINATE && result.new_pos.x <= MAX_COORDINATE,
            "x={} not clamped to ±{}", result.new_pos.x, MAX_COORDINATE);
        prop_assert!(result.new_pos.z >= -MAX_COORDINATE && result.new_pos.z <= MAX_COORDINATE,
            "z={} not clamped to ±{}", result.new_pos.z, MAX_COORDINATE);
    }
}
