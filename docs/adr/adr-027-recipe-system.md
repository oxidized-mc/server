# ADR-027: Recipe System

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P29 |
| Deciders | Oxidized Core Team |

## Context

Minecraft's crafting and smelting systems are driven by recipes loaded from JSON data packs.
As of version 26.1, there are 8 distinct recipe types: shaped crafting, shapeless crafting,
furnace smelting, blast furnace smelting, smoking, campfire cooking, stonecutting, and smithing
transformation. Each type has its own matching logic, timer, and output behavior. The recipe
system is invoked constantly — every time a player places or removes an item in a crafting
grid, the server must find the matching recipe (if any) and report the result to the client.
For a server with 100 players, many of whom are crafting simultaneously, recipe matching is a
significant workload.

Shaped crafting is the most complex matching problem. A shaped recipe defines a 2D pattern of
ingredients (up to 3×3) that must be matched against the crafting grid. The pattern can be
placed at any valid position within the grid, and optionally mirrored horizontally. A 3×3
recipe in a 3×3 grid has 1 position × 2 mirror states = 2 checks. A 2×2 recipe in a 3×3 grid
has 4 positions × 2 mirrors = 8 checks. Each check requires comparing every slot against its
expected ingredient (which may be an item, an item tag, or empty). Vanilla performs this as a
linear scan over all shaped recipes, trying all positions for each — O(R × P) where R is recipe
count and P is positions. With hundreds of shaped recipes, this is noticeable.

The recipe book adds another dimension. When a player unlocks a recipe (via advancement or
`/recipe give`), the server sends a `ClientboundRecipeBookAddPacket`. The recipe book UI
groups recipes by result item and shows them as clickable entries. The server must track per-
player recipe unlock state and provide efficient lookup by result item for the recipe book
display. This state persists across sessions in the player data file.

## Decision Drivers

- **Vanilla-correct matching**: Recipe matching must produce the same result as vanilla for
  any given grid configuration — including edge cases with overlapping recipes, item tags,
  and remainder items.
- **Fast crafting grid lookup**: Players interact with crafting tables at high frequency;
  recipe matching must be fast enough that moving items in the grid feels instant.
- **Data pack support**: Custom recipes from data packs must be loadable, and `/reload` must
  update the recipe set without restart.
- **Recipe book integration**: Efficient lookup by result item for the recipe book display;
  per-player unlock tracking with persistence.
- **Type safety**: Recipe deserialization should catch errors at load time, not at match time.
- **Memory efficiency**: The recipe index structures should not use excessive memory — there
  are ~800 vanilla recipes, but data packs can add thousands.

## Considered Options

### Option 1: Linear Scan Matching (Vanilla Approach)

For each crafting attempt, iterate over all recipes of the relevant type and test each one.
Shaped recipes try all positions and mirrors. First match wins.

**Pros**: Simple to implement, matches vanilla behavior exactly, trivially correct for recipe
ordering edge cases.

**Cons**: O(R × P) per lookup. With 800 recipes and crafting grids being updated on every
item move, this adds up. Data packs with 2000+ recipes make this unacceptable.

### Option 2: Trie-Based Pattern Matching

Build a trie where each level corresponds to a grid slot and edges are labeled with ingredient
types. Traversing the trie matches recipes in O(grid_size) regardless of recipe count.

**Pros**: Optimal asymptotic lookup time. Works well for exact-item recipes.

**Cons**: Item tag ingredients explode the trie (a tag with 50 items creates 50 edges per
node). Trie construction is complex for variable-position shaped recipes. Memory usage can be
high. Extremely complex to implement correctly with all edge cases.

### Option 3: Hash-Based Ingredient Matching with Collision Resolution

Hash the ingredient set (sorted for shapeless, positional for shaped) and use a hash map for
O(1) average-case lookup. Handle collisions (tag overlaps, variable positions) with fallback
linear scan within the collision bucket.

**Pros**: Excellent average-case performance, relatively simple implementation.

