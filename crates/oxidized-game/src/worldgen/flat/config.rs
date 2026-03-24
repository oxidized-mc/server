//! Flat world layer configuration.
//!
//! Defines the block layers that make up a flat world. The default
//! configuration matches vanilla: 1 bedrock + 2 dirt + 1 grass block,
//! starting at the minimum build height (y = −64).
//!
//! Internally, layers are pre-flattened into a `Vec<BlockStateId>` indexed
//! by Y offset from `OVERWORLD_MIN_Y`, giving O(1) block lookups. This
//! mirrors Java's `FlatLevelGeneratorSettings.updateLayers()`.

use oxidized_world::chunk::level_chunk::{OVERWORLD_HEIGHT, OVERWORLD_MIN_Y};
use oxidized_world::registry::{BEDROCK, BlockRegistry, BlockStateId, DIRT, GRASS_BLOCK};

/// Errors that can occur when parsing a flat world layer string.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FlatConfigError {
    /// The layer string was empty or contained no valid layers.
    #[error("flat world must have at least one layer")]
    Empty,
    /// A layer height value could not be parsed as a number.
    #[error("invalid layer height in '{entry}': {source}")]
    InvalidHeight {
        /// The raw layer entry that failed to parse.
        entry: String,
        /// The underlying parse error.
        source: std::num::ParseIntError,
    },
    /// A block name was not found in the registry.
    #[error("unknown block '{name}'")]
    UnknownBlock {
        /// The block name that was not found.
        name: String,
    },
}

/// One layer in the flat world stack.
///
/// Each layer fills the entire 16×16 chunk footprint for `height` blocks.
///
/// # Examples
///
/// ```
/// use oxidized_game::worldgen::flat::FlatLayerInfo;
/// use oxidized_world::registry::BEDROCK;
///
/// let layer = FlatLayerInfo { block: BEDROCK, height: 1 };
/// assert_eq!(layer.height, 1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlatLayerInfo {
    /// Block state ID for this layer.
    pub block: BlockStateId,
    /// Number of blocks tall this layer is (≥1).
    pub height: u32,
}

/// Complete flat world configuration.
///
/// Layers are ordered bottom-to-top. The first layer starts at
/// [`OVERWORLD_MIN_Y`] (−64). A pre-flattened lookup table is built
/// for O(1) access by Y offset.
///
/// # Examples
///
/// ```
/// use oxidized_game::worldgen::flat::FlatWorldConfig;
///
/// let config = FlatWorldConfig::default();
/// assert_eq!(config.total_height(), 4); // bedrock + 2 dirt + grass
/// ```
#[derive(Debug, Clone)]
pub struct FlatWorldConfig {
    /// Block layers, bottom to top.
    pub layers: Vec<FlatLayerInfo>,
    /// Pre-flattened block state IDs indexed by Y offset from `OVERWORLD_MIN_Y`.
    /// Length = `min(sum of layer heights, OVERWORLD_HEIGHT)`.
    flattened: Vec<BlockStateId>,
    /// Biome resource key (e.g. `"minecraft:plains"`).
    pub biome: String,
    /// Whether to place structures and decorations.
    pub has_features: bool,
    /// Whether to generate lakes.
    pub has_lakes: bool,
}

/// Builds the pre-flattened layer array from layer definitions, clamped to
/// the maximum world height.
fn flatten_layers(layers: &[FlatLayerInfo]) -> Vec<BlockStateId> {
    let max = OVERWORLD_HEIGHT as usize;
    let mut flat = Vec::with_capacity(
        layers
            .iter()
            .map(|l| l.height as usize)
            .sum::<usize>()
            .min(max),
    );
    for layer in layers {
        for _ in 0..layer.height {
            if flat.len() >= max {
                return flat;
            }
            flat.push(layer.block);
        }
    }
    flat
}

impl Default for FlatWorldConfig {
    /// Vanilla default: 1 bedrock + 2 dirt + 1 grass_block.
    ///
    /// Starting at y = −64:
    /// - y = −64: bedrock
    /// - y = −63: dirt
    /// - y = −62: dirt
    /// - y = −61: grass_block ← surface (player spawns at y = −60)
    fn default() -> Self {
        let layers = vec![
            FlatLayerInfo {
                block: BEDROCK,
                height: 1,
            },
            FlatLayerInfo {
                block: DIRT,
                height: 2,
            },
            FlatLayerInfo {
                block: GRASS_BLOCK,
                height: 1,
            },
        ];
        let flattened = flatten_layers(&layers);
        Self {
            layers,
            flattened,
            biome: "minecraft:plains".into(),
            has_features: false,
            has_lakes: false,
        }
    }
}

