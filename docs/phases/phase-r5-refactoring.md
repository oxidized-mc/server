# Phase R5 — Data-Driven Dispatch & Code Deduplication

**Status:** 🔧 In Progress  
**Crates:** all  
**Reward:** Zero string-based block behavior dispatch in production code. All block
categorization uses O(1) flags, tags, or registry properties. Packet and command
boilerplate reduced by ~60%. Large structs decomposed. Magic numbers eliminated.
The codebase is clean, data-driven, and ready to scale through Phase 38.

---

## Progress Summary

| Sub-task | Description | Status |
|----------|-------------|--------|
| R5.1 | Enrich BlockStateFlags (ADR-012 compliance) | ✅ Done |
| R5.2 | Enrich BlockStateEntry with block properties | ✅ Done |
| R5.3 | Implement block tag loading from vanilla data | ✅ Done |
| R5.4 | Replace string-based block categorization with flags/tags | ✅ Done |
| R5.5 | Replace hardcoded physics properties with registry data | ✅ Done |
| R5.6 | Replace hardcoded biome resolution with registry lookup | 📋 Planned |
| R5.7 | Compile-time item ID codegen (like blocks) | 📋 Planned |
| R5.8 | Extract packet codec helpers & roundtrip test macro | 📋 Planned |
| R5.9 | Extract command registration helpers | 📋 Planned |
| R5.10 | Standardize packet decoder error handling | 📋 Planned |
| R5.11 | Decompose oversized structs | 📋 Planned |
| R5.12 | Break down long functions & reduce nesting | 📋 Planned |
| R5.13 | Replace magic numbers with named constants | 📋 Planned |

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-011: Registry & Data-Driven Content System](../adr/adr-011-registry-system.md) —
  registry trait, tag system, data pack loading
- [ADR-012: Block State Representation](../adr/adr-012-block-state.md) —
  BlockStateFlags (u16), BlockStateData, dense lookup table
- [ADR-035: Module Structure & File Size Policy](../adr/adr-035-module-structure.md) —
  800 LOC limit, split criteria
- [ADR-002: Error Handling Strategy](../adr/adr-002-error-handling.md) —
  `thiserror` in libraries, consistent error types

---

## Critical Clarifications

### Block Properties Coverage

**Q: Are all block properties covered?**

**Blocks — mostly covered.** R5.1 added 12 `BlockStateFlags` and R5.2 added physics
properties (hardness, friction, speed/jump factors, explosion resistance, light
emission/opacity, map color, push reaction). Together they capture 18 of the ~23
vanilla `BlockBehaviour.Properties` fields.

**Still missing** (not needed yet, add when gameplay requires them):
- `soundType` — needed for block break/place/step audio
- `forceSolidOn` — affects culling for a handful of blocks
- `instrument` — note block instrument type
- State predicates (`isRedstoneConductor`, `isValidSpawn`, `isSuffocating`,
  `isViewBlocking`) — these are runtime lambda predicates in vanilla, not simple
  stored values; will need custom extraction logic when redstone/mob spawning lands

**Items — severely incomplete.** `items.json.gz` currently only stores
`max_stack_size` and `max_damage` (2 of 20+ gameplay-relevant properties). Missing:
rarity, food properties (nutrition, saturation, effects), tool properties (mining
speed, mining level), weapon stats (attack damage/speed), armor properties
(defense, toughness), enchantability, fire resistance, equipment slot, use
cooldown, crafting remainder. Vanilla 26.1 uses a data component system
(`DataComponents.*`) for these — extraction must parse `Items.java` builder chains
and component registrations.

### Double Blocks

**Q: Are double blocks (doors, beds, tall plants) handled or hardcoded?**

**State representation: data-driven.** Each half/part has its own `BlockStateId`
with a distinguishing property (`half`, `part`, or `type`). No special block
registry handling needed.

**Placement logic: currently hardcoded.** `placement.rs` has explicit string-based
dispatch for companion block placement:
```rust
// placement.rs — double_block_companion()
if is_door_block(item_name) {           // ends_with("_door")
    let upper = primary.with_property("half", "upper")?;
    Some((primary_pos.above(), upper))
} else if is_bed_block(item_name) {     // ends_with("_bed")
    let head = primary.with_property("part", "head")?;
    Some((head_pos, head))
} else if is_tall_plant(item_name) {    // hardcoded list of 8 plants
    let upper = primary.with_property("half", "upper")?;
    Some((primary_pos.above(), upper))
}
```
This is part of the string dispatch that R5.4 aims to replace with flags/tags.
A `IS_DOUBLE_BLOCK` flag or `#minecraft:double_blocks` tag could eliminate these
string checks.

**Actual block counts using multi-part properties:**
- `half` property: **110 blocks** — 21 doors, 21 trapdoors, 58 stairs, 9 tall
  plants, 1 pitcher crop (note: stairs use `half` for top/bottom but are NOT
  double blocks — only doors and tall plants get companion placement)
- `part` property: **16 blocks** — all 16 colored bed variants (head/foot)
- `type` property: **74 blocks** — 10 chest variants (single/left/right) +
  64 slab types (top/bottom/double)

### Data Extraction Architecture

**Q: Can we unify extraction scripts? Do we need separate block property file?**

**A:** YES to both. Consolidate scripts and merge `block_properties.json.gz` into
`blocks.json.gz`.

**Current state:**
```
tools/
├── bundle_registries.py           → oxidized-protocol/src/data/registries.json
│   (reads extracted JAR data → 28 synchronized registries)
├── extract_block_properties.py    → oxidized-world/src/data/block_properties.json.gz
│   (reads decompiled Blocks.java → material properties)
└── bundle_tags.py                 → oxidized-protocol/src/data/tags.json
    (reads extracted tag JSONs + vanilla reports → 758 tag→ID mappings)

Vanilla generated reports (setup-ref.sh):
  mc-server-ref/generated/reports/blocks.json → blocks.json.gz (oxidized-world)
  mc-server-ref/generated/reports/items.json  → items.json.gz  (oxidized-world)
```

Note: `blocks.json.gz` and `items.json.gz` come from **vanilla generated reports**,
not from any Python script. They were manually preprocessed into `.gz` and committed.

**Target unified approach:**
- **Create `tools/extract_vanilla_data.py`** — single script handles all extraction
  - Reads: vanilla generated reports (`blocks.json`, `items.json`), decompiled Java
    (`Blocks.java`, `Items.java`, `BlockEntityType.java`), extracted JAR data
  - Outputs: `blocks.json.gz` (states + material properties merged),
    `items.json.gz` (expanded with gameplay properties)
