# Phase 30 — Block Entities

**Crate:** `oxidized-game`  
**Reward:** Chests open and hold items, furnaces smelt, signs show text.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-024: Inventory](../adr/adr-024-inventory.md) — transactional slot modification with optimistic locking


## Goal

Implement the block entity system: a `BlockEntity` trait with NBT save/load,
tick dispatch, and update-tag/descriptor-packet generation. Implement concrete
block entities for chests (with lid animation and loot tables), furnaces (fuel
and cook progress), signs (rich-text front/back faces), hoppers (item transfer
pipeline), and mob spawners. Every block entity must correctly round-trip
through NBT and send the correct `ClientboundBlockEntityDataPacket` on initial
chunk send and on state change.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Block entity trait | `BlockEntity` | `net.minecraft.world.level.block.entity.BlockEntity` |
| Chest block entity | `ChestBlockEntity` | `net.minecraft.world.level.block.entity.ChestBlockEntity` |
| Abstract furnace BE | `AbstractFurnaceBlockEntity` | `net.minecraft.world.level.block.entity.AbstractFurnaceBlockEntity` |
| Hopper block entity | `HopperBlockEntity` | `net.minecraft.world.level.block.entity.HopperBlockEntity` |
| Sign block entity | `SignBlockEntity` | `net.minecraft.world.level.block.entity.SignBlockEntity` |
| Mob spawner BE | `SpawnerBlockEntity` | `net.minecraft.world.level.block.entity.SpawnerBlockEntity` |
| BE data packet | `ClientboundBlockEntityDataPacket` | `net.minecraft.network.protocol.game.ClientboundBlockEntityDataPacket` |
| Container set data | `ClientboundContainerSetDataPacket` | `net.minecraft.network.protocol.game.ClientboundContainerSetDataPacket` |
| Block event packet | `ClientboundBlockEventPacket` | `net.minecraft.network.protocol.game.ClientboundBlockEventPacket` |

---

## Tasks

### 30.1 — `BlockEntity` trait

```rust
// crates/oxidized-game/src/block_entity/mod.rs

use oxidized_nbt::NbtCompound;
use glam::IVec3;

/// Core trait every block entity must implement.
pub trait BlockEntity: Send + Sync {
    /// Unique block entity type key (e.g. `minecraft:chest`).
    fn type_id(&self) -> &'static str;

    /// World position of this block entity.
    fn pos(&self) -> IVec3;

    /// Serialize full state to NBT for chunk save / playerdata.
    fn save(&self, tag: &mut NbtCompound);

    /// Deserialize full state from saved NBT.
    fn load(&mut self, tag: &NbtCompound);

    /// Minimal NBT tag for the `ClientboundBlockEntityDataPacket`.
    /// Sent to clients when the chunk is loaded or the BE changes.
    /// May omit heavy data like full item lists (client only needs display info).
    fn get_update_tag(&self) -> NbtCompound;

    /// Handle the update tag received from the server (client-side).
    fn handle_update_tag(&mut self, tag: &NbtCompound) {
        self.load(tag);
    }

    /// Build the `ClientboundBlockEntityDataPacket` for this block entity.
    fn get_desc_update_packet(&self) -> ClientboundBlockEntityDataPacket {
        ClientboundBlockEntityDataPacket {
            pos:     self.pos(),
            type_id: self.type_id().to_string(),
            tag:     self.get_update_tag(),
        }
    }

    /// Optional: called every server tick. Not all block entities tick.
    fn tick(&mut self, _world: &mut dyn BlockEntityWorld) {}

    /// Whether this block entity needs a per-tick call.
    fn is_ticking(&self) -> bool { false }

    /// Mark this BE as needing re-save and packet re-send.
    fn set_changed(&mut self);

    fn is_changed(&self) -> bool;
}

#[derive(Debug, Clone)]
pub struct ClientboundBlockEntityDataPacket {
    pub pos:     IVec3,
    /// Numeric type ID on the wire (registry lookup). Stored as string here for clarity.
    pub type_id: String,
    pub tag:     NbtCompound,
}
```

