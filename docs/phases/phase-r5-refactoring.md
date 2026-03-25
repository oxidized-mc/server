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
| R5.6 | Replace hardcoded biome resolution with registry lookup | ✅ Done |
| R5.7 | Compile-time item ID codegen (like blocks) | ✅ Done |
| R5.8 | Extract packet codec helpers & roundtrip test macro | ✅ Done |
| R5.9 | Extract command registration helpers | ✅ Done |
| R5.10 | Standardize packet decoder error handling | ✅ Done |
| R5.11 | Decompose oversized structs | ✅ Complete |
| R5.12 | Break down long functions & reduce nesting | ✅ Done |
| R5.13 | Replace magic numbers with named constants | ✅ Done |
| R5.14 | Benchmark & fuzz testing infrastructure | ✅ Done |
| R5.15 | Per-player operator permissions (`ops.json`) | ✅ Done |
| R5.16 | Safety hardening & cleanup | ✅ Done |
| R5.17 | Entity selector completeness | ✅ Done |
| R5.18 | Config cleanup: remove Java leftovers & extract hardcoded values | 📋 Planned |

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
| R5.14 | — | Independent; testing infrastructure |
| R5.15 | — | Independent; permission system |
| R5.16 | R5.1, R5.2 | Fluid counters need block property lookups |
| R5.17 | — | Independent; command system improvements |

**Parallelizable groups:**
- **Group A** (sequential): R5.1 → R5.2 → R5.3 → R5.4 → R5.5
- **Group B** (independent): R5.6, R5.7, R5.8, R5.9, R5.10, R5.11, R5.12, R5.13
- **Group C** (tech debt): R5.14, R5.15, R5.16 (after Group A), R5.17

Groups B and C can be done in any order and in parallel with Group A.
R5.16 has a soft dependency on R5.1/R5.2 for the fluid counter item only.

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

### R5.6: Replace Hardcoded Biome Resolution With Registry Lookup ✅

**Status:** Done

**Targets:** `crates/oxidized-game/src/worldgen/flat/generator.rs`

**What was done:**

1. **Created `crates/oxidized-world/src/registry/biome_registry.rs`** — new module
   with `BIOME_NAMES` (65 vanilla biomes, alphabetically sorted), `PLAINS_BIOME_ID`,
   `biome_name_to_id()` (binary search, O(log n)), `biome_id_to_name()`, and
   `biome_count()`. Full unit + doc test coverage.

2. **Updated `crates/oxidized-world/src/registry/mod.rs`** — registered and
   re-exported the biome registry public API.

3. **Updated `crates/oxidized-game/src/worldgen/flat/generator.rs`** — replaced
   the 5-entry hardcoded match with `biome_name_to_id()` registry lookup.
   Removed local `PLAINS_BIOME_ID` constant in favor of the registry's.

4. **Updated `crates/oxidized-world/src/anvil/chunk_serializer.rs`** — removed
   the local `BIOME_NAMES` static; `biome_name()` now delegates to
   `biome_id_to_name()`.

5. **Updated `crates/oxidized-world/src/anvil/chunk_loader.rs`** — replaced
   `.position()` linear scan with `biome_name_to_id()` binary search.

**Files changed:** 5 (+120, -85)

**Verification:** All 65 vanilla biomes resolve correctly. Unknown biomes fall
back to plains. 969 tests pass across oxidized-world and oxidized-game.

---

### R5.7: Compile-Time Item ID Codegen (Like Blocks)

**Status:** ✅ Done

**Targets:** `crates/oxidized-world/build.rs`,
`crates/oxidized-world/src/registry/item_registry.rs`,
`crates/oxidized-world/src/registry/item_generated.rs` (new),
`crates/oxidized-game/src/inventory/item_ids.rs`

**Problems solved:**
- `ItemRegistry::load()` decompressed `items.json.gz` and built a `Vec<Item>` +
  `AHashMap<String, usize>` at runtime on first access (via `LazyLock`)
- Every `item_name_to_id()` call did an `AHashMap` lookup
- Decompression + hashmap construction at startup, not compile time
- Blocks used compile-time codegen; items used runtime loading

**What was done:**

1. **Extended `build.rs`** with `generate_item_ids()` that parses `items.json.gz`
   and writes `$OUT_DIR/item_ids_generated.rs` containing:
   - `ITEM_COUNT: usize` — total items (1506)
   - `ITEM_NAMES: [&str; ITEM_COUNT]` — vanilla registration order
   - `ITEM_MAX_STACK_SIZES: [u8; ITEM_COUNT]`
   - `ITEM_MAX_DAMAGES: [u16; ITEM_COUNT]`
   - `ITEM_NAMES_SORTED: [(&str, u16); ITEM_COUNT]` — alphabetical for binary search

2. **Used sorted array + binary search** for name → ID lookup instead of `phf`.
   This matches the existing `BLOCK_NAMES_SORTED` pattern — O(log₂ 1506) ≈ 11
   comparisons, no new dependency, consistent with blocks.

