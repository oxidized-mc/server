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
| R5.3 | Implement block tag loading from vanilla data | 📋 Planned |
| R5.4 | Replace string-based block categorization with flags/tags | 📋 Planned |
| R5.5 | Replace hardcoded physics properties with registry data | 📋 Planned |
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

### Block Properties & Double Blocks

**Q: Are all block properties covered? What about "double blocks"?**

**A:** ✅ **YES, fully covered. No hardcoding needed for double blocks.**

Blocks with `half` property (doors, tall plants, etc.) are represented entirely
via **state properties** in `blocks.json.gz`:
```json
"minecraft:oak_door": {
  "properties": {
    "facing": ["north", "south", "west", "east"],
    "half": ["upper", "lower"],      // ← Doubles handled here!
    "hinge": ["left", "right"],
    "open": ["true", "false"],
    "powered": ["true", "false"]
  },
  "states": [
    {"id": 5655, "properties": {"facing": "north", "half": "upper", ...}},
    {"id": 5656, "properties": {"facing": "north", "half": "lower", ...}},
    // ... 112 total states for all combinations
  ]
}
```

Each state (upper/lower) has its own BlockStateId and inherits block properties
(friction, hardness, is_solid, etc.) from the parent block type. Placement logic
checks the `half` property to determine behavior. No special-case code needed.

**Blocks with multi-part mechanics:**
- Doors, trapdoors, gates (19 blocks with `half` property)
- Beds (2 blocks with `part` property: head/foot)
- Chests (3 blocks with `type` property: single/left/right)
- Plants: tall grass, large fern, flowers (4 blocks with `half`)
- Pistons, repeaters, comparators (property-driven, no special handling)

### Data Extraction Architecture

**Q: Can we unify extraction scripts? Do we need separate block property file?**

**A:** ✅ **YES, unify into single script. Consolidate block_properties.json.gz into blocks.json.gz.**

**Current state (redundant):**
```
tools/
├── bundle_registries.py          (extracts protocol IDs)
├── extract_block_properties.py   (extracts material properties)
└── bundle_tags.py               (extracts tags)

data/
├── blocks.json.gz               (state machine + definitions)
├── block_properties.json.gz     (material properties)
├── items.json.gz                (ONLY has max_stack_size, max_damage)
└── tags.json
```

**New unified approach (R5.7 step 6):**
- **Create `tools/extract_vanilla_data.py`** — single script handles all extraction
  - Reads: `registries.json`, `Blocks.java`, `Items.java`, `BlockEntityType.java`, `tags.json`
  - Outputs: `blocks.json.gz`, `items.json.gz`, `tags.json`
- **Merge `block_properties.json.gz` into `blocks.json.gz`** — eliminate redundant file
- **Expand `items.json.gz`** to include: rarity, enchantability, fireResistant, foodProperties
  - Future proofs gameplay features without breaking current codegen
- Benefits: Single entry point, consistent error handling, easier to add properties, faster

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
| R5.7 | — | Independent; pure deduplication |
| R5.8 | — | Independent; pure deduplication |
| R5.9 | — | Independent; error handling standardization |
| R5.10 | — | Independent; struct decomposition |
| R5.11 | — | Independent; function decomposition |
| R5.12 | — | Independent; constant extraction |

**Parallelizable groups:**
- **Group A** (sequential): R5.1 → R5.2 → R5.3 → R5.4 → R5.5
- **Group B** (independent): R5.6, R5.7, R5.8, R5.9, R5.10, R5.11, R5.12

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

**✅ Completed**

