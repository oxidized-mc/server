# Phase 21 — Inventory & Items

**Crate:** `oxidized-game`  
**Reward:** Player inventory visible, items persist, creative mode gives items.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-024: Inventory](../adr/adr-024-inventory.md) — transactional slot modification with optimistic locking


## Goal

Implement the full player inventory model (36 main slots + armor + offhand),
the container packet protocol, slot click handling, hotbar selection, creative
mode item giving, and the pick-block flow. Items are serialized to/from NBT
using the 1.20.5+ `DataComponentPatch` format and persist across sessions via
the player data storage from Phase 20.

---

## Java Reference

| Concept | Java class |
|---------|-----------|
| Player inventory | `net.minecraft.world.entity.player.PlayerInventory` |
| Container menu base | `net.minecraft.world.inventory.AbstractContainerMenu` |
| Set carried item (C→S) | `net.minecraft.server.network.ServerGamePacketListenerImpl#handleSetCarriedItem` |
| Creative mode slot (C→S) | `net.minecraft.server.network.ServerGamePacketListenerImpl#handleSetCreativeModeSlot` |
| Container content packet | `net.minecraft.network.protocol.game.ClientboundContainerSetContentPacket` |
| Container slot packet | `net.minecraft.network.protocol.game.ClientboundContainerSetSlotPacket` |
| Set carried item packet (S→C) | `net.minecraft.network.protocol.game.ClientboundSetCarriedItemPacket` |
| Take item packet | `net.minecraft.network.protocol.game.ClientboundTakeItemEntityPacket` |
| Open screen packet | `net.minecraft.network.protocol.game.ClientboundOpenScreenPacket` |
| ItemStack | `net.minecraft.world.item.ItemStack` |

---

## Tasks

### 21.1 — ItemStack (`oxidized-game/src/inventory/item_stack.rs`)

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ItemStack {
    /// Canonical resource key, e.g. "minecraft:diamond_sword"
    pub item: ItemId,
    pub count: i32,
    /// 1.20.5+ DataComponentPatch: only components that differ from defaults.
    pub components: DataComponentPatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ItemId(pub String);

/// Sparse map of component type → component value (NBT).
/// Only entries that differ from the item's default prototype are stored.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DataComponentPatch {
    pub added:   std::collections::HashMap<String, NbtTag>,
    pub removed: std::collections::HashSet<String>,
}

pub const EMPTY: ItemStack = ItemStack {
    item: ItemId(String::new()),
    count: 0,
    components: DataComponentPatch { added: /* ... */ , removed: /* ... */ },
};