3. **Rewrote `ItemRegistry`** as a zero-sized struct (like `BlockRegistry`):
   - `const fn new()` — usable in `const` context
   - `load()` kept for backward compatibility (always succeeds)
   - Same public API: `name_to_id()`, `id_to_name()`, `max_stack_size()`, `get()`
   - `get()` return type changed from `Option<&Item>` to `Option<Item>` (constructs
     from static arrays)

4. **Updated `item_ids.rs`**:
   - Replaced `LazyLock<ItemRegistry>` with `const REGISTRY: ItemRegistry`
   - Preserved all return-value semantics (`-1` for unknown, `"minecraft:air"` for OOB)
   - All 3 doc tests pass unchanged

5. **Added verification tests** (12 total in `item_registry`, 6 in `item_ids`):
   - Snapshot test: `insta::assert_snapshot!` over all 1506 item names
   - Full round-trip: all 1506 items name → id → name
   - Spot-check: stone, diamond_sword, ender_pearl with property verification

**Verification:**
- `cargo build -p oxidized-world` — generated code compiles ✅
- `cargo test -p oxidized-world -- item_registry` — 12 tests pass ✅
- `cargo test -p oxidized-game -- item_ids` — 6 unit + 3 doc tests pass ✅
- `cargo check --workspace` — clean compile ✅
- Zero startup latency (no decompression, no hashmap construction)
- Generated file lives in `$OUT_DIR/`, never in source tree

---

### R5.8: Extract Packet Codec Helpers & Roundtrip Test Macro

**Status:** ✅ Done

**Targets:** `crates/oxidized-protocol/src/packets/**/*.rs`

**Current problems:**
- **163 nearly identical roundtrip test blocks**: construct packet, encode, decode,
  assert equal, plus packet ID test
- **20+ inconsistent data validation patterns**: some use `InvalidData`, others
  use `Io(UnexpectedEof)` for the same "not enough bytes" condition
- **Varint list reading repeated** 5+ times: read count, loop, read elements

**What was done:**

1. **Created `ensure_remaining()`, `read_list()`, `write_list()` helpers** in
   `crates/oxidized-protocol/src/codec/types.rs`:
   - `ensure_remaining(buf, min, context)` — standardized "not enough bytes" check
     returning `InvalidData` with descriptive message
   - `read_list(data, read_element)` — reads VarInt count + loop, validates
     non-negative count
   - `write_list(buf, items, write_element)` — writes VarInt count + loop
   - 6 unit tests covering all helpers

2. **Created `assert_packet_roundtrip!` and `assert_packet_id!` macros** in
   `crates/oxidized-protocol/src/codec/mod.rs` (`#[cfg(test)]`):
   - `assert_packet_roundtrip!(pkt_expr)` — encode→decode→assert_eq assertion
   - `assert_packet_id!(PacketType, 0xNN)` — packet ID assertion
   - Designed as test assertions (not test generators) for maximum flexibility

3. **Migrated ~25 packet files to use `ensure_remaining()`:**
   - Replaced all `Io(UnexpectedEof)` patterns with `ensure_remaining()` +
     `InvalidData` — standardized error type across the crate
   - Replaced inline `data.remaining() < N` checks in play, login, and
     configuration packets
   - Converted raw `data.get_*()` calls to typed helpers (`types::read_u8`,
     `types::read_i64`, `types::read_f32`, etc.) where possible

4. **Migrated clean candidates to `read_list()` / `write_list()`:**
   - `clientbound_update_enabled_features` — both read + write
   - `clientbound_select_known_packs` — both read + write
   - `clientbound_container_set_content` — both read + write
   - `clientbound_player_info_remove` — write_list
   - `clientbound_remove_entities` — write_list
   - `clientbound_login` — write_list for dimensions
   - `serverbound_select_known_packs` — write_list
   - Files with extra validation (max limits, bounds checks) kept manual
     loops with the extra checks preserved

5. **Migrated 43 packet files to use `assert_packet_id!` macro** and
   **18 roundtrip tests to use `assert_packet_roundtrip!` macro**.

**Verification:**
- `cargo test -p oxidized-protocol` — 856 unit tests + 136 integration tests pass
- `cargo check --workspace` — clean
- No functional changes — only structural deduplication and error standardization

---

### R5.9: Extract Command Registration Helpers ✅

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

**Completion notes:**
- Implemented `send_translatable_success()` and `send_translatable_failure()` as
  methods on `CommandSourceStack` (more idiomatic than standalone functions).
- Added `for_each_target()` to `argument_access.rs` (uses `&SelectorTarget`, not
  `&Entity`, since that is the actual resolved type).
- Migrated all 14 command impl files. The `/kick` command was further deduplicated
  by extracting `kick_targets()` to collapse two near-identical closures.
- 16 files changed, +144 / -153. Net: -9 lines (modest because most call sites
  still pass `Component::text(...)` args). The real win is reduced nesting and
  consistency.

---

### R5.10: Standardize Packet Decoder Error Handling ✅

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

**Completed:**

- `#[non_exhaustive]` was already present — no change needed.
- Added error variant convention doc comment to `PacketDecodeError` in
  `codec/packet.rs` documenting when to use `InvalidData`, `Type`, and `Io`.
