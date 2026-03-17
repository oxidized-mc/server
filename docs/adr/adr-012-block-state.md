# ADR-012: Block State Representation

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P08, P09, P22 |
| Deciders | Oxidized Core Team |

## Context

Minecraft's world is built from blocks, and each block can exist in multiple states defined by
properties (e.g. a staircase has `facing`, `half`, `shape`, and `waterlogged` properties,
yielding 80 distinct states). As of Minecraft 1.21+, there are approximately 24,000 unique block
states across roughly 1,000 block types. Every block position in every loaded chunk references
one of these states. When a player moves, the server checks collision shapes by reading block
states. When lighting updates, the server reads light opacity and emission from block states.
When chunks are serialized for sending to clients, every block in the chunk's palette is a block
state. This makes block state lookup the single hottest data access pattern in the entire server.

Vanilla Java represents block states as immutable `BlockState` objects. Each block type has a
`StateDefinition` that enumerates all valid property combinations and creates one `BlockState`
per combination. Properties are stored in an `ImmutableMap<Property<?>, Comparable<?>>` inside
each `BlockState`. State transitions (e.g. "give me this state but with `facing=north`") use a
precomputed table of neighbors. The `BlockState` object also caches commonly accessed data:
collision shape, light emission, opacity, and various boolean flags. While this works, the
indirection through object references and the per-property boxing add overhead that Rust can
eliminate entirely.

For Oxidized, we need a representation that minimizes memory usage per block in chunk storage
(since each chunk section holds 4096 blocks), provides O(1) access to all block state properties
and cached data, and supports efficient state transitions. The representation must also be
compatible with the network protocol, which uses a global palette where each block state has a
unique integer ID.

## Decision Drivers

- **Lookup speed**: Block state data (solidity, opacity, collision shape, light emission) must
  be accessible in a single array index — no HashMap, no pointer chasing, no virtual dispatch.
- **Memory efficiency in chunks**: Each block position in a chunk section should use the minimum
  number of bits. The global palette ID fits in a u16 (24,000 < 65,536).
- **State transition performance**: Getting "this state but with property X changed" must be
  fast, since it's used during block placement, redstone updates, and water flow.
- **Protocol compatibility**: The global state ID assigned to each block state must match
  vanilla's assignment so that chunk data and block change packets are correctly interpreted by
  the client.
- **Compile-time generation**: Block state data should be generated at compile time from
  extracted vanilla data to avoid runtime initialization cost.
- **Cache friendliness**: The most-accessed fields should be packed together to maximize CPU
  cache utilization.

## Considered Options

### Option 1: Object Per State Like Vanilla

Each block state is a Rust struct containing a `HashMap<String, PropertyValue>` for properties
and cached fields for commonly accessed data. This mirrors vanilla's design. The downside is
HashMap overhead per state (24,000 HashMaps), pointer indirection for property lookups, and
poor cache locality when iterating over block data. Rust's ownership model also makes shared
immutable state objects awkward without `Arc`, adding atomic reference count overhead.

### Option 2: Flat u16 State ID With Dense Lookup Table

Every block state is identified by a `u16` ID. A single `Vec<BlockStateData>` (or static array),
indexed by the u16 ID, holds all properties and cached data for each state. Looking up any
property is a single array index: `STATES[id].is_solid`. No HashMap, no pointers. The table is
generated at compile time and is cache-friendly for sequential access patterns. The downside is
that the table is relatively large (~24,000 entries × size of BlockStateData), but this is a
one-time static allocation.

### Option 3: Packed Bitfield Per Block

Instead of a state ID, store `block_type_id` + packed property bits inline in each block
position. Properties are encoded as bit fields within a u32. This avoids the lookup table
entirely — properties are decoded with bit shifts. However, different block types have different
property sets with different bit widths, making the encoding complex and block-type-dependent.
Property access requires knowing the block type first, adding a branch on every access. It
also doesn't map cleanly to the protocol's global palette IDs.

### Option 4: Flyweight Pattern — Shared Immutable State Objects

Allocate all 24,000 `BlockState` structs in a `Vec` and reference them by index (effectively
a u16 ID). Each struct contains its properties and cached data directly (not behind pointers).
This is essentially Option 2 with a more OOP framing. The practical implementation is identical
to flat u16 IDs with a dense table, making this option redundant with Option 2.

## Decision

We adopt **flat u16 state IDs with a dense lookup table**, generated at compile time from
extracted vanilla data.

### State ID Assignment