### 30.2 — `ChestBlockEntity`

```rust
// crates/oxidized-game/src/block_entity/chest.rs

use super::BlockEntity;
use oxidized_nbt::NbtCompound;
use glam::IVec3;

/// NBT schema:
/// - `Items`: list of `{Slot: byte, id: string, count: byte, components: compound?}`
/// - `LootTable`: string (resource location) — if present, Items is generated on open
/// - `LootTableSeed`: long
/// - `CustomName`: json text component string
pub struct ChestBlockEntity {
    pub pos:         IVec3,
    pub items:       [Option<ItemStack>; 27],
    pub loot_table:  Option<ResourceLocation>,
    pub custom_name: Option<String>,
    /// Number of players currently viewing this chest (for lid animation).
    pub open_count:  i32,
    changed: bool,
}

impl ChestBlockEntity {
    pub const SLOT_COUNT: usize = 27;

    pub fn new(pos: IVec3) -> Self {
        Self {
            pos,
            items: std::array::from_fn(|_| None),
            loot_table: None,
            custom_name: None,
            open_count: 0,
            changed: false,
        }
    }

    /// Called when a player opens the chest.
    /// Sends `ClientboundBlockEventPacket(pos, 1, open_count)` for lid animation.
    pub fn start_open(&mut self) {
        self.open_count += 1;
        // Send BlockEventPacket action=1 param=open_count
    }

    /// Called when a player closes the chest.
    pub fn stop_open(&mut self) {
        self.open_count -= 1;
        // Send BlockEventPacket action=1 param=open_count (0 = close lid)
    }

    /// Generate loot if `loot_table` is set and `Items` is empty.
    pub fn unpack_loot(&mut self, seed: i64) {
        if let Some(ref table) = self.loot_table {
            // Fill `items` using the loot table, seeded by seed
            self.loot_table = None;
        }
    }
}

impl BlockEntity for ChestBlockEntity {
    fn type_id(&self) -> &'static str { "minecraft:chest" }
    fn pos(&self) -> IVec3 { self.pos }
    fn set_changed(&mut self) { self.changed = true; }
    fn is_changed(&self) -> bool { self.changed }

    fn save(&self, tag: &mut NbtCompound) {
        let mut items_list = Vec::new();
        for (i, slot) in self.items.iter().enumerate() {
            if let Some(stack) = slot {
                let mut item_tag = NbtCompound::new();
                item_tag.put_byte("Slot", i as i8);
                item_tag.put_string("id", &stack.item.to_string());
                item_tag.put_byte("Count", stack.count as i8);
                items_list.push(item_tag);
            }
        }
        tag.put_list("Items", items_list);
        if let Some(ref lt) = self.loot_table {
            tag.put_string("LootTable", &lt.to_string());
        }
        if let Some(ref name) = self.custom_name {
            tag.put_string("CustomName", name);
        }
    }

    fn load(&mut self, tag: &NbtCompound) {
        self.items = std::array::from_fn(|_| None);
        if let Some(items) = tag.get_list("Items") {
            for item_tag in items {
                let slot  = item_tag.get_byte("Slot").unwrap_or(0) as usize;
                let id    = item_tag.get_string("id").unwrap_or("minecraft:air");
                let count = item_tag.get_byte("Count").unwrap_or(1) as u8;
                if slot < Self::SLOT_COUNT {
                    self.items[slot] = Some(ItemStack::new_count(ResourceLocation::new(id), count));
                }
            }
        }
        self.loot_table  = tag.get_string("LootTable").map(ResourceLocation::new);
        self.custom_name = tag.get_string("CustomName").map(String::from);
    }

    fn get_update_tag(&self) -> NbtCompound {
        // Client needs the full item list for display inside the chest UI
        let mut tag = NbtCompound::new();
        self.save(&mut tag);
        tag
    }
}
```