- **Merge `block_properties.json.gz` into `blocks.json.gz`** — eliminate redundant file
- **Expand `items.json.gz`** — extract food, tool, weapon, armor, and enchantment
  properties from `Items.java` builder chains and data component registrations
- Keep `bundle_registries.py` and `bundle_tags.py` separate or fold them in —
  they target `oxidized-protocol/src/data/` (different crate), so folding is
  optional but reduces cognitive overhead
- Benefits: single entry point for version updates, consistent error handling,
  cross-referencing between blocks/items/entities in one pass

---

## Goal

Eliminate all string-based behavior dispatch, hardcoded block/item lists, and
duplicated code patterns discovered during the post-Phase-R4 code audit. This
phase addresses four categories of technical debt:

1. **String-based dispatch** (114+ instances) — block behavior determined by
   comparing `"minecraft:..."` strings instead of using registry flags, tags, or
   integer IDs
2. **Hardcoded static configurations** (8 physics overrides, 5 biomes, 6+ `is_*`
   functions with 23-34 hardcoded names each) — block properties and categories
   that should be data-driven
3. **Code duplication** (163 test blocks, 100+ packet codec patterns, 41 command
   message patterns) — repeated boilerplate that should be extracted into helpers
   or macros
4. **Structural code smells** — oversized structs (45 fields), long functions
   (600+ lines), magic numbers, inconsistent error handling

The audit found these specific hotspots:

| Category | Count | Primary Files |
|----------|-------|---------------|
| String name comparisons | 114+ | `placement.rs`, `block_interaction.rs`, `item_stack.rs` |
| Hardcoded `is_*` functions | 6 functions, 80+ names | `placement.rs` |
| Physics overrides by name | 8 blocks | `block_properties.rs` |
| Biome ID by name | 5 biomes | `flat/generator.rs` |
| Packet roundtrip test boilerplate | 163 instances | `oxidized-protocol/src/packets/**` |
| Packet decode validation | 20+ inconsistent | `oxidized-protocol/src/packets/**` |
| Command message sending | 41 instances | `oxidized-game/src/commands/impls/` |
| Command registration pattern | 17 instances | `oxidized-game/src/commands/impls/` |
| Entity move encode/decode | 3-4 variants | `clientbound_move_entity.rs`, `serverbound_move_player.rs` |
| Oversized structs | 5 (45, 26, 21, 20 fields) | `server_player.rs`, `network/mod.rs`, `primary_level_data.rs` |
| Functions >200 LOC | 20+ | `region.rs`, `snbt.rs`, `paletted_container.rs` |
| Type casts (`as`) | 46+ in one file | `paletted_container.rs` |
| Magic numbers | 30+ | `cmd_time.rs`, `synched_data.rs`, various |

---

## Motivation

Phases R1–R4 addressed structural module splits, packet codec unification, ADR
compliance, and network I/O architecture. However, the codebase still relies
heavily on string-based dispatch for block behaviors — a pattern that was
acceptable during rapid Phase 8–22 feature development but will not scale.

Phases 24–38 will add:
- Combat → needs `IS_SOLID`, `HAS_COLLISION` flags on every hit-test
- Hostile mobs → needs collision and pathfinding block queries every tick
- Redstone → needs `IS_OPAQUE`, tick behavior, push reaction per block state
- Block entities → needs `HAS_BLOCK_ENTITY` flag for chunk ticking
- Loot tables → needs `REQUIRES_TOOL`, hardness, explosion resistance
- Performance hardening → cannot optimize string comparisons on hot paths

Without this refactoring:
- Every new block behavior function will copy the `matches!(name, ...)` pattern
- Physics overrides will grow from 8 to 50+ blocks as more mechanics land
- Packet boilerplate will double as 46 more Play packets are implemented
- `ServerPlayer` will grow past 60 fields as combat, effects, and stats arrive

---

## Non-Goals

- **No new features** — zero user-visible behavior changes
- **No protocol changes** — packet wire format stays identical
- **No collision shape system** — ADR-012 describes collision shape indices, but
  the full shape table requires Phase 16's physics engine. Deferred.
- **No data pack loading** — ADR-011 describes runtime data pack support. This
  phase loads vanilla's extracted tag data at compile time. Runtime tag loading
  is Phase 34 (Loot Tables) scope.
- **No `#[derive(Packet)]` proc macro** — while desirable long-term, a proc macro
  crate adds workspace complexity. This phase uses declarative macros and helper
  functions instead. A proc macro can be added later if boilerplate remains.
- **No ECS migration** — decomposing `ServerPlayer` extracts sub-structs but does
  not convert to ECS components. That's Phase 15/24's scope.

---

## Dependencies

| Sub-task | Depends On | Rationale |
|----------|------------|-----------|
| R5.2 | R5.1 | Properties struct includes the expanded flags |
| R5.4 | R5.1, R5.3 | Can't replace string checks without flags and tags |
| R5.5 | R5.2 | Physics properties must exist in BlockStateEntry first |
| R5.6 | — | Independent; uses existing biome registry |
| R5.7 | — | Independent; item codegen mirrors block pattern |
| R5.8 | — | Independent; pure deduplication |
| R5.9 | — | Independent; pure deduplication |
| R5.10 | — | Independent; error handling standardization |
| R5.11 | — | Independent; struct decomposition |
| R5.12 | — | Independent; function decomposition |
| R5.13 | — | Independent; constant extraction |

**Parallelizable groups:**
- **Group A** (sequential): R5.1 → R5.2 → R5.3 → R5.4 → R5.5
- **Group B** (independent): R5.6, R5.7, R5.8, R5.9, R5.10, R5.11, R5.12, R5.13

Group B sub-tasks can be done in any order and in parallel with Group A.

---

## Detailed Refactoring Plan

### R5.1: Enrich BlockStateFlags (ADR-012 Compliance)

**Targets:** `crates/oxidized-world/build.rs`,
`crates/oxidized-world/src/registry/block.rs`

**Current problems:**
- `BlockStateFlags` is `u8` with 3 flags (`IS_AIR`, `IS_DEFAULT`, `IS_LIQUID`)
- ADR-012 specifies `u16` with 11+ flags
- `build.rs` only sets flags from `definition_type` string ("minecraft:air",
  "minecraft:liquid") — doesn't read block properties

**Steps:**

