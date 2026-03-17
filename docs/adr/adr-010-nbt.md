# ADR-010: NBT Library Design

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P05, P10, P20 |
| Deciders | Oxidized Core Team |

## Context

Named Binary Tag (NBT) is Minecraft's universal serialization format, permeating every layer of
the server. Chunk data on disk is NBT. Player inventories, block entity data (chests, signs,
banners), entity state, and even portions of the network protocol carry NBT payloads. Data packs
encode recipes, loot tables, and advancements as JSON that is ultimately converted into NBT-like
structures at runtime. Any server reimplementation must provide a robust, performant NBT library
that handles all of these use cases without becoming a bottleneck.

Vanilla Java's NBT implementation allocates a tree of `Tag` objects — `CompoundTag` wraps a
`HashMap<String, Tag>`, `ListTag` wraps an `ArrayList<Tag>`, and so on. Every field read during
chunk deserialization creates at least one heap object, and compound tags create a full HashMap.
For a high-throughput server that may load hundreds of chunks per second during player movement
or world generation, NBT parsing sits squarely on the hot path. Profiling of vanilla shows that
NBT deserialization accounts for a significant fraction of chunk-load CPU time, with most of the
cost in allocation and HashMap overhead rather than actual I/O.

Rust gives us tools that Java lacks: arena allocators, zero-copy slicing over borrowed byte
buffers, and compile-time serialization via serde. We need a design that leverages these tools
while remaining ergonomic for the many callsites that create, modify, and query NBT structures.
The NBT specification also has quirks — Modified UTF-8 string encoding, strict insertion-order
preservation for compound tags (required by protocol hashing), and a 64 MiB memory accounting
limit — that must be faithfully handled.

## Decision Drivers

- **Performance on the hot path**: Chunk loading deserializes NBT hundreds of times per second;
  parsing must be fast enough that it never dominates chunk-load latency.
- **Mutation ergonomics**: Many systems (block entity updates, player data saves, command
  modifications) need to build or modify NBT trees in place.
- **Spec compliance**: Compound tag ordering must be preserved (protocol hash, snapshot testing).
  Modified UTF-8 encoding must be supported for string tags. Memory accounting must enforce the
  64 MiB limit to prevent malicious payloads from exhausting server memory.
- **Serde integration**: Rust structs should be convertible to/from NBT with minimal boilerplate,
  enabling type-safe access to data that is stored as NBT on disk.
- **Zero-copy where possible**: For read-only paths (chunk loading, packet inspection), avoid
  allocating a full tree when only a few fields are needed.
- **SNBT support**: Commands like `/data merge` use Stringified NBT (SNBT) syntax, requiring a
  parser and formatter.

## Considered Options

### Option 1: Tree-Based Like Vanilla (NbtCompound with HashMap)

A direct port of vanilla's approach: `NbtCompound` wraps a `HashMap<String, NbtTag>`, `NbtList`
wraps a `Vec<NbtTag>`, and each tag variant is a Rust enum. Simple to implement and reason about.
However, HashMap has high per-entry overhead (hashing, bucket pointers, potential resizing), and
every deserialization builds the full tree even when only a few fields are needed. Insertion order
is not preserved by `HashMap`, requiring either a `BTreeMap` (log-time lookup) or `IndexMap`
(extra memory for index). This approach trades performance for simplicity.

### Option 2: Zero-Copy Streaming Parser

A SAX-style parser that walks the NBT byte stream and calls visitor methods for each tag without
building an in-memory tree. Fields are read on demand by seeking to the correct offset. This is
maximally efficient for read-only paths — no allocations at all. However, it makes mutation
impossible (there is no tree to modify), and random access to deeply nested fields requires
re-scanning from the root. Many server systems (block entities, player data) need to modify NBT
and write it back, which this approach cannot support without a separate mutable representation.

### Option 3: Hybrid — Tree for Mutation, Zero-Copy for Read-Only

