# ADR-026: Loot Table & Predicate Engine

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P34 |
| Deciders | Oxidized Core Team |

## Context

Loot tables are the backbone of Minecraft's item generation system. Every mob death, block
break, chest opening, fishing attempt, and advancement reward ultimately resolves through a
loot table. Vanilla Minecraft loads loot tables from JSON files (both built-in and from data
packs), constructing an in-memory tree of pools, entries, conditions, and functions. This tree
is evaluated at runtime whenever an event triggers a loot table. The evaluation engine must
produce drops that are bit-for-bit identical to vanilla for server compliance — clients and
tools depend on deterministic loot generation seeded by world seed and position.

The vanilla Java implementation uses a deep class hierarchy rooted in `LootTable`, with
polymorphic `LootPool`, `LootPoolEntryContainer`, `LootItemCondition`, and `LootItemFunction`
types. Each node is deserialized from JSON via Gson with runtime type adapters, and evaluation
walks the tree with virtual dispatch at every node. While correct, this design is allocation-
heavy (each evaluation creates intermediate lists, random instances, and boxed predicates) and
cache-unfriendly (nodes are scattered across the heap). For a Rust server targeting 100+
players, loot evaluation is a hot path — mob farms alone can trigger thousands of evaluations
per second.

The predicate system is tightly coupled to loot tables. Conditions like `random_chance`,
`entity_properties`, `match_tool`, `block_state_property`, and `weather_check` are reused in
item predicates, advancement triggers, and command predicates (`/execute if predicate`). Any
design must treat predicates as a shared subsystem, not loot-table-specific. Additionally,
data packs can override any loot table, so the engine must support hot-reloading without
server restart.

## Decision Drivers

- **Vanilla correctness**: Drops must match vanilla exactly for the same seed and context,
  including edge cases like Looting enchantment interaction with random chance conditions.
- **Evaluation performance**: Loot evaluation is a hot path; we need sub-microsecond
  evaluation for simple tables and minimal allocation during evaluation.
- **Data pack support**: Custom loot tables from data packs must be loadable, validatable,
  and hot-reloadable via `/reload`.
- **Shared predicate system**: Conditions must be reusable across loot tables, item
  predicates, advancement triggers, and the `/execute if predicate` command.
- **Type safety**: Invalid loot tables should fail at load time with clear errors, not at
  runtime with cryptic panics.
- **Extensibility**: New condition types and functions are added in nearly every Minecraft
  version; the system must be easy to extend.

## Considered Options

### Option 1: Interpreted JSON Tree Walk at Runtime

Retain the JSON structure in memory and evaluate by walking the JSON tree at runtime. Each
node is a `serde_json::Value` that is pattern-matched during evaluation.

**Pros**: Minimal parsing code, trivial hot-reload (just re-parse JSON), easy to debug
(structure mirrors source file).

**Cons**: Extremely slow — JSON value access involves string key lookups and type checks on
every evaluation. No type safety: invalid tables only fail when evaluated, not when loaded.
High allocation pressure from `serde_json::Value` (each node is a heap allocation). Not
viable for a hot path.

### Option 2: Pre-compiled Loot Table Bytecode

Parse loot tables into a custom bytecode format. A small VM evaluates the bytecode with a
stack machine. Instructions like `PUSH_RANDOM`, `CHECK_CONDITION`, `APPLY_FUNCTION`,
`EMIT_ITEM` drive evaluation.

**Pros**: Potentially very fast (tight evaluation loop, good instruction cache locality),
compact representation, could support JIT compilation later.

**Cons**: Enormous implementation complexity. Debugging is opaque (bytecode doesn't map
obviously to JSON source). Bytecode format must be versioned and maintained. Overkill for the
problem size — loot tables rarely exceed 50 nodes. The compilation step adds latency to
`/reload`.

### Option 3: Rust Trait Objects for Loot Tree Nodes

Define a `LootEntry` trait, `LootCondition` trait, `LootFunction` trait, etc. Each vanilla
type gets a struct implementing the trait. Evaluation uses `dyn LootEntry` virtual dispatch.

**Pros**: Familiar OOP pattern, extensible (new types just implement the trait), matches
vanilla's architecture closely.