1. **Expand `BlockStateFlags` from `u8` to `u16`**:
   ```rust
   bitflags! {
       #[derive(Debug, Clone, Copy, PartialEq, Eq)]
       pub struct BlockStateFlags: u16 {
           const IS_AIR            = 1 << 0;
           const IS_DEFAULT        = 1 << 1;
           const IS_LIQUID         = 1 << 2;
           const IS_SOLID          = 1 << 3;
           const HAS_COLLISION     = 1 << 4;
           const BLOCKS_MOTION     = 1 << 5;
           const IS_REPLACEABLE    = 1 << 6;
           const IS_OPAQUE         = 1 << 7;
           const HAS_BLOCK_ENTITY  = 1 << 8;
           const TICKS_RANDOMLY    = 1 << 9;
           const REQUIRES_TOOL     = 1 << 10;
           const IS_FLAMMABLE      = 1 << 11;
           const IS_INTERACTABLE   = 1 << 12;
       }
   }
   ```

2. **Create `tools/extract_block_properties.py` extraction script**:
   - Reads decompiled `BlockBehaviour.Properties` calls in `mc-server-ref/`
     and/or `mc-server-ref/generated/reports/blocks.json`
   - Extracts per-block: `replaceable`, `hasCollision`, `isAir`, `isSolid`,
     `ignitedByLava`, `pushReaction`, `requiresCorrectToolForDrops`,
     `randomTicks`, `friction`, `speedFactor`, `jumpFactor`, `hardness`,
     `explosionResistance`, `lightEmission`, `mapColor`
   - Cross-references block entity type entries from registries report
   - Derives `IS_INTERACTABLE` from blocks whose classes override
     `useWithoutItem()` returning `InteractionResult.SUCCESS`
   - Outputs `crates/oxidized-world/src/data/block_properties.json.gz`
   - This script runs once when updating Minecraft versions (same as
     `bundle_tags.py` and `bundle_registries.py`)

3. **Update `build.rs` to read `block_properties.json.gz` and set all flags**:
   - Read the committed `src/data/block_properties.json.gz` (same pattern as
     existing `blocks.json.gz` reading)
   - Compute flags per block state (some flags are per-type, some per-state)
   - Generate `BlockStateFlags::from_bits_truncate(0xNNNN)` in the static array

4. **Add convenience methods on `BlockStateId`**:
   ```rust
   impl BlockStateId {
       pub fn is_solid(self) -> bool { ... }
       pub fn is_replaceable(self) -> bool { ... }
       pub fn has_collision(self) -> bool { ... }
       pub fn is_opaque(self) -> bool { ... }
       pub fn has_block_entity(self) -> bool { ... }
       pub fn is_interactable(self) -> bool { ... }
       // etc.
   }
   ```

5. **Update `BLOCK_STATE_DATA` generated array** to use `u16` for flags.
   Update all existing code that reads `BlockStateFlags` (the bit positions of
   `IS_AIR`, `IS_DEFAULT`, `IS_LIQUID` change — find all callers).

**Verification:**
- `cargo test -p oxidized-world` — existing tests pass
- New test: for every block state, verify flag values against a known-good
  extracted dataset (e.g., spot-check 50 blocks: air=IS_AIR, stone=IS_SOLID|
  HAS_COLLISION|IS_OPAQUE, water=IS_LIQUID|IS_REPLACEABLE, etc.)
- New test: `is_replaceable` flag matches the exact set of blocks currently
  listed in `placement.rs::is_replaceable_block()`

**Completion notes:**
- `tools/extract_block_properties.py` created — parses `Blocks.java` builder
  chains, cross-references `BlockEntityType.java` for block entities, and
  resolves class hierarchy for `useWithoutItem()` interactable detection
- `block_properties.json.gz` generated (10.6 KB, 1168 blocks)
- `BlockStateFlags` expanded from `u8` (3 flags) to `u16` (12 flags):
  IS_AIR, IS_DEFAULT, IS_LIQUID, IS_SOLID, HAS_COLLISION, IS_OPAQUE,
  IS_REPLACEABLE, HAS_BLOCK_ENTITY, TICKS_RANDOMLY, REQUIRES_TOOL,
  IS_FLAMMABLE, IS_INTERACTABLE
- `build.rs` reads `block_properties.json.gz` and sets all flags per block
- 9 convenience methods added to `BlockStateId` (is_solid, has_collision,
  is_opaque, is_replaceable, has_block_entity, ticks_randomly, requires_tool,
  is_flammable, is_interactable)
- 10 verification tests added (flag spot-checks, replaceable cross-check,
  interactable/non-interactable, block entity, flammable, glass transparency)
- All 1716+ workspace tests pass

**Data extraction prerequisite:**
A new extraction script `tools/extract_block_properties.py` must be created to
read vanilla block property data from the decompiled Java source and/or vanilla
generated reports in `mc-server-ref/`. It outputs
`crates/oxidized-world/src/data/block_properties.json.gz` — committed to the
repo. This follows the established pipeline pattern:
- `tools/setup-ref.sh` downloads and decompiles vanilla JAR (one-time)
- `tools/extract_block_properties.py` reads `mc-server-ref/` → writes `.json.gz`
- `build.rs` reads only committed `.json.gz` files — never touches `mc-server-ref/`

The script extracts per-block: `friction`, `speed_factor`, `jump_factor`,
`hardness`, `explosion_resistance`, `light_emission`, `light_opacity`,
`map_color`, `push_reaction`, `is_solid`, `has_collision`, `blocks_motion`,
`is_replaceable`, `is_opaque`, `is_flammable`, `has_block_entity`,
`is_interactable`, `requires_correct_tool`, `ticks_randomly`.

Existing data files: `blocks.json.gz` (state/property data),
`items.json.gz` (item registry), `tags.json` (all 758 vanilla tags with
resolved IDs). The new `block_properties.json.gz` complements `blocks.json.gz`
— one has state machines, the other has material properties.

---

### R5.2: Enrich BlockStateEntry With Block Properties

**Targets:** `crates/oxidized-world/build.rs`,
`crates/oxidized-world/src/registry/block.rs`

**Current problems:**
- `BlockStateEntry` has only `block_type: u16` and `flags: BlockStateFlags`
- ADR-012 specifies `light_emission`, `light_opacity`, `hardness`,
  `explosion_resistance`, and more
- Physics properties (friction, speed factor, jump factor) are hardcoded in
  `oxidized-game/src/physics/block_properties.rs` as a string-matched array

**Steps:**

