//! Neighbor requirements for the chunk generation pipeline.
//!
//! Each [`ChunkStatus`] has a neighbor requirement that must be satisfied
//! before a chunk can advance to that status. This encodes the vanilla
//! dependency table from ADR-016, extended for the full 12-status pipeline.

use oxidized_types::ChunkPos;

use super::ChunkStatus;

/// Neighbor requirement for advancing a chunk to a given status.
///
/// A radius of 0 means no neighbors are required. A radius of 1 means all
/// 8 adjacent chunks (Chebyshev distance ≤ 1) must be at `min_neighbor_status`
/// or later. Radius 8 means all chunks within a 17×17 square.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusRequirement {
    /// Chebyshev radius of required neighbors (0 = self only).
    pub radius: u8,
    /// Minimum status that all neighbors within `radius` must have reached.
    /// Ignored when `radius` is 0.
    pub min_neighbor_status: ChunkStatus,
}

/// Returns the neighbor requirement for advancing a chunk to the given status.
///
/// The table matches vanilla's generation pipeline (ADR-016). Statuses with
/// radius 0 have no neighbor dependencies and can be generated independently.
///
/// # Vanilla neighbor table
///
/// | Status | Radius | Min Neighbor |
/// |--------|--------|--------------|
/// | `Empty` | 0 | — |
/// | `StructureStarts` | 0 | — |
/// | `StructureReferences` | 8 | `StructureStarts` |
/// | `Biomes` | 0 | — |
/// | `Noise` | 0 | — |
/// | `Surface` | 0 | `Noise` |
/// | `Carvers` | 0 | `Noise` |
/// | `Features` | 1 | `Carvers` |
/// | `InitializeLight` | 0 | `Features` |
/// | `Light` | 1 | `Features` |
/// | `Spawn` | 0 | `Light` |
/// | `Full` | 0 | `Light` |
#[must_use]
pub const fn requirements(status: ChunkStatus) -> StatusRequirement {
    match status {
        ChunkStatus::Empty => StatusRequirement {
            radius: 0,
            min_neighbor_status: ChunkStatus::Empty,
        },
        ChunkStatus::StructureStarts => StatusRequirement {
            radius: 0,
            min_neighbor_status: ChunkStatus::Empty,
        },
        // Structures can span up to 8 chunks; references must propagate.
        ChunkStatus::StructureReferences => StatusRequirement {
            radius: 8,
            min_neighbor_status: ChunkStatus::StructureStarts,
        },
        ChunkStatus::Biomes => StatusRequirement {
            radius: 0,
            min_neighbor_status: ChunkStatus::Empty,
        },
        ChunkStatus::Noise => StatusRequirement {
            radius: 0,
            min_neighbor_status: ChunkStatus::Empty,
        },
        ChunkStatus::Surface => StatusRequirement {
            radius: 0,
            min_neighbor_status: ChunkStatus::Noise,
        },
        ChunkStatus::Carvers => StatusRequirement {
            radius: 0,
            min_neighbor_status: ChunkStatus::Noise,
        },
        // Trees/structures can extend up to 16 blocks into neighbors.
        ChunkStatus::Features => StatusRequirement {
            radius: 1,
            min_neighbor_status: ChunkStatus::Carvers,
        },
        ChunkStatus::InitializeLight => StatusRequirement {
            radius: 0,
            min_neighbor_status: ChunkStatus::Features,
        },
        // Light propagates across chunk boundaries.
        ChunkStatus::Light => StatusRequirement {
            radius: 1,
            min_neighbor_status: ChunkStatus::Features,
        },
        ChunkStatus::Spawn => StatusRequirement {
            radius: 0,
            min_neighbor_status: ChunkStatus::Light,
        },
        ChunkStatus::Full => StatusRequirement {
            radius: 0,
            min_neighbor_status: ChunkStatus::Light,
        },
    }
}

