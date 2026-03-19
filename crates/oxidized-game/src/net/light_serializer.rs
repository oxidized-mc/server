//! Serializes light data for network packets.
//!
//! Builds [`LightUpdateData`] from a chunk's sky/block light layers,
//! using Java BitSet encoding for section masks and 2048-byte nibble
//! arrays for per-section light data.

use oxidized_protocol::packets::play::LightUpdateData;
use oxidized_world::chunk::DataLayer;

/// Builds [`LightUpdateData`] from per-section sky/block light layers.
///
/// The layers slice includes the two border sections (one below, one above),
/// so for a 24-section overworld chunk there are 26 entries.
///
/// Light encoding rules (matching vanilla `ClientboundLightUpdatePacketData`):
/// - A section with non-empty data (at least one non-zero nibble) → set bit in
///   `sky_y_mask` / `block_y_mask` and include the 2048-byte array.
/// - A section with empty data (all zeros) or `None` → set bit in
///   `empty_sky_y_mask` / `empty_block_y_mask`.
/// - Masks use Java's `BitSet.toLongArray()` format: `VarInt(count) i64[]`.
pub fn build_light_data(
    sky_light: &[Option<DataLayer>],
    block_light: &[Option<DataLayer>],
) -> LightUpdateData {
    let section_count = sky_light.len();
    let mut sky_mask = 0u64;
    let mut block_mask = 0u64;
    let mut empty_sky = 0u64;
    let mut empty_block = 0u64;
    let mut sky_updates = Vec::new();
    let mut block_updates = Vec::new();

    for i in 0..section_count {
        match &sky_light[i] {
            Some(layer) if !layer.is_empty() => {
                sky_mask |= 1u64 << i;
                sky_updates.push(layer.as_bytes().to_vec());
            },
            _ => {
                empty_sky |= 1u64 << i;
            },
        }

        if i < block_light.len() {
            match &block_light[i] {
                Some(layer) if !layer.is_empty() => {
                    block_mask |= 1u64 << i;
                    block_updates.push(layer.as_bytes().to_vec());
                },
                _ => {
                    empty_block |= 1u64 << i;
                },
            }
        } else {
            empty_block |= 1u64 << i;
        }
    }

    LightUpdateData {
        sky_y_mask: bitset_to_longs(sky_mask),
        block_y_mask: bitset_to_longs(block_mask),
        empty_sky_y_mask: bitset_to_longs(empty_sky),
        empty_block_y_mask: bitset_to_longs(empty_block),
        sky_updates,
        block_updates,
    }
}

