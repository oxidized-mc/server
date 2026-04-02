//! View-distance tracking and spiral chunk iteration.
//!
//! Provides [`spiral_chunks`] for iterating chunk positions in send order
//! (closest first) and delta functions for computing which chunks to load
//! or unload when a player moves between chunk columns.

use oxidized_types::ChunkPos;

/// Iterates chunk positions in a square spiral from `center`, out to `radius`.
///
/// Yields `(2*radius+1)^2` positions, shell by shell (Chebyshev distance 0,
/// then 1, then 2, …). Within each shell, positions are iterated in a
/// deterministic order.
///
/// # Examples
///
/// ```
/// use oxidized_game::chunk::view_distance::spiral_chunks;
/// use oxidized_types::ChunkPos;
///
/// let chunks: Vec<ChunkPos> = spiral_chunks(ChunkPos::new(0, 0), 1).collect();
/// assert_eq!(chunks.len(), 9); // 3×3
/// assert_eq!(chunks[0], ChunkPos::new(0, 0)); // center first
/// ```
pub fn spiral_chunks(center: ChunkPos, radius: i32) -> impl Iterator<Item = ChunkPos> {
    let mut result = Vec::with_capacity(((2 * radius + 1) * (2 * radius + 1)) as usize);
    for r in 0..=radius {
        for dx in -r..=r {
            for dz in -r..=r {
                if dx.abs() == r || dz.abs() == r {
                    result.push(ChunkPos::new(center.x + dx, center.z + dz));
                }
            }
        }
    }
    result.into_iter()
}

/// Returns the set of chunks that a player at `new_center` needs but did not
/// need at `old_center`, given `radius` (Chebyshev view distance).
///
/// # Examples
///
/// ```
/// use oxidized_game::chunk::view_distance::{chunks_to_load, chunks_to_unload};
/// use oxidized_types::ChunkPos;
///
/// let old = ChunkPos::new(0, 0);
/// let new = ChunkPos::new(2, 0);
/// let radius = 1;
///
/// let to_load = chunks_to_load(old, new, radius);
/// let to_unload = chunks_to_unload(old, new, radius);
///
/// // No position should appear in both lists
/// for pos in &to_load {
///     assert!(!to_unload.contains(pos));
/// }
/// ```
pub fn chunks_to_load(old_center: ChunkPos, new_center: ChunkPos, radius: i32) -> Vec<ChunkPos> {
    spiral_chunks(new_center, radius)
        .filter(|&pos| chebyshev(pos, old_center) > radius)
        .collect()
}

/// Returns chunks in the old view that are no longer in the new view.
pub fn chunks_to_unload(old_center: ChunkPos, new_center: ChunkPos, radius: i32) -> Vec<ChunkPos> {
    spiral_chunks(old_center, radius)
        .filter(|&pos| chebyshev(pos, new_center) > radius)
        .collect()
}

/// Chebyshev (L∞) distance between two chunk positions.
#[inline]
fn chebyshev(a: ChunkPos, b: ChunkPos) -> i32 {
    (a.x - b.x).abs().max((a.z - b.z).abs())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_spiral_radius_0() {
        let chunks: Vec<_> = spiral_chunks(ChunkPos::new(0, 0), 0).collect();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], ChunkPos::new(0, 0));
    }

    #[test]
    fn test_spiral_radius_1_count() {
        let chunks: Vec<_> = spiral_chunks(ChunkPos::new(0, 0), 1).collect();
        assert_eq!(chunks.len(), 9); // 3×3
        assert!(chunks.contains(&ChunkPos::new(0, 0)));
        assert!(chunks.contains(&ChunkPos::new(1, 0)));
        assert!(chunks.contains(&ChunkPos::new(-1, -1)));
        assert!(chunks.contains(&ChunkPos::new(1, 1)));
    }

    #[test]
    fn test_spiral_radius_2_count() {
        let chunks: Vec<_> = spiral_chunks(ChunkPos::new(0, 0), 2).collect();
        assert_eq!(chunks.len(), 25); // 5×5
    }

    #[test]
    fn test_spiral_center_first() {
        let chunks: Vec<_> = spiral_chunks(ChunkPos::new(5, -3), 3).collect();
        assert_eq!(chunks[0], ChunkPos::new(5, -3));
    }

    #[test]
    fn test_spiral_no_duplicates() {
        let chunks: Vec<_> = spiral_chunks(ChunkPos::new(0, 0), 5).collect();
        let expected_count = (2 * 5 + 1) * (2 * 5 + 1);
        assert_eq!(chunks.len(), expected_count as usize);

        let mut unique = chunks.clone();
        unique.sort_by_key(|c| (c.x, c.z));
        unique.dedup();
        assert_eq!(unique.len(), chunks.len(), "spiral has duplicates");
    }

    #[test]
    fn test_load_unload_disjoint() {
        let old = ChunkPos::new(0, 0);
        let new = ChunkPos::new(1, 0);
        let r = 3;
        let to_load = chunks_to_load(old, new, r);
        let to_unload = chunks_to_unload(old, new, r);
        for pos in &to_load {
            assert!(!to_unload.contains(pos), "{pos:?} in both load and unload");
        }
    }

    #[test]
    fn test_no_movement_no_changes() {
        let pos = ChunkPos::new(5, 5);
        let to_load = chunks_to_load(pos, pos, 4);
        let to_unload = chunks_to_unload(pos, pos, 4);
        assert!(to_load.is_empty());
        assert!(to_unload.is_empty());
    }

    #[test]
    fn test_load_unload_complementary() {
        let old = ChunkPos::new(0, 0);
        let new = ChunkPos::new(2, 0);
        let r = 2;

        let old_chunks: Vec<_> = spiral_chunks(old, r).collect();
        let new_chunks: Vec<_> = spiral_chunks(new, r).collect();
        let to_load = chunks_to_load(old, new, r);
        let to_unload = chunks_to_unload(old, new, r);

        // Loaded chunks should be in new but not old
        for pos in &to_load {
            assert!(new_chunks.contains(pos));
            assert!(!old_chunks.contains(pos));
        }

        // Unloaded chunks should be in old but not new
        for pos in &to_unload {
            assert!(old_chunks.contains(pos));
            assert!(!new_chunks.contains(pos));
        }
    }

    #[test]
    fn test_chebyshev_distance() {
        assert_eq!(chebyshev(ChunkPos::new(0, 0), ChunkPos::new(3, 2)), 3);
        assert_eq!(chebyshev(ChunkPos::new(0, 0), ChunkPos::new(2, 5)), 5);
        assert_eq!(chebyshev(ChunkPos::new(0, 0), ChunkPos::new(0, 0)), 0);
        assert_eq!(chebyshev(ChunkPos::new(-1, -1), ChunkPos::new(1, 1)), 2);
    }
}