impl ItemStack {
    pub fn new(item: impl Into<String>, count: i32) -> Self {
        Self {
            item: ItemId(item.into()),
            count,
            components: DataComponentPatch::default(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.count <= 0 || self.item.0.is_empty()
    }

    pub fn with_count(mut self, count: i32) -> Self {
        self.count = count;
        self
    }

    pub fn split(&mut self, amount: i32) -> Self {
        let taken = amount.min(self.count);
        self.count -= taken;
        Self {
            item: self.item.clone(),
            count: taken,
            components: self.components.clone(),
        }
    }

    /// Serialize to NBT for the network protocol (slot data inside container packets).
    pub fn to_nbt(&self) -> Option<CompoundTag> {
        if self.is_empty() { return None; }
        let mut tag = CompoundTag::new();
        tag.put_string("id", &self.item.0);
        tag.put_int("count", self.count);
        if !self.components.added.is_empty() || !self.components.removed.is_empty() {
            let patch_tag = self.components.to_nbt();
            tag.put_compound("components", patch_tag);
        }
        Some(tag)
    }

    pub fn from_nbt(tag: &CompoundTag) -> anyhow::Result<Self> {
        let item = tag.get_string("id")?.to_string();
        let count = tag.get_int("count").unwrap_or(1);
        let components = if let Ok(c) = tag.get_compound("components") {
            DataComponentPatch::from_nbt(c)?
        } else {
            DataComponentPatch::default()
        };
        Ok(Self { item: ItemId(item), count, components })
    }
}
```

### 21.2 — PlayerInventory (`oxidized-game/src/inventory/player_inventory.rs`)

```rust
/// All player inventory slots.
/// Slot numbers follow the Minecraft protocol window 0 convention:
///   0     = crafting output
///   1–4   = crafting grid (2×2)
///   5–8   = armor (head/chest/legs/feet)
///   9–35  = main inventory
///   36–44 = hotbar (visual slots 0–8)
///   45    = offhand
pub struct PlayerInventory {
    /// 36 main + 4 armor + 1 offhand = 41 physical slots.
    /// Stored as: [hotbar 0..9][main 9..36][armor 36..40][offhand 40]
    slots: [ItemStack; 41],
    /// Selected hotbar slot (0–8)
    pub selected: u8,
}

impl PlayerInventory {
    pub const HOTBAR_START: usize = 0;
    pub const HOTBAR_END: usize = 9;    // exclusive
    pub const MAIN_START: usize = 9;
    pub const MAIN_END: usize = 36;
    pub const ARMOR_START: usize = 36;
    pub const ARMOR_END: usize = 40;
    pub const OFFHAND_SLOT: usize = 40;
    pub const TOTAL_SLOTS: usize = 41;

    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| ItemStack::empty()),
            selected: 0,
        }
    }

    pub fn get(&self, slot: usize) -> &ItemStack {
        &self.slots[slot]
    }

    pub fn get_mut(&mut self, slot: usize) -> &mut ItemStack {
        &mut self.slots[slot]
    }

    pub fn set(&mut self, slot: usize, stack: ItemStack) {
        self.slots[slot] = stack;
    }

    pub fn get_selected(&self) -> &ItemStack {
        &self.slots[self.selected as usize]
    }

    pub fn get_armor(&self, slot: u8) -> &ItemStack {
        &self.slots[Self::ARMOR_START + slot as usize]
    }

    pub fn get_offhand(&self) -> &ItemStack {
        &self.slots[Self::OFFHAND_SLOT]
    }

    /// Iterator over all slots as (internal_index, &ItemStack).
    pub fn all_slots(&self) -> impl Iterator<Item = (usize, &ItemStack)> {
        self.slots.iter().enumerate()
    }

    /// Add a stack to the first available slot. Returns the remaining amount that
    /// couldn't be inserted (0 = fully inserted).
    pub fn add_item(&mut self, mut stack: ItemStack) -> i32 {
        // First fill existing stacks of the same item
        for i in 0..Self::TOTAL_SLOTS {
            if self.slots[i].item == stack.item && !self.slots[i].is_empty() {
                let max = max_stack_size(&stack.item);
                let space = max - self.slots[i].count;
                if space > 0 {
                    let moved = stack.count.min(space);
                    self.slots[i].count += moved;
                    stack.count -= moved;
                    if stack.count <= 0 { return 0; }
                }
            }
        }
        // Then fill empty slots
        for i in 0..Self::TOTAL_SLOTS {
            if self.slots[i].is_empty() {
                self.slots[i] = stack.clone();
                return 0;
            }
        }
        stack.count // leftovers
    }

    /// Convert internal slot index to protocol window-0 slot index.
    /// Window 0 crafting/armor layout:
    ///   5-8 → armor; 9-35 → main; 36-44 → hotbar; 45 → offhand
    pub fn to_protocol_slot(internal: usize) -> i16 {
        match internal {
            0..9   => (internal as i16) + 36, // hotbar → protocol 36–44
            9..36  => internal as i16,         // main   → protocol 9–35
            36..40 => (internal as i16) - 31,  // armor  → protocol 5–8
            40     => 45,                       // offhand → protocol 45
            _      => -1,
        }
    }

    /// Convert protocol window-0 slot index to internal index.
    pub fn from_protocol_slot(protocol: i16) -> Option<usize> {
        match protocol {
            5..=8  => Some((protocol as usize) + 31),  // armor
            9..=35 => Some(protocol as usize),           // main
            36..=44 => Some((protocol as usize) - 36),  // hotbar
            45     => Some(40),                           // offhand
            _      => None,
        }
    }
}
```

### 21.3 — Container packets (`oxidized-protocol/src/packets/clientbound/game.rs`)

```rust
/// 0x13 – full inventory sync
#[derive(Debug, Clone)]
pub struct ClientboundContainerSetContentPacket {
    pub container_id: u8,
    pub state_id: i32,              // VarInt
    pub items: Vec<Option<ItemStack>>,
    pub cursor_item: Option<ItemStack>,
}

