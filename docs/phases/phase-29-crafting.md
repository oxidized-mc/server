# Phase 29 — Crafting

**Crate:** `oxidized-game`  
**Reward:** Player can craft items using 2×2 grid; `/recipe` command works.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-024: Inventory](../adr/adr-024-inventory.md) — transactional slot modification with optimistic locking
- [ADR-027: Recipe System](../adr/adr-027-recipe-system.md) — pre-indexed recipe lookup with shaped pattern matching


## Goal

Implement the full recipe system: load JSON recipe files from the `data/` pack,
match shaped and shapeless recipes against inventory grids, drive `CraftingMenu`
(2×2) and `CraftingTableMenu` (3×3) menus, handle shift-click routing via
`quickMoveStack`, and unlock recipes in the player's recipe book on first craft.
Furnace smelting, blasting, smoking, and campfire cooking must also work with
correct burn-time and cook-time tracking.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Recipe manager | `RecipeManager` | `net.minecraft.world.item.crafting.RecipeManager` |
| Shaped recipe | `ShapedRecipe` | `net.minecraft.world.item.crafting.ShapedRecipe` |
| Shapeless recipe | `ShapelessRecipe` | `net.minecraft.world.item.crafting.ShapelessRecipe` |
| Ingredient | `Ingredient` | `net.minecraft.world.item.crafting.Ingredient` |
| Crafting menu (2×2) | `CraftingMenu` | `net.minecraft.world.inventory.CraftingMenu` |
| Crafting table menu (3×3) | `CraftingTableMenu` | `net.minecraft.world.inventory.CraftingTableMenu` |
| Abstract furnace menu | `AbstractFurnaceMenu` | `net.minecraft.world.inventory.AbstractFurnaceMenu` |
| Quick move logic | `AbstractContainerMenu#quickMoveStack` | `net.minecraft.world.inventory.AbstractContainerMenu` |
| Ghost recipe packet | `ClientboundPlaceGhostRecipePacket` | `net.minecraft.network.protocol.game.ClientboundPlaceGhostRecipePacket` |
| Recipe book unlock | `ClientboundRecipeBookAddPacket` | `net.minecraft.network.protocol.game.ClientboundRecipeBookAddPacket` |
| Container data packet | `ClientboundContainerSetDataPacket` | `net.minecraft.network.protocol.game.ClientboundContainerSetDataPacket` |

---

## Tasks

### 29.1 — Recipe JSON format and `Ingredient`

```rust
// crates/oxidized-game/src/crafting/ingredient.rs

use serde::{Deserialize, Serialize};

/// A single ingredient slot: matches any item in the list or any item with the given tag.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Ingredient {
    /// Single item: `{"item": "minecraft:stick"}`
    Item { item: ResourceLocation },
    /// Item tag: `{"tag": "minecraft:planks"}`
    Tag { tag: ResourceLocation },
    /// Array of alternatives: `[{"item": "..."}, {"item": "..."}]`
    List(Vec<IngredientEntry>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IngredientEntry {
    Item { item: ResourceLocation },
    Tag  { tag:  ResourceLocation },
}

impl Ingredient {
    /// Returns true if `stack` matches this ingredient.
    pub fn test(&self, stack: &ItemStack, tag_registry: &TagRegistry) -> bool {
        match self {
            Self::Item { item }  => stack.item == *item,
            Self::Tag  { tag }   => tag_registry.is_in_tag(&stack.item, tag),
            Self::List(entries)  => entries.iter().any(|e| match e {
                IngredientEntry::Item { item } => stack.item == *item,
                IngredientEntry::Tag  { tag }  => tag_registry.is_in_tag(&stack.item, tag),
            }),
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, Self::List(v) if v.is_empty())
    }
}
```

### 29.2 — `ShapedRecipe`