### 30.3 — `AbstractFurnaceBlockEntity`

```rust
// crates/oxidized-game/src/block_entity/furnace.rs

/// NBT schema:
/// - `Items`: list `[{Slot:0 ingredient}, {Slot:1 fuel}, {Slot:2 output}]`
/// - `BurnTime`: short — ticks of fuel remaining
/// - `CookTime`: short — ticks the current item has cooked
/// - `CookTimeTotal`: short — ticks required for current recipe (default 200)
/// - `RecipesUsed`: compound {recipe_id: uses_count (int)}
pub struct FurnaceBlockEntity {
    pub pos:              IVec3,
    pub kind:             FurnaceKind,
    pub ingredient:       Option<ItemStack>,
    pub fuel:             Option<ItemStack>,
    pub output:           Option<ItemStack>,
    pub burn_time:        u16,
    pub fuel_total:       u16,
    pub cook_time:        u16,
    pub cook_time_total:  u16,
    pub stored_xp:        f32,
    pub recipes_used:     std::collections::HashMap<ResourceLocation, u32>,
    changed: bool,
}

impl FurnaceBlockEntity {
    pub fn new(pos: IVec3, kind: FurnaceKind) -> Self {
        Self {
            pos, kind,
            ingredient: None, fuel: None, output: None,
            burn_time: 0, fuel_total: 0,
            cook_time: 0, cook_time_total: 200,
            stored_xp: 0.0,
            recipes_used: Default::default(),
            changed: false,
        }
    }

    pub fn is_lit(&self) -> bool { self.burn_time > 0 }
}

impl BlockEntity for FurnaceBlockEntity {
    fn type_id(&self) -> &'static str { "minecraft:furnace" }
    fn pos(&self) -> IVec3 { self.pos }
    fn is_ticking(&self) -> bool { true }
    fn set_changed(&mut self) { self.changed = true; }
    fn is_changed(&self) -> bool { self.changed }

    fn tick(&mut self, world: &mut dyn BlockEntityWorld) {
        // Mirror AbstractFurnaceBlockEntity.serverTick logic:
        // 1. Decrement burn_time.
        // 2. If ingredient present and recipe matches, increment cook_time.
        // 3. On cook_time == cook_time_total, produce output, award XP.
        // 4. If fuel slot non-empty and burn_time == 0, consume fuel.
        // 5. Send ClientboundContainerSetDataPacket for all open viewers:
        //    property 0=burn_time, 1=fuel_total, 2=cook_time, 3=cook_time_total.
        // 6. Update lit blockstate (LIT block property).
    }

    fn save(&self, tag: &mut NbtCompound) {
        let mut items = Vec::new();
        let push_item = |items: &mut Vec<NbtCompound>, slot: i8, stack: &Option<ItemStack>| {
            if let Some(s) = stack {
                let mut t = NbtCompound::new();
                t.put_byte("Slot", slot);
                t.put_string("id", &s.item.to_string());
                t.put_byte("Count", s.count as i8);
                items.push(t);
            }
        };
        push_item(&mut items, 0, &self.ingredient);
        push_item(&mut items, 1, &self.fuel);
        push_item(&mut items, 2, &self.output);
        tag.put_list("Items", items);
        tag.put_short("BurnTime",      self.burn_time as i16);
        tag.put_short("CookTime",      self.cook_time as i16);
        tag.put_short("CookTimeTotal", self.cook_time_total as i16);
    }

    fn load(&mut self, tag: &NbtCompound) {
        self.ingredient = None; self.fuel = None; self.output = None;
        if let Some(items) = tag.get_list("Items") {
            for item_tag in items {
                let slot = item_tag.get_byte("Slot").unwrap_or(0);
                let id   = item_tag.get_string("id").unwrap_or("minecraft:air");
                let cnt  = item_tag.get_byte("Count").unwrap_or(1) as u8;
                let stack = Some(ItemStack::new_count(ResourceLocation::new(id), cnt));
                match slot {
                    0 => self.ingredient = stack,
                    1 => self.fuel       = stack,
                    2 => self.output     = stack,
                    _ => {}
                }
            }
        }
        self.burn_time       = tag.get_short("BurnTime").unwrap_or(0) as u16;
        self.cook_time       = tag.get_short("CookTime").unwrap_or(0) as u16;
        self.cook_time_total = tag.get_short("CookTimeTotal").unwrap_or(200) as u16;
    }

    fn get_update_tag(&self) -> NbtCompound {
        let mut tag = NbtCompound::new();
        self.save(&mut tag);
        tag
    }
}
```

