# ADR-011: Registry & Data-Driven Content System

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P06, P08, P34, P35 |
| Deciders | Oxidized Core Team |

## Context

Minecraft has evolved from a hardcoded game into a heavily data-driven one. As of 1.21+,
enchantments, biomes, damage types, trim materials, wolf variants, painting variants, banner
patterns, recipes, loot tables, advancements, and dozens of other content types are defined in
JSON files that the server loads at startup. The server must also synchronize registry contents
with connecting clients during the CONFIGURATION protocol state — the client needs to know what
biomes, damage types, and other data-driven entries exist so it can render the world correctly.

Vanilla Java uses a `BuiltInRegistries` singleton class containing static `Registry<T>` fields.
Each registry maps `ResourceLocation` keys to objects and assigns integer IDs at registration
time. Some registries (blocks, items, entity types) are hardcoded in Java source code. Others
(biomes, enchantments) have built-in defaults that can be overridden or extended by data packs.
The registration phase happens at startup and the registries are frozen afterward — no new
entries can be added once the server starts ticking. Integer IDs are used in the network protocol
because sending string keys for every block in a chunk would be prohibitively expensive.

For Oxidized, we need registries that are fast to query (used every game tick for block lookups,
entity type checks, biome access), support the data pack override mechanism, correctly assign
protocol-compatible integer IDs, and can be serialized to NBT for client synchronization. The
registry system is foundational — nearly every other system depends on it.

## Decision Drivers

- **Query performance**: Block state lookups (via block registry), item lookups, and entity type
  checks happen millions of times per tick. O(1) by integer ID is mandatory.
- **Protocol compatibility**: Integer IDs sent over the wire must match vanilla's assignment for
  the default data pack configuration. Clients expect specific IDs for blocks, items, etc.
- **Data pack support**: Server operators must be able to add custom biomes, enchantments, damage
  types, etc. via data packs, just like vanilla.
- **Startup performance**: Loading hundreds of JSON files and building registries should complete
  in under 2 seconds for the default vanilla data pack.
- **Type safety**: Each registry should be strongly typed — `Registry<Block>` cannot accidentally
  be queried for an `Item`.
- **Freeze semantics**: After startup, registries must be immutable to enable lock-free
  concurrent access from multiple threads.

## Considered Options

### Option 1: Static Compile-Time Registries

Embed all vanilla JSON data into the binary via `include_bytes!` and generate Rust code at
compile time using `build.rs`. Every registry entry becomes a `const` or `static` value. This
provides the fastest possible startup and query times — everything is baked into the binary. The
major downside is that data packs cannot add new entries; the binary would need recompilation to
support custom content. This is unacceptable for a general-purpose server that operators expect
to customize.

### Option 2: Runtime Registries Loaded From JSON

Load all registries from JSON files at server startup, including core registries like blocks and
items. This maximizes flexibility — every entry can be overridden. However, it means the server
must ship a complete set of vanilla JSON files, parse them all at startup, and cannot benefit
from compile-time optimizations for the thousands of block/item entries that never change. It
also means block and item lookups go through a runtime-built table rather than compile-time
constants, potentially adding indirection on the hottest paths.

### Option 3: Hybrid — Core Compiled, Data-Driven Loaded at Runtime

Core registries that are protocol-critical and never change across data packs (blocks, items,
entity types, block entity types, fluids) are compiled into the binary via `build.rs` codegen.
Data-driven registries that data packs can extend or override (biomes, enchantments, damage
types, loot tables, recipes, advancements, tags) are loaded from JSON at startup. Both share a
common `Registry<T>` trait for uniform access. This provides maximum performance for hot-path
lookups while retaining full data pack flexibility for content that is designed to be customized.

### Option 4: Dynamic Registries With Hot-Reload

