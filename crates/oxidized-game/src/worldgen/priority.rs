//! Priority levels for chunk generation scheduling.
//!
//! Chunks closer to players are generated first. The priority determines
//! dispatch order in the [`super::scheduler::WorldgenScheduler`].

use std::fmt;

/// Priority level for a chunk generation task.
///
/// Higher-priority chunks are dispatched to the Rayon thread pool first.
/// The ordering is `Low < Normal < High < Urgent`, matching the derived
/// [`Ord`] implementation on the discriminant values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChunkGenPriority {
    /// Background pre-generation or exploration lookahead.
    Low = 0,
    /// Default priority for spawn-area fill and ticket-based loading.
    #[default]
    Normal = 1,
    /// Within a player's view distance.
    High = 2,
    /// Player is waiting — within 2 chunks of their position.
    Urgent = 3,
}

impl fmt::Display for ChunkGenPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Urgent => write!(f, "urgent"),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn priority_ordering() {
        assert!(ChunkGenPriority::Urgent > ChunkGenPriority::High);
        assert!(ChunkGenPriority::High > ChunkGenPriority::Normal);
        assert!(ChunkGenPriority::Normal > ChunkGenPriority::Low);
        assert!(ChunkGenPriority::Low < ChunkGenPriority::Urgent);
    }

    #[test]
    fn priority_default_is_normal() {
        assert_eq!(ChunkGenPriority::default(), ChunkGenPriority::Normal);
    }

    #[test]
    fn priority_display() {
        assert_eq!(ChunkGenPriority::Urgent.to_string(), "urgent");
        assert_eq!(ChunkGenPriority::Low.to_string(), "low");
    }
}
