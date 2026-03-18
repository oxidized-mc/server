//! Minecraft dimension identifiers.

use std::fmt;

/// The three built-in Minecraft dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    /// The overworld — the default dimension.
    Overworld,
    /// The Nether — accessed via nether portals.
    Nether,
    /// The End — accessed via the end portal.
    End,
}

impl Dimension {
    /// Returns the folder name suffix used for this dimension's data.
    ///
    /// The overworld uses the world root directly; Nether uses `DIM-1`;
    /// End uses `DIM1`.
    #[must_use]
    pub const fn folder_name(self) -> Option<&'static str> {
        match self {
            Self::Overworld => None,
            Self::Nether => Some("DIM-1"),
            Self::End => Some("DIM1"),
        }
    }

    /// Returns the Minecraft resource identifier for this dimension.
    #[must_use]
    pub const fn resource_id(self) -> &'static str {
        match self {
            Self::Overworld => "minecraft:overworld",
            Self::Nether => "minecraft:the_nether",
            Self::End => "minecraft:the_end",
        }
    }
}

impl fmt::Display for Dimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.resource_id())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_folder_names() {
        assert_eq!(Dimension::Overworld.folder_name(), None);
        assert_eq!(Dimension::Nether.folder_name(), Some("DIM-1"));
        assert_eq!(Dimension::End.folder_name(), Some("DIM1"));
    }

    #[test]
    fn test_resource_ids() {
        assert_eq!(Dimension::Overworld.resource_id(), "minecraft:overworld");
        assert_eq!(Dimension::Nether.resource_id(), "minecraft:the_nether");
        assert_eq!(Dimension::End.resource_id(), "minecraft:the_end");
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", Dimension::Overworld), "minecraft:overworld");
    }
}