```rust
// crates/oxidized-game/src/crafting/shaped_recipe.rs

use super::ingredient::Ingredient;
use serde::Deserialize;
use std::collections::HashMap;

/// Recipe JSON for `minecraft:crafting_shaped`.
#[derive(Debug, Clone, Deserialize)]
pub struct ShapedRecipeJson {
    pub group:    Option<String>,
    pub category: Option<String>,
    pub pattern:  Vec<String>,             // e.g. ["##", "##"]
    pub key:      HashMap<char, serde_json::Value>, // char → Ingredient JSON
    pub result:   RecipeResultJson,
    pub show_notification: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecipeResultJson {
    pub id: ResourceLocation,
    pub count: Option<u8>,
    pub components: Option<serde_json::Value>,
}

/// Compiled shaped recipe ready for fast matching.
#[derive(Debug, Clone)]
pub struct ShapedRecipe {
    pub id:     ResourceLocation,
    pub group:  String,
    pub width:  usize,
    pub height: usize,
    /// Flattened ingredient grid [row * width + col].
    pub ingredients: Vec<Ingredient>,
    pub result: ItemStack,
    pub show_notification: bool,
}

impl ShapedRecipe {
    /// Compile from JSON representation.
    pub fn from_json(id: ResourceLocation, json: ShapedRecipeJson) -> anyhow::Result<Self> {
        let height = json.pattern.len();
        anyhow::ensure!(height >= 1 && height <= 3, "pattern height out of range");
        let width = json.pattern[0].len();
        anyhow::ensure!(width >= 1 && width <= 3, "pattern width out of range");
        let mut ingredients = Vec::with_capacity(height * width);
        for row in &json.pattern {
            for ch in row.chars() {
                if ch == ' ' {
                    ingredients.push(Ingredient::List(vec![]));
                } else {
                    let raw = json.key.get(&ch)
                        .ok_or_else(|| anyhow::anyhow!("key '{ch}' not in recipe key map"))?;
                    ingredients.push(serde_json::from_value(raw.clone())?);
                }
            }
        }
        Ok(Self {
            id,
            group: json.group.unwrap_or_default(),
            width,
            height,
            ingredients,
            result: ItemStack {
                item: json.result.id,
                count: json.result.count.unwrap_or(1),
                nbt: None,
            },
            show_notification: json.show_notification.unwrap_or(true),
        })
    }

    /// Test whether the given grid matches this recipe (including mirrored).
    /// `grid` is row-major with `grid_width` columns. Empty slots are air stacks.
    pub fn matches(
        &self,
        grid: &[ItemStack],
        grid_width: usize,
        grid_height: usize,
        tag_registry: &TagRegistry,
    ) -> bool {
        for offset_x in 0..=(grid_width.saturating_sub(self.width)) {
            for offset_y in 0..=(grid_height.saturating_sub(self.height)) {
                if self.matches_at(grid, grid_width, offset_x, offset_y, false, tag_registry) {
                    return true;
                }
                if self.matches_at(grid, grid_width, offset_x, offset_y, true, tag_registry) {
                    return true;
                }
            }
        }
        false
    }

    fn matches_at(
        &self,
        grid: &[ItemStack],
        grid_width: usize,
        ox: usize,
        oy: usize,
        mirror: bool,
        tag_registry: &TagRegistry,
    ) -> bool {
        for row in 0..self.height {
            for col in 0..self.width {
                let recipe_col = if mirror { self.width - 1 - col } else { col };
                let ingredient = &self.ingredients[row * self.width + recipe_col];
                let grid_idx = (oy + row) * grid_width + (ox + col);
                let stack = grid.get(grid_idx).cloned().unwrap_or_default();
                if ingredient.is_empty() {
                    if !stack.is_empty() { return false; }
                } else if !ingredient.test(&stack, tag_registry) {
                    return false;
                }
            }
        }
        true
    }
}
```

### 29.3 — `ShapelessRecipe`

