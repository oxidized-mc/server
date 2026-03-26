//! VoxelShape face occlusion for directional light blocking.
//!
//! Vanilla Minecraft uses `VoxelShape` face projection to determine whether light
//! is blocked at a block boundary. Two adjacent blocks occlude light in a given
//! direction when either the exit face of the source block or the entry face of the
//! target block fully covers the boundary.
//!
//! This module implements a simplified boolean-per-face approximation: each block
//! state has a 6-bit mask indicating which faces are fully occluding. The occlusion
//! check becomes:
//!
//! ```text
//! source.face(dir) || target.face(dir.opposite())
//! ```
//!
//! This is sufficient for slabs, stairs, snow layers, pistons, and other common
//! partial blocks. Sub-block precision (e.g., fence post outlines) is not needed
//! because those shapes never fully cover a face.

use oxidized_protocol::types::Direction;
use oxidized_world::registry::BlockStateId;

/// Checks whether two adjacent block states form a full face occlusion at their
/// shared boundary in the given direction.
///
/// Returns `true` if either the source block's exit face (in `dir`) or the
/// target block's entry face (opposite of `dir`) fully covers the boundary.
///
/// Both blocks must have `use_shape_for_light_occlusion` set for shape-based
/// checking to apply — blocks without that flag are treated as having no
/// occluding faces (empty shape).
#[inline]
pub fn shape_occludes(from: BlockStateId, to: BlockStateId, dir: Direction) -> bool {
    let from_face = !from.is_empty_shape() && from.occlusion_face(dir.to_3d_data_value());
    let to_face = !to.is_empty_shape() && to.occlusion_face(dir.opposite().to_3d_data_value());
    from_face || to_face
}