### 30.4 — `SignBlockEntity`

```rust
// crates/oxidized-game/src/block_entity/sign.rs

/// NBT schema:
/// - `front_text`: compound { messages: list[4 json strings], color: string, has_glowing_text: bool, is_waxed: bool }
/// - `back_text`:  compound (same schema as front_text)
/// - `is_waxed`:   bool (top-level for 1.20+)
#[derive(Debug, Clone, Default)]
pub struct SignText {
    /// Four lines of JSON text component strings.
    pub messages:         [String; 4],
    pub color:            SignColor,
    pub has_glowing_text: bool,
    /// Filtered text shown to players with chat filtering enabled.
    pub filtered_messages: [String; 4],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SignColor {
    #[default] Black, Red, Green, Brown, Blue, Purple, Cyan, LightGray,
    Gray, Pink, Lime, Yellow, LightBlue, Magenta, Orange, White,
}

pub struct SignBlockEntity {
    pub pos:       IVec3,
    pub front:     SignText,
    pub back:      SignText,
    pub is_waxed:  bool,
    /// UUID of the player currently editing this sign.
    pub editing:   Option<uuid::Uuid>,
    changed: bool,
}

impl SignBlockEntity {
    pub fn new(pos: IVec3) -> Self {
        Self { pos, front: SignText::default(), back: SignText::default(), is_waxed: false, editing: None, changed: false }
    }

    fn save_text(text: &SignText, tag: &mut NbtCompound) {
        let msgs: Vec<NbtCompound> = text.messages.iter()
            .map(|m| { let mut t = NbtCompound::new(); t.put_string("text", m); t })
            .collect();
        tag.put_list("messages", msgs);
        tag.put_string("color", &format!("{:?}", text.color).to_lowercase());
        tag.put_bool("has_glowing_text", text.has_glowing_text);
    }
}

impl BlockEntity for SignBlockEntity {
    fn type_id(&self) -> &'static str { "minecraft:sign" }
    fn pos(&self) -> IVec3 { self.pos }
    fn set_changed(&mut self) { self.changed = true; }
    fn is_changed(&self) -> bool { self.changed }

    fn save(&self, tag: &mut NbtCompound) {
        let mut front_tag = NbtCompound::new();
        let mut back_tag  = NbtCompound::new();
        Self::save_text(&self.front, &mut front_tag);
        Self::save_text(&self.back,  &mut back_tag);
        tag.put_compound("front_text", front_tag);
        tag.put_compound("back_text",  back_tag);
        tag.put_bool("is_waxed", self.is_waxed);
    }

    fn load(&mut self, tag: &NbtCompound) {
        // Parse front_text / back_text compounds, restore messages and color.
        self.is_waxed = tag.get_bool("is_waxed").unwrap_or(false);
    }

    fn get_update_tag(&self) -> NbtCompound {
        let mut tag = NbtCompound::new();
        self.save(&mut tag);
        tag
    }
}
```

### 30.5 — `HopperBlockEntity`