```rust
// crates/oxidized-game/src/crafting/shapeless_recipe.rs

use super::ingredient::Ingredient;

#[derive(Debug, Clone)]
pub struct ShapelessRecipe {
    pub id:          ResourceLocation,
    pub group:       String,
    pub ingredients: Vec<Ingredient>,
    pub result:      ItemStack,
}

impl ShapelessRecipe {
    /// Backtracking match: every ingredient must be satisfied by a unique grid slot.
    pub fn matches(
        &self,
        grid: &[ItemStack],
        tag_registry: &TagRegistry,
    ) -> bool {
        let non_empty: Vec<&ItemStack> = grid.iter().filter(|s| !s.is_empty()).collect();
        if non_empty.len() != self.ingredients.len() { return false; }
        self.backtrack_match(&self.ingredients, &non_empty, tag_registry)
    }

    fn backtrack_match(
        &self,
        remaining_ingredients: &[Ingredient],
        slots: &[&ItemStack],
        tag_registry: &TagRegistry,
    ) -> bool {
        if remaining_ingredients.is_empty() { return true; }
        let ingredient = &remaining_ingredients[0];
        for (i, stack) in slots.iter().enumerate() {
            if ingredient.test(stack, tag_registry) {
                let mut remaining_slots = slots.to_vec();
                remaining_slots.remove(i);
                if self.backtrack_match(&remaining_ingredients[1..], &remaining_slots, tag_registry) {
                    return true;
                }
            }
        }
        false
    }
}
```

### 29.4 — `RecipeManager`

```rust
// crates/oxidized-game/src/crafting/recipe_manager.rs

use std::collections::HashMap;
use super::{ShapedRecipe, ShapelessRecipe};

/// All recipe types supported by the recipe manager.
#[derive(Debug, Clone)]
pub enum Recipe {
    Shaped(ShapedRecipe),
    Shapeless(ShapelessRecipe),
    Smelting(FurnaceRecipe),
    Blasting(FurnaceRecipe),
    Smoking(FurnaceRecipe),
    CampfireCooking(FurnaceRecipe),
    Stonecutting(StonecuttingRecipe),
    SmithingTransform(SmithingRecipe),
    SmithingTrim(SmithingRecipe),
}

#[derive(Debug, Clone)]
pub struct FurnaceRecipe {
    pub id:           ResourceLocation,
    pub group:        String,
    pub ingredient:   Ingredient,
    pub result:       ItemStack,
    /// Experience awarded when the result is taken out.
    pub experience:   f32,
    /// Cook time in ticks: smelting=200, blasting/smoking=100, campfire=600.
    pub cook_time:    u32,
}

#[derive(Debug, Clone)]
pub struct StonecuttingRecipe {
    pub id:         ResourceLocation,
    pub ingredient: Ingredient,
    pub result:     ItemStack,
}

#[derive(Debug, Clone)]
pub struct SmithingRecipe {
    pub id:         ResourceLocation,
    pub template:   Ingredient,
    pub base:       Ingredient,
    pub addition:   Ingredient,
    pub result:     ItemStack,
}

/// Loaded recipe collection, indexed by id and queryable by type.
pub struct RecipeManager {
    recipes: HashMap<ResourceLocation, Recipe>,
}

impl RecipeManager {
    pub fn new() -> Self { Self { recipes: HashMap::new() } }

    pub fn register(&mut self, recipe: Recipe) {
        let id = match &recipe {
            Recipe::Shaped(r)          => r.id.clone(),
            Recipe::Shapeless(r)       => r.id.clone(),
            Recipe::Smelting(r)        => r.id.clone(),
            Recipe::Blasting(r)        => r.id.clone(),
            Recipe::Smoking(r)         => r.id.clone(),
            Recipe::CampfireCooking(r) => r.id.clone(),
            Recipe::Stonecutting(r)    => r.id.clone(),
            Recipe::SmithingTransform(r) | Recipe::SmithingTrim(r) => r.id.clone(),
        };
        self.recipes.insert(id, recipe);
    }

    /// Find the first crafting recipe matching the given 2×2 or 3×3 grid.
    pub fn find_crafting_recipe(
        &self,
        grid: &[ItemStack],
        width: usize,
        height: usize,
        tag_registry: &TagRegistry,
    ) -> Option<&Recipe> {
        self.recipes.values().find(|r| match r {
            Recipe::Shaped(s)    => s.matches(grid, width, height, tag_registry),
            Recipe::Shapeless(s) => s.matches(grid, tag_registry),
            _ => false,
        })
    }

    /// Find a smelting/blasting/smoking recipe for the given ingredient.
    pub fn find_furnace_recipe(
        &self,
        ingredient: &ItemStack,
        kind: FurnaceKind,
        tag_registry: &TagRegistry,
    ) -> Option<&FurnaceRecipe> {
        self.recipes.values().find_map(|r| {
            let fr = match (r, kind) {
                (Recipe::Smelting(f),       FurnaceKind::Smelting)       => Some(f),
                (Recipe::Blasting(f),       FurnaceKind::Blasting)       => Some(f),
                (Recipe::Smoking(f),        FurnaceKind::Smoking)        => Some(f),
                (Recipe::CampfireCooking(f),FurnaceKind::CampfireCooking)=> Some(f),
                _ => None,
            }?;
            if fr.ingredient.test(ingredient, tag_registry) { Some(fr) } else { None }
        })
    }

    pub fn get(&self, id: &ResourceLocation) -> Option<&Recipe> {
        self.recipes.get(id)
    }

    pub fn all_ids(&self) -> impl Iterator<Item = &ResourceLocation> {
        self.recipes.keys()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FurnaceKind { Smelting, Blasting, Smoking, CampfireCooking }
```