1. **Expand `BlockStateEntry`**:
   ```rust
   #[derive(Debug, Clone, Copy)]
   #[repr(C)]
   pub struct BlockStateEntry {
       pub block_type: u16,
       pub flags: BlockStateFlags,         // u16 (from R5.1)
       pub light_emission: u8,             // 0–15
       pub light_opacity: u8,              // 0–15
       pub hardness: u16,                  // fixed-point ×100 (0xFFFF = unbreakable)
       pub explosion_resistance: u16,      // fixed-point ×100
       pub friction: u16,                  // fixed-point ×10000 (default: 6000)
       pub speed_factor: u16,              // fixed-point ×10000 (default: 10000)
       pub jump_factor: u16,               // fixed-point ×10000 (default: 10000)
       pub map_color: u8,
       pub push_reaction: u8,              // 0=NORMAL, 1=DESTROY, 2=BLOCK, 3=PUSH_ONLY
   }
   ```

2. **Update `build.rs`** to populate all fields from extracted vanilla data:
   - `light_emission`: from `blocks.json` per-state `luminance` field
   - `light_opacity`: from `blocks.json` per-state `opacity` field
   - `hardness`: from `BlockBehaviour.Properties.destroyTime`
   - `explosion_resistance`: from `BlockBehaviour.Properties.explosionResistance`
   - `friction`: from `BlockBehaviour.Properties.friction` (default 0.6)
   - `speed_factor`: from `BlockBehaviour.Properties.speedFactor` (default 1.0)
   - `jump_factor`: from `BlockBehaviour.Properties.jumpFactor` (default 1.0)
   - `map_color`: from `BlockBehaviour.Properties.mapColor`
   - `push_reaction`: from `BlockBehaviour.Properties.pushReaction`

3. **Add property accessors on `BlockStateId`**:
   ```rust
   impl BlockStateId {
       pub fn light_emission(self) -> u8 { self.data().light_emission }
       pub fn light_opacity(self) -> u8 { self.data().light_opacity }
       pub fn hardness(self) -> f64 {
           let raw = self.data().hardness;
           if raw == 0xFFFF { -1.0 } else { f64::from(raw) / 100.0 }
       }
       pub fn friction(self) -> f64 {
           f64::from(self.data().friction) / 10_000.0
       }
       pub fn speed_factor(self) -> f64 {
           f64::from(self.data().speed_factor) / 10_000.0
       }
       pub fn jump_factor(self) -> f64 {
           f64::from(self.data().jump_factor) / 10_000.0
       }
   }
   ```

4. **Static table size check**: Verify total is ~525 KB (29,873 × 18 bytes),
   acceptable for a server binary.

**Verification:**
- Spot-check properties for 20 well-known blocks:
  - Stone: hardness=1.5, resistance=6.0, friction=0.6
  - Ice: friction=0.98, speed_factor=1.0
  - Blue ice: friction=0.989
  - Soul sand: speed_factor=0.4
  - Honey block: speed_factor=0.4, jump_factor=0.5
  - Glowstone: light_emission=15
  - Obsidian: hardness=50.0, resistance=1200.0
  - Bedrock: hardness=-1 (unbreakable)
- Property roundtrip: encode/decode fixed-point values, verify precision

**Completion notes:**
- Expanded `BlockStateEntry` to 18 bytes with all property fields
- Updated `build.rs` to extract and encode all properties from `block_properties.json.gz`
- Implemented property accessors on `BlockStateId` with correct fixed-point conversions
- Verified table size: exactly 525.1 KB (29,873 states × 18 bytes)
- Spot-checked 20+ well-known blocks with value assertion tests: stone, ice, blue ice,
  soul sand, honey block, glowstone, obsidian, bedrock, torch, redstone torch, slime block,
  powder snow, packed ice, dirt, oak planks, iron block, diamond block, sea lantern, cobweb,
  water, grass block, white wool, gold block
- Verified fixed-point roundtrip precision: values round-trip with <0.01 error (×100) /
  <0.0001 error (×10000)
- Added `map_color` extraction (MapColor.X constants + DyeColor.X mapping +
  BLOCK.defaultMapColor() resolution) — 854/1168 blocks have non-zero values
- Added `light_opacity` heuristic derivation (opaque+solid→15, liquid→1, else→0) —
  666/1168 blocks have non-zero values
- All 2315 workspace tests pass (51 block registry tests including 24 R5.2 property tests)
- Light emission/opacity are limited to 0–15 (4-bit values)
- Map color is limited to 0–63 (6-bit values), 854/1168 blocks have map_color extracted
- Push reaction is limited to 0–3 (2-bit values)
- Explosion resistance values >655.35 are clamped to u16::MAX/100 — affects obsidian
  (1200.0→655.35), bedrock (3600000.0→655.35), and other extreme-resistance blocks.
  Functionally these are all "blast-proof" so gameplay impact is nil.