/// Converts a u64 bitmask to Java BitSet long array format.
///
/// An empty BitSet is represented as an empty array (0 longs).
fn bitset_to_longs(bits: u64) -> Vec<i64> {
    if bits == 0 {
        Vec::new()
    } else {
        vec![bits as i64]
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use oxidized_world::chunk::DataLayer;

    #[test]
    fn test_empty_light_produces_zero_masks() {
        let sky: Vec<Option<DataLayer>> = vec![None; 26];
        let block: Vec<Option<DataLayer>> = vec![None; 26];
        let data = build_light_data(&sky, &block);
        assert!(data.sky_y_mask.is_empty());
        assert!(data.block_y_mask.is_empty());
        assert!(data.sky_updates.is_empty());
        assert!(data.block_updates.is_empty());
        // All sections should be empty
        assert!(!data.empty_sky_y_mask.is_empty());
        assert!(!data.empty_block_y_mask.is_empty());
    }

    #[test]
    fn test_full_sky_light_section() {
        let mut sky: Vec<Option<DataLayer>> = vec![None; 26];
        sky[1] = Some(DataLayer::filled(15));
        let block: Vec<Option<DataLayer>> = vec![None; 26];
        let data = build_light_data(&sky, &block);

        // Bit 1 should be set in sky mask
        assert_eq!(data.sky_y_mask.len(), 1);
        assert_eq!(data.sky_y_mask[0] & (1 << 1), 1 << 1);
        assert_eq!(data.sky_updates.len(), 1);
        assert_eq!(data.sky_updates[0].len(), 2048);
    }

    #[test]
    fn test_all_zero_layer_is_empty() {
        let mut sky: Vec<Option<DataLayer>> = vec![None; 26];
        // DataLayer::new() is all zeros — should be treated as empty
        sky[5] = Some(DataLayer::new());
        let block: Vec<Option<DataLayer>> = vec![None; 26];
        let data = build_light_data(&sky, &block);

        // Should NOT appear in sky_y_mask (it's all zeros)
        if !data.sky_y_mask.is_empty() {
            assert_eq!(data.sky_y_mask[0] & (1 << 5), 0);
        }
        // Should appear in empty_sky_y_mask
        assert_eq!(data.empty_sky_y_mask[0] & (1 << 5), 1 << 5);
    }

    #[test]
    fn test_multiple_sections() {
        let mut sky: Vec<Option<DataLayer>> = vec![None; 26];
        sky[0] = Some(DataLayer::filled(15));
        sky[3] = Some(DataLayer::filled(10));
        let block: Vec<Option<DataLayer>> = vec![None; 26];
        let data = build_light_data(&sky, &block);

        assert_eq!(data.sky_y_mask.len(), 1);
        assert_eq!(data.sky_y_mask[0] & (1 << 0), 1);
        assert_eq!(data.sky_y_mask[0] & (1 << 3), 1 << 3);
        assert_eq!(data.sky_updates.len(), 2);
    }

    #[test]
    fn test_block_light_single_section() {
        let sky: Vec<Option<DataLayer>> = vec![None; 26];
        let mut block: Vec<Option<DataLayer>> = vec![None; 26];
        block[4] = Some(DataLayer::filled(12));
        let data = build_light_data(&sky, &block);

        assert_eq!(data.block_y_mask.len(), 1);
        assert_eq!(data.block_y_mask[0] & (1 << 4), 1 << 4);
        assert_eq!(data.block_updates.len(), 1);
        assert_eq!(data.block_updates[0].len(), 2048);
        // Sky should still be all empty
        assert!(data.sky_y_mask.is_empty());
    }

    #[test]
    fn test_mixed_sky_and_block_light() {
        let mut sky: Vec<Option<DataLayer>> = vec![None; 26];
        let mut block: Vec<Option<DataLayer>> = vec![None; 26];
        sky[2] = Some(DataLayer::filled(15));
        block[5] = Some(DataLayer::filled(8));
        block[2] = Some(DataLayer::filled(4));
        let data = build_light_data(&sky, &block);

        // Sky: bit 2 set
        assert_eq!(data.sky_y_mask.len(), 1);
        assert_ne!(data.sky_y_mask[0] & (1 << 2), 0);
        assert_eq!(data.sky_updates.len(), 1);

        // Block: bits 2 and 5 set
        assert_eq!(data.block_y_mask.len(), 1);
        assert_ne!(data.block_y_mask[0] & (1 << 2), 0);
        assert_ne!(data.block_y_mask[0] & (1 << 5), 0);
        assert_eq!(data.block_updates.len(), 2);
    }

    #[test]
    fn test_all_sections_filled() {
        let sky: Vec<Option<DataLayer>> = (0..26).map(|_| Some(DataLayer::filled(15))).collect();
        let block: Vec<Option<DataLayer>> = vec![None; 26];
        let data = build_light_data(&sky, &block);

        // All 26 bits should be set in sky_y_mask
        assert_eq!(data.sky_y_mask.len(), 1);
        let expected_mask = (1i64 << 26) - 1;
        assert_eq!(data.sky_y_mask[0], expected_mask);
        assert_eq!(data.sky_updates.len(), 26);
        // Empty sky mask should have no bits set
        assert!(data.empty_sky_y_mask.is_empty());
    }
}