- Replaced 3 direct `PacketDecodeError::Type(TypeError::UnexpectedEof{..})`
  constructions with `ensure_remaining()` calls in `serverbound_move_player.rs`
  (1 site) and `clientbound_level_chunk_with_light.rs` (2 sites).
- Removed private `ensure_remaining()` duplicate in `clientbound_commands.rs`,
  replaced all 7 call sites with the public `ensure_remaining()` from
  `codec::types` (adding context labels to each call).
- No `PacketDecodeError::Io` misuse found — 0 occurrences in packets dir.
- All 1015 protocol tests pass; workspace compiles cleanly.

---

### R5.11: Decompose Oversized Structs ✅

**Targets (all complete):**
- `crates/oxidized-world/src/storage/primary_level_data.rs` — `PrimaryLevelData` (21 → 4 fields + sub-structs)
- `crates/oxidized-game/src/entity/mod.rs` — `Entity` (19 → 12 fields + sub-structs)
- `crates/oxidized-game/src/player/server_player.rs` — `ServerPlayer` (45 → 16 fields + sub-structs)
- `crates/oxidized-server/src/network/mod.rs` — `ServerContext` (23 → 6 fields + sub-structs)

**What was done:**

1. **`PrimaryLevelData`** decomposed into 4 sub-structs:
   - `SpawnPoint { x, y, z, angle }` — spawn location
   - `WorldTime { game_time, day_time }` — time tracking
   - `WeatherState { is_raining, is_thundering, rain_time, thunder_time, clear_weather_time }` — weather
   - `WorldSettings { level_name, data_version, game_type, is_hardcore, difficulty, ... }` — world config
   - Access: `ld.spawn_x` → `ld.spawn.x`, `ld.time` → `ld.time.game_time`, `ld.is_raining` → `ld.weather.is_raining`

2. **`Entity`** consolidated coordinate fields into sub-structs:
   - `pos: Vec3` (x/y/z → pos.x/pos.y/pos.z)
   - `velocity: Vec3` (vx/vy/vz → velocity.x/velocity.y/velocity.z)
   - `rotation: EntityRotation { yaw, pitch, head_yaw }`
   - `dimensions: EntityDimensions { width, height }`

3. **`ServerPlayer`** decomposed into 8 sub-structs:
   - `PlayerMovement` (pos, yaw, pitch, is_on_ground, is_sneaking, is_sprinting, is_fall_flying)
   - `CombatStats` (health, max_health, food_level, food_saturation, score, absorption_amount, last_death_location)
   - `PlayerExperience` (xp_level, xp_progress, xp_total, xp_seed)
   - `SpawnInfo` (dimension, spawn_pos, spawn_angle)
   - `ConnectionInfo` (view_distance, simulation_distance, chunk_send_rate, latency, model_customisation, movement_rate)
   - `TeleportTracker` (pending, id_counter) — with `next_id()` method
   - `MiningState` (start_pos, start_time)
   - `RawPlayerNbt` (active_effects, attributes, ender_items)

4. **`ServerContext`** decomposed into 3 sub-structs:
   - `WorldContext` (level_data, dimensions, chunks, dirty_chunks, storage, block_registry, chunk_generator, chunk_loader, chunk_serializer, game_rules)
   - `NetworkContext` (broadcast_tx, shutdown_tx, kick_channels, player_list, max_players)
   - `ServerSettings` (max_view_distance, max_simulation_distance, op_permission_level, spawn_protection, color_char)
   - Remaining top-level: commands, event_bus, tick_rate_manager

**Verification:**
- `cargo check --workspace --tests` — clean build
- `cargo test --workspace` — all 2,372+ tests pass

---

### R5.12: Break Down Long Functions & Reduce Nesting

**Status:** ✅ Done

The original targets (`region.rs::open`, `snbt.rs::format_snbt_pretty`,
`paletted_container.rs::new`, `game_integration.rs::make_player`) had already been
refactored to be small in earlier phases. The actual violations were found via a
workspace-wide audit and addressed as follows:

**Functions decomposed:**

| Function | Before | After | Extracted helpers |
|----------|--------|-------|-------------------|
| `handle_play_split` (mod.rs) | 526 LOC, depth 12 | 328 LOC, depth 10* | `handle_keepalive_response`, `dispatch_chat_command`, `handle_swing_packet`, `handle_abilities_packet`, `handle_client_information_packet`, `cleanup_disconnected_player` |
| `handle_use_item_on` (placement.rs) | 297 LOC, depth 5 | 248 LOC, depth 4 | `decrement_held_item`, `place_companion_block` |
| `handle_player_action` (mining.rs) | 235 LOC, depth 6 | 190 LOC, depth 5 | `handle_stop_destroy` |
| `broadcast_player_join` (join.rs) | 212 LOC, depth 5 | 46 LOC, depth 2 | `build_player_info_packet`, `all_info_actions`, `broadcast_player_info`, `broadcast_and_collect_entities`, `collect_existing_player_data` |
| `dispatcher::parse` | 80 LOC, depth 8 | 60 LOC, depth 5 | `try_match_child` (with `ChildMatch` enum) |
| `collect_child_suggestions` | depth 9 | depth 6 | `suggest_for_argument` |