### 29.5 — `CraftingMenu` and `CraftingTableMenu`

```rust
// crates/oxidized-game/src/inventory/crafting_menu.rs

/// 2×2 crafting menu (player inventory screen).
/// Slot 0 = output, Slots 1-4 = input (row-major).
/// Window type: `minecraft:crafting`.
pub struct CraftingMenu {
    pub container_id: u8,
    /// Output slot (index 0).
    pub output:       ItemStack,
    /// Input grid (indices 1-4, 2×2 row-major).
    pub input:        [ItemStack; 4],
    /// Cached result of last recipe match.
    pub last_recipe:  Option<ResourceLocation>,
}

impl CraftingMenu {
    pub const SLOT_OUTPUT: usize = 0;
    pub const SLOTS_INPUT_START: usize = 1;
    pub const GRID_WIDTH:  usize = 2;
    pub const GRID_HEIGHT: usize = 2;

    pub fn new(container_id: u8) -> Self {
        Self {
            container_id,
            output: ItemStack::EMPTY,
            input: [ItemStack::EMPTY; 4],
            last_recipe: None,
        }
    }

    /// Called whenever an input slot changes. Re-evaluates the recipe and updates the output slot.
    pub fn slot_changed(
        &mut self,
        recipes: &RecipeManager,
        tag_registry: &TagRegistry,
    ) {
        let grid: &[ItemStack] = &self.input;
        if let Some(recipe) = recipes.find_crafting_recipe(grid, Self::GRID_WIDTH, Self::GRID_HEIGHT, tag_registry) {
            let result = match recipe {
                Recipe::Shaped(r)    => r.result.clone(),
                Recipe::Shapeless(r) => r.result.clone(),
                _ => unreachable!(),
            };
            self.output = result;
            self.last_recipe = Some(match recipe {
                Recipe::Shaped(r)    => r.id.clone(),
                Recipe::Shapeless(r) => r.id.clone(),
                _ => unreachable!(),
            });
        } else {
            self.output = ItemStack::EMPTY;
            self.last_recipe = None;
        }
    }

    /// Called when the player takes from the output slot.
    /// Consumes one of each ingredient, awards recipe unlock if first time.
    pub fn take_result(
        &mut self,
        player_id: Uuid,
        unlocked_recipes: &mut PlayerRecipeBook,
    ) -> ItemStack {
        if self.output.is_empty() { return ItemStack::EMPTY; }
        // Consume one item from each non-empty input slot
        for slot in self.input.iter_mut() {
            if !slot.is_empty() {
                slot.count -= 1;
                if slot.count == 0 { *slot = ItemStack::EMPTY; }
            }
        }
        // Unlock recipe on first craft
        if let Some(ref id) = self.last_recipe {
            unlocked_recipes.try_unlock(id);
        }
        std::mem::replace(&mut self.output, ItemStack::EMPTY)
    }
}

/// 3×3 crafting table (window type `minecraft:crafting_table`).
/// Slot 0 = output, Slots 1-9 = input grid.
pub struct CraftingTableMenu {
    pub container_id: u8,
    pub output:       ItemStack,
    pub input:        [ItemStack; 9],
    pub last_recipe:  Option<ResourceLocation>,
}

impl CraftingTableMenu {
    pub const GRID_WIDTH:  usize = 3;
    pub const GRID_HEIGHT: usize = 3;
}
```

