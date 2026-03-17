# Phase 8 — Block & Item Registry

**Crate:** `oxidized-world`  
**Reward:** Any block can be looked up by name or state ID; any item by name.
The global palettes are correct and match vanilla.

---

## Goal

Load the block and item registries from vanilla JSON data and provide fast
lookup by both `ResourceLocation` (name) and numeric ID.

---

## Java Reference

| Concept | Java class |
|---------|-----------|
| Block registry | `net.minecraft.world.level.block.Blocks` |
| Block base class | `net.minecraft.world.level.block.Block` |
| Block state | `net.minecraft.world.level.block.state.BlockState` |
| State definition | `net.minecraft.world.level.block.state.StateDefinition` |
| State holder | `net.minecraft.world.level.block.state.StateHolder` |
| Properties | `net.minecraft.world.level.block.state.properties.*Property` |
| Item registry | `net.minecraft.world.item.Items` |
| Item | `net.minecraft.world.item.Item` |
| Item stack | `net.minecraft.world.item.ItemStack` |
| Data components | `net.minecraft.core.component.DataComponents` |
| Registry | `net.minecraft.core.Registry`, `net.minecraft.core.registries.BuiltInRegistries` |
| Holder | `net.minecraft.core.Holder` |

---

## Data Files

These are extracted from the vanilla server JAR:

- `blocks.json` — all block states with their properties and state IDs
- `items.json` — all items with their registry IDs
- `registries.json` — all vanilla registry data

Extract with:
```bash
java -DbundlerMainClass=net.minecraft.data.Main \
     -jar mc-server-ref/server.jar --reports
# Creates: generated/reports/blocks.json, items.json, registries.json
```

### `blocks.json` structure
```json
{
  "minecraft:stone": {
    "properties": {},
    "states": [
      { "id": 1, "default": true, "properties": {} }
    ]
  },
  "minecraft:grass_block": {
    "properties": { "snowy": ["false", "true"] },
    "states": [
      { "id": 9, "default": true,  "properties": { "snowy": "false" } },
      { "id": 10, "default": false, "properties": { "snowy": "true" } }
    ]
  }
}
```

---

## Tasks

### 8.1 — Block State ID Type

```rust
/// Raw block state ID as used in protocol and chunk storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockStateId(pub u32);

impl BlockStateId {
    pub const AIR: BlockStateId = BlockStateId(0);
}
```

### 8.2 — Block Properties (`src/registry/block_properties.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyValue {
    Bool(bool),
    Int(u8),
    Enum(String),
}

#[derive(Debug, Clone)]
pub struct BlockProperties(HashMap<String, PropertyValue>);

impl BlockProperties {
    pub fn get_bool(&self, key: &str) -> Option<bool>;
    pub fn get_int(&self, key: &str) -> Option<u8>;
    pub fn get_enum(&self, key: &str) -> Option<&str>;
}
```

### 8.3 — Block State (`src/registry/block_state.rs`)

```rust
#[derive(Debug, Clone)]
pub struct BlockState {
    pub id: BlockStateId,
    pub block_id: ResourceLocation,
    pub properties: BlockProperties,
    pub is_default: bool,
    // Derived from properties (set during registry load)
    pub solid: bool,
    pub air: bool,
    pub liquid: bool,
    pub opaque: bool,
    pub light_emission: u8,
    pub destroy_time: f32,
}
```

### 8.4 — Block Definition

```rust
#[derive(Debug)]
pub struct Block {
    pub id: ResourceLocation,
    pub default_state: BlockStateId,
    pub states: Vec<BlockStateId>,
    pub property_keys: Vec<String>,
}
```

### 8.5 — Block Registry (`src/registry/blocks.rs`)

```rust
pub struct BlockRegistry {
    // Forward lookups
    by_id: HashMap<ResourceLocation, Arc<Block>>,
    // State lookups
    states: Vec<BlockState>,  // indexed by state ID
    // Reverse: state_id → block
    state_to_block: Vec<ResourceLocation>,
}

impl BlockRegistry {
    pub fn load(blocks_json: &[u8]) -> Result<Self, RegistryError>;
    
    pub fn get_block(&self, id: &ResourceLocation) -> Option<&Block>;
    pub fn get_state(&self, id: BlockStateId) -> Option<&BlockState>;
    pub fn default_state(&self, block: &ResourceLocation) -> Option<BlockStateId>;
    pub fn state_count(&self) -> usize;
    pub fn block_count(&self) -> usize;
    