**Cons**: Hashing item tags is problematic (a tag's contents can change on reload). Shaped
recipe positioning means multiple hash entries per recipe. Hash function design for
ingredients is non-trivial.

### Option 4: Pre-Indexed Recipe Tables

Build multiple index structures at load time: ingredient-set index for shapeless, result-item
index for recipe book, type index for filtering. Use the index to narrow candidates, then
linear scan the candidates.

**Pros**: Dramatically reduces the search space without the complexity of tries or perfect
hashing. Index rebuild on `/reload` is fast. Naturally supports recipe book queries. Handles
item tags correctly (index by concrete items in the tag).

**Cons**: Not O(1) — still linear in the candidate set. Requires maintaining multiple indices.
Slightly more memory than pure linear scan.

## Decision

**Pre-indexed recipe lookup with fallback linear scan.** At load time, we build index
structures optimized for each recipe type. The recipe registry holds all recipes and provides
type-safe access through a `RecipeManager` that owns the indices and the recipe data.

### Shaped Crafting Index

For shaped recipes, we build an index keyed by the recipe's "ingredient signature" — the set
of distinct item IDs that appear in the pattern. When a player updates their crafting grid,
we compute the ingredient signature of the grid contents and look up candidate recipes.
Candidates are then tested with full positional matching (all valid positions × mirror).

```rust
pub struct ShapedRecipe {
    pub id: ResourceLocation,
    pub group: String,
    pub category: CraftingCategory,
    pub pattern: ShapedPattern,
    pub result: ItemStack,
    pub show_notification: bool,
}

pub struct ShapedPattern {
    pub width: u8,    // 1-3
    pub height: u8,   // 1-3
    pub ingredients: Vec<Ingredient>,  // row-major, width * height elements
}
```

Matching algorithm for shaped recipes:

1. Compute the bounding box of non-empty slots in the crafting grid.
2. If bounding box dimensions don't match any recipe's dimensions, skip shaped matching.
3. For recipes matching the bounding box size, try all valid placements:
   - For a W×H recipe in a 3×3 grid: (3-W+1) × (3-H+1) placements.
   - For each placement, try normal and mirrored orientation.
   - Short-circuit: on first slot mismatch, skip to next placement.
4. Return the first recipe that matches (recipe ordering is significant).

### Shapeless Crafting Index

Shapeless recipes are indexed by their sorted ingredient set. At match time, collect the non-
empty grid items, sort them, and look up candidates with the same item count. For each
candidate, verify that every ingredient matches using a bipartite matching algorithm (since
ingredients may use tags that overlap).

```rust
pub struct ShapelessRecipe {
    pub id: ResourceLocation,
    pub group: String,
    pub category: CraftingCategory,
    pub ingredients: Vec<Ingredient>,  // 1-9 ingredients
    pub result: ItemStack,
}
```

### Ingredient Matching

An `Ingredient` represents a set of items that satisfy a recipe slot:

```rust
pub enum Ingredient {
    Empty,
    Items(Vec<ItemStack>),   // any of these items match
    TagKey(TagKey<Item>),    // any item in this tag matches
}
```

Tag-based ingredients are resolved at load time to their concrete item set, but the tag
reference is retained so that `/reload` can update the resolution. Matching uses
`ingredient.test(stack)` which checks item ID equality (ignoring count and NBT unless the
recipe specifies otherwise).

### Smelting Recipes (Furnace, Blast Furnace, Smoker, Campfire)

Smelting recipes are simpler — single input, single output, with cook time and experience:

```rust
pub struct SmeltingRecipe {
    pub id: ResourceLocation,
    pub group: String,
    pub category: CookingCategory,
    pub ingredient: Ingredient,
    pub result: ItemStack,
    pub experience: f32,
    pub cooking_time: u32,  // ticks
}
```

Cook times by type:

| Type | Default Ticks | Seconds |
|------|---------------|---------|
| Furnace | 200 | 10.0 |
| Blast Furnace | 100 | 5.0 |
| Smoker | 100 | 5.0 |
| Campfire | 600 | 30.0 |

Smelting recipes are indexed by input item ID for O(1) lookup. Fuel burn times are a separate
registry (not a recipe type) — a `HashMap<ItemId, u32>` mapping items to burn duration in
ticks (e.g., coal = 1600, blaze rod = 2400, lava bucket = 20000, wooden slab = 150).

### Stonecutting Recipes

Stonecutting is a simple 1-input → 1-output mapping. All matching recipes are shown
simultaneously (the player picks which output they want). Indexed by input item ID.

```rust
pub struct StonecuttingRecipe {
    pub id: ResourceLocation,
    pub ingredient: Ingredient,
    pub result: ItemStack,
}
```

### Smithing Recipes

Smithing transforms combine a template, a base item, and an addition item to produce a result.
Used primarily for netherite upgrades and armor trims.

```rust
pub struct SmithingTransformRecipe {
    pub id: ResourceLocation,
    pub template: Ingredient,
    pub base: Ingredient,
    pub addition: Ingredient,
    pub result: ItemStack,
}

pub struct SmithingTrimRecipe {
    pub id: ResourceLocation,
    pub template: Ingredient,
    pub base: Ingredient,
    pub addition: Ingredient,
}
```

### Recipe Book Integration

The recipe book requires:

1. **By-result index**: `HashMap<ItemId, Vec<RecipeId>>` for displaying recipes that produce
   a given item.
2. **Per-player state**: Each player has a `RecipeBookState` tracking unlocked recipes,
   display filters, and category open/close state. Persisted in player data NBT.
3. **Packets**: `ClientboundRecipeBookAddPacket` (send on unlock),
   `ClientboundRecipeBookRemovePacket` (send on `/recipe take`),
   `ClientboundRecipeBookSettingsPacket` (sync filter state).

```rust
pub struct RecipeBookState {
    pub unlocked: HashSet<ResourceLocation>,
    pub to_highlight: HashSet<ResourceLocation>,  // show notification dot
    pub gui_open: bool,
    pub filtering_craftable: bool,
    pub furnace_gui_open: bool,
    pub furnace_filtering_craftable: bool,
    pub blast_furnace_gui_open: bool,
    pub blast_furnace_filtering_craftable: bool,
    pub smoker_gui_open: bool,
    pub smoker_filtering_craftable: bool,
}
```

### Experience Rewards

Smelting recipes grant experience when the result is extracted. Experience accumulates as a
floating-point value and is dropped as XP orbs (rounded probabilistically — 0.7 XP means
70% chance of 1 XP orb, 30% chance of 0). This is handled by the furnace block entity, not
the recipe system itself.

## Consequences

### Positive

- **Fast common case**: The ingredient index reduces candidate sets to typically < 5 recipes
  for crafting, making matching nearly instant even with large data packs.
- **Correct matching**: Full positional + mirror testing for shaped recipes matches vanilla
  behavior exactly.
- **Efficient recipe book**: By-result index provides O(1) recipe book display queries.
- **Clean separation**: Recipe types are distinct Rust types, not a single polymorphic
  hierarchy; each is optimized for its specific matching pattern.
- **Safe reload**: Index rebuild on `/reload` atomically swaps the entire recipe set.

### Negative

- **Multiple indices**: Maintaining ingredient index, result index, and type-specific indices
  adds memory overhead (~100KB for vanilla recipe set, scales linearly with data packs).
- **Index rebuild cost**: `/reload` must rebuild all indices. With 2000+ recipes this takes
  ~10ms — acceptable but noticeable if data packs grow very large.
- **Tag resolution coupling**: Ingredient tags must be resolved before recipe indexing,
  creating a load-order dependency (tags → recipes).

### Neutral

- Recipe ordering within a type matters for matching priority. We preserve insertion order
  (data pack load order) and document this requirement.
- The recipe system is read-heavy, write-rare (only changes on `/reload`), which aligns well
  with the immutable-index-with-atomic-swap pattern.

## Compliance

- [ ] All ~800 vanilla recipes parse without error.
- [ ] Shaped recipe matching produces correct results for all vanilla crafting recipes,
  including mirror variants and multi-position placements.
- [ ] Shapeless matching handles tag-based ingredients with overlapping items correctly.
- [ ] Smelting cook times match vanilla defaults (200/100/100/600 ticks).
- [ ] Recipe book packets (`ClientboundRecipeBookAddPacket`) contain correct recipe data.
- [ ] `/reload` updates recipes and indices atomically; in-progress crafting is not affected.
- [ ] Benchmark: crafting grid match < 10μs for vanilla recipe set, < 50μs with 2000 recipes.
- [ ] Fuel burn times for all vanilla fuel items are correct.

## Related ADRs

- **ADR-005**: Data-Driven Registry Architecture (recipes are a data-driven registry)
- **ADR-012**: NBT & Data Codec (smithing recipes modify NBT for trim patterns)
- **ADR-026**: Loot Table & Predicate Engine (shares NumberProvider and Ingredient patterns)
- **ADR-028**: Chat & Text Component System (recipe toast notifications use text components)

## References

- [Minecraft Wiki — Recipe](https://minecraft.wiki/w/Recipe)
- [Minecraft Wiki — Crafting](https://minecraft.wiki/w/Crafting)
- [Minecraft Wiki — Smelting](https://minecraft.wiki/w/Smelting)
- [Minecraft Wiki — Smithing](https://minecraft.wiki/w/Smithing)
- [Vanilla recipe JSON format](https://minecraft.wiki/w/Recipe#JSON_format)
- [Data pack recipe loading](https://minecraft.wiki/w/Data_pack#Recipes)