Provide two representations: a mutable tree (`NbtCompound`, `NbtList`, etc.) for systems that
modify NBT, and a zero-copy borrowed reader (`NbtSlice`) for read-only paths like chunk loading
and packet inspection. The tree representation uses `IndexMap` for ordered compounds. The
borrowed reader references the source `&[u8]` buffer and provides field-access methods that
parse on demand. Conversion between the two is explicit. This captures the performance of
zero-copy parsing on hot paths while retaining full mutation support elsewhere. The cost is two
code paths and potential API confusion.

### Option 4: Arena-Allocated Tree

All tree nodes for a single NBT document are allocated from a bump arena (e.g. `bumpalo`). This
amortizes allocation cost — a single large allocation instead of thousands of small ones. The
tree structure is identical to Option 1, but with dramatically reduced allocator pressure. The
arena is freed in one shot when the NBT document is no longer needed. Downsides: lifetimes are
tied to the arena (cannot move individual tags between documents without cloning), and the arena
may waste memory for small documents.

## Decision

We adopt a **hybrid approach combining a tree-based mutable representation with a zero-copy
borrowed reader, plus arena allocation for bulk-read hot paths**.

The primary mutable representation is a tree of `NbtTag` enum variants:

```rust
pub enum NbtTag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List(NbtList),
    Compound(NbtCompound),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}
```

`NbtCompound` wraps an `IndexMap<String, NbtTag>` to preserve insertion order (required by the
NBT spec and protocol hashing). `IndexMap` provides O(1) amortized lookup by key while
maintaining the order in which keys were inserted, matching vanilla's `LinkedHashMap` behavior.
`NbtList` wraps a `Vec<NbtTag>` with a tag-type invariant enforced at insertion time.

For bulk read-only paths (chunk deserialization, packet NBT inspection), we provide an
arena-allocated variant. A `BumpNbt` type allocates all tags from a `bumpalo::Bump` arena,
reducing thousands of allocations to one. The arena is scoped to the operation — for chunk
loading, one arena per chunk, freed after the chunk's section data has been extracted into the
chunk's native storage format.

Additionally, a `BorrowedNbtCompound<'a>` type provides zero-copy field access over a `&'a [u8]`
buffer. It lazily parses fields on demand without building a full tree, suitable for extracting a
few known fields from a large NBT payload (e.g. reading `DataVersion` from a chunk's root tag
before deciding how to deserialize the rest).

**Serde integration** is a first-class feature. `#[derive(Serialize, Deserialize)]` works on any
Rust struct to convert it to/from `NbtCompound`:

```rust
#[derive(Serialize, Deserialize)]
struct PlayerData {
    #[serde(rename = "Health")]
    health: f32,
    #[serde(rename = "Pos")]
    position: Vec<f64>,
    #[serde(rename = "Inventory")]
    inventory: Vec<InventorySlot>,
}
```

This eliminates manual tag-by-tag extraction and is the recommended API for all typed data access.

**Modified UTF-8** is handled in string serialization/deserialization. Java's Modified UTF-8
differs from standard UTF-8 in its encoding of the null character (0x00 → 0xC0 0x80) and
supplementary characters (surrogate pairs instead of 4-byte sequences). Our `nbt::string` module
provides `encode_modified_utf8` and `decode_modified_utf8` functions used internally by the
parser and serializer.

**Memory accounting** is enforced via `NbtAccounter`. Every deserialization operation takes an
accounter that tracks total bytes consumed (tag headers + payload). The default limit is 64 MiB,
matching vanilla. If the limit is exceeded, deserialization returns `Err(NbtError::SizeLimit)`.
For trusted internal data (e.g. reading our own level.dat), an unlimited accounter can be used.

**SNBT (Stringified NBT)** parsing and formatting are provided for commands. The SNBT parser
handles the full syntax including typed suffixes (`1b`, `1s`, `1L`, `1.0f`, `1.0d`), quoted and
unquoted strings, boolean shorthand, and nested compounds/lists. SNBT output is used for
`/data get` command responses and debug logging.

**Performance targets**: Parsing a typical chunk's NBT payload (~10–20 KB compressed, ~50–100 KB
uncompressed) should complete in under 50 µs using the arena-allocated path. The serde
deserialization path for typed structs should be within 2× of hand-written field extraction.
Benchmarks using `criterion` are included in the `nbt` crate with representative chunk data.

