# Phase R1 — Architectural Refactoring

**Status:** ✅ Complete  
**Crates:** `oxidized-server`, `oxidized-protocol`, `oxidized-game`  
**Reward:** Codebase is modular, extensible, and ready to scale to 58 Play packets
and 38 phases without structural degradation.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-035: Module Structure & File Size Policy](../adr/adr-035-module-structure.md) —
  when and how to split files
- [ADR-036: Packet Handler Architecture](../adr/adr-036-packet-handler-architecture.md) —
  network.rs restructuring plan
- [ADR-037: Coordinate & Vector Type Macros](../adr/adr-037-vector-type-macros.md) —
  boilerplate elimination for types

Also relevant:
- [ADR-003: Crate Workspace Architecture](../adr/adr-003-crate-architecture.md) —
  crate-level boundaries (unchanged)
- [ADR-006: Network I/O Architecture](../adr/adr-006-network-io.md) — reader/writer tasks
  (unchanged)
- [ADR-008: Connection State Machine](../adr/adr-008-connection-state-machine.md) —
  typestate pattern (pragmatically amended; module-level safety instead)

---

## Goal

Refactor the existing codebase to address architectural issues identified after Phase 18
completion. This is a **pure structural refactoring** — no new features, no behavior changes,
no protocol additions. Every test that passes before must pass after.

The refactoring fixes 6 categories of structural problems:

1. God files (>800 LOC with 3+ responsibilities)
2. Giant dispatch chains (if-else/match >20 arms)
3. Duplicated serialization logic
4. Copy-pasted argument parsing and getter functions
5. Boilerplate across vector/coordinate types
6. Small localized duplication (array formatting, serde type dispatchers, etc.)

---

## Motivation

The codebase is at 50K LOC across 6 crates. Phases 19–38 will roughly triple it. Without
structural improvements now:

- `network.rs` will grow to 5000+ LOC as 46 more Play packets are implemented
- `component.rs` serialization duplication will compound when new serialization formats
  are added (e.g., registry codec in Phase 31)
- `context.rs` argument dispatch will grow from 30 to 57 argument types
- New contributors will struggle to navigate files where 6 concerns are interleaved

Refactoring now, between Phase 18 and Phase 19, is optimal because:
- Phase 18 (commands) is complete — a natural stabilization point
- Phase 19 (world ticking) will heavily modify `network.rs` (tick integration)
- The test suite is comprehensive (1400+ tests) providing refactoring safety

---

## Non-Goals

- **No new features** — this phase adds zero user-visible behavior
- **No API changes** — all public `pub` items keep their paths; `pub(crate)` items may move
  but callers are updated in the same PR
- **No dependency changes** — no new crates added, no versions bumped
- **No ADR-008 typestate retrofit** — the runtime enum stays; module-level safety is sufficient
- **No performance optimization** — structure only; perf is Phase 38

---

## Detailed Refactoring Plan

### R2: network.rs → network/ Module Split

**Target:** `crates/oxidized-server/src/network.rs` (2079 LOC → ~10 files, ~150 LOC avg)

**Current problems:**
- `handle_play_entry()` is 715 lines with 6 responsibilities
- 433-line if-else chain for packet dispatch
- 19 instances of repeated decode+match+log pattern
- Authentication, chat, commands, movement all inline

**Steps:**

1. **Create `src/network/` directory** with `mod.rs`
   - Move `listen()`, `handle_connection()`, `ServerContext`, `LoginContext`,
     `ChatBroadcastMessage` to `mod.rs`
   - These are the shared types and entry points

2. **Extract state handlers to separate files:**
   - `handshake.rs` ← `handle_handshake()` (~50 LOC)
   - `status.rs` ← `handle_status()` + status tests (~100 LOC)
   - `login.rs` ← `handle_login()`, `authenticate_online()` (~200 LOC)
   - `configuration.rs` ← `handle_configuration()` (~200 LOC)

3. **Create `helpers.rs`:**
   - `decode_packet<T>()` — generic decode helper (see ADR-036)
   - `disconnect()` — shared disconnect utility
   - Move redundant error mapping to this single location

4. **Split play state into `play/` submodule:**
   - `play/mod.rs` — main `select!` loop, keepalive timer, packet dispatch `match`
   - `play/movement.rs` — `handle_movement()`, chunk tracking, view distance updates
     (currently 157 lines inline)
   - `play/chat.rs` — `handle_chat()`, `handle_chat_command()`, rate limiting
     (currently ~120 lines inline)
   - `play/commands.rs` — `handle_command_suggestion()`, `make_command_source()`,
     `make_command_source_for_player()`, `commands_packet_from_tree()`
   - `play/helpers.rs` — `send_initial_chunks()`, login sequence packet building

5. **Replace if-else chain** with `match pkt.id { ... }` + handler function calls
   - Each arm is 1-3 lines: delegate to handler function
   - Handler functions live in their respective submodules