/// Computes the effective light attenuation between two adjacent blocks.
///
/// Mirrors vanilla's `getLightBlockInto()`:
/// - If both blocks have empty shapes, returns the target's scalar `light_opacity`.
/// - Otherwise, checks face occlusion: if the faces occlude, returns 16 (fully
///   blocked); otherwise returns the target's `light_opacity`.
///
/// # Arguments
///
/// * `from` — the block state light is leaving
/// * `to` — the block state light is entering
/// * `dir` — the direction from `from` to `to`
#[inline]
pub fn get_light_block_into(from: BlockStateId, to: BlockStateId, dir: Direction) -> u8 {
    let simple_opacity = to.light_opacity();

    // Fast path: both blocks have empty shapes — just use scalar opacity.
    if from.is_empty_shape() && to.is_empty_shape() {
        return simple_opacity;
    }

    // Shape-based check: if either face fully covers the boundary, block all light.
    if shape_occludes(from, to, dir) {
        16
    } else {
        simple_opacity
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use oxidized_protocol::types::{Direction, direction};
    use oxidized_world::registry::BlockRegistry;

    fn state(name: &str) -> BlockStateId {
        BlockRegistry
            .default_state(name)
            .unwrap_or_else(|| panic!("{name} not found"))
    }

    fn state_with_props(name: &str, props: &[(&str, &str)]) -> BlockStateId {
        let mut sid = state(name);
        for &(key, value) in props {
            sid = sid
                .with_property(key, value)
                .unwrap_or_else(|| panic!("{name}[{key}={value}] not found"));
        }
        sid
    }

    // ── Full blocks and air ──────────────────────────────────────────────

    #[test]
    fn full_block_to_full_block_uses_scalar_opacity() {
        let stone = state("minecraft:stone");
        // Stone is opaque but is_empty_shape (no shape occlusion flag).
        // So it uses scalar opacity, not face occlusion.
        for dir in direction::ALL {
            let opacity = get_light_block_into(stone, stone, dir);
            assert_eq!(opacity, stone.light_opacity(), "dir={dir:?}");
        }
    }

    #[test]
    fn air_to_air_passes_light() {
        let air = BlockStateId(0);
        for dir in direction::ALL {
            assert_eq!(get_light_block_into(air, air, dir), 0, "dir={dir:?}");
            assert!(!shape_occludes(air, air, dir), "dir={dir:?}");
        }
    }

    #[test]
    fn air_to_full_block_uses_scalar() {
        let air = BlockStateId(0);
        let stone = state("minecraft:stone");
        for dir in direction::ALL {
            let opacity = get_light_block_into(air, stone, dir);
            assert_eq!(opacity, stone.light_opacity(), "dir={dir:?}");
        }
    }

    // ── Bottom slab ──────────────────────────────────────────────────────

    #[test]
    fn bottom_slab_blocks_down() {
        let slab = state_with_props("minecraft:stone_slab", &[("type", "bottom")]);
        let air = BlockStateId(0);

        // Bottom slab's DOWN face is occluding.
        assert!(
            slab.occlusion_face(Direction::Down.to_3d_data_value()),
            "bottom slab should occlude DOWN face"
        );

        // From slab going down: shape_occludes should be true.
        assert!(shape_occludes(slab, air, Direction::Down));

        // Effective opacity = 16 (fully blocked).
        assert_eq!(get_light_block_into(slab, air, Direction::Down), 16);
    }

    #[test]
    fn bottom_slab_passes_up() {
        let slab = state_with_props("minecraft:stone_slab", &[("type", "bottom")]);
        let air = BlockStateId(0);

        // Bottom slab's UP face is NOT occluding.
        assert!(!slab.occlusion_face(Direction::Up.to_3d_data_value()));

        // Light should pass upward through slab — no face occlusion.
        assert!(!shape_occludes(slab, air, Direction::Up));
        // Effective opacity is the scalar opacity (0 for slabs), not 16.
        let opacity = get_light_block_into(slab, air, Direction::Up);
        assert_ne!(opacity, 16, "should not be fully blocked");
    }

    #[test]
    fn bottom_slab_passes_sideways() {
        let slab = state_with_props("minecraft:stone_slab", &[("type", "bottom")]);
        let air = BlockStateId(0);

        for dir in [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
        ] {
            assert!(!shape_occludes(slab, air, dir), "dir={dir:?}");
        }
    }

    // ── Top slab ─────────────────────────────────────────────────────────

    #[test]
    fn top_slab_blocks_up() {
        let slab = state_with_props("minecraft:stone_slab", &[("type", "top")]);
        let air = BlockStateId(0);

        assert!(slab.occlusion_face(Direction::Up.to_3d_data_value()));
        assert!(shape_occludes(slab, air, Direction::Up));
        assert_eq!(get_light_block_into(slab, air, Direction::Up), 16);
    }

    #[test]
    fn top_slab_passes_down() {
        let slab = state_with_props("minecraft:stone_slab", &[("type", "top")]);
        let air = BlockStateId(0);

        assert!(!slab.occlusion_face(Direction::Down.to_3d_data_value()));
        assert!(!shape_occludes(slab, air, Direction::Down));
    }

    // ── Double slab (no shape occlusion) ─────────────────────────────────

    #[test]
    fn double_slab_uses_scalar_opacity() {
        let slab = state_with_props("minecraft:stone_slab", &[("type", "double")]);
        let air = BlockStateId(0);
        // Double slabs have use_shape_for_light_occlusion = false
        // (cleared by build.rs when occlusion == 0x3F).
        assert!(slab.is_empty_shape(), "double slab should have empty shape");
        // From double slab to air: both have empty shapes, so we use the
        // TARGET's scalar opacity (air = 0). No shape blocking.
        for dir in direction::ALL {
            let opacity = get_light_block_into(slab, air, dir);
            assert_eq!(opacity, air.light_opacity(), "dir={dir:?}");
        }
        // From air to double slab: target is the slab, so we use slab's opacity.
        for dir in direction::ALL {
            let opacity = get_light_block_into(air, slab, dir);
            assert_eq!(opacity, slab.light_opacity(), "dir={dir:?}");
        }
    }

    // ── Stairs ───────────────────────────────────────────────────────────

    #[test]
    fn bottom_straight_stairs_block_down() {
        // Bottom stairs (half=bottom, shape=straight) should block DOWN.
        let stairs = state_with_props(
            "minecraft:oak_stairs",
            &[
                ("half", "bottom"),
                ("shape", "straight"),
                ("facing", "north"),
            ],
        );

        assert!(
            stairs.occlusion_face(Direction::Down.to_3d_data_value()),
            "bottom stairs should occlude DOWN face"
        );
    }

    #[test]
    fn bottom_straight_stairs_block_back_face() {
        // Facing north → back face is SOUTH.
        let stairs = state_with_props(
            "minecraft:oak_stairs",
            &[
                ("half", "bottom"),
                ("shape", "straight"),
                ("facing", "north"),
            ],
        );

        assert!(
            stairs.occlusion_face(Direction::South.to_3d_data_value()),
            "north-facing bottom stairs should occlude SOUTH (back) face"
        );
    }

    #[test]
    fn bottom_straight_stairs_pass_front_face() {
        // Facing north → front face is NORTH (step side).
        let stairs = state_with_props(
            "minecraft:oak_stairs",
            &[
                ("half", "bottom"),
                ("shape", "straight"),
                ("facing", "north"),
            ],
        );

        assert!(
            !stairs.occlusion_face(Direction::North.to_3d_data_value()),
            "north-facing bottom stairs should NOT occlude NORTH (front) face"
        );
    }

    #[test]
    fn top_straight_stairs_block_up() {
        let stairs = state_with_props(
            "minecraft:oak_stairs",
            &[("half", "top"), ("shape", "straight"), ("facing", "east")],
        );

        assert!(
            stairs.occlusion_face(Direction::Up.to_3d_data_value()),
            "top stairs should occlude UP face"
        );
    }

    // ── Slab-to-slab interaction ─────────────────────────────────────────

    #[test]
    fn bottom_slab_on_top_slab_occlude_vertically() {
        let bottom = state_with_props("minecraft:stone_slab", &[("type", "bottom")]);
        let top = state_with_props("minecraft:stone_slab", &[("type", "top")]);

        // Light going down from top slab to bottom slab:
        // top's DOWN face is not occluding, but bottom's UP face is not occluding
        // either. So light should pass.
        assert!(!shape_occludes(top, bottom, Direction::Down));

        // Light going up from bottom slab to top slab: same logic.
        assert!(!shape_occludes(bottom, top, Direction::Up));
    }

    #[test]
    fn bottom_slab_below_top_slab_occlude_at_contact() {
        let top = state_with_props("minecraft:stone_slab", &[("type", "top")]);
        let bottom = state_with_props("minecraft:stone_slab", &[("type", "bottom")]);

        // From bottom going DOWN: bottom slab's DOWN face IS occluding.
        assert!(shape_occludes(bottom, top, Direction::Down));
        // From top going UP: top slab's UP face IS occluding.
        assert!(shape_occludes(top, bottom, Direction::Up));
    }

    // ── Property-based: air never occludes ───────────────────────────────

    #[test]
    fn air_never_shape_occludes_with_any_block() {
        let air = BlockStateId(0);
        // Test against several shape-occluding blocks.
        let blocks = [
            state_with_props("minecraft:stone_slab", &[("type", "bottom")]),
            state_with_props("minecraft:stone_slab", &[("type", "top")]),
            state_with_props(
                "minecraft:oak_stairs",
                &[
                    ("half", "bottom"),
                    ("shape", "straight"),
                    ("facing", "north"),
                ],
            ),
            state("minecraft:dirt_path"),
        ];

        for block in blocks {
            for dir in direction::ALL {
                // Air as source: no exit-face occlusion from air side.
                assert!(
                    !shape_occludes(air, block, dir)
                        || block.occlusion_face(dir.opposite().to_3d_data_value()),
                    "air->block should only occlude if target's entry face is full"
                );
            }
        }
    }
}