## Consequences

### Positive

- **Hot-path performance**: Arena allocation and zero-copy borrowed reading eliminate per-tag
  heap allocations on the chunk-loading critical path, targeting < 50 µs per chunk NBT parse.
- **Spec compliance**: `IndexMap` preserves compound tag ordering. Modified UTF-8 encoding is
  correct for all string values. `NbtAccounter` prevents memory exhaustion from malicious data.
- **Ergonomic mutation**: The tree-based `NbtCompound`/`NbtList` API allows intuitive in-place
  modification for block entity updates, player data saves, and command operations.
- **Type-safe data access**: Serde integration means most callsites work with typed Rust structs
  rather than stringly-typed tag trees, catching schema mismatches at compile time.
- **SNBT completeness**: Full support for Stringified NBT enables accurate command parsing and
  debug output, matching vanilla behavior.

### Negative

- **Multiple representations**: Three NBT representations (owned tree, arena tree, borrowed
  reader) increase API surface area and require documentation on when to use which.
- **IndexMap overhead**: `IndexMap` uses ~50% more memory per entry than `HashMap` due to the
  insertion-order index. For deeply nested compounds this adds up, though most compounds are
  shallow (< 20 keys).
- **Arena lifetime management**: Arena-allocated NBT cannot outlive its arena. Code that
  accidentally holds a reference past the arena's scope will fail to compile (Rust's borrow
  checker catches this), but the error messages can be confusing for newcomers.

### Neutral

- **Modified UTF-8 is rare in practice**: Almost all Minecraft strings are pure ASCII or BMP
  Unicode. The Modified UTF-8 path is exercised mainly by edge-case player names and custom NBT
  from data packs.
- **SNBT parsing is command-path only**: The SNBT parser is not performance-critical since it
  runs only on player command input, not on the hot chunk-loading path.

## Compliance

- **Unit tests**: Comprehensive round-trip tests for all 12 tag types. Property-based tests
  (proptest) for arbitrary NBT trees: `serialize(deserialize(bytes)) == bytes`.
- **Modified UTF-8 tests**: Specific test cases for null byte encoding, surrogate pair handling,
  and invalid sequences.
- **NbtAccounter tests**: Verify that payloads exceeding 64 MiB are rejected. Verify unlimited
  accounter permits any size.
- **Ordering tests**: Serialize a compound with known key order, deserialize, verify order is
  preserved.
- **Benchmark suite**: `criterion` benchmarks for chunk NBT parsing (arena path), player data
  round-trip (serde path), and SNBT formatting. CI tracks regressions.
- **Fuzz testing**: `cargo-fuzz` target for NBT deserialization with arbitrary byte inputs to
  catch panics and memory safety issues.

## Related ADRs

- **ADR-012** (Block State Representation): Block states in chunk NBT use palette + packed array
  encoding; the NBT library must efficiently parse this.
- **ADR-014** (Chunk Storage): Chunk loading is the primary consumer of NBT deserialization;
  performance requirements flow from chunk load targets.
- **ADR-015** (Disk I/O): NBT compression/decompression interacts with the I/O strategy; the
  NBT library handles raw bytes, the I/O layer handles zlib.
- **ADR-011** (Registry System): Registry entries encoded in NBT (e.g. registry sync packets)
  use the serde integration path.

## References

- [NBT Specification](https://wiki.vg/NBT) — canonical format documentation
- [Modified UTF-8](https://docs.oracle.com/javase/8/docs/api/java/io/DataInput.html#modified-utf-8) — Java's Modified UTF-8 encoding rules
- [SNBT Format](https://minecraft.wiki/w/NBT_format#SNBT_format) — Stringified NBT syntax
- [IndexMap crate](https://docs.rs/indexmap) — insertion-order-preserving hash map
- [bumpalo crate](https://docs.rs/bumpalo) — bump allocation arena for Rust
- [Minecraft Anvil format](https://minecraft.wiki/w/Anvil_file_format) — region file format using NBT