6. **Create `PlayContext` struct** to bundle the 8+ parameters passed to every handler

**Verification:** `cargo test --workspace` — all existing tests must pass unchanged.
Integration tests in `network.rs` move to `network/mod.rs` or relevant submodule.

---

### R3: component.rs → Serialization Split

**Target:** `crates/oxidized-protocol/src/chat/component.rs` (1439 LOC → ~500 LOC core +
3 files)

**Current problems:**
- JSON serialization (Serialize + Deserialize): ~250 LOC
- NBT serialization (to_nbt + from_nbt + from_nbt_compound): ~350 LOC
- Style fields encoded 3 times (JSON, NBT, legacy) with copy-pasted match blocks
- 11 large control-flow structures handling the same 6 content variants

**Steps:**

1. **Create `component_json.rs`:**
   - Move `impl Serialize for Component` (~88 lines)
   - Move `impl<'de> Deserialize<'de> for Component` (~116 lines)
   - Move `count_style_fields()` helper
   - Keep `use super::component::*` for access to types

2. **Create `component_nbt.rs`:**
   - Move `Component::to_nbt()` (~154 lines)
   - Move `Component::from_nbt()` and `from_nbt_compound()` (~157 lines)

3. **Enhance `style.rs`:**
   - Add `ClickEvent::to_nbt_action()` / `from_nbt_action()` methods
   - Add `HoverEvent::to_nbt()` / `from_nbt()` methods
   - Add `Style::serialize_json_fields()` / `Style::to_nbt_fields()` methods
   - This centralizes event/style serialization that's currently duplicated

4. **Add methods on `ComponentContent`:**
   - `fn serialize_json_fields<S: SerializeMap>(&self, map: &mut S)` — eliminates
     the 56-line match in JSON serializer
   - `fn to_nbt_compound(&self) -> NbtCompound` — eliminates the match in NBT encoder
   - `fn count_json_fields(&self) -> usize` — eliminates the count match
   - These DRY methods live in `component.rs` on the enum itself

5. **Extract `LegacyParser` struct** from `from_legacy_with_char()` (68 lines → struct with
   `process_char()` and `finish()` methods)

**Result:** `component.rs` shrinks from 1439 to ~400 LOC (structs, builders, Display, legacy).

**Verification:** `cargo test -p oxidized-protocol` — all chat tests pass.

---

### R4: Command Context — Argument Dispatch Refactor

**Target:** `crates/oxidized-game/src/commands/context.rs` (642 LOC)

**Current problems:**
- `parse_argument()` is a 155-line match with 30+ arms
- Numeric validation (min/max check) is copy-pasted 4 times (Integer/Long/Float/Double)
- 13 typed getter functions are near-identical (get arg → match result type → return/error)
- Vec2/Vec3 coordinate parsing is repeated

**Steps:**

1. **Create argument parser registry:**
   ```rust
   type ArgParser = fn(&mut StringReader, &ArgumentProperties) -> Result<ParsedArgument, CommandError>;
   
   fn get_parser(arg_type: &ArgumentType) -> ArgParser {
       match arg_type {
           ArgumentType::Integer { min, max } => parse_integer,
           ArgumentType::Long { min, max } => parse_long,
           // ... one line per type
       }
   }
   ```
   Each parser is a small function (5-15 lines) in its own logical group.

2. **Extract `validate_range<T: PartialOrd + Display>()`:**
   - Replaces 4 copies of min/max checking for Integer/Long/Float/Double
   - Single generic function: `validate_range(value, min, max, type_name)`

3. **Consolidate typed getters:**
   - Create `fn get_typed<T>(args, name, expected_type, extract) -> Result<T>`
   - Each public getter becomes a 2-line wrapper:
     ```rust
     pub fn get_integer(args: &Args, name: &str) -> Result<i32, CommandError> {
         get_typed(args, name, "integer", |arg| match arg {
             ParsedArgument::Integer(v) => Some(*v),
             _ => None,
         })
     }
     ```

4. **Extract coordinate parsing:**
   - `parse_vec2_coordinates()` and `parse_vec3_coordinates()` as shared helpers
   - Used by Vec2/Vec3/BlockPos/ColumnPos argument types

**Verification:** `cargo test -p oxidized-game` — all command tests pass (1400+ tests).

---

### R5: Protocol Type Macros

**Target:** `crates/oxidized-protocol/src/types/*.rs` and `codec/*.rs`

**Current problems:**
- Operator overloading duplicated across Vec3, Vec3i (~40 lines)
- Directional accessors duplicated across BlockPos, Vec3i, SectionPos (~54 lines)
- Axis accessors duplicated across 4 types (~48 lines)
- Wire format read/write pairs duplicated in types.rs (~150 lines)
- VarInt/VarLong encode/decode duplicated (~67 lines)

