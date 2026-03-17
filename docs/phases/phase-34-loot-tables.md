# Phase 34 — Loot Tables

**Crate:** `oxidized-game`  
**Reward:** Mobs drop correct items on death; chest loot generates correctly;
fishing gives varied catches.

**Depends on:** Phase 24 (combat/death), Phase 25 (hostile mobs), Phase 30
(block entities), Phase 8 (item registry)

---

## Goal

Implement the full loot table engine that Minecraft uses for all randomized item
generation: mob drops, chest contents, block drops, fishing loot, and gift
tables. Load tables from the data pack's `data/minecraft/loot_tables/` tree and
evaluate them at the correct game events.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Loot table | `LootTable` | `net.minecraft.world.level.storage.loot.LootTable` |
| Loot pool | `LootPool` | `net.minecraft.world.level.storage.loot.LootPool` |
| Entry container base | `LootPoolEntryContainer` | `net.minecraft.world.level.storage.loot.entries.LootPoolEntryContainer` |
| Item entry | `LootItem` | `net.minecraft.world.level.storage.loot.entries.LootItem` |
| Condition base | `LootItemCondition` | `net.minecraft.world.level.storage.loot.predicates.LootItemCondition` |
| Function base | `LootItemFunction` | `net.minecraft.world.level.storage.loot.functions.LootItemFunction` |
| Loot context | `LootContext` | `net.minecraft.world.level.storage.loot.LootContext` |
| Loot params | `LootParams` | `net.minecraft.world.level.storage.loot.LootParams` |
| Number provider base | `NumberProvider` | `net.minecraft.world.level.storage.loot.providers.number.NumberProvider` |
| Built-in tables registry | `BuiltInLootTables` | `net.minecraft.world.level.storage.loot.BuiltInLootTables` |

---

## Tasks

### 34.1 — JSON schema and loading (`oxidized-game/src/loot/mod.rs`)

Loot tables live at `data/<namespace>/loot_tables/<path>.json`. The root object:

```json
{
  "type": "minecraft:entity",
  "random_sequence": "minecraft:entities/zombie",
  "pools": [
    {
      "rolls": 1,
      "bonus_rolls": 0.0,
      "entries": [ ... ],
      "conditions": [ ... ],
      "functions": [ ... ]
    }
  ],
  "functions": [ ... ]
}
```

**Table types:**

| `type` value | Used for |
|---|---|
| `minecraft:chest` | Chest/container loot |
| `minecraft:entity` | Mob drops |
| `minecraft:block` | Block drops |
| `minecraft:fishing` | Fishing catches |
| `minecraft:gift` | Cat/hero gift tables |
| `minecraft:empty` | No drops (explicit) |
| `minecraft:archaeology` | Archaeology brush loot |
| `minecraft:shearing` | Shearing sheep/snow golem |
| `minecraft:advancement_reward` | Advancement reward items |

```rust
// crates/oxidized-game/src/loot/table.rs

#[derive(Debug, Clone)]
pub struct LootTable {
    pub table_type: LootTableType,
    pub random_sequence: Option<ResourceLocation>,
    pub pools: Vec<LootPool>,
    pub global_functions: Vec<LootFunction>,
}

impl LootTable {
    /// Evaluate this table and return all generated item stacks.
    pub fn generate(&self, ctx: &LootContext) -> Vec<ItemStack> {
        let mut items = Vec::new();
        for pool in &self.pools {
            pool.add_random_items(&mut items, ctx);
        }
        // Apply table-level functions
        for func in &self.global_functions {
            items = func.apply_batch(items, ctx);
        }
        items
    }
}
```

### 34.2 — `LootPool` evaluation

```rust
// crates/oxidized-game/src/loot/pool.rs

#[derive(Debug, Clone)]
pub struct LootPool {
    pub rolls: NumberProvider,
    pub bonus_rolls: NumberProvider,
    pub entries: Vec<LootEntry>,
    pub conditions: Vec<LootCondition>,
    pub functions: Vec<LootFunction>,
}

impl LootPool {
    pub fn add_random_items(&self, out: &mut Vec<ItemStack>, ctx: &LootContext) {
        // Check pool conditions first
        if !self.conditions.iter().all(|c| c.test(ctx)) {
            return;
        }
        let luck = ctx.luck();
        let rolls = self.rolls.get_int(ctx.rng())
            + (self.bonus_rolls.get_float(ctx.rng()) * luck).floor() as i32;
        for _ in 0..rolls.max(0) {
            let entry = self.pick_weighted_entry(ctx);
            if let Some(entry) = entry {
                entry.create_items(out, ctx);
            }
        }
        // Apply pool-level functions to all items generated this pool
        for func in &self.functions {
            for item in out.iter_mut() {
                func.apply(item, ctx);
            }
        }
    }

    fn pick_weighted_entry<'a>(&'a self, ctx: &LootContext) -> Option<&'a LootEntry> {
        let total_weight: i32 = self.entries.iter()
            .filter(|e| e.can_generate(ctx))
            .map(|e| e.effective_weight(ctx.luck()))
            .sum();
        if total_weight == 0 { return None; }
        let mut roll = ctx.rng().gen_range(0..total_weight);
        self.entries.iter()
            .filter(|e| e.can_generate(ctx))
            .find(|e| {
                roll -= e.effective_weight(ctx.luck());
                roll < 0
            })
    }
}
```