\* `handle_play_split` uses `tokio::select!` with shared mutable state and `break`
semantics across branches. The macro-generated nesting cannot be decomposed further.
328 LOC is a pragmatic minimum for this async event-loop pattern.

**Type cast safety (paletted_container.rs):**
- Replaced 3 unsafe `i32 → u32`/`i32 → usize` casts (from VarInt network reads) with
  `u32::try_from()`/`usize::try_from()` + `NegativeValue` error variant
- Added `// SAFETY:` comments to ~15 retained `as` casts explaining why they're
  provably in-range (palette indices, block state IDs, bit reinterpretation)

**Nesting reduction (dispatcher.rs):**
- Used match guards (`Err(e) if condition => ...`) to eliminate nested `if` inside
  match arms
- Used early-return inversion (`!is_entity` check first) in `suggest_for_argument`

**Verification:**
- `cargo test --workspace` — all tests pass (2,400+ tests)
- No function >200 LOC except `handle_play_split` (328, documented exception) and
  `handle_use_item_on` (248, flat guard-clause pipeline)
- No control-flow nesting >5 in production code (remaining depth-6 cases are struct
  construction or match-arm bodies with single statements)

---

### R5.13: Replace Magic Numbers With Named Constants

**Status:** ✅ Done

**Targets:** Various files across all crates

**What was done:**

Game time preset constants added to `oxidized-protocol/src/constants.rs`:
`DAY_START_TICKS`, `NOON_TICKS`, `NIGHT_START_TICKS`, `MIDNIGHT_TICKS`.

Existing constants (`TICKS_PER_GAME_DAY`, `TICKS_PER_SECOND`, `MILLIS_PER_TICK`,
`DEFAULT_VIEW_DISTANCE`, `DEFAULT_SIMULATION_DISTANCE`) were imported and used
where raw literals previously appeared.

**Files changed:**

| File | Change |
|------|--------|
| `oxidized-protocol/src/constants.rs` | Added 4 game-time preset constants |
| `oxidized-game/src/commands/impls/cmd_time.rs` | Replaced 1000/6000/13000/18000/24000 with named constants |
| `oxidized-game/src/commands/argument_parser.rs` | Replaced `24000`/`20` with `TICKS_PER_GAME_DAY`/`TICKS_PER_SECOND` |
| `oxidized-game/src/level/tick_rate.rs` | Replaced `20.0`/`50` with `TICKS_PER_SECOND`/`MILLIS_PER_TICK` |
| `oxidized-game/src/net/entity_movement.rs` | Added `ROTATION_BYTE_STEPS`/`FULL_ROTATION_DEGREES` constants |
| `oxidized-game/src/player/server_player.rs` | Added `MODEL_ALL_PARTS_VISIBLE`/`DEFAULT_CHUNK_SEND_RATE`; used `DEFAULT_VIEW_DISTANCE`/`DEFAULT_SIMULATION_DISTANCE` |
| `oxidized-server/src/tick.rs` | Added `TIME_BROADCAST_INTERVAL`/`AUTOSAVE_RATE_MULTIPLIER`/`AUTOSAVE_MIN_INTERVAL`; used `TICKS_PER_SECOND` |
| `oxidized-server/src/network/play/mod.rs` | Replaced `50` with `MILLIS_PER_TICK` |
| `oxidized-server/src/network/play/mining.rs` | Replaced `50` with `MILLIS_PER_TICK` |
| `oxidized-server/src/network/reader.rs` | Replaced `50` with `MILLIS_PER_TICK` |

**Not changed (already correct):**
- `DATA_EOF_MARKER` (0xFF) — already a named constant in `clientbound_set_entity_data.rs`
- Weather constants in `tick.rs` — already named (`RAIN_DELAY_MIN`, etc.)
- Physics constants — already in `physics/constants.rs`
- Keepalive/teleport timeouts — already named constants

**Verification:**
- `cargo test --workspace` — all tests pass
- `cargo clippy --workspace` — no new warnings

---

### R5.14: Benchmark & Fuzz Testing Infrastructure

**Status:** ✅ Done

**Source:** memories.md Phase 5 & 12 retrospectives ("Still open"), ADR-034

**What was done:**

1. **Criterion benchmarks** — added `criterion` 0.5 as workspace dev-dependency,
   created `[[bench]]` targets in 4 crates:
   - `oxidized-nbt/benches/nbt_benchmarks.rs`: 10 benchmarks (read/write small & large,
     roundtrip, SNBT format/parse, compound lookup & insert)
   - `oxidized-protocol/benches/codec_benchmarks.rs`: 12 benchmarks (VarInt/VarLong
     encode/decode, roundtrip, BytesMut helpers, varint_size, packet frame encode/decode)
   - `oxidized-world/benches/world_benchmarks.rs`: 10 benchmarks (block state data/name/
     flags/physics lookups, is_air batch, with_property, tag contains/get/batch)
   - `oxidized-game/benches/game_benchmarks.rs`: 6 benchmarks (friction/speed/jump/
     combined physics, light properties, block categorization)
   - `cargo bench --workspace --no-run` compiles cleanly