**Steps:**

1. **Create `types/type_macros.rs`:**
   - Define `impl_vector_ops!`, `impl_directional!`, `impl_axis_accessor!`
   - See ADR-037 for macro definitions

2. **Apply macros to existing types:**
   - `vec3.rs`: Replace manual Add/Sub/Neg with `impl_vector_ops!(Vec3)`
   - `vec3i.rs`: Replace manual Add/Sub/Neg + directional with macros
   - `block_pos.rs`: Replace directional + axis accessors with macros
   - `section_pos.rs`: Replace directional + axis accessors with macros
   - Remove the now-redundant hand-written impls

3. **Create `codec/wire_macros.rs`:**
   - Define `impl_wire_primitive!` macro
   - Apply to 11 read/write pairs in `types.rs`

4. **Unify VarInt/VarLong in `varint.rs`:**
   - Create `VarEncoding` trait for i32/i64
   - Generic `encode_var<T>()` and `decode_var<T>()`
   - Keep `encode_varint`/`encode_varlong` as thin wrappers for API compatibility

5. **Extract `expand_axis()` helper** in `aabb.rs`:
   - Eliminates repeated `if dx < 0.0 { min += dx } else { max += dx }` per axis
   - `contract()` becomes `self.expand_towards(-dx, -dy, -dz)`

**Verification:** `cargo test -p oxidized-protocol` — all type + codec tests pass.
Run `cargo expand -p oxidized-protocol` to verify macro output is correct.

---

### R6: Small Wins (Do Opportunistically)

These are lower-priority improvements that can be done individually or batched:

1. **snbt.rs array formatting** (~90 LOC saved):
   - Extract `format_typed_array()` for ByteArray/IntArray/LongArray
   - Three identical `for (i, v) in arr.iter().enumerate()` loops become one generic

2. **serde.rs type dispatchers** (~120 LOC saved):
   - `deserialize_prim!` macro for the 12 near-identical `deserialize_*` methods
   - Pattern: match NbtTag variant → visitor call → error message

3. **paletted_container.rs palette tier selection:**
   - Extract `determine_palette_tier(bits_needed, strategy) -> PaletteTier`
   - Currently duplicated in `upgrade_and_set()`, `read_from_bytes()`, `from_nbt_data()`

4. **bit_storage.rs bounds check:**
   - Extract `validate_index_and_value()` (duplicated 3x)
   - Extract `long_bit_offset()` (duplicated 4x)

5. **login.rs packet builders:**
   - Split 146-line `build_login_sequence()` into per-packet builders
   - `build_login_packet()`, `build_abilities_packet()`, `build_spawn_packet()`, etc.

---

## Acceptance Criteria

- [ ] All 3 ADRs (035, 036, 037) are written and committed
- [ ] `network.rs` is split into `network/` module tree per ADR-036
- [ ] No single file exceeds 800 LOC (excluding tests) per ADR-035
- [ ] `component.rs` serialization is split into separate files
- [ ] Command context argument dispatch uses table-driven parsing
- [ ] Type macros eliminate operator/directional/axis duplication
- [ ] `cargo test --workspace` passes with zero test failures
- [ ] `cargo clippy --workspace` produces no new warnings
- [ ] All refactored modules have doc comments explaining their responsibility
- [ ] ADR README index is updated with ADR-035/036/037

---

## Ordering & Dependencies

```
R1 (ADRs) ─────────┬── R2 (network.rs split) ─── depends on ADR-036
                    ├── R3 (component.rs split) ── depends on ADR-035
                    └── R5 (type macros) ────────── depends on ADR-037

R4 (command context) ── independent, can parallel with R2/R3

R6 (small wins) ── independent, do opportunistically
```

**Critical path:** R1 → R2 (biggest impact, blocks Phase 19 network changes)

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Moving code breaks `pub(crate)` visibility | Audit all `pub(crate)` items before moving; use `grep` to find all callers |
| Test imports break after module restructure | Use `cargo test` after every individual file move |
| Macro bugs affect multiple types | Run `cargo expand` to inspect macro output; keep named methods as underlying impl |
| Merge conflicts with in-progress Phase 19 work | Complete R2 before starting Phase 19; rebase if needed |
| Refactoring introduces subtle behavior change | No logic modifications — only code moves and boilerplate extraction |

---

## ADR-008 Status Amendment

ADR-008 (Connection State Machine) specified a typestate pattern that was not implemented.
The runtime `ConnectionState` enum was a pragmatic choice that works correctly. This
refactoring phase does **not** retroactively implement typestate. Instead, the module split
in R2 achieves the same safety goal through file-level type isolation:

- `handshake.rs` can only import Handshaking packets
- `login.rs` can only import Login packets
- `play/movement.rs` can only import Play packets

This is documented in ADR-036 as an architectural amendment to ADR-008.