Every block state receives a unique `u16` ID matching vanilla's global palette assignment. The
assignment is deterministic: block types are enumerated in registry order, and within each block
type, states are enumerated by iterating properties in definition order with values in their
natural order. This ensures our IDs match vanilla's without any runtime negotiation.

The state ID type is a newtype wrapper for type safety:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockState(u16);

impl BlockState {
    pub const AIR: BlockState = BlockState(0);

    #[inline]
    pub fn id(self) -> u16 { self.0 }

    #[inline]
    pub fn data(self) -> &'static BlockStateData { &BLOCK_STATES[self.0 as usize] }
}
```

### Dense Lookup Table

A single static array holds all block state data:

```rust
pub static BLOCK_STATES: &[BlockStateData] = &[
    // Generated by build.rs — ~24,000 entries
    BlockStateData { /* air */ },
    BlockStateData { /* stone */ },
    // ...
];
```

`BlockStateData` is a cache-line-friendly struct packing the most commonly accessed fields at
the beginning:

```rust
#[repr(C)]
pub struct BlockStateData {
    // Hot fields — accessed every block check (first cache line, 64 bytes)
    pub block_type: u16,          // index into block type registry
    pub collision_shape: u16,     // index into shared collision shape table
    pub light_emission: u8,       // 0–15
    pub light_opacity: u8,        // 0–15
    pub flags: BlockStateFlags,   // bitfield: is_solid, is_liquid, is_air, has_collision,
                                  //           blocks_motion, is_replaceable, requires_tool, etc.
    pub map_color: u8,            // map color index for cartography

    // Warm fields — accessed during specific operations
    pub push_reaction: u8,        // NORMAL, DESTROY, BLOCK, PUSH_ONLY
    pub instrument: u8,           // note block instrument
    pub hardness: f32,            // break speed
    pub explosion_resistance: f32,

    // Properties — block-type-specific
    pub properties: PackedProperties, // encoded property values
}
```

`BlockStateFlags` is a bitfield packed into a `u16`:

```rust
bitflags::bitflags! {
    pub struct BlockStateFlags: u16 {
        const IS_SOLID       = 0b0000_0000_0000_0001;
        const IS_LIQUID      = 0b0000_0000_0000_0010;
        const IS_AIR         = 0b0000_0000_0000_0100;
        const HAS_COLLISION  = 0b0000_0000_0000_1000;
        const BLOCKS_MOTION  = 0b0000_0000_0001_0000;
        const IS_REPLACEABLE = 0b0000_0000_0010_0000;
        const REQUIRES_TOOL  = 0b0000_0000_0100_0000;
        const IS_WATERLOGGED = 0b0000_0000_1000_0000;
        const IS_OPAQUE      = 0b0000_0001_0000_0000;
        const HAS_BLOCK_ENTITY = 0b0000_0010_0000_0000;
        const TICKS_RANDOMLY = 0b0000_0100_0000_0000;
        // ...
    }
}
```

Property access for typed queries uses the block type's property definition:

```rust
impl BlockState {
    #[inline]
    pub fn is_solid(self) -> bool {
        self.data().flags.contains(BlockStateFlags::IS_SOLID)
    }

    #[inline]
    pub fn light_emission(self) -> u8 {
        self.data().light_emission
    }