Like Option 2 but with support for modifying registries at runtime (e.g. `/reload` reloads data
packs and rebuilds registries without a server restart). This is the most flexible option but
introduces enormous complexity: every system holding a reference to a registry entry must handle
invalidation, integer IDs may change on reload (requiring re-sync with all clients), and
concurrent access during reload requires careful synchronization. Vanilla supports `/reload` for
some data-pack-driven content (recipes, advancements, tags) but not for registry entries that
have been synchronized to clients during CONFIGURATION.

## Decision

We adopt the **hybrid approach**: core registries are compiled into the binary from extracted
vanilla data, and data-driven registries are loaded from JSON at startup with data pack override
support.

### Core (Compiled) Registries

Block, Item, EntityType, BlockEntityType, Fluid, and other protocol-critical registries are
generated at compile time. A `build.rs` script reads extracted vanilla server data (JSON files in
`mc-server-ref/generated/`) and generates Rust source code:

```rust
// Auto-generated by build.rs — do not edit
pub struct Blocks;
impl Blocks {
    pub const AIR: BlockType = BlockType { id: 0, default_state: 0, /* ... */ };
    pub const STONE: BlockType = BlockType { id: 1, default_state: 1, /* ... */ };
    // ... ~1000 block types
}

pub static BLOCK_REGISTRY: Registry<BlockType> = Registry::from_static(&[
    ("minecraft:air", &Blocks::AIR),
    ("minecraft:stone", &Blocks::STONE),
    // ...
]);
```

Integer IDs match vanilla exactly. The generated `Registry` uses a dense `Vec` for ID→entry
lookup and a perfect hash map (PHF) for name→ID lookup at zero runtime cost.

### Data-Driven (Runtime) Registries

Biomes, enchantments, damage types, trim materials, wolf variants, painting variants, banner
patterns, and other data-driven registries are loaded from JSON at startup:

1. **Built-in defaults**: The binary embeds vanilla's default entries (via `include_bytes!` JSON).
2. **Data pack loading**: Data packs are loaded in priority order. Entries from higher-priority
   packs override lower-priority ones. New entries can be added.
3. **Registry freeze**: After all data packs are loaded, the registry is frozen. Integer IDs are
   assigned in a deterministic order (sorted by `ResourceLocation`). The frozen registry is an
   `Arc<FrozenRegistry<T>>` that can be shared across threads without synchronization.

### Common Registry Trait

All registries implement a common interface:

```rust
pub trait RegistryAccess<T> {
    fn get_by_id(&self, id: u32) -> Option<&T>;
    fn get_by_key(&self, key: &ResourceLocation) -> Option<&T>;
    fn id_of(&self, key: &ResourceLocation) -> Option<u32>;
    fn key_of(&self, id: u32) -> Option<&ResourceLocation>;
    fn len(&self) -> usize;
    fn iter(&self) -> impl Iterator<Item = (u32, &ResourceLocation, &T)>;
}
```

`ResourceLocation` is the `namespace:path` key type (e.g. `minecraft:stone`). All lookups by
integer ID are O(1) via direct indexing into a dense `Vec`. Lookups by key are O(1) amortized
via `HashMap` (or PHF for compiled registries).

### Tags

Tags group registry entries (e.g. `#minecraft:logs` includes all log block types). Tags are
loaded from data packs as JSON files listing the entries in each tag. After loading, tags are
resolved to `FrozenSet<u32>` (a sorted, deduplicated list of registry integer IDs) for O(1)
membership testing via binary search or bitset. Tags are sent to clients during CONFIGURATION.

```rust
pub struct Tag {
    key: ResourceLocation,
    entries: FrozenSet<u32>, // sorted IDs for O(log n) contains, or bitset for O(1)
}
```

### Data Pack Loading Order

Data packs are loaded in the order specified by `level.dat`'s `DataPacks.Enabled` list. Within
each pack, files are organized by registry type:

```
data/<namespace>/<registry_path>/<entry>.json
data/<namespace>/tags/<registry_path>/<tag>.json
```

Later packs override earlier packs for the same `ResourceLocation`. The `minecraft` namespace in
the vanilla data pack provides defaults; a custom data pack can override `minecraft:plains` to
change the plains biome.

