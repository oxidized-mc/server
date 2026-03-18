//! Well-known block state ID constants from vanilla 26.1-pre-3.

use super::block::BlockStateId;

/// Air — state 0, the empty block.
pub const AIR: BlockStateId = BlockStateId(0);
/// Stone — default state.
pub const STONE: BlockStateId = BlockStateId(1);
/// Granite — default state.
pub const GRANITE: BlockStateId = BlockStateId(2);
/// Diorite — default state.
pub const DIORITE: BlockStateId = BlockStateId(4);
/// Andesite — default state.
pub const ANDESITE: BlockStateId = BlockStateId(6);
/// Grass block — default state (snowy=false).
pub const GRASS_BLOCK: BlockStateId = BlockStateId(9);
/// Dirt — default state.
pub const DIRT: BlockStateId = BlockStateId(10);
/// Cobblestone — default state.
pub const COBBLESTONE: BlockStateId = BlockStateId(14);
/// Oak planks — default state.
pub const OAK_PLANKS: BlockStateId = BlockStateId(15);
/// Bedrock — default state.
pub const BEDROCK: BlockStateId = BlockStateId(85);
/// Water — default state (level=0).
pub const WATER: BlockStateId = BlockStateId(86);
/// Lava — default state (level=0).
pub const LAVA: BlockStateId = BlockStateId(102);
/// Sand — default state.
pub const SAND: BlockStateId = BlockStateId(118);
/// Gravel — default state.
pub const GRAVEL: BlockStateId = BlockStateId(124);
/// Oak log — default state (axis=y).
pub const OAK_LOG: BlockStateId = BlockStateId(137);
/// Oak leaves — default state.
pub const OAK_LEAVES: BlockStateId = BlockStateId(279);
/// Glass — default state.
pub const GLASS: BlockStateId = BlockStateId(562);
/// Iron block — default state.
pub const IRON_BLOCK: BlockStateId = BlockStateId(2339);
/// Gold block — default state.
pub const GOLD_BLOCK: BlockStateId = BlockStateId(2338);
/// Obsidian — default state.
pub const OBSIDIAN: BlockStateId = BlockStateId(3369);
/// Torch — default state.
pub const TORCH: BlockStateId = BlockStateId(3370);
/// Spawner — default state.
pub const SPAWNER: BlockStateId = BlockStateId(3888);
/// Oak stairs — default state.
pub const OAK_STAIRS: BlockStateId = BlockStateId(3918);
/// Chest — default state.
pub const CHEST: BlockStateId = BlockStateId(3988);
/// Diamond ore — default state.
pub const DIAMOND_ORE: BlockStateId = BlockStateId(5307);
/// Diamond block — default state.
pub const DIAMOND_BLOCK: BlockStateId = BlockStateId(5309);
/// Crafting table — default state.
pub const CRAFTING_TABLE: BlockStateId = BlockStateId(5310);
/// Furnace — default state.
pub const FURNACE: BlockStateId = BlockStateId(5328);
/// Redstone wire — default state.
pub const REDSTONE_WIRE: BlockStateId = BlockStateId(5171);
/// Oak door — default state.
pub const OAK_DOOR: BlockStateId = BlockStateId(5666);
/// Ladder — default state.
pub const LADDER: BlockStateId = BlockStateId(5720);
/// Rail — default state.
pub const RAIL: BlockStateId = BlockStateId(5728);
/// Lever — default state.
pub const LEVER: BlockStateId = BlockStateId(6780);
/// Stone pressure plate — default state.
pub const STONE_PRESSURE_PLATE: BlockStateId = BlockStateId(6796);
/// Redstone torch — default state.
pub const REDSTONE_TORCH: BlockStateId = BlockStateId(6885);
/// Stone button — default state.
pub const STONE_BUTTON: BlockStateId = BlockStateId(6904);
/// Ice — default state.
pub const ICE: BlockStateId = BlockStateId(6927);
/// Snow block — default state.
pub const SNOW_BLOCK: BlockStateId = BlockStateId(6928);
/// Cactus — default state.
pub const CACTUS: BlockStateId = BlockStateId(6929);
/// Netherrack — default state.
pub const NETHERRACK: BlockStateId = BlockStateId(6997);
/// Glowstone — default state.
pub const GLOWSTONE: BlockStateId = BlockStateId(7016);
/// End stone — default state.
pub const END_STONE: BlockStateId = BlockStateId(9477);
/// Emerald block — default state.
pub const EMERALD_BLOCK: BlockStateId = BlockStateId(9727);
/// Command block — default state.
pub const COMMAND_BLOCK: BlockStateId = BlockStateId(9974);
/// Barrier — default state.
pub const BARRIER: BlockStateId = BlockStateId(12534);
/// Iron trapdoor — default state.
pub const IRON_TRAPDOOR: BlockStateId = BlockStateId(12582);

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_air_constant() {
        assert_eq!(AIR, BlockStateId(0));
    }

    #[test]
    fn test_stone_constant() {
        assert_eq!(STONE, BlockStateId(1));
    }
}