```rust
// crates/oxidized-game/src/block_entity/hopper.rs

/// NBT schema:
/// - `Items`: list of 5 item slots
/// - `TransferCooldown`: int — ticks until next transfer attempt (default 8)
pub struct HopperBlockEntity {
    pub pos:               IVec3,
    pub items:             [Option<ItemStack>; 5],
    pub transfer_cooldown: i32,
    changed: bool,
}

impl HopperBlockEntity {
    pub const SLOT_COUNT: usize = 5;
    /// Ticks between each push/pull attempt.
    pub const TRANSFER_COOLDOWN: i32 = 8;

    pub fn new(pos: IVec3) -> Self {
        Self { pos, items: Default::default(), transfer_cooldown: 0, changed: false }
    }

    /// Try to pull one item from the container or item entity directly above.
    /// Returns true if an item was successfully moved.
    pub fn try_pull_from_above(&mut self, above: Option<&mut dyn Container>) -> bool {
        if let Some(container) = above {
            for slot in 0..container.slot_count() {
                if let Some(stack) = container.get_slot(slot) {
                    if let Some(dest) = self.first_available_slot() {
                        self.items[dest] = Some(stack.clone_one());
                        container.remove_one(slot);
                        self.transfer_cooldown = Self::TRANSFER_COOLDOWN;
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Push one item to the container below. Returns true on success.
    pub fn try_push_to_below(&mut self, below: &mut dyn Container) -> bool {
        for slot in 0..Self::SLOT_COUNT {
            if let Some(ref stack) = self.items[slot] {
                if below.can_accept(stack) {
                    let moved = self.items[slot].take().unwrap();
                    if below.add_item(moved) {
                        self.transfer_cooldown = Self::TRANSFER_COOLDOWN;
                        return true;
                    }
                }
            }
        }
        false
    }

    fn first_available_slot(&self) -> Option<usize> {
        self.items.iter().position(|s| s.is_none())
    }
}

impl BlockEntity for HopperBlockEntity {
    fn type_id(&self) -> &'static str { "minecraft:hopper" }
    fn pos(&self) -> IVec3 { self.pos }
    fn is_ticking(&self) -> bool { true }
    fn set_changed(&mut self) { self.changed = true; }
    fn is_changed(&self) -> bool { self.changed }

    fn tick(&mut self, world: &mut dyn BlockEntityWorld) {
        if self.transfer_cooldown > 0 {
            self.transfer_cooldown -= 1;
            return;
        }
        // Attempt push to below, then pull from above.
        // Reset cooldown to TRANSFER_COOLDOWN if either succeeds.
    }

    fn save(&self, tag: &mut NbtCompound) {
        let items: Vec<NbtCompound> = self.items.iter().enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|stack| {
                let mut t = NbtCompound::new();
                t.put_byte("Slot", i as i8);
                t.put_string("id", &stack.item.to_string());
                t.put_byte("Count", stack.count as i8);
                t
            }))
            .collect();
        tag.put_list("Items", items);
        tag.put_int("TransferCooldown", self.transfer_cooldown);
    }

    fn load(&mut self, tag: &NbtCompound) {
        self.items = Default::default();
        if let Some(items) = tag.get_list("Items") {
            for item_tag in items {
                let slot = item_tag.get_byte("Slot").unwrap_or(0) as usize;
                if slot < Self::SLOT_COUNT {
                    self.items[slot] = Some(ItemStack::new_count(
                        ResourceLocation::new(item_tag.get_string("id").unwrap_or("minecraft:air")),
                        item_tag.get_byte("Count").unwrap_or(1) as u8,
                    ));
                }
            }
        }
        self.transfer_cooldown = tag.get_int("TransferCooldown").unwrap_or(0);
    }

    fn get_update_tag(&self) -> NbtCompound {
        let mut tag = NbtCompound::new();
        self.save(&mut tag);
        tag
    }
}
```

### 30.6 — `MobSpawnerBlockEntity`