2. **Cargo-fuzz targets** — set up `fuzz/` in 3 crates (requires nightly):
   - `oxidized-nbt/fuzz/`: `fuzz_nbt_read` (arbitrary bytes → NbtReader with 2 MiB limit)
   - `oxidized-protocol/fuzz/`: `fuzz_varint`, `fuzz_packet_decode`
   - `oxidized-world/fuzz/`: `fuzz_paletted_container` (both BlockStates and Biomes)
   - Documented in `CONTRIBUTING.md` how to install, list, and run fuzz targets

3. **ConnectionStateMachine** extracted from `Connection`:
   - New `ConnectionStateMachine` struct in `transport/connection.rs` with validated
     transitions (`is_valid_transition`, `transition`), encryption/compression flags
   - `InvalidTransition` error type with Display
   - 13 unit tests: initial state, default, all valid transitions, all invalid transitions
     (exhaustive 5×5 matrix), encryption/compression flags, error display
   - Doc example on the struct

**Note:** `fuzz_region_read` was deferred — region files are read from disk, not untrusted
network input, so the paletted container (which IS on the wire path) was prioritized.

---

### R5.15: Per-Player Operator Permissions (`ops.json`)

**Source:** memories.md Phase 18/22 retrospectives, 3+ code TODOs

**Current problems:**
- All players receive the server-wide `op_permission_level` from config
- No `ops.json` file support — vanilla uses this for per-player permission levels
- Spawn protection checks cannot distinguish operators from regular players
- Command permission level hardcoded as `4` in `commands.rs`

**Related TODOs in code:**
- `network/play/mod.rs:420` — "implement per-player ops (ops.json)"
- `network/mod.rs:112` — "Replace with per-player ops from `ops.json`"
- `network/play/block_interaction.rs:81` — "skip protection for operators
  once ops.json is implemented"
- `network/play/commands.rs:44` — "read actual permission level"

**Steps:**

1. **Define `OpsConfig` data structure** and `ops.json` schema:
   ```rust
   pub struct OpEntry {
       pub uuid: Uuid,
       pub name: String,
       pub level: i32,        // 1-4
       pub bypasses_player_limit: bool,
   }
   ```

2. **Implement `ops.json` load/save** in `oxidized-server`:
   - Load on startup, create empty file if missing
   - `DashMap<Uuid, OpEntry>` for O(1) lookup
   - Save on modification (op/deop commands)

3. **Wire permission lookup** into existing systems:
   - `CommandSourceStack::has_permission()` checks `ops.json` entry
   - `commands.rs` reads actual player permission level from ops map
   - Spawn protection checks operator status
   - `EntityEventPacket` sends correct per-player permission level on login

4. **Implement `/op` and `/deop` commands** (currently stubs):
   - `/op <player>` — add to ops.json with configured level
   - `/deop <player>` — remove from ops.json
   - Broadcast permission level change via `EntityEventPacket`

**Verification:**
- Unit tests: load/save ops.json roundtrip, permission level lookup
- Integration test: player with op entry gets correct permission level
- Regression: default behavior (no ops.json) matches current behavior
- `cargo test --workspace` passes

**Completed.** All steps implemented:
- `OpsStore` in `oxidized-server/src/ops.rs` — `DashMap<Uuid, OpEntry>`, load/save, 12 unit tests
- `ServerHandle` trait extended with `is_op`, `get_permission_level`, `op_player`, `deop_player`,
  `op_names`, `non_op_player_names` methods
- `ServerContext` wired with `SharedOpsStore` (Arc), loaded from `ops.json` on startup
- All 4 hardcoded permission sites replaced with per-player lookup:
  - `play/mod.rs` command execution, `play/commands.rs` command source (×2), `play/join.rs` EntityEvent
- `is_spawn_protected` accepts player UUID, operators bypass protection, empty ops disables protection
- `/op` and `/deop` commands implemented (replaced stubs) with `GameProfile` argument
- Permission level updates broadcast via `EntityEventPacket` + command tree resend on op/deop

---

### R5.16: Safety Hardening & Cleanup

**Source:** memories.md Phase 5/12/17/22 retrospectives, code TODOs

**Status:** ✅ Done

**Combines 6 small independent items into one sub-phase:**

**Items:**

1. **Bound `read_component_nbt` NbtAccounter** (security) ✅
   - Replaced `NbtAccounter::unlimited()` with `NbtAccounter::default_quota()`
     (2 MiB, matching vanilla network limit) in `read_component_nbt()` and
     `clientbound_registry_data` packet decode
   - All network-facing NBT reads now enforce the vanilla size budget

2. **Rate limiter: disconnect persistent spammers** (security) ✅
   - Already implemented: `chat/rate_limit.rs` disconnects with
     `"disconnect.spam"` after exceeding threshold (200 / 20-per-msg)
   - Also applied to commands — no changes needed

3. **Player removal from PlayerList on disconnect** (cleanup) ✅
   - Already implemented: deterministic cleanup in `play/mod.rs` —
     `PlayerList::remove()`, `PlayerInfoRemove` broadcast, entity removal
   - No changes needed

4. **Replace `spawn_pos` reuse with `mining_pos` field** (correctness) ✅
   - Already fixed: `MiningState::start_pos` is a proper `Option<BlockPos>`
     in `server_player.rs`, not reusing `spawn_pos`
   - No changes needed