### 29.6 — `quickMoveStack` shift-click routing

```rust
// crates/oxidized-game/src/inventory/quick_move.rs

/// Determines where to route shift-clicked items in a container.
/// Mirrors `AbstractContainerMenu.quickMoveStack` in Java.
pub struct QuickMoveStack;

impl QuickMoveStack {
    /// Shift-click slot `slot_index` in a crafting menu (2×2 + player inventory).
    /// `slots` layout: [0=output, 1-4=crafting, 5-31=inventory, 32-40=hotbar].
    pub fn crafting_menu(
        slots: &mut [ItemStack],
        slot_index: usize,
    ) -> ItemStack {
        let original = slots[slot_index].clone();
        match slot_index {
            0 => {
                // Output → try hotbar (32-40) first, then inventory (5-31)
                Self::move_to_range(slots, 0, 32..40)
                    .or_else(|| Self::move_to_range(slots, 0, 5..32));
            }
            1..=4 => {
                // Crafting input → inventory then hotbar
                Self::move_to_range(slots, slot_index, 5..32)
                    .or_else(|| Self::move_to_range(slots, slot_index, 32..40));
            }
            5..=31 => {
                // Inventory → try crafting inputs first, then hotbar
                Self::move_to_range(slots, slot_index, 1..5)
                    .or_else(|| Self::move_to_range(slots, slot_index, 32..40));
            }
            32..=40 => {
                // Hotbar → try crafting inputs, then inventory
                Self::move_to_range(slots, slot_index, 1..5)
                    .or_else(|| Self::move_to_range(slots, slot_index, 5..32));
            }
            _ => {}
        }
        original
    }

    /// Attempt to move stack at `source` into the first empty slot in `range`.
    /// Returns Some(()) if moved, None if no space.
    fn move_to_range(
        slots: &mut [ItemStack],
        source: usize,
        range: std::ops::Range<usize>,
    ) -> Option<()> {
        if slots[source].is_empty() { return None; }
        for i in range {
            if slots[i].is_empty() {
                slots[i] = slots[source].clone();
                slots[source] = ItemStack::EMPTY;
                return Some(());
            }
        }
        None
    }
}
```

### 29.7 — `FurnaceMenu` and fuel/cook tracking