    pub fn get_property<P: BlockProperty>(self, property: P) -> Option<P::Value> {
        let def = &BLOCK_TYPES[self.data().block_type as usize];
        def.decode_property(property, self.data().properties)
    }
}
```

### State Transitions

State transitions ("this state but with `facing=north`") use a precomputed neighbor table. For
each block type, a 2D table maps `(property_index, value_index) → state_id_offset`. Applying a
transition is:

```rust
impl BlockState {
    pub fn with_property<P: BlockProperty>(self, property: P, value: P::Value) -> BlockState {
        let def = &BLOCK_TYPES[self.data().block_type as usize];
        def.transition(self, property, value)
    }
}
```

The transition table is generated at compile time. For blocks with few properties (most blocks),
this is a small table. For blocks with many property combinations (e.g. redstone wire with 1296
states), the table is larger but still only a few KB.

### Collision Shapes

Block states reference collision shapes by index into a shared shape table. Many block states
share the same collision shape (e.g. all full cubes share one shape, all slabs-bottom share
another). The shape table contains `VoxelShape` definitions as lists of axis-aligned bounding
boxes (AABBs). This deduplication means ~24,000 block states reference only ~300 unique shapes.

```rust
pub static COLLISION_SHAPES: &[VoxelShape] = &[
    VoxelShape::EMPTY,                              // 0: air, fluids
    VoxelShape::full_cube(),                         // 1: stone, dirt, etc.
    VoxelShape::slab_bottom(),                       // 2: bottom slabs
    // ...~300 unique shapes
];
```

### Memory Usage

- **Lookup table**: ~24,000 entries × ~48 bytes = ~1.1 MB static data (generated at compile time)
- **Collision shapes**: ~300 shapes × ~64 bytes = ~19 KB
- **Transition tables**: ~200 KB total across all block types
- **Per block in chunk**: 2 bytes (u16 state ID) — minimum possible for 24,000 states
- **Total static overhead**: ~1.4 MB — negligible for a server process

## Consequences

### Positive

- **O(1) everything**: Property queries, flag checks, collision shape access, and state
  transitions are all direct array indexing or bitfield operations. No HashMap, no branching
  on block type for common queries, no pointer chasing.
- **Cache-friendly**: The `BlockStateData` struct packs hot fields at the start, fitting the
  most-accessed data in a single cache line. Sequential block iteration (e.g. lighting pass)
  benefits from spatial locality in the lookup table.
- **Minimal per-block storage**: 2 bytes per block position in chunk storage. A full chunk
  section (4096 blocks) uses 8 KB for block data — same as vanilla's minimum palette encoding.
- **Protocol-compatible**: State IDs match vanilla's global palette exactly, so chunk data and
  block change packets can use the IDs directly without translation.
- **Compile-time safety**: All block state data is generated from vanilla's extracted data and
  verified at build time. A Minecraft version update regenerates the table automatically.

### Negative

- **Build-time dependency**: The `build.rs` code generation step requires extracted vanilla data
  in `mc-server-ref/generated/`. Updating to a new Minecraft version requires re-extracting
  block state data.
- **Large generated code**: The static arrays add ~1.4 MB to the binary. While negligible at
  runtime, the generated source file is large and should be excluded from IDE indexing and
  code review diffs.
- **Property access is indirect for dynamic queries**: Getting a property by name (e.g. from a
  command like `/setblock ~ ~ ~ oak_stairs[facing=north]`) requires looking up the property
  definition by string, which is slower than the typed `get_property::<Facing>()` path. This is
  acceptable since string-based queries only happen on command input, not hot paths.

### Neutral

- **u16 is sufficient for current state count**: With ~24,000 states, u16 (max 65,535) has
  headroom. If a future Minecraft version exceeds 65,535 states, we would need u32, doubling
  per-block storage. This is unlikely in the near term.
- **Static table size grows with Minecraft versions**: Each version may add blocks, increasing
  the table. The growth rate (~500 new states per major version) is manageable.

## Compliance

- **ID verification**: Automated test compares every generated state ID against vanilla's
  `blocks.json` data to ensure exact match.
- **Property round-trip**: For every block type, enumerate all property combinations, verify that
  `state.get_property(P)` returns the correct value for each.
- **Transition correctness**: For every block type, verify that `state.with_property(P, V)`
  returns the state with exactly that property changed and all others preserved.
- **Collision shape verification**: Spot-check that collision shapes for known blocks (stairs,
  slabs, fences) match vanilla's shapes with AABB equality within epsilon.
- **Benchmark**: `criterion` benchmark for random block state lookups (property access, flag
  checks, collision shape retrieval) to verify O(1) performance and track regressions.

## Related ADRs

- **ADR-011** (Registry System): Block states are derived from the block registry; state ID
  assignment depends on block registration order.
- **ADR-010** (NBT Library): Chunk NBT contains palette entries that reference block states by
  name + properties; the NBT parser must map these to state IDs efficiently.
- **ADR-014** (Chunk Storage): Chunk sections store block states as u16 IDs in palette-compressed
  format; this ADR defines what those IDs mean.
- **ADR-013** (Coordinate Types): Block positions index into chunk section arrays to retrieve
  block state IDs.
- **ADR-017** (Lighting Engine): Light opacity and emission values come from `BlockStateData`,
  accessed via the lookup table on every lighting calculation.

## References

- [Minecraft Wiki — Block States](https://minecraft.wiki/w/Block_states) — property definitions
- [wiki.vg — Chunk Format](https://wiki.vg/Chunk_Format) — palette encoding in chunk data
- [wiki.vg — Protocol — Block Change](https://wiki.vg/Protocol#Block_Update) — state ID in packets
- [bitflags crate](https://docs.rs/bitflags) — Rust bitfield macro
- [Vanilla blocks.json](https://minecraft.wiki/w/Data_generators) — extracted block state data