```rust
// crates/oxidized-game/src/block_entity/spawner.rs

/// NBT schema:
/// - `EntityId`: string (e.g. `minecraft:zombie`) — legacy; SpawnData preferred
/// - `SpawnData`: compound { entity: compound { id: string, ... } }
/// - `Delay`: short — ticks until next spawn (negative = use random)
/// - `MinSpawnDelay`: short (default 200)
/// - `MaxSpawnDelay`: short (default 800)
/// - `SpawnCount`: short — mobs per spawn event (default 4)
/// - `MaxNearbyEntities`: short — max nearby before suppressing (default 6)
/// - `RequiredPlayerRange`: short — player must be within this range (default 16)
/// - `SpawnRange`: short — spawn radius in blocks (default 4)
pub struct MobSpawnerBlockEntity {
    pub pos:                   IVec3,
    pub entity_id:             ResourceLocation,
    pub delay:                 i16,
    pub min_spawn_delay:       i16,
    pub max_spawn_delay:       i16,
    pub spawn_count:           i16,
    pub max_nearby_entities:   i16,
    pub required_player_range: i16,
    pub spawn_range:           i16,
    changed: bool,
}

impl MobSpawnerBlockEntity {
    pub fn new(pos: IVec3) -> Self {
        Self {
            pos,
            entity_id:             ResourceLocation::new("minecraft:pig"),
            delay:                 20,
            min_spawn_delay:       200,
            max_spawn_delay:       800,
            spawn_count:           4,
            max_nearby_entities:   6,
            required_player_range: 16,
            spawn_range:           4,
            changed: false,
        }
    }
}

impl BlockEntity for MobSpawnerBlockEntity {
    fn type_id(&self) -> &'static str { "minecraft:mob_spawner" }
    fn pos(&self) -> IVec3 { self.pos }
    fn is_ticking(&self) -> bool { true }
    fn set_changed(&mut self) { self.changed = true; }
    fn is_changed(&self) -> bool { self.changed }

    fn tick(&mut self, world: &mut dyn BlockEntityWorld) {
        if self.delay > 0 { self.delay -= 1; return; }
        // Check player within required_player_range.
        // Count nearby entities of entity_id type.
        // If count < max_nearby_entities: spawn up to spawn_count in spawn_range.
        // Reset delay to random in [min_spawn_delay, max_spawn_delay].
    }

    fn save(&self, tag: &mut NbtCompound) {
        tag.put_string("EntityId",            &self.entity_id.to_string());
        tag.put_short("Delay",                self.delay);
        tag.put_short("MinSpawnDelay",        self.min_spawn_delay);
        tag.put_short("MaxSpawnDelay",        self.max_spawn_delay);
        tag.put_short("SpawnCount",           self.spawn_count);
        tag.put_short("MaxNearbyEntities",    self.max_nearby_entities);
        tag.put_short("RequiredPlayerRange",  self.required_player_range);
        tag.put_short("SpawnRange",           self.spawn_range);
    }

    fn load(&mut self, tag: &NbtCompound) {
        self.entity_id             = tag.get_string("EntityId").map(ResourceLocation::new).unwrap_or_else(|| ResourceLocation::new("minecraft:pig"));
        self.delay                 = tag.get_short("Delay").unwrap_or(20);
        self.min_spawn_delay       = tag.get_short("MinSpawnDelay").unwrap_or(200);
        self.max_spawn_delay       = tag.get_short("MaxSpawnDelay").unwrap_or(800);
        self.spawn_count           = tag.get_short("SpawnCount").unwrap_or(4);
        self.max_nearby_entities   = tag.get_short("MaxNearbyEntities").unwrap_or(6);
        self.required_player_range = tag.get_short("RequiredPlayerRange").unwrap_or(16);
        self.spawn_range           = tag.get_short("SpawnRange").unwrap_or(4);
    }

    fn get_update_tag(&self) -> NbtCompound {
        let mut tag = NbtCompound::new();
        self.save(&mut tag);
        tag
    }
}
```

### 30.7 — Block entity registry and world dispatch