/// Checks whether all neighbor dependencies are satisfied for advancing
/// the chunk at `pos` to `target_status`.
///
/// `get_chunk_status` returns the current generation status of a chunk, or
/// `None` if the chunk has no generation data (treated as [`ChunkStatus::Empty`]
/// equivalent, which is below any meaningful requirement).
///
/// # Returns
///
/// `true` if every neighbor within the required Chebyshev radius has reached
/// at least the minimum neighbor status for `target_status`.
///
/// # Examples
///
/// ```
/// use oxidized_game::worldgen::{ChunkStatus, status_requirements};
/// use oxidized_types::ChunkPos;
///
/// // Features requires radius-1 neighbors at Carvers.
/// let all_at_carvers = |_: ChunkPos| Some(ChunkStatus::Carvers);
/// assert!(status_requirements::dependencies_satisfied(
///     ChunkPos::new(0, 0),
///     ChunkStatus::Features,
///     all_at_carvers,
/// ));
/// ```
#[must_use]
pub fn dependencies_satisfied(
    pos: ChunkPos,
    target_status: ChunkStatus,
    get_chunk_status: impl Fn(ChunkPos) -> Option<ChunkStatus>,
) -> bool {
    let req = requirements(target_status);

    if req.radius == 0 {
        return true;
    }

    let radius = i32::from(req.radius);

    for dx in -radius..=radius {
        for dz in -radius..=radius {
            if dx == 0 && dz == 0 {
                continue;
            }

            let neighbor = ChunkPos::new(pos.x.wrapping_add(dx), pos.z.wrapping_add(dz));
            let neighbor_status = get_chunk_status(neighbor).unwrap_or(ChunkStatus::Empty);

            if !neighbor_status.is_or_after(req.min_neighbor_status) {
                return false;
            }
        }
    }

    true
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn requirement_table_is_consistent() {
        // No status should require neighbors at a higher status than itself.
        let all_statuses = [
            ChunkStatus::Empty,
            ChunkStatus::StructureStarts,
            ChunkStatus::StructureReferences,
            ChunkStatus::Biomes,
            ChunkStatus::Noise,
            ChunkStatus::Surface,
            ChunkStatus::Carvers,
            ChunkStatus::Features,
            ChunkStatus::InitializeLight,
            ChunkStatus::Light,
            ChunkStatus::Spawn,
            ChunkStatus::Full,
        ];

        for &status in &all_statuses {
            let req = requirements(status);
            if req.radius > 0 {
                assert!(
                    status.is_or_after(req.min_neighbor_status),
                    "{status:?} requires neighbors at {min:?}, which is after itself",
                    min = req.min_neighbor_status,
                );
            }
        }
    }

    #[test]
    fn no_neighbors_required_for_empty() {
        let req = requirements(ChunkStatus::Empty);
        assert_eq!(req.radius, 0);
    }

    #[test]
    fn features_requires_radius_1_at_carvers() {
        let req = requirements(ChunkStatus::Features);
        assert_eq!(req.radius, 1);
        assert_eq!(req.min_neighbor_status, ChunkStatus::Carvers);
    }

    #[test]
    fn light_requires_radius_1_at_features() {
        let req = requirements(ChunkStatus::Light);
        assert_eq!(req.radius, 1);
        assert_eq!(req.min_neighbor_status, ChunkStatus::Features);
    }

    #[test]
    fn structure_references_requires_radius_8() {
        let req = requirements(ChunkStatus::StructureReferences);
        assert_eq!(req.radius, 8);
        assert_eq!(req.min_neighbor_status, ChunkStatus::StructureStarts);
    }

    #[test]
    fn dependencies_satisfied_no_radius() {
        // Statuses with radius 0 are always satisfied.
        let never_generated = |_: ChunkPos| None;
        assert!(dependencies_satisfied(
            ChunkPos::new(0, 0),
            ChunkStatus::Noise,
            never_generated,
        ));
    }

    #[test]
    fn dependencies_satisfied_all_neighbors_ready() {
        let all_at_carvers = |_: ChunkPos| Some(ChunkStatus::Carvers);
        assert!(dependencies_satisfied(
            ChunkPos::new(5, 5),
            ChunkStatus::Features,
            all_at_carvers,
        ));
    }

    #[test]
    fn dependencies_satisfied_all_neighbors_above_min() {
        // Neighbors at Features (above Carvers) should still satisfy.
        let all_at_features = |_: ChunkPos| Some(ChunkStatus::Features);
        assert!(dependencies_satisfied(
            ChunkPos::new(5, 5),
            ChunkStatus::Features,
            all_at_features,
        ));
    }

    #[test]
    fn dependencies_not_satisfied_missing_neighbor() {
        let one_missing = |pos: ChunkPos| {
            if pos == ChunkPos::new(6, 5) {
                None // not generated
            } else {
                Some(ChunkStatus::Carvers)
            }
        };
        assert!(!dependencies_satisfied(
            ChunkPos::new(5, 5),
            ChunkStatus::Features,
            one_missing,
        ));
    }

    #[test]
    fn dependencies_not_satisfied_neighbor_too_low() {
        let one_too_low = |pos: ChunkPos| {
            if pos == ChunkPos::new(4, 4) {
                Some(ChunkStatus::Noise) // below Carvers
            } else {
                Some(ChunkStatus::Carvers)
            }
        };
        assert!(!dependencies_satisfied(
            ChunkPos::new(5, 5),
            ChunkStatus::Features,
            one_too_low,
        ));
    }

    #[test]
    fn dependencies_satisfied_checks_correct_neighbor_count() {
        // Features radius 1 → 8 neighbors. Count how many are checked.
        let checked = std::sync::atomic::AtomicU32::new(0);
        let counter = |pos: ChunkPos| {
            if pos != ChunkPos::new(0, 0) {
                checked.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            Some(ChunkStatus::Full)
        };
        let _ = dependencies_satisfied(ChunkPos::new(0, 0), ChunkStatus::Features, counter);
        // Radius 1 Chebyshev → 3×3 - 1 = 8 neighbors
        assert_eq!(checked.load(std::sync::atomic::Ordering::Relaxed), 8);
    }
}