### 34.3 — Entry types

Each entry type implements `LootEntry`:

```rust
pub enum LootEntryKind {
    /// Single item with optional weight/quality.
    Item { item: ResourceLocation, weight: i32, quality: f32,
           conditions: Vec<LootCondition>, functions: Vec<LootFunction> },
    /// Inline reference to another loot table.
    LootTable { name: ResourceLocation },
    /// Selects items from a tag.
    Tag { name: ResourceLocation, expand: bool },
    /// Dynamic content: "minecraft:contents" (block entity) or "minecraft:fishing_fish".
    Dynamic { name: ResourceLocation },
    /// Always generates nothing (explicit empty slot).
    Empty { weight: i32 },
    /// All children are evaluated; each generates items.
    Group { children: Vec<LootEntry> },
    /// First child whose conditions pass is chosen (early-exit).
    Alternatives { children: Vec<LootEntry> },
    /// All children evaluated in order; stops at first failed condition.
    Sequence { children: Vec<LootEntry> },
}
```

**Weight vs quality:**  
`effective_weight = max(0, weight + floor(quality × luck))`

### 34.4 — Condition types

Implement at minimum the following conditions:

| Condition type | Key parameter(s) | Notes |
|---|---|---|
| `minecraft:random_chance` | `probability: f32` | Pass if `rng < probability` |
| `minecraft:random_chance_with_enchanted_bonus` | `enchantment, unenchanted_chance, enchanted_chances[]` | Looting-like |
| `minecraft:killed_by_player` | — | `LAST_DAMAGE_PLAYER` param present |
| `minecraft:entity_properties` | `entity` (THIS/KILLER/DIRECT_KILLER/PLAYER), `predicate` | Entity predicate match |
| `minecraft:match_tool` | `predicate` | Item predicate on TOOL param |
| `minecraft:table_bonus` | `enchantment`, `chances: [f32]` | Index by enchant level |
| `minecraft:survives_explosion` | — | Random chance = 1/explosion_radius |
| `minecraft:block_state_property` | `block`, `properties: {name:value}` | Match blockstate |
| `minecraft:location_check` | `predicate`, `offsetX/Y/Z` | Location predicate |
| `minecraft:weather_check` | `raining`, `thundering` | Weather state |
| `minecraft:reference` | `name` | Delegate to named condition in data pack |
| `minecraft:value_check` | `value`, `range` | NumberProvider in range |
| `minecraft:time_check` | `value`, `period` | Time of day check |
| `minecraft:entity_scores` | `entity`, `scores: {name: range}` | Scoreboard |
| `minecraft:inverted` | `term` | Negate another condition |
| `minecraft:any_of` | `terms: []` | OR over conditions |
| `minecraft:all_of` | `terms: []` | AND over conditions |

```rust
// crates/oxidized-game/src/loot/condition.rs

pub trait LootCondition: Send + Sync {
    fn test(&self, ctx: &LootContext) -> bool;
}
```

### 34.5 — Function types

Implement all standard item modification functions:

| Function type | Effect |
|---|---|
| `minecraft:set_count` | Set/adjust item count via `NumberProvider` |
| `minecraft:set_damage` | Set tool durability damage (0.0–1.0) |
| `minecraft:enchant_with_levels` | Enchant with `levels` provider; optional `treasure` flag |
| `minecraft:enchant_randomly` | One random enchantment from optional `enchantments` list |
| `minecraft:looting_enchant` | +`count` per Looting level, capped at `limit` |
| `minecraft:furnace_smelt` | Replace with smelting recipe output |
| `minecraft:set_name` | Set display name from Component; `entity` source for owner |
| `minecraft:set_nbt` | Merge literal NBT tag onto item |
| `minecraft:set_contents` | Set container contents from sub-entries |
| `minecraft:set_loot_table` | Set container loot table reference |
| `minecraft:set_attributes` | Add attribute modifiers |
| `minecraft:copy_name` | Copy block entity custom name |
| `minecraft:copy_state` | Copy blockstate properties to item NBT |
| `minecraft:copy_block_entity_contents` | Copy all block entity data |
| `minecraft:fill_player_head` | Set skull owner from entity |
| `minecraft:exploration_map` | Convert to explorer map pointing to `destination` |
| `minecraft:set_stew_effect` | Add potion effects to suspicious stew |
| `minecraft:set_enchantments` | Set or add specific enchantment levels |
| `minecraft:set_potion` | Set potion type |
| `minecraft:modify_contents` | Apply functions to each contained item |
| `minecraft:filtered` | Apply function only if item matches predicate |
| `minecraft:limit_count` | Clamp item count to `[min, max]` range |
| `minecraft:apply_bonus` | Apply ore fortune formula (uniform_bonus_count/binomial/ore_drops) |
| `minecraft:set_ominous_bottle_amplifier` | Set ominous bottle amplifier |
| `minecraft:sequence` | Apply list of functions in order |
| `minecraft:reference` | Delegate to named function in data pack |