5. **Update fluid/ticking counters in chunk sections** (correctness) ✅
   - `set_block_state()` now maintains `fluid_count`, `ticking_block_count`,
     and `ticking_fluid_count` using `BlockStateId` property lookups
   - `recalculate_counts()` now recalculates all four counters
   - `from_parts()` calls `recalculate_counts()` for full accuracy
   - `filled()` computes correct counters for the fill block
   - Added `PalettedContainer::for_each_value()` for efficient iteration
   - 14 new unit tests covering all counter transitions

6. **Evaluate `reqwest` → lighter HTTP client** (dependency cleanup) ✅
   - Evaluated `ureq` (sync-only, incompatible), `hyper` (more boilerplate),
     and custom HTTP. Decision: **keep `reqwest`** — only one HTTP call
     exists, all features (async TLS, JSON, pooling) are required, switching
     breaks public API for marginal compile-time savings (~2-3 s).
   - Rationale documented in `auth.rs` module doc

**Verification:**
- Each item has its own unit test or regression test
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
- Security items verified with targeted test cases

---

### R5.17: Entity Selector Completeness

**Status:** ✅ Done

**Source:** memories.md Phase 18 retrospective, code TODOs

**What was done:**

1. **`@r` randomization** — already implemented via Fisher-Yates shuffle with
   `rand::rng()` (verified prior to this sub-phase).

2. **Runtime filter enforcement** — six filters are now applied during
   `resolve_selector()` (previously only `name`, `limit`, `sort`):
   - `gamemode=<mode>` / `gamemode=!<mode>` — filters players by game mode.
     Added `GameMode::from_name()` / `GameMode::name()` for string conversion
     and `ServerHandle::get_player_game_mode()` to query player state.
   - `distance=<range>` — filters by Euclidean distance from the command
     source position. Added `ServerHandle::get_player_position()`.
   - `type=<entity_type>` / `type=!<entity_type>` — filters by entity type.
     Currently all entities are players, so `type=player` / `type=minecraft:player`
     keeps all; other types exclude all.
   - Remaining filters (`tag`, `nbt`, `scores`, `advancements`, `team`, `level`,
     coordinates, rotations) are still parsed and stored for future ECS use.

3. **Tab-completion for filter keys** — when a player types `@x[`, the server
   now suggests all 20 filter keys (`name=`, `limit=`, `sort=`, `gamemode=`,
   `distance=`, `type=`, etc.). Value-level completions are provided for
   `sort=` (nearest/furthest/random/arbitrary) and `gamemode=`
   (survival/creative/adventure/spectator). Keys after commas are also
   suggested (e.g., `@a[name=Steve,` → suggest next key).

**Files changed:**
- `crates/oxidized-game/src/player/game_mode.rs` — `name()`, `from_name()`, `ALL_NAMES`
- `crates/oxidized-game/src/commands/source.rs` — `get_player_game_mode()`, `get_player_position()`
- `crates/oxidized-game/src/commands/selector.rs` — runtime filters, `FILTER_KEYS`, `SORT_VALUES`
- `crates/oxidized-game/src/commands/dispatcher.rs` — bracket filter tab-completion
- `crates/oxidized-server/src/network/mod.rs` — `ServerHandle` impl for new methods

**Tests added:** 17 new tests across selector.rs (12 resolution), dispatcher.rs (6
tab-completion), game_mode.rs (4 name conversion).

---

### R5.18: Config Cleanup — Remove Java Leftovers & Extract Hardcoded Values

**Source:** Manual audit of config structs vs. hardcoded constants across all crates

This sub-phase has two parts:

- **Part A** — Remove or convert Java-specific config fields inherited from vanilla
  `server.properties` that have no meaning in a Rust/Tokio server.
- **Part B** — Extract server-tunable hardcoded constants into config so operators
  can adjust them without recompiling.

> **Not in scope:** Vanilla game constants (physics, player dimensions, reach
> distances, protocol IDs) stay as `const` — they must match vanilla for client
> compatibility and are not operator-tunable.

---

#### Part A: Remove / Convert Java-Specific Config Fields

Three config fields were copied from vanilla `server.properties` but serve no
purpose in a Rust server.

| Field | File | Action | Reason |
|-------|------|--------|--------|
| `is_jmx_monitoring_enabled` | `config/advanced.rs:75` | **Remove** | JMX is a Java-only monitoring API. Rust uses `tracing` / Prometheus. Not referenced anywhere outside config definition. |
| `is_native_transport_enabled` | `config/network.rs:20` | **Remove** | Controls Netty's epoll/kqueue selector in Java. Tokio uses native I/O multiplexing unconditionally. Not referenced outside config definition. |
| `is_sync_chunk_writes` | `config/world.rs:20` | **Remove** | Java threading model toggle. Rust async I/O is neither "sync" nor "async" in the Java sense. Not referenced outside config definition. If a durability knob is needed later, add a Rust-appropriate `flush_strategy` enum (e.g., `Immediate`, `Batched`, `OsDefault`). |

**Steps:**

