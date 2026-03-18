//! Block update flags for `set_block_state`.
//!
//! Mirrors the flag constants from Java's `Block` class that control
//! update propagation, client notification, and side-effect suppression.

use bitflags::bitflags;

/// Default recursion depth limit for block updates (Java `Block.UPDATE_LIMIT`).
pub const UPDATE_LIMIT: i32 = 512;

bitflags! {
    /// Flags passed to `set_block_state`. Mirror Java's `Block` constants.
    ///
    /// Backed by `u16` to accommodate flags up to bit 9 (0x200).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BlockFlags: u16 {
        /// Propagate block updates to neighbouring blocks (bit 0).
        const UPDATE_NEIGHBORS = 0x01;
        /// Notify clients — send block change packet (bit 1).
        const UPDATE_CLIENTS = 0x02;
        /// Suppress re-renders on the client side (bit 2).
        const UPDATE_INVISIBLE = 0x04;
        /// Force immediate re-render on the client (bit 3).
        const UPDATE_IMMEDIATE = 0x08;
        /// Skip neighbour shape updates (bit 4).
        /// When this flag is **not** set, neighbour shapes are updated.
        const UPDATE_KNOWN_SHAPE = 0x10;
        /// Prevent item drops when breaking blocks (bit 5).
        const UPDATE_SUPPRESS_DROPS = 0x20;
        /// Block was moved by a piston (bit 6).
        const UPDATE_MOVE_BY_PISTON = 0x40;
        /// Skip shape update propagation over the wire (bit 7).
        const UPDATE_SKIP_SHAPE_UPDATE_ON_WIRE = 0x80;
        /// Skip block entity side-effects (bit 8).
        const UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS = 0x100;
        /// Skip on-place callbacks (bit 9).
        const UPDATE_SKIP_ON_PLACE = 0x200;

        // --- Composite constants ---

        /// Default: neighbours + clients (`UPDATE_ALL` in Java).
        const DEFAULT = Self::UPDATE_NEIGHBORS.bits() | Self::UPDATE_CLIENTS.bits();
        /// Alias for `DEFAULT`: update neighbours and clients.
        const UPDATE_ALL = Self::DEFAULT.bits();
        /// Neighbours + clients + immediate re-render.
        const UPDATE_ALL_IMMEDIATE =
            Self::UPDATE_NEIGHBORS.bits()
            | Self::UPDATE_CLIENTS.bits()
            | Self::UPDATE_IMMEDIATE.bits();
        /// No neighbours, no client updates, but skip block-entity side-effects
        /// and on-place callbacks. Java `UPDATE_NONE = 260` (0x104).
        const UPDATE_NONE =
            Self::UPDATE_INVISIBLE.bits()
            | Self::UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS.bits();
        /// Suppress drops + skip known shape +
        /// skip block-entity side-effects + skip on-place.
        /// Java `UPDATE_SKIP_ALL_SIDEEFFECTS = 816` (0x330).
        const UPDATE_SKIP_ALL_SIDEEFFECTS =
            Self::UPDATE_KNOWN_SHAPE.bits()
            | Self::UPDATE_SUPPRESS_DROPS.bits()
            | Self::UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS.bits()
            | Self::UPDATE_SKIP_ON_PLACE.bits();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn default_includes_neighbors_and_clients() {
        let flags = BlockFlags::DEFAULT;
        assert!(flags.contains(BlockFlags::UPDATE_NEIGHBORS));
        assert!(flags.contains(BlockFlags::UPDATE_CLIENTS));
        assert!(!flags.contains(BlockFlags::UPDATE_INVISIBLE));
    }

    #[test]
    fn update_all_equals_default() {
        assert_eq!(BlockFlags::UPDATE_ALL, BlockFlags::DEFAULT);
        assert_eq!(BlockFlags::UPDATE_ALL.bits(), 3);
    }

    #[test]
    fn update_all_immediate() {
        let flags = BlockFlags::UPDATE_ALL_IMMEDIATE;
        assert_eq!(flags.bits(), 11);
        assert!(flags.contains(BlockFlags::UPDATE_NEIGHBORS));
        assert!(flags.contains(BlockFlags::UPDATE_CLIENTS));
        assert!(flags.contains(BlockFlags::UPDATE_IMMEDIATE));
    }

    #[test]
    fn update_none_value() {
        // Java: UPDATE_NONE = 260 = UPDATE_INVISIBLE(4) | UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS(256)
        assert_eq!(BlockFlags::UPDATE_NONE.bits(), 260);
        assert!(BlockFlags::UPDATE_NONE.contains(BlockFlags::UPDATE_INVISIBLE));
        assert!(BlockFlags::UPDATE_NONE.contains(BlockFlags::UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS));
    }

    #[test]
    fn update_skip_all_sideeffects_value() {
        // Java: UPDATE_SKIP_ALL_SIDEEFFECTS = 816
        // = UPDATE_KNOWN_SHAPE(16) | UPDATE_SUPPRESS_DROPS(32) |
        //   UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS(256) | UPDATE_SKIP_ON_PLACE(512)
        assert_eq!(BlockFlags::UPDATE_SKIP_ALL_SIDEEFFECTS.bits(), 816);
    }

    #[test]
    fn flags_combine() {
        let flags = BlockFlags::UPDATE_NEIGHBORS | BlockFlags::UPDATE_SUPPRESS_DROPS;
        assert!(flags.contains(BlockFlags::UPDATE_NEIGHBORS));
        assert!(flags.contains(BlockFlags::UPDATE_SUPPRESS_DROPS));
        assert!(!flags.contains(BlockFlags::UPDATE_CLIENTS));
    }

    #[test]
    fn empty_flags() {
        let flags = BlockFlags::empty();
        assert!(!flags.contains(BlockFlags::UPDATE_NEIGHBORS));
        assert!(!flags.contains(BlockFlags::UPDATE_CLIENTS));
    }

    #[test]
    fn piston_and_skip_flags() {
        let flags =
            BlockFlags::UPDATE_MOVE_BY_PISTON | BlockFlags::UPDATE_SKIP_SHAPE_UPDATE_ON_WIRE;
        assert_eq!(flags.bits(), 0x40 | 0x80);
        assert!(flags.contains(BlockFlags::UPDATE_MOVE_BY_PISTON));
        assert!(flags.contains(BlockFlags::UPDATE_SKIP_SHAPE_UPDATE_ON_WIRE));
    }

    #[test]
    fn update_limit_constant() {
        assert_eq!(UPDATE_LIMIT, 512);
    }
}