**Cons**: Virtual dispatch at every node (indirect call, poor branch prediction). Trait
objects are heap-allocated and scattered in memory. Each node is behind a `Box<dyn Trait>`,
preventing the compiler from inlining evaluation. Harder to serialize/deserialize (requires
`typetag` or manual registry). Memory layout is suboptimal.

### Option 4: Enum-Based Discriminated Union Tree

Parse loot tables into a tree of Rust enums: `LootEntry::Item { ... }`,
`LootEntry::Tag { ... }`, `LootCondition::RandomChance { chance: f32 }`, etc. Evaluation is
a recursive `match` on enums.

**Pros**: No virtual dispatch — enum matching compiles to jump tables. Enums are sized at
compile time, enabling stack allocation and contiguous memory layout. Pattern matching is
exhaustive — the compiler catches missing variants. Excellent debuggability (enums derive
`Debug`). Serde support is straightforward with `#[serde(tag = "type")]`.

**Cons**: Adding a new variant requires touching the enum definition (but this is a feature,
not a bug — we want compile-time verification). Large enums can waste memory if variants
differ significantly in size (mitigated with `Box` for rare large variants).

## Decision

**Enum-based loot tree with pre-validated structure.** Loot tables are parsed from JSON into
a tree of Rust enums. The core types are:

```rust
pub enum LootEntry {
    Item { name: ItemId, weight: i32, quality: i32, conditions: Vec<LootCondition>, functions: Vec<LootFunction> },
    Tag { tag: TagKey, expand: bool, weight: i32, conditions: Vec<LootCondition>, functions: Vec<LootFunction> },
    LootTable { table: ResourceLocation, conditions: Vec<LootCondition>, functions: Vec<LootFunction> },
    Alternatives { children: Vec<LootEntry>, conditions: Vec<LootCondition> },
    Sequence { children: Vec<LootEntry>, conditions: Vec<LootCondition> },
    Group { children: Vec<LootEntry>, conditions: Vec<LootCondition> },
    Dynamic { name: ResourceLocation, conditions: Vec<LootCondition> },
    Empty { weight: i32, conditions: Vec<LootCondition> },
}

pub enum LootCondition {
    RandomChance { chance: NumberProvider },
    RandomChanceWithEnchantedBonus { unenchanted_chance: f32, enchanted_chance: LevelBasedValue, enchantment: EnchantmentId },
    EntityProperties { entity: LootContextEntity, predicate: Box<EntityPredicate> },
    KilledByPlayer,
    BlockStateProperty { block: BlockId, properties: HashMap<String, StatePropertyMatcher> },
    MatchTool { predicate: Box<ItemPredicate> },
    TableBonus { enchantment: EnchantmentId, chances: Vec<f32> },
    Inverted { term: Box<LootCondition> },
    AllOf { terms: Vec<LootCondition> },
    AnyOf { terms: Vec<LootCondition> },
    WeatherCheck { raining: Option<bool>, thundering: Option<bool> },
    Reference { name: ResourceLocation },
    TimeCheck { value: NumberProvider, period: Option<i64> },
    LocationCheck { predicate: Box<LocationPredicate>, offset_x: f64, offset_y: f64, offset_z: f64 },
    DamageSourceProperties { predicate: Box<DamageSourcePredicate> },
    SurvivesExplosion,
    EnchantmentActiveCheck { active: bool },
}

pub enum LootFunction {
    SetCount { count: NumberProvider, add: bool },
    SetDamage { damage: NumberProvider, add: bool },
    SetNbt { tag: NbtCompound },
    EnchantWithLevels { levels: NumberProvider, options: Option<TagKey> },
    EnchantRandomly { options: Option<TagKey> },
    ApplyBonus { enchantment: EnchantmentId, formula: BonusFormula },
    LootingEnchant { count: NumberProvider, limit: i32 },
    SetPotion { id: PotionId },
    FurnaceSmelt,
    ExplosionDecay,
    CopyName { source: CopyNameSource },
    CopyNbt { source: NbtProviderType, operations: Vec<CopyNbtOperation> },
    LimitCount { limit: IntRange },
    SetContents { entries: Vec<LootEntry>, content_type: BlockEntityType },
    FillPlayerHead { entity: LootContextEntity },
    // ... additional function variants
}

pub enum NumberProvider {
    Constant(f32),
    Uniform { min: Box<NumberProvider>, max: Box<NumberProvider> },
    Binomial { n: Box<NumberProvider>, p: Box<NumberProvider> },
    Score { target: ScoreboardTarget, score: String, scale: f32 },
    Storage { storage: ResourceLocation, path: String },
    EnchantmentLevel,
}
```

