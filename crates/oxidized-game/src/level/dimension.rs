//! [`DimensionType`] — static properties of a dimension.
//!
//! Each dimension (overworld, nether, end) has a set of static properties
//! that control world height, lighting, weather, and other mechanics.
//! Mirrors Java's `DimensionType` record from 26.1.

use oxidized_protocol::types::ResourceLocation;

/// Maximum valid Y coordinate (exclusive upper bound on `min_y + height`).
///
/// Derived from Java: `(1 << BITS_FOR_Y) / 2 - 1` where `BITS_FOR_Y = 24`.
const MAX_Y_BOUND: i32 = (1 << 23) - 1; // 8_388_607

/// Static properties of a dimension type, loaded from the `dimension_type` registry.
///
/// Field set aligned with Java 26.1's `DimensionType` record.
#[derive(Debug, Clone)]
pub struct DimensionType {
    /// Registry key (e.g., `minecraft:overworld`).
    pub id: ResourceLocation,
    /// Whether this dimension has a fixed time of day (e.g., the End is always noon).
    pub has_fixed_time: bool,
    /// Whether this dimension has skylight.
    pub has_skylight: bool,
    /// Whether this dimension has a ceiling (nether).
    pub has_ceiling: bool,
    /// Whether an Ender Dragon fight can occur in this dimension.
    pub has_ender_dragon_fight: bool,
    /// Coordinate scale relative to the overworld (nether = 8.0).
    pub coordinate_scale: f64,
    /// Lowest world Y coordinate (inclusive). Overworld: −64. Nether/End: 0.
    /// Must be a multiple of 16.
    pub min_y: i32,
    /// Height of the world in blocks. Overworld: 384. Nether/End: 256.
    /// Must be ≥ 16 and a multiple of 16.
    pub height: i32,
    /// Highest Y for the purposes of logical height. Overworld: 384.
    /// Must be ≤ `height`.
    pub logical_height: i32,
    /// Sea level Y coordinate. Overworld: 63.
    pub sea_level: i32,
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
    /// Validates dimension constraints (mirrors Java's `DimensionType` constructor).
    ///
    /// # Panics
    ///
    /// Panics if any constraint is violated:
    /// - `height` < 16
    /// - `height` not a multiple of 16
    /// - `min_y` not a multiple of 16
    /// - `min_y + height` > `MAX_Y + 1`
    /// - `logical_height` > `height`
    fn validate(&self) {
        assert!(
            self.height >= 16,
            "height must be at least 16, got {}",
            self.height
        );
        assert!(
            self.height % 16 == 0,
            "height must be a multiple of 16, got {}",
            self.height
        );
        assert!(
            self.min_y % 16 == 0,
            "min_y must be a multiple of 16, got {}",
            self.min_y
        );
        assert!(
            self.min_y + self.height <= MAX_Y_BOUND + 1,
            "min_y + height cannot exceed {}, got {} + {} = {}",
            MAX_Y_BOUND + 1,
            self.min_y,
            self.height,
            self.min_y + self.height
        );
        assert!(
            self.logical_height <= self.height,
            "logical_height ({}) cannot exceed height ({})",
            self.logical_height,
            self.height
        );
    }

    /// Returns the overworld dimension type with standard vanilla values.
    #[must_use]
    pub fn overworld() -> Self {
        let dt = Self {
            id: ResourceLocation::minecraft("overworld"),
            has_fixed_time: false,
            has_skylight: true,
            has_ceiling: false,
            has_ender_dragon_fight: false,
            coordinate_scale: 1.0,
            min_y: -64,
            height: 384,
            logical_height: 384,
            sea_level: 63,
            ultrawarm: false,
            natural: true,
            ambient_light: 0.0,
            infiniburn: ResourceLocation::minecraft("infiniburn_overworld"),
            effects: ResourceLocation::minecraft("overworld"),
        };
        dt.validate();
        dt
    }

    /// Returns the nether dimension type with standard vanilla values.
    #[must_use]
    pub fn nether() -> Self {
        let dt = Self {
            id: ResourceLocation::minecraft("the_nether"),
            has_fixed_time: true,
            has_skylight: false,
            has_ceiling: true,
            has_ender_dragon_fight: false,
            coordinate_scale: 8.0,
            min_y: 0,
            height: 256,
            logical_height: 128,
            sea_level: 32,
            ultrawarm: true,
            natural: false,
            ambient_light: 0.1,
            infiniburn: ResourceLocation::minecraft("infiniburn_nether"),
            effects: ResourceLocation::minecraft("the_nether"),
        };
        dt.validate();
        dt
    }

    /// Returns the end dimension type with standard vanilla values.
    #[must_use]
    pub fn the_end() -> Self {
        let dt = Self {
            id: ResourceLocation::minecraft("the_end"),
            has_fixed_time: true,
            has_skylight: true,
            has_ceiling: false,
            has_ender_dragon_fight: true,
            coordinate_scale: 1.0,
            min_y: 0,
            height: 256,
            logical_height: 256,
            sea_level: 0,
            ultrawarm: false,
            natural: false,
            ambient_light: 0.25,
            infiniburn: ResourceLocation::minecraft("infiniburn_end"),
            effects: ResourceLocation::minecraft("the_end"),
        };
        dt.validate();
        dt
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
        assert!(!dt.has_fixed_time);
        assert!(!dt.has_ender_dragon_fight);
        assert!((dt.coordinate_scale - 1.0).abs() < f64::EPSILON);
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
        assert!(dt.has_fixed_time);
        assert!(!dt.has_ender_dragon_fight);
        assert!((dt.coordinate_scale - 8.0).abs() < f64::EPSILON);
        assert!((dt.ambient_light - 0.1).abs() < f32::EPSILON);
    }

    #[test]
    fn end_properties() {
        let dt = DimensionType::the_end();
        assert!(dt.has_fixed_time);
        assert!(dt.has_ender_dragon_fight);
        assert!(dt.has_skylight);
        assert!((dt.coordinate_scale - 1.0).abs() < f64::EPSILON);
        assert!(!dt.natural);
        assert!((dt.ambient_light - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    #[should_panic(expected = "height must be at least 16")]
    fn validation_height_too_small() {
        let mut dt = DimensionType::overworld();
        dt.height = 8;
        dt.validate();
    }

    #[test]
    #[should_panic(expected = "height must be a multiple of 16")]
    fn validation_height_not_multiple_of_16() {
        let mut dt = DimensionType::overworld();
        dt.height = 100;
        dt.validate();
    }

    #[test]
    #[should_panic(expected = "min_y must be a multiple of 16")]
    fn validation_min_y_not_multiple_of_16() {
        let mut dt = DimensionType::overworld();
        dt.min_y = -60;
        dt.validate();
    }

    #[test]
    #[should_panic(expected = "logical_height")]
    fn validation_logical_height_exceeds_height() {
        let mut dt = DimensionType::overworld();
        dt.logical_height = 400;
        dt.validate();
    }
}