### Client Synchronization

During the CONFIGURATION protocol state, the server sends registry data to the client via
`RegistryData` packets. Each data-driven registry is serialized to NBT and sent in its entirety.
The client uses this data to configure rendering, particle effects, damage calculations, etc.
Core registries (blocks, items) are not sent — the client has its own copy.

## Consequences

### Positive

- **Maximum hot-path performance**: Block, item, and entity type lookups are direct array indexes
  into compile-time-generated tables. No HashMap lookup, no indirection.
- **Protocol correctness**: Compiled registries use the exact same integer IDs as vanilla,
  ensuring wire compatibility.
- **Full data pack support**: Data-driven registries support the same override mechanism as
  vanilla, enabling server customization without code changes.
- **Thread safety**: Frozen registries are `Arc`-wrapped immutable data structures, safely shared
  across all server threads with zero synchronization overhead.
- **Type safety**: `Registry<Block>` and `Registry<Item>` are distinct types; the compiler
  prevents cross-registry lookups.

### Negative

- **Build complexity**: The `build.rs` codegen step adds complexity to the build process and
  requires extracted vanilla data to be present in the repository (`mc-server-ref/generated/`).
  Updating to a new Minecraft version requires re-extracting this data.
- **Binary size**: Embedding all block, item, and entity type data into the binary increases its
  size. For ~1000 blocks × ~24000 states, the generated data is approximately 2–5 MB — acceptable
  for a server binary.
- **No hot-reload for core registries**: Block and item types cannot be modified without
  recompilation. This is acceptable because vanilla also does not support modifying these at
  runtime.

### Neutral

- **Data pack reload for runtime registries**: Supporting `/reload` for data-driven registries
  (recipes, tags, advancements) is a future enhancement. The current design freezes registries
  at startup; a future version could rebuild and atomically swap `Arc<FrozenRegistry<T>>`.
- **Registry sync packet size**: Sending all data-driven registry entries during CONFIGURATION
  adds to initial connection time. Vanilla has the same cost; we can optimize later with
  delta compression if needed.

## Compliance

- **Protocol verification**: Automated tests compare compiled integer IDs against extracted
  vanilla data to ensure they match exactly.
- **Round-trip tests**: Serialize a registry to client sync format, deserialize, verify all
  entries and IDs are preserved.
- **Data pack override tests**: Load a base pack + override pack, verify that overridden entries
  use the override's data while non-overridden entries retain defaults.
- **Tag resolution tests**: Verify that tag membership queries return correct results after
  loading, including transitive tag includes (`#minecraft:logs` includes `#minecraft:oak_logs`).
- **Freeze semantics tests**: Attempting to register a new entry after freeze panics or returns
  an error.
- **Build.rs output validation**: CI step that regenerates compiled registries and diffs against
  committed output to detect drift.

## Related ADRs

- **ADR-010** (NBT Library): Registry sync packets use NBT serialization; the NBT serde
  integration is used to encode registry entries for client synchronization.
- **ADR-012** (Block State Representation): Block states are the primary consumer of the block
  registry; state IDs are assigned based on block registry order.
- **ADR-014** (Chunk Storage): Chunk sections reference block states by integer ID, which come
  from the block registry's compiled ID assignment.
- **ADR-016** (Worldgen Pipeline): Biome registry entries drive world generation; the data-driven
  biome registry must be loaded before worldgen can start.

## References

- [Minecraft Wiki — Data Pack](https://minecraft.wiki/w/Data_pack) — data pack format and loading
- [wiki.vg — Registry Data](https://wiki.vg/Registry_Data) — client registry sync protocol
- [wiki.vg — Protocol](https://wiki.vg/Protocol) — integer ID usage in packets
- [ResourceLocation](https://minecraft.wiki/w/Resource_location) — namespace:path key format
- [PHF crate](https://docs.rs/phf) — compile-time perfect hash functions for Rust
- [DashMap crate](https://docs.rs/dashmap) — concurrent HashMap for potential runtime use