impl FlatWorldConfig {
    /// Creates a configuration from a slice of `(block, height)` pairs.
    ///
    /// Layers are ordered bottom to top. Uses default biome (plains) and
    /// disables features / lakes. Layers with height 0 are silently skipped.
    #[must_use]
    pub fn from_layers(layers: &[(BlockStateId, u32)]) -> Self {
        let layers: Vec<FlatLayerInfo> = layers
            .iter()
            .filter(|(_, height)| *height > 0)
            .map(|&(block, height)| FlatLayerInfo { block, height })
            .collect();
        let flattened = flatten_layers(&layers);
        Self {
            layers,
            flattened,
            biome: "minecraft:plains".to_owned(),
            has_features: false,
            has_lakes: false,
        }
    }

    /// Total height of all layers combined, clamped to the world height.
    #[must_use]
    pub fn total_height(&self) -> u32 {
        self.flattened.len() as u32
    }

    /// Returns the pre-flattened block state array (indexed by Y offset
    /// from `OVERWORLD_MIN_Y`).
    #[must_use]
    pub fn flattened_layers(&self) -> &[BlockStateId] {
        &self.flattened
    }

    /// Returns the block state ID at a given absolute Y coordinate.
    ///
    /// Returns `None` if `y` is below the first layer or above the last.
    #[must_use]
    pub fn block_at_y(&self, y: i32) -> Option<BlockStateId> {
        let offset = y - OVERWORLD_MIN_Y;
        if offset < 0 {
            return None;
        }
        self.flattened.get(offset as usize).copied()
    }