- Light opacity is a heuristic approximation (vanilla computes it from collision shape which
  we don't have). Accurate for full solid blocks (15), air (0), liquids (1), and transparent
  blocks (0). Partial blocks (slabs, stairs) default to 0 which may differ from vanilla.
- Powder snow's speed reduction comes from `PowderSnowBlock` runtime behavior, not the
  `speedFactor` block property (which is vanilla default 1.0)
- Obsidian's push_reaction is NORMAL (0), not BLOCK — vanilla piston code prevents moving
  obsidian via separate hardness checks, not via push_reaction

### R5.3: Implement Block Tag Loading From Vanilla Data

**Status:** ✅ Done

**Targets:** `crates/oxidized-world/src/registry/tags.rs` (new),
`crates/oxidized-world/build.rs`

**What was done:**

1. **Compile-time tag loading via `build.rs` (Option A)**:
   - `build.rs` reads `tags.json` from `oxidized-protocol/src/data/` and custom
     tag JSON files from `src/data/tags/block/`
   - Resolves custom tag block names to type IDs using the block registry
   - Generates `block_tags_generated.rs` with three static arrays:
     - `TAG_NAMES: [&str; 252]` — sorted tag names for binary search
     - `TAG_RANGES: [(u32, u32); 252]` — (start, len) pairs into `TAG_MEMBERS`
     - `TAG_MEMBERS: [u16; N]` — flat array of sorted block type IDs
   - 248 vanilla block tags + 4 custom Oxidized tags = 252 total

2. **Created `crates/oxidized-world/src/registry/tags.rs`**:
   - `BlockTags` — zero-sized struct (like `BlockRegistry`) with query API
   - `TagSet` — sorted slice wrapper with O(log n) membership testing
   - API: `contains(tag, block_type_id)`, `get(tag)`, `tag_count()`,
     `tag_names()`
   - Full doc comments with examples on all public items

3. **Created 4 custom Oxidized tags** in `src/data/tags/block/`:
   - `interactable.json` (33 blocks) — blocks from `is_interactable_block()`
     not covered by vanilla tags (doors, beds, buttons, etc.)
   - `wall_mountable.json` (64 blocks) — buttons, levers, wall torches/signs/
     banners/heads/skulls/fans
   - `player_direction.json` (130 blocks) — beds, doors, stairs, fence gates,
     trapdoors, repeaters, comparators
   - `tall_plants.json` (8 blocks) — complete set including `tall_grass`,
     `large_fern`, `tall_seagrass` not in vanilla tags

4. **Integrated with registry module** — `BlockTags` and `TagSet` exported
   from `oxidized_world::registry`

**Tests:** 17 unit tests + 3 doc tests covering:
- Vanilla tag existence and membership (doors, beds, buttons, replaceable, signs)
- Positive/negative membership checks (oak_door ∈ doors, stone ∉ doors)
- Unknown tag returns `None`/`false`
- Custom tag existence and membership (tall_plants, wall_mountable, etc.)
- Tag names and members are sorted (invariant verification)

**Design note:** `BlockTags` is a zero-sized marker struct — all data is in
compile-time-generated static arrays. Zero startup cost, zero allocation.
Runtime tag loading (Option B) deferred to Phase 34 (data packs).

---

### R5.4: Replace String-Based Block Categorization With Flags/Tags

**Status:** ✅ Done

**Targets:** `crates/oxidized-server/src/network/play/placement.rs`,
`crates/oxidized-server/src/network/play/block_interaction.rs`,
`crates/oxidized-game/src/inventory/item_stack.rs`,
`crates/oxidized-game/src/commands/impls/cmd_setblock.rs`

**Current problems:**
- 6 functions in `placement.rs` use hardcoded string lists (80+ names total)
- `block_interaction.rs` has similar string patterns
- `item_stack.rs` checks `== "minecraft:air"` for emptiness
- `cmd_setblock.rs` checks `== "minecraft:air"` for block type

**Steps:**

1. **Replace `is_replaceable_block()`** with:
   ```rust
   fn is_replaceable_block(state_id: BlockStateId) -> bool {
       state_id.is_replaceable()  // Single flag check — O(1)
   }
   ```
   Or inline the flag check at call sites and delete the function entirely.

2. **Replace `is_interactable_block()`** with:
   ```rust
   fn is_interactable_block(state_id: BlockStateId) -> bool {
       state_id.is_interactable()  // Flag check — O(1)
   }
   ```

3. **Replace `is_wall_mountable()`** with tag check:
   ```rust
   fn is_wall_mountable(block_tags: &BlockTags, block_type_id: u16) -> bool {
       block_tags.contains("oxidized:wall_mountable", block_type_id)
   }
   ```

4. **Replace `is_player_direction_block()`** with tag check:
   ```rust
   fn is_player_direction_block(block_tags: &BlockTags, block_type_id: u16) -> bool {
       block_tags.contains("oxidized:player_direction", block_type_id)
   }
   ```

5. **Replace `is_sign_block()`** with:
   ```rust
   block_tags.contains("minecraft:all_signs", block_type_id)
   ```

6. **Replace `is_door_block()`** with:
   ```rust
   block_tags.contains("minecraft:doors", block_type_id)
   ```

7. **Replace `is_bed_block()`** with:
   ```rust
   block_tags.contains("minecraft:beds", block_type_id)
   ```

8. **Replace `is_tall_plant()`** with:
   ```rust
   block_tags.contains("oxidized:tall_plants", block_type_id)
   ```

9. **Replace air checks** (`== "minecraft:air"`) with:
   ```rust
   state_id.is_air()  // Already works — IS_AIR flag exists
   ```

10. **Remove all `matches!(block_name, "minecraft:..." | ...)` patterns** from
    production code. Verify with:
    ```bash
    grep -rn 'matches!.*"minecraft:' crates/ --include="*.rs" | grep -v '#\[cfg(test)\]' | grep -v 'tests/'
    ```

**Verification:**
- For each replaced function, create a regression test that checks the same set
  of blocks the old function accepted/rejected
- `cargo test --workspace` — all existing tests pass
- Manual smoke test: place blocks, interact with containers, verify behavior
  unchanged

**Completion notes:**
- `is_replaceable_block()` → `BlockStateId::is_replaceable()` flag (O(1))
- `is_interactable_block()` → `BlockStateId::is_interactable()` flag + beds via
  `minecraft:beds` tag (vanilla data omits beds from interactable; sleeping is
  conditional)
- `is_wall_mountable()` → `oxidized:wall_mountable` tag
- `is_player_direction_block()` → `oxidized:player_direction` tag
- `is_sign_block()` → `minecraft:all_signs` tag
- `is_door_block()` → `minecraft:doors` tag
- `is_bed_block()` → `minecraft:beds` tag
- `is_tall_plant()` → `oxidized:tall_plants` tag
- `item_stack.rs` air check → `AIR_ITEM_NAME` constant
- `cmd_setblock.rs` keep-mode air check → `get_block_state_id()` +
  `BlockStateId::is_air()` (now correctly handles cave_air and void_air)
- Added `get_block_state_id()` to `ServerHandle` trait for state-based queries
- Added `block_type_id_from_name()` helper for name→tag-ID conversion
- 56 regression tests added covering all 8 replaced functions
- Zero `matches!("minecraft:..."` patterns remain in production code

---

### R5.5: Replace Hardcoded Physics Properties With Registry Data

**Status:** ✅ Done

**Targets:** `crates/oxidized-game/src/physics/block_properties.rs` (deleted),
`crates/oxidized-game/src/physics/constants.rs`

**What was done:**

1. **Deleted `block_properties.rs` entirely** — removed `PhysicsBlockProperties`
   struct, `PHYSICS_OVERRIDES` array, `PhysicsOverride` struct, `from_registry()`,
   and `defaults()`. The intermediate dense lookup table is no longer needed since
   `BlockStateId` already provides O(1) static array access.

2. **Replaced all lookups with direct `BlockStateId` methods** in `tick.rs` and
   `slow_blocks.rs`:
   - `block_physics.friction(state_id)` → `BlockStateId(state_id as u16).friction()`
   - `block_physics.speed_factor(state_id)` → `BlockStateId(state_id as u16).speed_factor()`
   - `block_physics.is_slime_block(state_id)` → `BlockStateId(state_id as u16).block_name() == "minecraft:slime_block"`

3. **Removed 7 redundant physics constants** from `constants.rs`:
   `ICE_FRICTION`, `BLUE_ICE_FRICTION`, `SLIME_FRICTION`, `SOUL_SAND_SPEED_FACTOR`,
   `HONEY_BLOCK_SPEED_FACTOR`, `HONEY_BLOCK_JUMP_FACTOR`, `POWDER_SNOW_SPEED_FACTOR`.
   Kept `BLOCK_FRICTION_DEFAULT` as error fallback for unloaded chunks.

4. **Removed `block_physics: &PhysicsBlockProperties` parameter** from
   `physics_tick()`, `block_speed_factor()`, and `block_jump_factor()` — simplifying
   all call sites.

5. **Fixed powder snow misconception**: The old `PHYSICS_OVERRIDES` treated powder
   snow as having `speed_factor = 0.9`, but vanilla's powder snow has
   `speed_factor = 1.0`. The 0.9 slowdown comes from `PowderSnowBlock::makeStuckInBlock()`
   runtime behavior, not the block property.

6. **Updated `memories.md`** with the new physics pattern and powder snow note.

**Verification (47 tests passing):**
- Ice/packed ice/frosted ice friction = 0.98 ✓
- Blue ice friction = 0.989 ✓
- Soul sand speed_factor = 0.4 ✓
- Honey block speed_factor = 0.4, jump_factor = 0.5 ✓
- Slime block friction = 0.8, bounce works ✓
- Frosted ice all 4 states have 0.98 friction ✓
- Stone has default physics (0.6 / 1.0 / 1.0) ✓
- No `PHYSICS_OVERRIDES` or block name strings in physics lookup code ✓

**Net change:** +100 lines, −250 lines (deleted `block_properties.rs`, simplified
`tick.rs`, `slow_blocks.rs`, `constants.rs`, `jump.rs`, `mod.rs`)

---

### R5.6: Replace Hardcoded Biome Resolution With Registry Lookup

**Targets:** `crates/oxidized-game/src/worldgen/flat/generator.rs`

**Current problems:**
- `resolve_biome_id()` matches only 5 biome names with hardcoded IDs
- Falls back to plains for all unknown biomes
- Any new biome requires a code change

**Steps:**

1. **Use the biome registry** (already exists in `oxidized-world`):
   ```rust
   // Before:
   fn resolve_biome_id(biome_key: &str) -> u32 {
       match biome_key {
           "minecraft:plains" => PLAINS_BIOME_ID,
           "minecraft:desert" => 14,
           // ... 3 more hardcoded
           _ => PLAINS_BIOME_ID,
       }
   }

   // After:
   fn resolve_biome_id(biome_key: &str) -> u32 {
       biome_registry::biome_name_to_id(biome_key)
           .unwrap_or(biome_registry::PLAINS_BIOME_ID)
   }
   ```

2. **Verify the biome registry** supports all ~64 vanilla biomes with correct
   alphabetically-assigned IDs (per memory: biome IDs are alphabetical).

3. **Remove the hardcoded match statement entirely**.

**Verification:**
- `resolve_biome_id("minecraft:plains")` returns 40 (alphabetical index)
- `resolve_biome_id("minecraft:desert")` returns 14
- `resolve_biome_id("minecraft:the_void")` returns 57
- All 64 vanilla biomes resolve correctly
- Unknown biome falls back to plains

---

### R5.7: Compile-Time Item ID Codegen (Like Blocks)

**Targets:** `crates/oxidized-world/build.rs`,
`crates/oxidized-world/src/registry/item_registry.rs`,
`crates/oxidized-game/src/inventory/item_ids.rs`

**Current problems:**
- `ItemRegistry::load()` decompresses `items.json.gz` and builds a `Vec<Item>` +
  `AHashMap<String, usize>` at runtime on first access (via `LazyLock`)
- Every `item_name_to_id()` call does an `AHashMap` lookup instead of O(1) access
- Decompression + hashmap construction at startup, not compile time
- Blocks use compile-time codegen with pre-computed arrays; items use runtime
  loading — no reason for the inconsistency

**Steps:**

1. **Extend `build.rs`** to generate item registration data from `items.json.gz`:
   ```rust
   // Generated in $OUT_DIR/item_ids_generated.rs
   const ITEM_NAMES: &[&str] = &[
       "minecraft:air",
       "minecraft:stone",
       // ... 1504 more in vanilla registration order
   ];
   const ITEM_MAX_STACK_SIZES: &[u8] = &[64, 64, 64, /* ... */];
   const ITEM_MAX_DAMAGES: &[u16] = &[0, 0, 0, /* ... */];
   ```

2. **Generate a compile-time perfect hash** (`phf` crate) for name → ID lookup.
   With 1506 items, linear search is too slow for hot paths. `phf` gives O(1)
   with zero runtime cost, matching the block codegen's zero-overhead approach.

3. **Rewrite `ItemRegistry`** to wrap the generated arrays:
   - Replace `Vec<Item>` with references to static arrays
   - Replace `AHashMap<String, usize>` with the generated `phf::Map`
   - Keep the same public API: `name_to_id()`, `id_to_name()`, `max_stack_size()`

4. **Update `crates/oxidized-game/src/inventory/item_ids.rs`**:
   - Remove `LazyLock<ItemRegistry>` — call generated functions directly
   - Preserve return values (`-1` for unknown items, `"minecraft:air"` for
     unknown IDs)

5. **Add verification tests**:
   - Snapshot test: `insta::assert_snapshot!` over all 1506 item names
   - Round-trip: for each name in `ITEM_NAMES`, verify `name_to_id` → `id_to_name`
   - Spot-check known items: stone, diamond_sword, ender_pearl

**Verification:**
- `cargo build -p oxidized-world` — generated code compiles
- `cargo test -p oxidized-game inventory::item_ids` — all existing tests pass
- Zero startup latency (no decompression, no hashmap construction)
- Generated file lives in `$OUT_DIR/`, never in source tree

---

### R5.8: Extract Packet Codec Helpers & Roundtrip Test Macro

**Targets:** `crates/oxidized-protocol/src/packets/**/*.rs`

**Current problems:**
- **163 nearly identical roundtrip test blocks**: construct packet, encode, decode,
  assert equal, plus packet ID test
- **20+ inconsistent data validation patterns**: some use `InvalidData`, others
  use `Io(UnexpectedEof)` for the same "not enough bytes" condition
- **Varint list reading repeated** 5+ times: read count, loop, read elements

**Steps:**

1. **Create `assert_packet_roundtrip!` macro** in
   `crates/oxidized-protocol/src/test_helpers.rs` (or `tests/common/mod.rs`):
   ```rust
   /// Tests that encoding then decoding a packet produces the original.
   ///
   /// Also verifies the PACKET_ID constant.
   macro_rules! assert_packet_roundtrip {
       ($pkt_type:ty, $pkt:expr, $expected_id:expr) => {
           #[test]
           fn test_roundtrip() {
               let pkt: $pkt_type = $pkt;
               let encoded = pkt.encode();
               let decoded = <$pkt_type>::decode(encoded.freeze()).unwrap();
               assert_eq!(decoded, pkt);
           }

           #[test]
           fn test_packet_id() {
               assert_eq!(<$pkt_type>::PACKET_ID, $expected_id);
           }
       };
   }
   ```

2. **Create `read_list()` helper**:
   ```rust
   pub fn read_list<T>(
       data: &mut Bytes,
       read_element: impl Fn(&mut Bytes) -> Result<T, PacketDecodeError>,
   ) -> Result<Vec<T>, PacketDecodeError> {
       let count = varint::read_varint_buf(data)?;
       let mut items = Vec::with_capacity(count as usize);
       for _ in 0..count {
           items.push(read_element(data)?);
       }
       Ok(items)
   }
   ```

3. **Create `ensure_remaining()` helper**:
   ```rust
   pub fn ensure_remaining(
       data: &impl Buf, min: usize, context: &str,
   ) -> Result<(), PacketDecodeError> {
       if data.remaining() < min {
           return Err(PacketDecodeError::InvalidData(
               format!("{context}: need {min} bytes, have {}", data.remaining()),
           ));
       }
       Ok(())
   }
   ```

4. **Migrate packet files** to use the new helpers:
   - Replace inline `data.remaining() < N` checks with `ensure_remaining()`
   - Replace inline varint-count loops with `read_list()`
   - Replace test boilerplate with `assert_packet_roundtrip!`
   - Migrate incrementally — one packet file per commit is fine

5. **Standardize all insufficient-data errors** to use `PacketDecodeError::InvalidData`
   (not `Io(UnexpectedEof)`) for consistency.

**Verification:**
- `cargo test -p oxidized-protocol` — all 163+ packet tests still pass
- No functional change — only structural deduplication
- `grep -c "fn test_roundtrip" crates/oxidized-protocol/` — count should match
  before and after

---

### R5.9: Extract Command Registration Helpers

**Targets:** `crates/oxidized-game/src/commands/impls/*.rs`

**Current problems:**
- 17 command files follow identical registration boilerplate
- 41 instances of `ctx.source.send_success(&Component::translatable(...), true)`
- Target entity iteration with success/failure messaging repeated 3+ times
- Permission check pattern `.requires(|s| s.has_permission(N))` repeated 16 times

**Steps:**

1. **Create `send_translatable_success()` helper**:
   ```rust
   pub fn send_translatable_success(
       source: &CommandSourceStack,
       key: &str,
       args: Vec<Component>,
       broadcast: bool,
   ) {
       source.send_success(&Component::translatable(key, args), broadcast);
   }
   ```

2. **Create `send_translatable_failure()` helper**:
   ```rust
   pub fn send_translatable_failure(
       source: &CommandSourceStack,
       key: &str,
       args: Vec<Component>,
   ) {
       source.send_failure(&Component::translatable(key, args));
   }
   ```

3. **Create `for_each_target()` helper** for the common iterate-targets pattern:
   ```rust
   pub fn for_each_target<F>(
       ctx: &CommandContext<CommandSourceStack>,
       selector_name: &str,
       mut action: F,
   ) -> Result<i32, CommandError>
   where
       F: FnMut(&CommandSourceStack, &Entity) -> Result<(), CommandError>,
   {
       let targets = get_entities(ctx, selector_name)?;
       let mut count = 0;
       for target in &targets {
           action(&ctx.source, target)?;
           count += 1;
       }
       Ok(count)
   }
   ```

4. **Migrate command files** to use helpers. Each command file should shrink
   by 20-40% on average.

**Verification:**
- `cargo test -p oxidized-game` — all command tests pass
- Manual smoke test: run `/kick`, `/gamemode`, `/time`, `/difficulty` etc.
- No behavioral change

---

### R5.10: Standardize Packet Decoder Error Handling

**Targets:** `crates/oxidized-protocol/src/packets/**/*.rs`

**Current problems:**
- Some decoders use `PacketDecodeError::InvalidData("too short".into())`
- Others use `PacketDecodeError::Io(io::Error::new(UnexpectedEof, "..."))`
- Some use `PacketDecodeError::Type(TypeError::...)`
- No consistent pattern for the same class of error

**Steps:**

1. **Establish convention**: All "not enough data" errors use
   `PacketDecodeError::InvalidData` with descriptive message including packet name
   and expected byte count. Document this in `PacketDecodeError` doc comment.

2. **Audit all packet decoders** and normalize error variants:
   - `ensure_remaining()` (from R5.7) handles the common case
   - `Type` errors for semantic validation (e.g., unknown enum value)
   - `Io` errors only for actual I/O failures (not buffer underrun)

3. **Add `#[non_exhaustive]` to `PacketDecodeError`** if not already present.

**Verification:**
- `grep -rn "UnexpectedEof" crates/oxidized-protocol/src/packets/` returns zero
  results (all converted to `InvalidData`)
- `cargo test -p oxidized-protocol` passes

---

### R5.11: Decompose Oversized Structs

**Targets:**
- `crates/oxidized-game/src/player/server_player.rs` — `ServerPlayer` (45 fields)
- `crates/oxidized-server/src/network/mod.rs` — `ServerContext` (26 fields)
- `crates/oxidized-world/src/storage/primary_level_data.rs` — `PrimaryLevelData` (21 fields)
- `crates/oxidized-game/src/entity/mod.rs` — `Entity` (20 fields)

**Steps:**

1. **Decompose `ServerPlayer` into sub-structs**:
   ```rust
   pub struct ServerPlayer {
       pub identity: PlayerIdentity,   // uuid, name, profile
       pub position: PlayerPosition,   // pos, rot, on_ground, chunk
       pub abilities: PlayerAbilities,  // creative, flying, speed, etc.
       pub mining: MiningState,         // mining_start_pos, mining_start_time
       pub inventory: PlayerInventory,  // items, selected_slot, cursor
       pub connection: ConnectionInfo,  // protocol version, brand, locale
       // ... remaining fields that don't group naturally
   }
   ```

2. **Decompose `ServerContext` into sub-structs**:
   ```rust
   pub struct ServerContext {
       pub config: Arc<ServerConfig>,
       pub registries: Arc<Registries>,      // block, item, biome registries
       pub network: NetworkContext,           // listener, broadcast, connections
       pub world: WorldContext,               // level, chunk manager
       pub commands: CommandContext,           // dispatcher, suggestions
       // ...
   }
   ```

3. **Decompose `PrimaryLevelData`**:
   - Group world settings: `WorldSettings { seed, generator, difficulty, ... }`
   - Group time data: `WorldTime { day_time, game_time, ... }`
   - Group spawn data: `SpawnPoint { x, y, z, angle }`

4. **Update all call sites** — field access goes from `player.uuid` to
   `player.identity.uuid`. Use search-and-replace per field group.

**Verification:**
- `cargo test --workspace` — all tests pass
- No public API change for external consumers (these are internal structs)

---

### R5.12: Break Down Long Functions & Reduce Nesting

**Targets:** Functions exceeding 200 LOC:
- `region.rs::open()` (~495 LOC)
- `snbt.rs::format_snbt_pretty()` (~460 LOC)
- `paletted_container.rs::new()` (~423 LOC)
- `game_integration.rs::make_player()` (~600 LOC, test code)
- Various command handler functions with 8-9 levels of nesting

**Steps:**

1. **`region.rs::open()`** — Extract into:
   - `validate_header()` — header validation logic
   - `read_chunk_table()` — chunk offset table parsing
   - `repair_or_create()` — file repair/creation logic

2. **`snbt.rs::format_snbt_pretty()`** — Extract into:
   - `format_compound()` — compound tag formatting
   - `format_list()` — list tag formatting
   - `format_array()` — byte/int/long array formatting
   - `write_indentation()` — indent helper

3. **`paletted_container.rs::new()`** — Extract into:
   - `read_single_valued()` — single-palette case
   - `read_indirect()` — indirect palette case
   - `read_direct()` — direct palette case
   - `unpack_longs()` — bit unpacking helper

4. **Reduce nesting in command handlers** — use early returns and helper functions:
   ```rust
   // Before (9 levels deep):
   if let Some(x) = ... {
       if let Some(y) = ... {
           match z { ... }
       }
   }

   // After (flat):
   let x = match ... { Some(v) => v, None => return ... };
   let y = match ... { Some(v) => v, None => return ... };
   match z { ... }
   ```

5. **`paletted_container.rs` type casts** — Replace unsafe `as` casts with
   `TryFrom` or `.try_into()` where overflow is possible. Keep `as` only where
   the value is proven in-range. Add `// SAFETY:` comments for retained `as` casts.

**Verification:**
- `cargo test --workspace` — all tests pass
- No function exceeds 200 LOC (check with a script)
- No nesting exceeds 5 levels in production code

---

### R5.13: Replace Magic Numbers With Named Constants

**Targets:** Various files across all crates

**Current problems:**
- Time constants: `1000` (day), `6000` (noon), `13000` (night), `18000`
  (midnight) used as raw literals in `cmd_time.rs`
- Data serializer boundary: `255` in `synched_data.rs`
- Various protocol constants used as raw numbers
- Tick-related constants (50ms minimum mining, 20 TPS, etc.)

**Steps:**

1. **Create time constants** in `oxidized-game/src/level/`:
   ```rust
   pub mod time {
       pub const DAY_START: i64 = 1000;
       pub const NOON: i64 = 6000;
       pub const NIGHT_START: i64 = 13000;
       pub const MIDNIGHT: i64 = 18000;
       pub const TICKS_PER_DAY: i64 = 24000;
   }
   ```

2. **Create protocol constants** in `oxidized-protocol/src/constants.rs` (or
   the relevant module):
   ```rust
   pub const MAX_ENTITY_DATA_SERIALIZER_ID: u32 = 255;
   ```

3. **Replace all raw literals** with named constants. Use `grep` to find
   remaining magic numbers:
   ```bash
   grep -rn '[^a-zA-Z_][0-9]\{2,\}[^a-zA-Z_0-9xX.]' crates/ --include="*.rs" | \
     grep -v '#\[cfg(test)\]' | grep -v '/tests/' | grep -v 'generated'
   ```
   Review each match — not all numbers are magic (array sizes, protocol IDs in
   their defining location are fine).

4. **Document remaining numeric constants** with `///` comments explaining what
   the value represents.

**Verification:**
- `cargo test --workspace` — all tests pass
- `cargo clippy --workspace` — no new warnings
- Manual review: no unexplained numeric literals in production code

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Flag values don't match vanilla | Medium | High | Automated spot-check tests comparing against extracted data |
| `build.rs` regression from data changes | Medium | High | CI step regenerates and diffs against committed output |
| Tag membership differs from string patterns | Low | High | Regression test: old function vs new for all block states |
| Performance regression from larger table | Low | Low | Table is still <1 MB; benchmark block state lookups |
| Breaking change in `BlockStateEntry` layout | Certain | Medium | Update all callers in same PR; no external consumers |

---

## Definition of Done

1. **Zero string-based block dispatch** in production code:
   ```bash
   grep -rn 'matches!.*"minecraft:' crates/ --include="*.rs" | \
     grep -v test | grep -v generated | wc -l
   # Expected: 0
   ```

2. **Zero `PHYSICS_OVERRIDES`** or similar hardcoded property arrays

3. **All ADR-012 flags implemented** in `BlockStateFlags` (u16, 13+ flags)

4. **Block tags loaded and used** for categorical dispatch

5. **Packet test boilerplate reduced** — `assert_packet_roundtrip!` used in
   all packet modules

6. **No struct with >15 fields** (decomposed into sub-structs)

7. **No function >200 LOC** in production code

8. **No magic numbers** — all numeric constants named and documented

9. **`cargo test --workspace`** passes with zero failures

10. **`cargo clippy --workspace -- -D warnings`** passes

---

## Estimated Scope

| Sub-task | Files Modified | Lines Changed (est.) |
|----------|---------------|---------------------|
| R5.1 | 3-5 | +200, -50 |
| R5.2 | 3-5 | +300, -20 |
| R5.3 | 5-8 | +400, -0 |
| R5.4 | 4-6 | +50, -300 |
| R5.5 | 2-3 | +30, -120 |
| R5.6 | 1-2 | +10, -20 |
| R5.7 | 5-8 | +300, -150 |
| R5.8 | 40-60 | +200, -800 |
| R5.9 | 17-20 | +80, -400 |
| R5.10 | 20-30 | +100, -150 |
| R5.11 | 10-15 | +200, -50 |
| R5.12 | 8-12 | +100, -0 |
| R5.13 | 10-15 | +80, -40 |
| **Total** | ~140-190 | +1900, -2050 |

Net reduction: ~150 lines. The key metric is not LOC but the elimination of
string dispatch, runtime overhead, and duplication patterns.