```rust
pub trait LootFunction: Send + Sync {
    fn apply(&self, stack: &mut ItemStack, ctx: &LootContext);
}
```

### 34.6 — `NumberProvider` types

```rust
pub enum NumberProvider {
    Constant(f64),
    Uniform { min: Box<NumberProvider>, max: Box<NumberProvider> },
    Binomial { n: Box<NumberProvider>, p: Box<NumberProvider> },
    Score { target: ScoreboardNameResolver, score: String, scale: f64 },
    Storage { storage: ResourceLocation, path: String, scale: f64 },
    EnchantmentLevel { amount: Box<NumberProvider> },
}
```

### 34.7 — `LootContext` and parameters

```rust
// crates/oxidized-game/src/loot/context.rs

pub struct LootContext<'w> {
    pub level: &'w ServerLevel,
    pub rng: &'w mut dyn RngCore,
    pub luck: f32,
    params: HashMap<LootContextParam, LootContextValue>,
}

/// Every optional value that can be present in a loot evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LootContextParam {
    BlockState,         // The broken block state
    BlockEntity,        // The broken block entity
    Origin,             // Vec3 position of the event
    Tool,               // The item used (mining tool, weapon, fishing rod)
    ThisEntity,         // The entity being looted
    LastDamagePlayer,   // Player who last damaged the entity
    DamageSource,       // The damage source that killed the entity
    AttackingEntity,    // Direct attacker
    DirectAttackingEntity, // The entity that directly caused damage (arrow vs player)
    ExplosionRadius,    // For survives_explosion condition
    EnchantmentLevel,   // Current enchantment level (for table_bonus)
    EnchantmentActive,  // Whether enchantment is active
}
```

### 34.8 — Common vanilla mob loot tables

These loot tables are defined in data/minecraft/loot_tables/entities/:

**Zombie** (`entities/zombie.json`):
- Pool 1: 0–2 rotten flesh (looting: +1 per level, max 5)
- Pool 2 (killed_by_player + table_bonus): 5% iron ingot / 5% carrot / 5% potato

**Skeleton** (`entities/skeleton.json`):
- Pool 1: 0–2 arrows (looting: +1 per level)
- Pool 2 (killed_by_player + table_bonus): 8.5% bow (damaged)

**Creeper** (`entities/creeper.json`):
- Pool 1: 0–2 gunpowder (looting: +1 per level)
- Pool 2 (killed_by_player): music disc if killer is skeleton/stray

**Spider** (`entities/spider.json`):
- Pool 1: 0–2 string (looting: +1 per level)
- Pool 2 (killed_by_player): 33% spider eye

**Chicken** (`entities/chicken.json`):
- Pool 1: 1–3 feather (looting: +1 per level)
- Pool 2: 1 raw chicken (or cooked_chicken if on fire; `furnace_smelt`)

### 34.9 — Integration with death system (Phase 24)

When an entity dies in `EntityDeathEvent`:

```rust
// crates/oxidized-game/src/entity/death.rs

pub fn generate_mob_loot(
    entity: &LivingEntity,
    damage_source: &DamageSource,
    level: &mut ServerLevel,
) -> Vec<ItemStack> {
    let loot_table_id = entity.loot_table();
    let Some(table) = level.loot_tables().get(&loot_table_id) else { return vec![] };

    let mut params = LootParamsBuilder::new();
    params.with_param(LootContextParam::ThisEntity, entity.id());
    params.with_param(LootContextParam::Origin, entity.position());
    params.with_param(LootContextParam::DamageSource, damage_source.clone());
    if let Some(player) = damage_source.last_player() {
        params.with_param(LootContextParam::LastDamagePlayer, player.id());
        params.with_param(LootContextParam::Tool, player.main_hand_item().clone());
    }

    let ctx = LootContext::new(level, params.build());
    table.generate(&ctx)
}
```

### 34.10 — Block drops integration (Phase 22)

When a block is broken:
- Look up `data/minecraft/loot_tables/blocks/<block_id>.json`
- Build context with `BlockState`, `BlockEntity` (if present), `Tool`, `Origin`
- Evaluate; drop each ItemStack at origin

The `minecraft:apply_bonus` with `ore_drops` formula implements Fortune:

```
fortune_drops = rolls * max(1, floor(uniform(0, fortune_level + 2) - 1))
```

---

## Acceptance Criteria

- [ ] Killing a zombie drops 0–2 rotten flesh (count varies per kill)
- [ ] Killing a zombie with a Looting III sword consistently drops more
- [ ] Skeleton killed by another skeleton drops a music disc
- [ ] Mining diamond ore with Fortune III gives > 1 diamond
- [ ] Mining diamond ore with Silk Touch drops `diamond_ore` not `diamond`
- [ ] Opening a naturally generated chest (Phase 36) has loot matching vanilla
- [ ] Loot tables reference other tables (`loot_table` entry type) correctly