```rust
// crates/oxidized-game/src/inventory/furnace_menu.rs

/// Furnace container menu.
/// Slot 0 = ingredient, Slot 1 = fuel, Slot 2 = output.
/// Window type: `minecraft:furnace` / `minecraft:blast_furnace` / `minecraft:smoker`.
pub struct FurnaceMenu {
    pub container_id: u8,
    pub kind:         FurnaceKind,
    pub ingredient:   ItemStack,
    pub fuel:         ItemStack,
    pub output:       ItemStack,
    /// Ticks remaining for current fuel item.
    pub burn_time:      u16,
    /// Total burn time of the current fuel item (for progress bar).
    pub fuel_total:     u16,
    /// Ticks the current item has been cooking.
    pub cook_time:      u16,
    /// Total cook time for the current recipe.
    pub cook_time_total: u16,
    /// Experience accumulated (awarded when output is taken).
    pub stored_xp:      f32,
}

impl FurnaceMenu {
    /// `ClientboundContainerSetDataPacket` property IDs.
    pub const DATA_BURN_TIME:       u16 = 0;
    pub const DATA_FUEL_TOTAL:      u16 = 1;
    pub const DATA_COOK_TIME:       u16 = 2;
    pub const DATA_COOK_TIME_TOTAL: u16 = 3;

    pub fn is_burning(&self) -> bool { self.burn_time > 0 }

    /// Called every server tick. Returns true when the output changes.
    pub fn server_tick(
        &mut self,
        recipes: &RecipeManager,
        tag_registry: &TagRegistry,
    ) -> bool {
        let mut changed = false;

        if self.is_burning() {
            self.burn_time -= 1;
        }

        if !self.ingredient.is_empty() {
            if let Some(recipe) = recipes.find_furnace_recipe(&self.ingredient, self.kind, tag_registry) {
                self.cook_time_total = recipe.cook_time as u16;

                if !self.is_burning() {
                    // Consume fuel item
                    if !self.fuel.is_empty() {
                        let burn = fuel_burn_time(&self.fuel.item);
                        if burn > 0 {
                            self.fuel_total = burn;
                            self.burn_time = burn;
                            self.fuel.count -= 1;
                            if self.fuel.count == 0 { self.fuel = ItemStack::EMPTY; }
                            changed = true;
                        }
                    }
                }

                if self.is_burning() {
                    self.cook_time += 1;
                    if self.cook_time >= self.cook_time_total {
                        self.cook_time = 0;
                        // Move result to output if space available
                        if self.output.is_empty() || self.output.item == recipe.result.item {
                            self.stored_xp += recipe.experience;
                            self.output.count += recipe.result.count;
                            self.ingredient.count -= 1;
                            if self.ingredient.count == 0 { self.ingredient = ItemStack::EMPTY; }
                        }
                        changed = true;
                    }
                }
            } else {
                self.cook_time = 0;
            }
        } else {
            self.cook_time = 0;
        }
        changed
    }
}

/// Return the burn duration (ticks) for a fuel item.
pub fn fuel_burn_time(item: &ResourceLocation) -> u16 {
    match item.as_str() {
        "minecraft:lava_bucket"  => 20_000,
        "minecraft:coal_block"   => 16_000,
        "minecraft:dried_kelp_block" => 4_001,
        "minecraft:coal"         => 1_600,
        "minecraft:charcoal"     => 1_600,
        "minecraft:blaze_rod"    => 2_400,
        "minecraft:log"  | "minecraft:planks" => 300,
        "minecraft:stick"        => 100,
        "minecraft:wooden_slab"  => 150,
        "minecraft:bamboo"       =>  50,
        _ => 0,
    }
}
```

### 29.8 — Recipe book and unlock packets

```rust
// crates/oxidized-game/src/crafting/recipe_book.rs

use std::collections::HashSet;

/// Per-player recipe book state.
pub struct PlayerRecipeBook {
    pub unlocked: HashSet<ResourceLocation>,
    pub highlighted: HashSet<ResourceLocation>,
    pub open: bool,
    pub filtering_craftable: bool,
    pub open_furnace: bool,
    pub filtering_furnace: bool,
}

impl PlayerRecipeBook {
    pub fn new() -> Self {
        Self {
            unlocked: HashSet::new(),
            highlighted: HashSet::new(),
            open: false,
            filtering_craftable: false,
            open_furnace: false,
            filtering_furnace: false,
        }
    }

    /// Unlock a recipe. Returns true if it was newly unlocked.
    pub fn try_unlock(&mut self, id: &ResourceLocation) -> bool {
        self.unlocked.insert(id.clone())
    }

    pub fn is_unlocked(&self, id: &ResourceLocation) -> bool {
        self.unlocked.contains(id)
    }
}

/// `ClientboundRecipeBookAddPacket` (0x42): sent on first craft.
/// `entries`: list of recipe IDs newly unlocked this tick.
/// `replace`: false → append; true → replace entire book.
#[derive(Debug, Clone)]
pub struct ClientboundRecipeBookAddPacket {
    pub entries:  Vec<RecipeBookEntry>,
    pub replace:  bool,
}

#[derive(Debug, Clone)]
pub struct RecipeBookEntry {
    pub recipe_id:    ResourceLocation,
    pub notification: bool,
    pub highlight:    bool,
}

/// `ClientboundPlaceGhostRecipePacket` (0x31): show ghost recipe overlay in crafting grid.
#[derive(Debug, Clone)]
pub struct ClientboundPlaceGhostRecipePacket {
    pub container_id: u8,
    pub recipe:       ResourceLocation,
}
```