```rust
// crates/oxidized-game/src/block_entity/registry.rs

use std::collections::HashMap;
use glam::IVec3;
use super::BlockEntity;

/// All block entities loaded in a single dimension.
pub struct BlockEntityMap {
    entities: HashMap<IVec3, Box<dyn BlockEntity>>,
}

impl BlockEntityMap {
    pub fn new() -> Self { Self { entities: HashMap::new() } }

    pub fn insert(&mut self, entity: Box<dyn BlockEntity>) {
        self.entities.insert(entity.pos(), entity);
    }

    pub fn get(&self, pos: IVec3) -> Option<&dyn BlockEntity> {
        self.entities.get(&pos).map(|b| b.as_ref())
    }

    pub fn get_mut(&mut self, pos: IVec3) -> Option<&mut dyn BlockEntity> {
        self.entities.get_mut(&pos).map(|b| b.as_mut())
    }

    pub fn remove(&mut self, pos: IVec3) -> Option<Box<dyn BlockEntity>> {
        self.entities.remove(&pos)
    }

    /// Tick all ticking block entities. Collect changed ones for packet dispatch.
    pub fn tick_all(&mut self, world: &mut dyn BlockEntityWorld) -> Vec<IVec3> {
        let mut changed = Vec::new();
        for (pos, be) in self.entities.iter_mut() {
            if be.is_ticking() {
                be.tick(world);
            }
            if be.is_changed() {
                changed.push(*pos);
            }
        }
        changed
    }
}
```

---

## Data Structures Summary