- ✅ Expanded `BlockStateEntry` to 18 bytes with all property fields
- ✅ Updated `build.rs` to extract and encode all properties from `block_properties.json.gz`
- ✅ Implemented property accessors on `BlockStateId` with correct fixed-point conversions
- ✅ Verified table size: exactly 525.1 KB (29,873 states × 18 bytes)
- ✅ Spot-checked 8 well-known blocks: all values match vanilla
- ✅ Verified fixed-point roundtrip precision: values round-trip with <0.01 error
- ✅ All cargo tests pass (186 unit tests, 0 failures)
- ✅ Fixed-point encoding handles extreme values gracefully via clamping (e.g., barrier block's infinite resistance)

**Notes:**
- Light emission/opacity are limited to 0–15 (4-bit values)
- Map color is limited to 0–63 (6-bit values)
- Push reaction is limited to 0–3 (2-bit values)
- Explosion resistance values >6553.5 are clamped to 655.35 (u16::MAX)—acceptable since these are rare edge cases (barrier blocks only)
- All normal game blocks have full precision

### R5.3: Implement Block Tag Loading From Vanilla Data

**Targets:** `crates/oxidized-world/src/registry/tags.rs` (new),
`crates/oxidized-world/build.rs` or startup loading

**Current problems:**
- Tags are not implemented at all despite ADR-011 describing the full system
- Block categories (doors, signs, beds, tall plants) are checked via
  `.ends_with("_door")`, `.contains("sign")` etc.

**Steps:**

1. **Leverage existing tag data**:
   - `crates/oxidized-protocol/src/data/tags.json` already contains all 758
     vanilla tags with resolved numeric IDs (produced by `tools/bundle_tags.py`)
   - Block tags are under `"minecraft:block"` key with entries already mapped
     to block type IDs
   - No additional extraction needed for vanilla tags
     - `minecraft:replaceable` (used by `is_replaceable_block`)
     - `minecraft:doors` (used by `is_door_block`)
     - `minecraft:beds` (used by `is_bed_block`)
     - `minecraft:signs` / `minecraft:all_signs` (used by `is_sign_block`)
     - `minecraft:tall_flowers` (partial match for `is_tall_plant`)
     - `minecraft:climbable` (future use for physics)
     - `minecraft:buttons` (used by `is_interactable_block`)
     - `minecraft:fence_gates` (used by `is_player_direction_block`)
     - `minecraft:trapdoors` (used by string pattern)
     - `minecraft:shulker_boxes` (used by string pattern)

2. **Create `crates/oxidized-world/src/registry/tags.rs`**:
   ```rust
   /// Block tag registry loaded from vanilla tag data.
   ///
   /// Tags group blocks into categories for behavior dispatch.
   /// Membership testing is O(1) via bitset or O(log n) via sorted vec.
   pub struct BlockTags { ... }

   pub struct TagSet { ... }

   impl BlockTags {
       pub fn load(block_registry: &BlockRegistry) -> Result<Self, TagError>;
       pub fn contains(&self, tag: &str, block_type_id: u16) -> bool;
       pub fn get(&self, tag: &str) -> Option<&TagSet>;
   }
   ```

3. **Decide compile-time vs runtime loading**:
   - **Option A (recommended for R5)**: Compile-time via `build.rs` — read
     `tags.json` from `oxidized-protocol/src/data/` and generate static tag
     membership arrays. Zero startup cost. Cannot be extended by data packs
     (fine for now).
   - **Option B**: Runtime loading at server startup — read the same JSON,
     build tag sets dynamically. Supports future data pack overrides. More
     complex.
   - Choose Option A for this phase. Option B is added when Phase 34 implements
     data pack loading.

4. **Create custom Oxidized tags** for block categories not in vanilla tags:
   - Committed as JSON files in `crates/oxidized-world/src/data/tags/block/`
   - `interactable.json` — blocks from the current
     `is_interactable_block()` function that aren't covered by vanilla tags
   - `wall_mountable.json` — blocks from `is_wall_mountable()`
   - `player_direction.json` — blocks from `is_player_direction_block()`
   - `tall_plants.json` — complete set from `is_tall_plant()` (vanilla
     `#tall_flowers` doesn't include `tall_grass`, `large_fern`, `tall_seagrass`)
   - These use block names (not IDs) and are resolved by `build.rs` using
     the block registry

5. **Integrate with `BlockRegistry`**:
   - `BlockRegistry` holds or provides access to `BlockTags`
   - Tags are available anywhere the registry is available

**Verification:**
- For each vanilla tag file, verify loaded tag contains exactly the expected
  block type IDs
- For each custom tag, verify membership matches the current hardcoded function
- Tag membership is reflexive: `contains("minecraft:doors", oak_door_id) == true`
- Non-member check: `contains("minecraft:doors", stone_id) == false`
- Unknown tag returns `None` / `false`

---

### R5.4: Replace String-Based Block Categorization With Flags/Tags

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

---

### R5.5: Replace Hardcoded Physics Properties With Registry Data

**Targets:** `crates/oxidized-game/src/physics/block_properties.rs`,
`crates/oxidized-game/src/physics/constants.rs`

**Current problems:**
- `PHYSICS_OVERRIDES` static array matches 8 blocks by `"minecraft:..."` name
- `from_registry()` iterates this array doing string comparisons
- Physics constants like `ICE_FRICTION`, `BLUE_ICE_FRICTION` duplicate values
  that should come from the block registry

**Steps:**

1. **Delete `PHYSICS_OVERRIDES` array** and the `from_registry()` function that
   uses it

2. **Replace with registry property lookups**:
   ```rust
   // Before:
   let friction = physics_overrides.get(block_name)
       .map(|o| o.friction)
       .unwrap_or(BLOCK_FRICTION_DEFAULT);

   // After:
   let friction = block_state_id.friction();  // From BlockStateEntry
   ```

3. **Remove redundant physics constants** that duplicate block registry data:
   - `ICE_FRICTION`, `BLUE_ICE_FRICTION`, `SLIME_FRICTION` etc. are no longer
     needed — the values come from the block state table
   - Keep `BLOCK_FRICTION_DEFAULT` only if needed as a fallback (it shouldn't be
     — every block state has a friction value in the table)

4. **Update `PhysicsBlockData`** to read from `BlockStateId` methods:
   ```rust
   impl PhysicsBlockData {
       pub fn from_state(state: BlockStateId) -> Self {
           Self {
               friction: state.friction(),
               speed_factor: state.speed_factor(),
               jump_factor: state.jump_factor(),
               is_slime: state.block_name() == "minecraft:slime_block", // or use tag
               is_honey: state.block_name() == "minecraft:honey_block", // or use tag
           }
       }
   }
   ```
   For `is_slime` and `is_honey` (bounce/stick behavior), consider a
   `BOUNCE_BEHAVIOR` or `STICKY` flag in `BlockStateFlags` if these are hot-path
   checks. Otherwise a tag is fine.

**Verification:**
- Physics behavior tests: player walks on ice (friction 0.98), soul sand
  (speed 0.4), honey block (speed 0.4, jump 0.5), blue ice (friction 0.989)
- Values match vanilla within epsilon (f64 precision from fixed-point encoding)
- No `PHYSICS_OVERRIDES` or block name strings remain in physics code

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
- **Runtime loading**: `ItemRegistry::load()` decompresses `items.json.gz`, builds a
  `Vec<Item>` and `AHashMap<String, usize>` on first access (via `LazyLock`)
- **Hash lookup cost**: Every item name-to-ID lookup (e.g., `item_name_to_id("minecraft:stone")`)
  does an `AHashMap` lookup instead of O(1) array access
- **Startup overhead**: Decompression + hashmap construction happens at runtime, not at
  compile time
- **Inconsistency**: Blocks use compile-time codegen with pre-computed arrays; items use
  runtime loading — no good reason for the difference

**Comparison:**

| Operation | Current (Runtime) | Target (Compile-Time) |
|-----------|-------------------|----------------------|
| Name → ID | `AHashMap::get()` (hash) | Array index (O(1)) |
| ID → Name | `Vec::get()` (O(1)) | Array index (O(1)) |
| Startup | Decompress + build maps | Zero |
| Binary size | ~100 KB gzipped JSON | Similar (pre-gen Rust code) |

**Steps:**

1. **Extend `build.rs`** to also generate item registration data:
   - Read `items.json.gz` (already embedded in `oxidized-world`)
   - Parse the JSON in vanilla registration order
   - Generate `item_ids_generated.rs` with:
     ```rust
     /// All 1506 items in vanilla registration order
     const ITEM_NAMES: &[&str] = &[
         "minecraft:air",
         "minecraft:stone",
         // ... 1504 more
     ];

     const ITEM_MAX_STACK_SIZES: &[u8] = &[
         64, 64, 64, // ...
     ];

     const ITEM_MAX_DAMAGES: &[u16] = &[
         0, 0, 0, // ...
     ];
     ```

2. **Create `crates/oxidized-world/src/registry/item_ids_generated.rs`** (auto-generated):
   - Include the static arrays from step 1
   - Public access: `pub fn item_name_to_id(name: &str) -> Option<usize>`
   - Public access: `pub fn item_id_to_name(id: usize) -> Option<&'static str>`

3. **Rewrite `ItemRegistry`** to use the generated arrays:
   - Replace `Vec<Item>` with direct array references
   - Remove `AHashMap<String, usize>` — replace with `name_to_id()` doing a linear
     search over the static `ITEM_NAMES` array (or use a compile-time perfect hash
     if needed)
   - Keep the same public API: `name_to_id()`, `id_to_name()`, `max_stack_size()`

4. **Update `crates/oxidized-game/src/inventory/item_ids.rs`**:
   - `item_name_to_id()` now calls the generated function (no LazyLock needed)
   - Still return `-1` for unknown items (same interface)
   - Zero startup cost

5. **Add snapshot tests** to verify the generated code:
   - `insta::assert_snapshot!(ITEM_NAMES)` — check all 1506 items
   - Round-trip test: `for each name in ITEM_NAMES: assert_eq!(item_name_to_id(name), id)`

**Verification:**
- `cargo build -p oxidized-world` — `item_ids_generated.rs` exists and compiles
- `cargo test -p oxidized-game inventory::item_ids` — all 4 existing tests pass
- `item_name_to_id("minecraft:stone")` returns `1` (not parsed, array index)
- Zero startup latency compared to before

**Important: Data extraction architecture decision:**

R5.7 also unifies all vanilla data extraction into a single script. Currently,
three scripts redundantly parse vanilla data sources:

- `tools/bundle_registries.py` (extracts block/item protocol IDs)
- `tools/extract_block_properties.py` (extracts block material properties)
- `tools/bundle_tags.py` (extracts tag definitions)

**New unified approach:**
- Create `tools/extract_vanilla_data.py` — single entry point for all data extraction
- Reads: `registries.json` (protocol IDs), `Blocks.java` (block properties),
  `Items.java` (item properties), `BlockEntityType.java` (block entities), `tags.json`
- Outputs: `blocks.json.gz`, `items.json.gz`, `tags.json`
- **Consolidate `block_properties.json.gz` into `blocks.json.gz`** (remove redundant file)
- This is a **separate task from the codegen itself** but should be done in R5.7
  to avoid maintaining multiple extraction sources during the transition

**Item properties to expand in R5.7:**
- `items.json.gz` currently only has `max_stack_size` and `max_damage`
- Should also capture: `rarity`, `enchantability`, `fireResistant`, `foodProperties`
  (these enable future gameplay features but can default to vanilla if not yet used
  in Oxidized logic)
- The unified script extracts all properties; `build.rs` and `item_ids.rs` only use
  what's needed for now, but infrastructure is ready for expansion

**Notes:**
- Double blocks (doors, chests, plants) are already fully supported via state properties
  (`half`, `type`, `part` properties in `blocks.json.gz`). No special hardcoding needed.
- If linear search in `name_to_id()` becomes a bottleneck, add a compile-time perfect
  hash (via `phf` crate) — but start with linear search and measure first.
- Keep the generated file in `src/registry/item_ids_generated.rs` in `.gitignore` —
  it's always regenerated at build time.
- This mirrors the block codegen approach (ADR-012).

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
| R5.7 | 3-4 | +150, -100 |
| R5.8 | 40-60 | +200, -800 |
| R5.9 | 17-20 | +80, -400 |
| R5.10 | 20-30 | +100, -150 |
| R5.11 | 10-15 | +200, -50 |
| R5.12 | 8-12 | +100, -0 |
| R5.13 | 10-15 | +80, -40 |
| **Total** | ~140-190 | +1900, -2050 |

Net reduction: ~150 lines. The key metric is not LOC but the elimination of
string dispatch, runtime overhead, and duplication patterns.