---

## Data Structures Summary

```rust
// Key types in oxidized-game::crafting

pub use ingredient::Ingredient;
pub use shaped_recipe::ShapedRecipe;
pub use shapeless_recipe::ShapelessRecipe;
pub use recipe_manager::{Recipe, RecipeManager, FurnaceRecipe, FurnaceKind, StonecuttingRecipe};
pub use inventory::crafting_menu::{CraftingMenu, CraftingTableMenu};
pub use inventory::furnace_menu::{FurnaceMenu, fuel_burn_time};
pub use inventory::quick_move::QuickMoveStack;
pub use recipe_book::{PlayerRecipeBook, ClientboundRecipeBookAddPacket};
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn no_tags() -> TagRegistry { TagRegistry::empty() }

    fn stick() -> ItemStack { ItemStack::new(rl("minecraft:stick")) }
    fn plank() -> ItemStack { ItemStack::new(rl("minecraft:oak_planks")) }
    fn empty() -> ItemStack { ItemStack::EMPTY }
    fn rl(s: &str) -> ResourceLocation { ResourceLocation::new(s) }

    // --- ShapedRecipe matching ---

    fn two_sticks_recipe() -> ShapedRecipe {
        ShapedRecipe {
            id: rl("minecraft:test_shaped"),
            group: String::new(),
            width: 1,
            height: 2,
            ingredients: vec![
                Ingredient::Item { item: rl("minecraft:stick") },
                Ingredient::Item { item: rl("minecraft:stick") },
            ],
            result: ItemStack::new_count(rl("minecraft:test_result"), 1),
            show_notification: true,
        }
    }

    /// Exact match: two sticks vertically in a 2×2 grid.
    #[test]
    fn shaped_recipe_matches_exact() {
        let recipe = two_sticks_recipe();
        let grid = [stick(), empty(), stick(), empty()]; // 2×2 col-major
        assert!(recipe.matches(&grid, 2, 2, &no_tags()));
    }

    /// Mirror match: sticks in second column.
    #[test]
    fn shaped_recipe_matches_mirrored() {
        let recipe = two_sticks_recipe();
        let grid = [empty(), stick(), empty(), stick()];
        assert!(recipe.matches(&grid, 2, 2, &no_tags()));
    }

    /// Wrong item → no match.
    #[test]
    fn shaped_recipe_no_match_wrong_item() {
        let recipe = two_sticks_recipe();
        let grid = [plank(), empty(), plank(), empty()];
        assert!(!recipe.matches(&grid, 2, 2, &no_tags()));
    }

    // --- ShapelessRecipe matching ---

    #[test]
    fn shapeless_recipe_matches_any_order() {
        let recipe = ShapelessRecipe {
            id: rl("minecraft:test_shapeless"),
            group: String::new(),
            ingredients: vec![
                Ingredient::Item { item: rl("minecraft:stick") },
                Ingredient::Item { item: rl("minecraft:oak_planks") },
            ],
            result: ItemStack::new_count(rl("minecraft:result"), 1),
        };
        let grid1 = vec![stick(), plank()];
        let grid2 = vec![plank(), stick()];
        assert!(recipe.matches(&grid1, &no_tags()));
        assert!(recipe.matches(&grid2, &no_tags()));
    }

    #[test]
    fn shapeless_recipe_requires_exact_count() {
        let recipe = ShapelessRecipe {
            id: rl("test"),
            group: String::new(),
            ingredients: vec![
                Ingredient::Item { item: rl("minecraft:stick") },
            ],
            result: ItemStack::EMPTY,
        };
        // Extra item in grid → no match
        let grid = vec![stick(), plank()];
        assert!(!recipe.matches(&grid, &no_tags()));
    }

    // --- Furnace cook timer ---

    #[test]
    fn furnace_cooks_after_200_ticks() {
        let mut menu = FurnaceMenu {
            container_id: 1,
            kind: FurnaceKind::Smelting,
            ingredient: ItemStack::new(rl("minecraft:iron_ore")),
            fuel:       ItemStack::new_count(rl("minecraft:coal"), 1),
            output:     ItemStack::EMPTY,
            burn_time: 0, fuel_total: 0, cook_time: 0, cook_time_total: 200,
            stored_xp: 0.0,
        };
        // Build a recipe manager with an iron ore smelting recipe
        let mut rm = RecipeManager::new();
        rm.register(Recipe::Smelting(FurnaceRecipe {
            id: rl("minecraft:iron_nugget_from_smelting_iron_ore"),
            group: String::new(),
            ingredient: Ingredient::Item { item: rl("minecraft:iron_ore") },
            result: ItemStack::new(rl("minecraft:iron_ingot")),
            experience: 0.7,
            cook_time: 200,
        }));
        for _ in 0..201 {
            menu.server_tick(&rm, &no_tags());
        }
        assert!(!menu.output.is_empty(), "output should have iron ingot after 200 ticks");
    }

    #[test]
    fn furnace_stops_cooking_without_fuel() {
        let mut menu = FurnaceMenu {
            container_id: 1,
            kind: FurnaceKind::Smelting,
            ingredient: ItemStack::new(rl("minecraft:iron_ore")),
            fuel:       ItemStack::EMPTY,
            output:     ItemStack::EMPTY,
            burn_time: 0, fuel_total: 0, cook_time: 0, cook_time_total: 200,
            stored_xp: 0.0,
        };
        let rm = RecipeManager::new();
        menu.server_tick(&rm, &no_tags());
        assert_eq!(menu.cook_time, 0);
        assert!(!menu.is_burning());
    }

    // --- Fuel burn times ---

    #[test]
    fn coal_burns_for_1600_ticks() {
        assert_eq!(fuel_burn_time(&rl("minecraft:coal")), 1600);
    }

    #[test]
    fn lava_bucket_burns_longest() {
        assert!(fuel_burn_time(&rl("minecraft:lava_bucket"))
                > fuel_burn_time(&rl("minecraft:coal_block")));
    }

    #[test]
    fn unknown_item_has_zero_burn_time() {
        assert_eq!(fuel_burn_time(&rl("minecraft:dirt")), 0);
    }

    // --- Recipe book unlock ---

    #[test]
    fn recipe_unlocked_only_once() {
        let mut book = PlayerRecipeBook::new();
        let id = rl("minecraft:crafting_table");
        assert!(book.try_unlock(&id));   // first time: newly unlocked
        assert!(!book.try_unlock(&id)); // second time: already unlocked
    }

    // --- QuickMoveStack ---

    #[test]
    fn shift_click_output_moves_to_hotbar() {
        let mut slots: Vec<ItemStack> = vec![ItemStack::EMPTY; 41];
        slots[0] = ItemStack::new_count(rl("minecraft:iron_ingot"), 1); // output
        QuickMoveStack::crafting_menu(&mut slots, 0);
        assert!(slots[0].is_empty());
        // Should have moved to hotbar slot 32..40
        let in_hotbar = slots[32..40].iter().any(|s| !s.is_empty());
        assert!(in_hotbar);
    }
}
```