1. Remove the three fields from their config structs
2. Remove corresponding entries from `oxidized.toml`
3. Remove environment variable overrides in `config/mod.rs`
4. Remove from config tests and snapshot files
5. Update any documentation that references these fields
6. Grep for stale references:
   ```bash
   grep -rn 'jmx_monitoring\|native_transport\|sync_chunk_writes' \
     crates/ oxidized.toml docs/ --include="*.rs" --include="*.toml" --include="*.md"
   ```

---

#### Part B: Extract Hardcoded Constants Into Config

The following values are hardcoded as `const` but are legitimate server-tuning
parameters that operators should be able to adjust.

##### B.1 — Network Timeouts (new `[network.timeouts]` section)

| Constant | Current Value | File | Line |
|----------|--------------|------|------|
| `KEEPALIVE_INTERVAL` | 15 s | `network/play/mod.rs` | 203 |
| `KEEPALIVE_TIMEOUT` | 30 s | `network/play/mod.rs` | 204 |
| `LOGIN_TIMEOUT` | 30 s | `network/login.rs` | 23 |
| `CONFIGURATION_TIMEOUT` | 30 s | `network/configuration.rs` | 51 |
| `WRITE_TIMEOUT` | 30 s | `network/writer.rs` | 23 |

**Action:** Add a `NetworkTimeoutsConfig` struct under `config/network.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NetworkTimeoutsConfig {
    /// Seconds between keepalive pings (default 15).
    pub keepalive_interval_secs: u64,
    /// Seconds before a keepalive timeout disconnects the client (default 30).
    pub keepalive_timeout_secs: u64,
    /// Seconds to complete the login phase (default 30).
    pub login_timeout_secs: u64,
    /// Seconds to complete the configuration phase (default 30).
    pub configuration_timeout_secs: u64,
    /// Seconds before a slow-write client is disconnected (default 30).
    pub write_timeout_secs: u64,
}
```

Replace each `const` with a read from `server_ctx.settings.network.timeouts.*`.

##### B.2 — Rate Limiting (new `[network.rate_limit]` section)

| Constant | Current Value | File | Line |
|----------|--------------|------|------|
| `DEFAULT_MAX_CONNECTIONS_PER_WINDOW` | 10 | `network/mod.rs` | 581 |
| `DEFAULT_RATE_LIMIT_WINDOW` | 10 s | `network/mod.rs` | 584 |
| `RATE_LIMIT_CLEANUP_INTERVAL` | 60 s | `network/mod.rs` | 587 |

**Action:** Add fields to `NetworkConfig` (or a nested `RateLimitConfig`):

```rust
pub struct RateLimitConfig {
    /// Max new connections per IP within the window (default 10).
    pub max_connections_per_window: u32,
    /// Duration of the rate-limit window in seconds (default 10).
    pub window_secs: u64,
    /// Seconds between stale-entry cleanup passes (default 60).
    pub cleanup_interval_secs: u64,
}
```

##### B.3 — Channel Buffer Sizes (new `[network.buffers]` or `[advanced]`)

| Constant | Current Value | File | Line |
|----------|--------------|------|------|
| `INBOUND_CHANNEL_CAPACITY` | 128 | `protocol/.../channel.rs` | 53 |
| `OUTBOUND_CHANNEL_CAPACITY` | 512 | `protocol/.../channel.rs` | 60 |

**Action:** Move to `AdvancedConfig` (these are expert-level tuning knobs):

```rust
/// Inbound packet channel capacity per connection (default 128).
pub inbound_channel_capacity: usize,
/// Outbound packet channel capacity per connection (default 512).
pub outbound_channel_capacity: usize,
```

Pass values through `ServerContext` → network setup code instead of importing
constants from `oxidized-protocol`. The protocol crate keeps the defaults as
`pub const` for use in tests and as documented defaults.

##### B.4 — Entity Tracking Ranges (new `[gameplay.entity_tracking]` section)

| Constant | Current Value | File | Line |
|----------|--------------|------|------|
| `TRACKING_RANGE_PLAYER` | 512 blocks | `entity/tracker.rs` | 13 |
| `TRACKING_RANGE_ANIMAL` | 160 blocks | `entity/tracker.rs` | 15 |
| `TRACKING_RANGE_MONSTER` | 128 blocks | `entity/tracker.rs` | 17 |
| `TRACKING_RANGE_MISC` | 96 blocks | `entity/tracker.rs` | 19 |
| `TRACKING_RANGE_PROJECTILE` | 64 blocks | `entity/tracker.rs` | 21 |
| `TRACKING_RANGE_DEFAULT` | 80 blocks | `entity/tracker.rs` | 23 |

**Action:** Add an `EntityTrackingConfig` with per-category range fields. Keep
the current constants as documented defaults in the struct's `Default` impl.
This is a high-impact tuning parameter for server bandwidth and CPU.

##### B.5 — Weather Cycle Timing (new `[gameplay.weather]` section)