    /// Parses a flat world from the vanilla layer string format.
    ///
    /// Format: `"block_id*height,block_id*height,..."` (bottom to top).
    /// A layer without `*height` has height 1.
    ///
    /// # Errors
    ///
    /// Returns an error if the string is empty or contains invalid entries.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidized_game::worldgen::flat::FlatWorldConfig;
    /// use oxidized_world::registry::BlockRegistry;
    ///
    /// let registry = BlockRegistry::load().unwrap();
    /// let config = FlatWorldConfig::from_layers_string(
    ///     "minecraft:bedrock,minecraft:dirt*2,minecraft:grass_block",
    ///     &registry,
    /// ).unwrap();
    /// assert_eq!(config.total_height(), 4);
    /// ```
    pub fn from_layers_string(s: &str, registry: &BlockRegistry) -> Result<Self, FlatConfigError> {
        let mut layers = Vec::new();
        for part in s.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (block_name, height) = if let Some((id, count_str)) = part.split_once('*') {
                let count: u32 =
                    count_str
                        .parse()
                        .map_err(|source| FlatConfigError::InvalidHeight {
                            entry: part.to_owned(),
                            source,
                        })?;
                (id.trim(), count)
            } else {
                (part, 1)
            };
            if height == 0 {
                continue;
            }
            let block_state_id = registry.default_state(block_name).ok_or_else(|| {
                FlatConfigError::UnknownBlock {
                    name: block_name.to_owned(),
                }
            })?;
            layers.push(FlatLayerInfo {
                block: block_state_id,
                height,
            });
        }
        if layers.is_empty() {
            return Err(FlatConfigError::Empty);
        }
        let flattened = flatten_layers(&layers);
        Ok(Self {
            layers,
            flattened,
            biome: "minecraft:plains".to_owned(),
            has_features: false,
            has_lakes: false,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use oxidized_world::registry::STONE;

    use super::*;

    #[test]
    fn default_total_height_is_4() {
        let config = FlatWorldConfig::default();
        assert_eq!(config.total_height(), 4);
    }

    #[test]
    fn default_layers_correct() {
        let config = FlatWorldConfig::default();
        assert_eq!(config.layers.len(), 3);
        assert_eq!(config.layers[0].block, BEDROCK);
        assert_eq!(config.layers[0].height, 1);
        assert_eq!(config.layers[1].block, DIRT);
        assert_eq!(config.layers[1].height, 2);
        assert_eq!(config.layers[2].block, GRASS_BLOCK);
        assert_eq!(config.layers[2].height, 1);
    }

    #[test]
    fn flattened_layers_match_block_at_y() {
        let config = FlatWorldConfig::default();
        let flat = config.flattened_layers();
        assert_eq!(flat.len(), 4);
        assert_eq!(flat[0], BEDROCK);
        assert_eq!(flat[1], DIRT);
        assert_eq!(flat[2], DIRT);
        assert_eq!(flat[3], GRASS_BLOCK);
    }

    #[test]
    fn block_at_y_bottom_is_bedrock() {
        let config = FlatWorldConfig::default();
        assert_eq!(config.block_at_y(OVERWORLD_MIN_Y), Some(BEDROCK));
    }

    #[test]
    fn block_at_y_dirt_layers() {
        let config = FlatWorldConfig::default();
        assert_eq!(config.block_at_y(OVERWORLD_MIN_Y + 1), Some(DIRT));
        assert_eq!(config.block_at_y(OVERWORLD_MIN_Y + 2), Some(DIRT));
    }

    #[test]
    fn block_at_y_surface_is_grass() {
        let config = FlatWorldConfig::default();
        assert_eq!(config.block_at_y(OVERWORLD_MIN_Y + 3), Some(GRASS_BLOCK));
    }

    #[test]
    fn block_at_y_above_layers_returns_none() {
        let config = FlatWorldConfig::default();
        assert!(config.block_at_y(OVERWORLD_MIN_Y + 10).is_none());
    }

    #[test]
    fn block_at_y_below_min_returns_none() {
        let config = FlatWorldConfig::default();
        assert!(config.block_at_y(OVERWORLD_MIN_Y - 1).is_none());
    }

    #[test]
    fn from_layers_string_parses_correctly() {
        let registry = oxidized_world::registry::BlockRegistry::load().unwrap();
        let config = FlatWorldConfig::from_layers_string(
            "minecraft:bedrock,minecraft:dirt*2,minecraft:grass_block",
            &registry,
        )
        .unwrap();
        assert_eq!(config.total_height(), 4);
        assert_eq!(config.layers[0].block, BEDROCK);
        assert_eq!(config.layers[1].block, DIRT);
        assert_eq!(config.layers[1].height, 2);
        assert_eq!(config.layers[2].block, GRASS_BLOCK);
    }

    #[test]
    fn from_layers_string_rejects_empty() {
        let registry = oxidized_world::registry::BlockRegistry::load().unwrap();
        assert!(FlatWorldConfig::from_layers_string("", &registry).is_err());
    }

    #[test]
    fn from_layers_string_rejects_unknown_block() {
        let registry = oxidized_world::registry::BlockRegistry::load().unwrap();
        assert!(
            FlatWorldConfig::from_layers_string("minecraft:nonexistent_block", &registry).is_err()
        );
    }

    #[test]
    fn custom_config_total_height() {
        let config = FlatWorldConfig {
            layers: vec![
                FlatLayerInfo {
                    block: BEDROCK,
                    height: 5,
                },
                FlatLayerInfo {
                    block: STONE,
                    height: 50,
                },
            ],
            flattened: flatten_layers(&[
                FlatLayerInfo {
                    block: BEDROCK,
                    height: 5,
                },
                FlatLayerInfo {
                    block: STONE,
                    height: 50,
                },
            ]),
            biome: "minecraft:plains".into(),
            has_features: false,
            has_lakes: false,
        };
        assert_eq!(config.total_height(), 55);
    }

    #[test]
    fn from_layers_skips_zero_height() {
        let config = FlatWorldConfig::from_layers(&[(BEDROCK, 1), (DIRT, 0), (GRASS_BLOCK, 1)]);
        assert_eq!(config.layers.len(), 2);
        assert_eq!(config.total_height(), 2);
    }

    #[test]
    fn from_layers_string_skips_zero_height() {
        let registry = oxidized_world::registry::BlockRegistry::load().unwrap();
        let config = FlatWorldConfig::from_layers_string(
            "minecraft:bedrock,minecraft:dirt*0,minecraft:grass_block",
            &registry,
        )
        .unwrap();
        assert_eq!(config.layers.len(), 2);
        assert_eq!(config.total_height(), 2);
    }

    #[test]
    fn total_height_clamped_to_world_height() {
        let config = FlatWorldConfig::from_layers(&[(BEDROCK, 500)]);
        // 500 > OVERWORLD_HEIGHT (384), so clamped
        assert_eq!(config.total_height(), OVERWORLD_HEIGHT);
    }
}
