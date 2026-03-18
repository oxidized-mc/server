//! [`DimensionType`] — static properties of a dimension.
//!
//! Each dimension (overworld, nether, end) has a set of static properties
//! that control world height, lighting, weather, and other mechanics.

use oxidized_protocol::types::ResourceLocation;

/// Static properties of a dimension type, loaded from the `dimension_type` registry.
#[derive(Debug, Clone)]
pub struct DimensionType {
    /// Registry key (e.g., `minecraft:overworld`).
    pub id: ResourceLocation,
    /// Lowest world Y coordinate (inclusive). Overworld: −64. Nether/End: 0.
    pub min_y: i32,
    /// Height of the world in blocks. Overworld: 384. Nether/End: 256.
    pub height: i32,
    /// Highest Y for the purposes of logical height. Overworld: 320.
    pub logical_height: i32,
    /// Sea level Y coordinate. Overworld: 63.
    pub sea_level: i32,
    /// Whether this dimension has skylight.
    pub has_skylight: bool,
    /// Whether this dimension has a ceiling (nether).
    pub has_ceiling: bool,
    /// No rain, water evaporates. `true` for the Nether.
    pub ultrawarm: bool,
    /// `false` for the End (no natural mob spawning rules).
    pub natural: bool,
    /// Ambient light level (0.0 in Overworld, 0.1 in Nether).
    pub ambient_light: f32,
    /// Tag for blocks that burn infinitely in this dimension.
    pub infiniburn: ResourceLocation,
    /// Visual effects identifier (sky rendering, fog, etc.).
    pub effects: ResourceLocation,
}

impl DimensionType {
    /// Returns the overworld dimension type with standard vanilla values.
    #[must_use]
    pub fn overworld() -> Self {
        Self {
            id: ResourceLocation::minecraft("overworld"),
            min_y: -64,
            height: 384,
            logical_height: 320,
            sea_level: 63,
            has_skylight: true,
            has_ceiling: false,
            ultrawarm: false,
            natural: true,
            ambient_light: 0.0,
            infiniburn: ResourceLocation::minecraft("infiniburn_overworld"),
            effects: ResourceLocation::minecraft("overworld"),
        }
    }

    /// Returns the nether dimension type with standard vanilla values.
    #[must_use]
    pub fn nether() -> Self {
        Self {
            id: ResourceLocation::minecraft("the_nether"),
            min_y: 0,
            height: 256,
            logical_height: 128,
            sea_level: 32,
            has_skylight: false,
            has_ceiling: true,
            ultrawarm: true,
            natural: false,
            ambient_light: 0.1,
            infiniburn: ResourceLocation::minecraft("infiniburn_nether"),
            effects: ResourceLocation::minecraft("the_nether"),
        }
    }

    /// Returns the end dimension type with standard vanilla values.
    #[must_use]
    pub fn the_end() -> Self {
        Self {
            id: ResourceLocation::minecraft("the_end"),
            min_y: 0,
            height: 256,
            logical_height: 256,
            sea_level: 0,
            has_skylight: false,
            has_ceiling: false,
            ultrawarm: false,
            natural: false,
            ambient_light: 0.0,
            infiniburn: ResourceLocation::minecraft("infiniburn_end"),
            effects: ResourceLocation::minecraft("the_end"),
        }
    }

    /// Number of chunk sections in this dimension.
    #[must_use]
    pub fn section_count(&self) -> usize {
        (self.height >> 4) as usize
    }

    /// Minimum section Y index.
    #[must_use]
    pub fn min_section(&self) -> i32 {
        self.min_y >> 4
    }

    /// Maximum Y coordinate (exclusive).
    #[must_use]
    pub fn max_y(&self) -> i32 {
        self.min_y + self.height
    }

    /// Returns `true` if the given Y coordinate is within this dimension's bounds.
    #[must_use]
    pub fn is_valid_y(&self, y: i32) -> bool {
        y >= self.min_y && y < self.max_y()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn overworld_sections() {
        let dt = DimensionType::overworld();
        assert_eq!(dt.section_count(), 24);
        assert_eq!(dt.min_section(), -4);
        assert_eq!(dt.max_y(), 320);
    }

    #[test]
    fn nether_sections() {
        let dt = DimensionType::nether();
        assert_eq!(dt.section_count(), 16);
        assert_eq!(dt.min_section(), 0);
        assert_eq!(dt.max_y(), 256);
    }

    #[test]
    fn end_sections() {
        let dt = DimensionType::the_end();
        assert_eq!(dt.section_count(), 16);
        assert_eq!(dt.min_section(), 0);
    }

    #[test]
    fn overworld_valid_y() {
        let dt = DimensionType::overworld();
        assert!(dt.is_valid_y(-64));
        assert!(dt.is_valid_y(0));
        assert!(dt.is_valid_y(319));
        assert!(!dt.is_valid_y(-65));
        assert!(!dt.is_valid_y(320));
    }

    #[test]
    fn overworld_properties() {
        let dt = DimensionType::overworld();
        assert!(dt.has_skylight);
        assert!(!dt.has_ceiling);
        assert!(!dt.ultrawarm);
        assert!(dt.natural);
        assert_eq!(dt.sea_level, 63);
        assert!((dt.ambient_light - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn nether_properties() {
        let dt = DimensionType::nether();
        assert!(!dt.has_skylight);
        assert!(dt.has_ceiling);
        assert!(dt.ultrawarm);
        assert!(!dt.natural);
        assert!((dt.ambient_light - 0.1).abs() < f32::EPSILON);
    }
}