At load time, the entire loot table tree is validated: item IDs are checked against the
registry, tag references are verified, NumberProvider ranges are sanity-checked, and recursive
loot table references are detected. Invalid tables produce detailed error messages with JSON
path context (e.g., `loot_tables/entities/zombie.json: pools[0].entries[2].functions[1]:
unknown function type 'minecraft:invalid'`). This means runtime evaluation never encounters
invalid data and can omit defensive checks.

Evaluation uses `LootContext`, a struct carrying all parameters relevant to the current loot
event. Parameters are typed and gated by `LootContextParamSet` — a loot table declared as
`minecraft:entity` requires `THIS_ENTITY` and `LAST_DAMAGE_PLAYER` but not `BLOCK_STATE`.
The evaluator recursively matches on enums, accumulating items into a `Vec<ItemStack>`. The
random source is a `LootRng` wrapper around a seedable Xoshiro256StarStar PRNG, matching
vanilla's random sequence behavior.

## LootContext Parameters

The `LootContext` struct provides typed access to contextual data:

```rust
pub struct LootContext {
    pub this_entity: Option<EntityRef>,
    pub last_damage_player: Option<PlayerRef>,
    pub damage_source: Option<DamageSource>,
    pub attacking_entity: Option<EntityRef>,
    pub direct_attacking_entity: Option<EntityRef>,
    pub origin: Vec3,
    pub block_state: Option<BlockState>,
    pub tool: Option<ItemStack>,
    pub explosion_radius: Option<f32>,
    pub luck: f32,
    pub level: LevelRef,
    pub random: LootRng,
    pub enchantment_level: Option<i32>,
    pub enchantment_active: bool,
}
```

`LootContextParamSet` defines which parameters are required vs optional for each table type:

| Param Set | Required | Optional |
|-----------|----------|----------|
| `empty` | — | — |
| `chest` | origin | this_entity |
| `command` | origin | this_entity |
| `selector` | origin | this_entity |
| `fishing` | origin, tool | this_entity |
| `entity` | this_entity, origin, damage_source | last_damage_player, attacking_entity, direct_attacking_entity |
| `equipment` | this_entity, origin | — |
| `archaeology` | origin | — |
| `gift` | this_entity, origin | — |
| `barter` | this_entity | — |
| `advancement_reward` | this_entity, origin | — |
| `advancement_entity` | this_entity, origin | — |
| `advancement_location` | this_entity, origin, tool, block_state | — |
| `block` | block_state, origin, tool | this_entity, explosion_radius |
| `shearing` | origin | this_entity |
| `enchanted_damage` | this_entity, damage_source, attacking_entity, enchantment_level | — |
| `enchanted_wear` | this_entity, enchantment_level | — |
| `enchanted_item` | tool, enchantment_level | — |
| `enchanted_location` | this_entity, origin, enchantment_level | — |
| `enchanted_entity` | this_entity, origin, enchantment_level | — |

## Condition Composition and Function Chaining

Conditions compose with short-circuit evaluation: `AllOf` stops on first false, `AnyOf` stops
on first true. The `Inverted` wrapper provides negation. Conditions are evaluated before pool
rolls — if the pool-level conditions fail, no entries are evaluated. Entry-level conditions
gate individual entries within a pool.

Functions chain sequentially: each function receives the `ItemStack` produced by the previous
function (or the base item). Functions can modify count, damage, NBT, enchantments, lore,
name, and attributes. Pool-level functions apply to every item generated by the pool.
Table-level functions apply to every item generated by the entire table.

## Enchantment Integration

- **Looting**: `LootingEnchant` function adds `count * looting_level` items (with random
  variance from the `count` NumberProvider). The looting level comes from
  `LootContext.attacking_entity`'s held weapon.