/// 0x15 – single slot update
#[derive(Debug, Clone)]
pub struct ClientboundContainerSetSlotPacket {
    pub container_id: i8,           // -1 = cursor, 0 = player inventory
    pub state_id: i32,
    pub slot: i16,
    pub item: Option<ItemStack>,
}

/// 0x53 – tell the client which hotbar slot is selected
#[derive(Debug, Clone)]
pub struct ClientboundSetCarriedItemPacket {
    pub slot: i8,                   // 0–8
}

/// 0x6D – item pick-up animation (item entity absorbed by player)
#[derive(Debug, Clone)]
pub struct ClientboundTakeItemEntityPacket {
    pub collected_entity_id: i32,   // VarInt
    pub player_entity_id: i32,      // VarInt
    pub amount: i32,                 // VarInt
}

/// 0x34 – open a container screen
#[derive(Debug, Clone)]
pub struct ClientboundOpenScreenPacket {
    pub container_id: i32,          // VarInt
    pub menu_type: i32,             // VarInt registry id (e.g. minecraft:chest)
    pub title: Component,
}

impl Encode for ClientboundContainerSetContentPacket {
    fn encode(&self, buf: &mut impl BufMut) -> anyhow::Result<()> {
        self.container_id.encode(buf)?;
        VarInt(self.state_id).encode(buf)?;
        VarInt(self.items.len() as i32).encode(buf)?;
        for item in &self.items {
            encode_slot(buf, item.as_ref())?;
        }
        encode_slot(buf, self.cursor_item.as_ref())?;
        Ok(())
    }
}
```

### 21.4 — Serverbound inventory packets

```rust
/// 0x2F – player selects a hotbar slot (0–8)
#[derive(Debug, Clone)]
pub struct ServerboundSetCarriedItemPacket {
    pub slot: i16,
}

/// 0x30 – creative mode: place an item directly into a slot
#[derive(Debug, Clone)]
pub struct ServerboundSetCreativeModeSlotPacket {
    pub slot: i16,
    pub item: Option<ItemStack>,
}

/// 0x19 – pick block from block (middle-click on block)
#[derive(Debug, Clone)]
pub struct ServerboundPickItemFromBlockPacket {
    pub pos: BlockPos,
    pub include_data: bool,
}

/// 0x1A – pick block from entity (middle-click on entity)
#[derive(Debug, Clone)]
pub struct ServerboundPickItemFromEntityPacket {
    pub entity_id: i32,
    pub include_data: bool,
}
```

### 21.5 — Packet handlers (`oxidized-game/src/player/inventory_handler.rs`)

```rust
impl PlayerConnection {
    pub async fn handle_set_carried_item(
        &mut self,
        packet: ServerboundSetCarriedItemPacket,
    ) -> anyhow::Result<()> {
        let slot = packet.slot as u8;
        anyhow::ensure!(slot < 9, "hotbar slot out of range: {slot}");
        self.player.inventory.selected = slot;
        Ok(())
    }

    pub async fn handle_set_creative_mode_slot(
        &mut self,
        packet: ServerboundSetCreativeModeSlotPacket,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.player.game_mode == GameType::Creative,
            "SetCreativeModeSlot only allowed in creative mode"
        );
        let internal = PlayerInventory::from_protocol_slot(packet.slot)
            .ok_or_else(|| anyhow::anyhow!("invalid slot: {}", packet.slot))?;
        let stack = packet.item.unwrap_or_else(ItemStack::empty);
        self.player.inventory.set(internal, stack.clone());
        self.increment_state_id();