```rust
// Key types in oxidized-game::block_entity

pub use mod::{BlockEntity, ClientboundBlockEntityDataPacket};
pub use chest::ChestBlockEntity;
pub use furnace::FurnaceBlockEntity;
pub use sign::{SignBlockEntity, SignText, SignColor};
pub use hopper::HopperBlockEntity;
pub use spawner::MobSpawnerBlockEntity;
pub use registry::BlockEntityMap;
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use oxidized_nbt::NbtCompound;
    use glam::IVec3;

    fn pos() -> IVec3 { IVec3::new(5, 64, 5) }

    // --- ChestBlockEntity NBT round-trip ---

    #[test]
    fn chest_nbt_roundtrip_empty() {
        let chest = ChestBlockEntity::new(pos());
        let mut tag = NbtCompound::new();
        chest.save(&mut tag);
        let mut loaded = ChestBlockEntity::new(pos());
        loaded.load(&tag);
        assert!(loaded.items.iter().all(|s| s.is_none()));
    }

    #[test]
    fn chest_nbt_roundtrip_with_item() {
        let mut chest = ChestBlockEntity::new(pos());
        chest.items[0] = Some(ItemStack::new_count(ResourceLocation::new("minecraft:diamond"), 5));
        let mut tag = NbtCompound::new();
        chest.save(&mut tag);
        let mut loaded = ChestBlockEntity::new(pos());
        loaded.load(&tag);
        let item = loaded.items[0].as_ref().expect("slot 0 should have item");
        assert_eq!(item.item, ResourceLocation::new("minecraft:diamond"));
        assert_eq!(item.count, 5);
    }

    #[test]
    fn chest_custom_name_preserved() {
        let mut chest = ChestBlockEntity::new(pos());
        chest.custom_name = Some("{\"text\":\"Treasure\"}".to_string());
        let mut tag = NbtCompound::new();
        chest.save(&mut tag);
        let mut loaded = ChestBlockEntity::new(pos());
        loaded.load(&tag);
        assert_eq!(loaded.custom_name.as_deref(), Some("{\"text\":\"Treasure\"}"));
    }

    #[test]
    fn chest_open_count_increments() {
        let mut chest = ChestBlockEntity::new(pos());
        assert_eq!(chest.open_count, 0);
        chest.start_open();
        assert_eq!(chest.open_count, 1);
        chest.stop_open();
        assert_eq!(chest.open_count, 0);
    }

    // --- FurnaceBlockEntity NBT round-trip ---

    #[test]
    fn furnace_nbt_roundtrip_times() {
        let mut furnace = FurnaceBlockEntity::new(pos(), FurnaceKind::Smelting);
        furnace.burn_time       = 1234;
        furnace.cook_time       = 50;
        furnace.cook_time_total = 200;
        furnace.ingredient = Some(ItemStack::new(ResourceLocation::new("minecraft:iron_ore")));
        let mut tag = NbtCompound::new();
        furnace.save(&mut tag);
        let mut loaded = FurnaceBlockEntity::new(pos(), FurnaceKind::Smelting);
        loaded.load(&tag);
        assert_eq!(loaded.burn_time, 1234);
        assert_eq!(loaded.cook_time, 50);
        assert_eq!(loaded.cook_time_total, 200);
        assert!(loaded.ingredient.is_some());
    }

    #[test]
    fn furnace_is_lit_when_burn_time_positive() {
        let mut f = FurnaceBlockEntity::new(pos(), FurnaceKind::Smelting);
        f.burn_time = 100;
        assert!(f.is_lit());
        f.burn_time = 0;
        assert!(!f.is_lit());
    }

    // --- HopperBlockEntity transfer cooldown ---

    #[test]
    fn hopper_cooldown_ticks_down() {
        let mut hopper = HopperBlockEntity::new(pos());
        hopper.transfer_cooldown = 8;
        // Tick manually (without world context)
        for _ in 0..8 {
            if hopper.transfer_cooldown > 0 { hopper.transfer_cooldown -= 1; }
        }
        assert_eq!(hopper.transfer_cooldown, 0);
    }

    #[test]
    fn hopper_nbt_preserves_cooldown() {
        let mut h = HopperBlockEntity::new(pos());
        h.transfer_cooldown = 7;
        let mut tag = NbtCompound::new();
        h.save(&mut tag);
        let mut loaded = HopperBlockEntity::new(pos());
        loaded.load(&tag);
        assert_eq!(loaded.transfer_cooldown, 7);
    }

    #[test]
    fn hopper_slot_count_is_five() {
        assert_eq!(HopperBlockEntity::SLOT_COUNT, 5);
    }

    // --- MobSpawnerBlockEntity defaults ---

    #[test]
    fn spawner_defaults_are_vanilla_values() {
        let s = MobSpawnerBlockEntity::new(pos());
        assert_eq!(s.min_spawn_delay, 200);
        assert_eq!(s.max_spawn_delay, 800);
        assert_eq!(s.spawn_count, 4);
        assert_eq!(s.max_nearby_entities, 6);
        assert_eq!(s.required_player_range, 16);
        assert_eq!(s.spawn_range, 4);
    }

    #[test]
    fn spawner_nbt_roundtrip() {
        let mut s = MobSpawnerBlockEntity::new(pos());
        s.entity_id   = ResourceLocation::new("minecraft:zombie");
        s.spawn_count = 2;
        let mut tag   = NbtCompound::new();
        s.save(&mut tag);
        let mut loaded = MobSpawnerBlockEntity::new(pos());
        loaded.load(&tag);
        assert_eq!(loaded.entity_id, ResourceLocation::new("minecraft:zombie"));
        assert_eq!(loaded.spawn_count, 2);
    }

    // --- SignColor default ---

    #[test]
    fn sign_default_color_is_black() {
        let sign = SignBlockEntity::new(pos());
        assert_eq!(sign.front.color, SignColor::Black);
    }

    // --- BlockEntityMap ---

    #[test]
    fn block_entity_map_insert_and_retrieve() {
        let mut map = BlockEntityMap::new();
        map.insert(Box::new(ChestBlockEntity::new(pos())));
        assert!(map.get(pos()).is_some());
        assert!(map.get(IVec3::new(99, 99, 99)).is_none());
    }

    #[test]
    fn block_entity_map_remove() {
        let mut map = BlockEntityMap::new();
        map.insert(Box::new(ChestBlockEntity::new(pos())));
        let removed = map.remove(pos());
        assert!(removed.is_some());
        assert!(map.get(pos()).is_none());
    }
}
```