- **Fortune**: `ApplyBonus` with `BonusFormula::UniformBonusCount` adds 0 to
  `fortune_level` extra drops. `BonusFormula::OreDrops` uses the formula
  `count * (max(1, random(0, fortune_level + 2)))`.
- **Silk Touch**: Typically modeled as `MatchTool` condition checking for the enchantment,
  with an `Alternatives` entry that yields the block itself if silk touch, or ore drops if
  not.

## Random Source Design

Loot tables use `LootRng`, which wraps a deterministic PRNG seeded from the world seed and
event-specific data (entity UUID for mob drops, block position for block drops, chest position
for chest loot). This ensures:

- Same drops for the same event with the same seed (required for vanilla parity).
- Independent random sequences per loot context (one evaluation doesn't affect another).
- Efficient: Xoshiro256StarStar is branch-free and vectorizable.

## Data Pack Override Support

When `/reload` is executed, the loot table registry is rebuilt from scratch:

1. Load built-in vanilla loot tables from embedded data.
2. Layer data pack loot tables on top (last pack wins for same resource location).
3. Validate the entire merged set (cross-references between tables are checked).
4. Atomically swap the old registry with the new one (using `Arc::swap`).
5. In-flight evaluations against the old registry complete safely (Arc keeps it alive).

## Consequences

### Positive

- **Fast evaluation**: Enum matching compiles to efficient jump tables with no virtual
  dispatch. Simple tables (e.g., stone drops cobblestone) evaluate in ~100ns.
- **Compile-time exhaustiveness**: Adding a new condition or function variant causes compiler
  errors everywhere it needs handling — impossible to forget a case.
- **Load-time validation**: Invalid loot tables are caught immediately with clear error
  messages, not as runtime panics during gameplay.
- **Memory efficient**: Enums are stack-allocated inline; the tree has good cache locality.
- **Safe hot-reload**: Arc-based registry swap means `/reload` never races with evaluation.

### Negative

- **Enum growth**: Each new Minecraft version may add conditions/functions, requiring enum
  variant additions. This is manageable but requires tracking upstream changes.
- **Large enum size**: Some variants (e.g., `EntityProperties` with a full `EntityPredicate`)
  are much larger than others. We mitigate with `Box` for large inner types to keep the enum
  discriminant-adjacent data small.
- **No runtime plugins**: Unlike trait objects, enums can't be extended by third-party mods
  at runtime. If plugin support is added later, a `Custom(Box<dyn LootConditionExt>)`
  variant could be added as an escape hatch.

### Neutral

- The NumberProvider enum pattern is reused in recipes, particle effects, and other data-
  driven systems, establishing a project-wide convention.
- Predicate evaluation is shared with advancement triggers and command predicates, so
  correctness in loot tables implies correctness in those systems too.

## Compliance

- [ ] All vanilla loot tables (1400+) parse without error from embedded data.
- [ ] Evaluation of `entities/zombie.json` produces correct drops for 10,000 trials with
  Looting 0, I, II, III (chi-squared test against expected distribution).
- [ ] `blocks/diamond_ore.json` produces correct drops with Fortune 0–III and Silk Touch.
- [ ] Invalid loot table JSON produces a descriptive error message, not a panic.
- [ ] `/reload` swaps loot tables atomically; in-flight evaluations complete correctly.
- [ ] `LootContextParamSet` validation rejects tables that access unavailable parameters.
- [ ] Benchmark: simple loot table evaluation < 200ns, complex (10 pools) < 2μs.

## Related ADRs

- **ADR-005**: Data-Driven Registry Architecture (loot tables are a registry type)
- **ADR-012**: NBT & Data Codec (loot functions modify NBT)
- **ADR-027**: Recipe System (shares NumberProvider and predicate patterns)
- **ADR-032**: Performance & Scalability (loot evaluation is a hot path)

## References

- [Minecraft Wiki — Loot Table](https://minecraft.wiki/w/Loot_table)
- [Minecraft Wiki — Predicate](https://minecraft.wiki/w/Predicate)
- [Vanilla loot table source (decompiled)](https://github.com/misode/mcmeta)
- [Xoshiro256** PRNG](https://prng.di.unimi.it/)
- [`serde` tagged enum deserialization](https://serde.rs/enum-representations.html)