        // Echo back so client state is in sync
        self.send_packet(ClientboundContainerSetSlotPacket {
            container_id: 0,
            state_id: self.state_id,
            slot: packet.slot,
            item: if stack.is_empty() { None } else { Some(stack) },
        }).await?;
        Ok(())
    }

    pub async fn handle_pick_item_from_block(
        &mut self,
        packet: ServerboundPickItemFromBlockPacket,
    ) -> anyhow::Result<()> {
        let block = self.player.level().get_block_state(packet.pos);
        let pick_item = block.pick_item(packet.include_data);
        self.give_or_pick_to_hotbar(pick_item).await
    }

    async fn give_or_pick_to_hotbar(&mut self, stack: ItemStack) -> anyhow::Result<()> {
        // 1. If item already in hotbar, switch selected slot to it
        for slot in 0..9usize {
            if self.player.inventory.get(slot).item == stack.item {
                self.player.inventory.selected = slot as u8;
                self.send_packet(ClientboundSetCarriedItemPacket {
                    slot: slot as i8,
                }).await?;
                return Ok(());
            }
        }
        // 2. Creative: directly place in selected hotbar slot
        if self.player.game_mode == GameType::Creative {
            let sel = self.player.inventory.selected as usize;
            self.player.inventory.set(sel, stack.clone());
            self.send_slot_packet(sel).await?;
        }
        Ok(())
    }

    /// Send a full inventory resync (0x13) to the player.
    pub async fn send_inventory(&mut self) -> anyhow::Result<()> {
        let items: Vec<_> = (0..46)
            .map(|proto_slot| {
                PlayerInventory::from_protocol_slot(proto_slot)
                    .and_then(|i| {
                        let s = self.player.inventory.get(i);
                        if s.is_empty() { None } else { Some(s.clone()) }
                    })
            })
            .collect();

        self.send_packet(ClientboundContainerSetContentPacket {
            container_id: 0,
            state_id: self.state_id,
            items,
            cursor_item: None,
        }).await
    }
}
```

---

## Data Structures

```rust
// oxidized-game/src/inventory/container.rs

