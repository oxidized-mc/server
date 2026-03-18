//! Block update flags for `set_block_state`.
//!
//! Mirrors the flag constants in Java's `Level` class that control
//! update propagation and client notification.

use bitflags::bitflags;

bitflags! {
    /// Flags passed to `set_block_state`. Mirror Java's `Level` constants.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BlockFlags: u8 {
        /// Propagate block updates to neighbouring blocks.
        const UPDATE_NEIGHBORS = 0x01;
        /// Notify clients (send block change packet).
        const UPDATE_CLIENTS = 0x02;
        /// Suppress re-renders (used for invisible updates).
        const UPDATE_INVISIBLE = 0x04;
        /// Skip comparator updates.
        const UPDATE_KNOWN_SHAPE = 0x10;
        /// Prevent drops when breaking blocks.
        const UPDATE_SUPPRESS_DROPS = 0x20;
        /// Default: neighbours + clients.
        const DEFAULT = Self::UPDATE_NEIGHBORS.bits() | Self::UPDATE_CLIENTS.bits();
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
}