    /// Look up state by block + property map
    pub fn state_with_properties(
        &self,
        block: &ResourceLocation,
        properties: &BlockProperties,
    ) -> Option<BlockStateId>;
}
```

### 8.6 — Built-in Block Constants

```rust
pub mod blocks {
    pub const AIR: BlockStateId               = BlockStateId(0);
    pub const STONE: BlockStateId             = BlockStateId(1);
    pub const GRANITE: BlockStateId           = BlockStateId(2);
    pub const GRASS_BLOCK: BlockStateId       = BlockStateId(9);  // default (snowy=false)
    pub const DIRT: BlockStateId              = BlockStateId(11);
    pub const BEDROCK: BlockStateId           = BlockStateId(33);
    pub const WATER: BlockStateId             = BlockStateId(65);  // level=0
    pub const LAVA: BlockStateId              = BlockStateId(130); // level=0
    pub const SAND: BlockStateId              = BlockStateId(148);
    pub const OAK_LOG: BlockStateId           = BlockStateId(143);
    pub const OAK_LEAVES: BlockStateId        = BlockStateId(193);
    // ... all common blocks
    // Generated from blocks.json at build time using build.rs
}
```

### 8.7 — Item Registry (`src/registry/items.rs`)

```rust
#[derive(Debug, Clone)]
pub struct Item {
    pub id: ResourceLocation,
    pub numeric_id: u32,
    pub max_stack_size: u32,
    pub max_damage: Option<u32>,
    pub food: Option<FoodProperties>,
}

pub struct FoodProperties {
    pub nutrition: i32,
    pub saturation: f32,
    pub can_always_eat: bool,
}

pub struct ItemRegistry {
    by_id: HashMap<ResourceLocation, Arc<Item>>,
    by_num: Vec<Option<Arc<Item>>>,
}

impl ItemRegistry {
    pub fn load(items_json: &[u8]) -> Result<Self, RegistryError>;
    pub fn get(&self, id: &ResourceLocation) -> Option<&Item>;
    pub fn get_by_id(&self, id: u32) -> Option<&Item>;
    pub fn item_count(&self) -> usize;
}
```

### 8.8 — Item Stack (`src/types/item_stack.rs`)

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ItemStack {
    pub item: ResourceLocation,
    pub count: u32,
    pub components: DataComponentPatch,
}

impl ItemStack {
    pub const EMPTY: ItemStack;
    pub fn new(item: ResourceLocation, count: u32) -> Self;
    pub fn is_empty(&self) -> bool;
    pub fn with_count(self, count: u32) -> Self;
}

/// Sparse override of item components (custom name, enchants, etc.)
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DataComponentPatch {
    data: HashMap<ResourceLocation, NbtTag>,
}
```

### 8.9 — Global Registry (`src/registry/mod.rs`)

```rust
pub struct GameRegistries {
    pub blocks: BlockRegistry,
    pub items: ItemRegistry,
}

impl GameRegistries {
    pub fn load() -> Result<Self, RegistryError> {
        let blocks = BlockRegistry::load(include_bytes!("../../data/blocks.json"))?;
        let items  = ItemRegistry::load(include_bytes!("../../data/items.json"))?;
        Ok(GameRegistries { blocks, items })
    }
}

// Static global (initialized once)
pub static REGISTRY: OnceLock<GameRegistries> = OnceLock::new();
pub fn registry() -> &'static GameRegistries { REGISTRY.get().expect("registries not loaded") }
```

---

## Tests

```rust
#[test]
fn test_block_registry_loads() {
    let reg = BlockRegistry::load(BLOCKS_JSON).unwrap();
    assert!(reg.state_count() > 20_000);
    assert!(reg.block_count() > 900);
}
#[test]
fn test_air_is_state_zero() {
    let reg = BlockRegistry::load(BLOCKS_JSON).unwrap();
    let air = reg.get_state(BlockStateId(0)).unwrap();
    assert_eq!(air.block_id, ResourceLocation::minecraft("air"));
}
#[test]
fn test_default_state_lookup() {
    let reg = BlockRegistry::load(BLOCKS_JSON).unwrap();
    let stone_default = reg.default_state(&ResourceLocation::minecraft("stone")).unwrap();
    let state = reg.get_state(stone_default).unwrap();
    assert!(state.is_default);
}
#[test]
fn test_item_registry_loads() {
    let reg = ItemRegistry::load(ITEMS_JSON).unwrap();
    assert!(reg.item_count() > 1000);
}
#[test]
fn test_item_by_name() {
    let reg = ItemRegistry::load(ITEMS_JSON).unwrap();
    let diamond = reg.get(&ResourceLocation::minecraft("diamond")).unwrap();
    assert_eq!(diamond.max_stack_size, 64);
}
```