/// Trait implemented by every menu type (player, chest, furnace, etc.)
pub trait ContainerMenu: Send + Sync {
    fn container_id(&self) -> u8;
    fn menu_type(&self) -> MenuType;
    fn title(&self) -> Component;
    fn slot_count(&self) -> usize;
    fn get_slot(&self, slot: usize) -> &ItemStack;
    fn set_slot(&mut self, slot: usize, stack: ItemStack);
    fn can_take_stack(&self, player: &ServerPlayer, slot: usize) -> bool;
    /// Shift-click logic: returns the stack moved, or empty if nothing moved.
    fn quick_move_stack(&mut self, player: &mut ServerPlayer, slot: usize) -> ItemStack;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuType {
    // window type VarInt ids
    Generic9x1 = 0,
    Generic9x2 = 1,
    Generic9x3 = 2,
    Generic9x4 = 3,
    Generic9x5 = 4,
    Generic9x6 = 5,
    Generic3x3 = 6,
    Crafter3x3 = 7,
    Anvil = 8,
    Beacon = 9,
    BlastFurnace = 10,
    BrewingStand = 11,
    Crafting = 12,
    Enchantment = 13,
    Furnace = 14,
    Grindstone = 15,
    Hopper = 16,
    Lectern = 17,
    Loom = 18,
    Merchant = 19,
    ShulkerBox = 20,
    SmithingTable = 21,
    Smoker = 22,
    CartographyTable = 23,
    StoneCutter = 24,
}

// oxidized-game/src/inventory/slot.rs

pub struct Slot {
    pub index: usize,
    pub container: ContainerRef,
}

pub struct SlotAccess<'a> {
    pub item: &'a mut ItemStack,
    pub dirty: &'a mut bool,
}
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // --- ItemStack ---

    #[test]
    fn empty_item_stack_is_empty() {
        let s = ItemStack::empty();
        assert!(s.is_empty());
    }

    #[test]
    fn item_stack_split_reduces_count() {
        let mut s = ItemStack::new("minecraft:stone", 64);
        let split = s.split(16);
        assert_eq!(split.count, 16);
        assert_eq!(s.count, 48);
    }

    #[test]
    fn item_stack_split_does_not_exceed_count() {
        let mut s = ItemStack::new("minecraft:stone", 5);
        let split = s.split(10);
        assert_eq!(split.count, 5);
        assert_eq!(s.count, 0);
    }

    #[test]
    fn item_stack_nbt_roundtrip() {
        let original = ItemStack::new("minecraft:diamond_sword", 1);
        let nbt = original.to_nbt().unwrap();
        let decoded = ItemStack::from_nbt(&nbt).unwrap();
        assert_eq!(decoded.item.0, "minecraft:diamond_sword");
        assert_eq!(decoded.count, 1);
    }

    #[test]
    fn empty_item_stack_to_nbt_returns_none() {
        let s = ItemStack::empty();
        assert!(s.to_nbt().is_none());
    }

    // --- PlayerInventory slot mapping ---

    #[test]
    fn protocol_slot_hotbar_roundtrip() {
        for i in 0u8..9 {
            let proto = PlayerInventory::to_protocol_slot(i as usize);
            let back  = PlayerInventory::from_protocol_slot(proto).unwrap();
            assert_eq!(back, i as usize, "hotbar slot {i} roundtrip failed");
        }
    }

    #[test]
    fn protocol_slot_main_inventory_roundtrip() {
        for i in 9usize..36 {
            let proto = PlayerInventory::to_protocol_slot(i);
            let back  = PlayerInventory::from_protocol_slot(proto).unwrap();
            assert_eq!(back, i, "main inventory slot {i} roundtrip failed");
        }
    }

    #[test]
    fn protocol_slot_armor_roundtrip() {
        for i in 36usize..40 {
            let proto = PlayerInventory::to_protocol_slot(i);
            let back  = PlayerInventory::from_protocol_slot(proto).unwrap();
            assert_eq!(back, i, "armor slot {i} roundtrip failed");
        }
    }

    #[test]
    fn protocol_slot_offhand_roundtrip() {
        let proto = PlayerInventory::to_protocol_slot(40);
        assert_eq!(proto, 45);
        let back = PlayerInventory::from_protocol_slot(45).unwrap();
        assert_eq!(back, 40);
    }

    #[test]
    fn invalid_protocol_slot_returns_none() {
        assert!(PlayerInventory::from_protocol_slot(-1).is_none());
        assert!(PlayerInventory::from_protocol_slot(0).is_none());   // crafting output
        assert!(PlayerInventory::from_protocol_slot(1).is_none());   // crafting grid
        assert!(PlayerInventory::from_protocol_slot(46).is_none());
    }

    // --- PlayerInventory add_item ---

    #[test]
    fn add_item_fills_empty_slot() {
        let mut inv = PlayerInventory::new();
        let leftovers = inv.add_item(ItemStack::new("minecraft:stone", 1));
        assert_eq!(leftovers, 0);
        // First slot (hotbar 0) should contain stone
        assert_eq!(inv.get(0).item.0, "minecraft:stone");
    }

    #[test]
    fn add_item_stacks_with_existing() {
        let mut inv = PlayerInventory::new();
        inv.set(0, ItemStack::new("minecraft:stone", 32));
        let leftovers = inv.add_item(ItemStack::new("minecraft:stone", 16));
        assert_eq!(leftovers, 0);
        assert_eq!(inv.get(0).count, 48);
    }

    #[test]
    fn add_item_returns_overflow_when_full() {
        let mut inv = PlayerInventory::new();
        // Fill all 41 slots with max-stack items
        for i in 0..PlayerInventory::TOTAL_SLOTS {
            inv.set(i, ItemStack::new("minecraft:stone", 64));
        }
        let leftovers = inv.add_item(ItemStack::new("minecraft:stone", 32));
        assert_eq!(leftovers, 32, "should return unfitted amount");
    }

    // --- Creative mode slot packet handler ---

    #[tokio::test]
    async fn creative_slot_places_item_in_correct_slot() {
        let mut conn = make_test_connection(GameType::Creative);
        conn.handle_set_creative_mode_slot(ServerboundSetCreativeModeSlotPacket {
            slot: 36,  // protocol hotbar slot 0
            item: Some(ItemStack::new("minecraft:diamond", 1)),
        }).await.unwrap();

        let internal = PlayerInventory::from_protocol_slot(36).unwrap();
        assert_eq!(conn.player.inventory.get(internal).item.0, "minecraft:diamond");
    }

    #[tokio::test]
    async fn creative_slot_rejected_for_survival_player() {
        let mut conn = make_test_connection(GameType::Survival);
        let result = conn.handle_set_creative_mode_slot(ServerboundSetCreativeModeSlotPacket {
            slot: 36,
            item: Some(ItemStack::new("minecraft:diamond", 1)),
        }).await;
        assert!(result.is_err(), "should reject creative slot in survival");
    }
}
```