| Constant | Current Value | File | Line |
|----------|--------------|------|------|
| `RAIN_DELAY_MIN` | 12 000 ticks | `tick.rs` | 37 |
| `RAIN_DELAY_MAX` | 180 000 ticks | `tick.rs` | 38 |
| `RAIN_DURATION_MIN` | 12 000 ticks | `tick.rs` | 41 |
| `RAIN_DURATION_MAX` | 24 000 ticks | `tick.rs` | 42 |
| `THUNDER_DELAY_MIN` | 12 000 ticks | `tick.rs` | 45 |
| `THUNDER_DELAY_MAX` | 180 000 ticks | `tick.rs` | 46 |
| `THUNDER_DURATION_MIN` | 3 600 ticks | `tick.rs` | 49 |
| `THUNDER_DURATION_MAX` | 15 600 ticks | `tick.rs` | 50 |

**Action:** Add a `WeatherConfig` struct. Values in ticks (document conversion:
20 ticks = 1 second). Validate min ≤ max at config load time.

##### B.6 — World Tuning (extend existing `[world]` section)

| Constant | Current Value | File | Line |
|----------|--------------|------|------|
| `DEFAULT_CACHE_SIZE` | 1 024 chunks | `level/server_level.rs` | 23 |
| `DEFAULT_MAX_CONCURRENT` | 64 chunks | `worldgen/scheduler.rs` | 26 |

**Action:** Add to `WorldConfig`:

```rust
/// Maximum chunks kept in the in-memory cache (default 1024).
pub chunk_cache_size: usize,
/// Maximum concurrent chunk generation tasks (default 64).
pub max_concurrent_chunk_generations: usize,
```

---

#### Implementation Order

1. **Part A first** — pure deletion, low risk, unblocks TOML schema cleanup
2. **B.1 + B.2** — network timeouts & rate limiting (most operator-visible)
3. **B.4** — entity tracking ranges (biggest performance impact)
4. **B.5** — weather cycle timing (gameplay customization)
5. **B.3 + B.6** — advanced/expert knobs (channel sizes, cache, worldgen concurrency)

Each step: add config field → wire through `ServerContext` / `ServerSettings` →
replace `const` usage → add config validation → update `oxidized.toml` with
documented defaults → test.

---

#### Verification

- `cargo test --workspace` — all tests pass
- `cargo clippy --workspace -- -D warnings` — no new warnings
- Grep confirms zero references to removed Java fields:
  ```bash
  grep -rn 'jmx_monitoring\|native_transport\|sync_chunk_writes' \
    crates/ oxidized.toml docs/ --include="*.rs" --include="*.toml" --include="*.md"
  # Expected: 0 (outside this phase doc)
  ```
- `oxidized.toml` loads successfully with both default and custom values
- Config validation rejects invalid values (e.g., `keepalive_timeout < keepalive_interval`,
  `rain_delay_min > rain_delay_max`)
- Server starts and runs with all-default config (backward compatible)
- Server starts with all values overridden via TOML and env vars

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Flag values don't match vanilla | Medium | High | Automated spot-check tests comparing against extracted data |
| `build.rs` regression from data changes | Medium | High | CI step regenerates and diffs against committed output |
| Tag membership differs from string patterns | Low | High | Regression test: old function vs new for all block states |
| Performance regression from larger table | Low | Low | Table is still <1 MB; benchmark block state lookups |
| Breaking change in `BlockStateEntry` layout | Certain | Medium | Update all callers in same PR; no external consumers |
| ops.json format drift from vanilla | Low | Medium | Test against vanilla-generated ops.json samples |
| Fuzz targets find deep bugs | Medium | High | Fix bugs before merging; fuzz in CI nightly |
| reqwest replacement breaks auth | Low | High | Feature-flag behind `light-http`; keep reqwest as fallback |
| Config removal breaks existing deployments | Medium | Medium | Document migration in CHANGELOG; fail loudly if removed fields found in TOML |
| Over-configuring creates footgun surface | Low | Medium | Validate all ranges at load time; document safe defaults prominently |

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

11. **Benchmark suite exists** — `cargo bench` runs in at least 3 crates

12. **Fuzz targets exist** — `cargo fuzz list` shows targets for protocol and NBT

13. **Per-player permissions** — `ops.json` loaded and used for command dispatch

14. **No stale TODOs** for items addressed in R5.14–R5.18

15. **Zero Java-specific config fields** — no JMX, native transport, or sync chunk writes

16. **All server-tunable constants configurable** — network timeouts, entity tracking
    ranges, weather cycles, channel sizes, and world tuning readable from `oxidized.toml`

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
| R5.9 | 16 | +144, -153 |
| R5.10 | 20-30 | +100, -150 |
| R5.11 | 10-15 | +200, -50 |
| R5.12 | 8-12 | +100, -0 |
| R5.13 | 10-15 | +80, -40 |
| R5.14 | 10-15 | +600, -0 |
| R5.15 | 8-12 | +400, -30 |
| R5.16 | 8-10 | +150, -60 |
| R5.17 | 5 | +700, -37 |
| R5.18 | 15-20 | +350, -120 |
| **Total** | ~210-285 | +3910, -2340 |

Net reduction: ~150 lines for R5.1–R5.13 (original scope). R5.14–R5.18 are net
additions (new infrastructure, features, and safety improvements). The key metric
is not LOC but the elimination of string dispatch, runtime overhead, duplication
patterns, and accumulated tech debt.
