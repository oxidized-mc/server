//! Container menu types and the foundation for container interactions.
//!
//! This module defines the [`MenuType`] enum (all container types in Minecraft)
//! and will host the `ContainerMenu` trait in a future phase when full
//! container interaction (shift-click, drag, double-click) is implemented.

/// All container/menu types in Minecraft.
///
/// The discriminant values match the vanilla registry IDs sent on the wire
/// in `ClientboundOpenScreenPacket`.
///
/// # Examples
///
/// ```
/// use oxidized_game::inventory::MenuType;
///
/// let menu = MenuType::Generic9x3;
/// assert_eq!(menu as i32, 2);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum MenuType {
    /// 9×1 chest (e.g., dropper, dispenser).
    Generic9x1 = 0,
    /// 9×2 chest.
    Generic9x2 = 1,
    /// 9×3 chest (standard single chest).
    Generic9x3 = 2,
    /// 9×4 chest.
    Generic9x4 = 3,
    /// 9×5 chest.
    Generic9x5 = 4,
    /// 9×6 chest (double chest).
    Generic9x6 = 5,
    /// 3×3 grid (e.g., dropper/dispenser).
    Generic3x3 = 6,
    /// 3×3 crafter.
    Crafter3x3 = 7,
    /// Anvil.
    Anvil = 8,
    /// Beacon.
    Beacon = 9,
    /// Blast furnace.
    BlastFurnace = 10,
    /// Brewing stand.
    BrewingStand = 11,
    /// Crafting table (3×3).
    Crafting = 12,
    /// Enchantment table.
    Enchantment = 13,
    /// Furnace.
    Furnace = 14,
    /// Grindstone.
    Grindstone = 15,
    /// Hopper.
    Hopper = 16,
    /// Lectern.
    Lectern = 17,
    /// Loom.
    Loom = 18,
    /// Merchant (villager trade).
    Merchant = 19,
    /// Shulker box.
    ShulkerBox = 20,
    /// Smithing table.
    SmithingTable = 21,
    /// Smoker.
    Smoker = 22,
    /// Cartography table.
    CartographyTable = 23,
    /// Stonecutter.
    StoneCutter = 24,
}

impl MenuType {
    /// Creates a `MenuType` from a registry ID, returning `None` for unknown IDs.
    pub fn from_id(id: i32) -> Option<Self> {
        match id {
            0 => Some(Self::Generic9x1),
            1 => Some(Self::Generic9x2),
            2 => Some(Self::Generic9x3),
            3 => Some(Self::Generic9x4),
            4 => Some(Self::Generic9x5),
            5 => Some(Self::Generic9x6),
            6 => Some(Self::Generic3x3),
            7 => Some(Self::Crafter3x3),
            8 => Some(Self::Anvil),
            9 => Some(Self::Beacon),
            10 => Some(Self::BlastFurnace),
            11 => Some(Self::BrewingStand),
            12 => Some(Self::Crafting),
            13 => Some(Self::Enchantment),
            14 => Some(Self::Furnace),
            15 => Some(Self::Grindstone),
            16 => Some(Self::Hopper),
            17 => Some(Self::Lectern),
            18 => Some(Self::Loom),
            19 => Some(Self::Merchant),
            20 => Some(Self::ShulkerBox),
            21 => Some(Self::SmithingTable),
            22 => Some(Self::Smoker),
            23 => Some(Self::CartographyTable),
            24 => Some(Self::StoneCutter),
            _ => None,
        }
    }

    /// Returns the registry ID for this menu type.
    pub fn id(self) -> i32 {
        self as i32
    }
}

/// State ID for container synchronization.
///
/// Wraps at 32768 (`& 0x7FFF`) to match vanilla behavior. Incremented
/// on every server-side slot change; the client sends its known state_id
/// with each click for optimistic locking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ContainerStateId(i32);

impl ContainerStateId {
    /// Creates a new state ID starting at 0.
    pub fn new() -> Self {
        Self(0)
    }

    /// Returns the current value.
    pub fn value(self) -> i32 {
        self.0
    }

    /// Increments the state ID, wrapping at 32768.
    pub fn increment(&mut self) -> i32 {
        self.0 = (self.0 + 1) & 0x7FFF;
        self.0
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_type_from_id_roundtrip() {
        for id in 0..=24 {
            let mt = MenuType::from_id(id).unwrap();
            assert_eq!(mt.id(), id);
        }
    }

    #[test]
    fn test_menu_type_unknown_id() {
        assert!(MenuType::from_id(25).is_none());
        assert!(MenuType::from_id(-1).is_none());
        assert!(MenuType::from_id(100).is_none());
    }

    #[test]
    fn test_container_state_id_increment() {
        let mut sid = ContainerStateId::new();
        assert_eq!(sid.value(), 0);
        assert_eq!(sid.increment(), 1);
        assert_eq!(sid.increment(), 2);
        assert_eq!(sid.value(), 2);
    }

    #[test]
    fn test_container_state_id_wraps() {
        let mut sid = ContainerStateId(0x7FFE);
        assert_eq!(sid.increment(), 0x7FFF);
        assert_eq!(sid.increment(), 0); // wraps
        assert_eq!(sid.increment(), 1);
    }
}
