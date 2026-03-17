# ADR-013: Type-Safe Coordinate System

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P07 |
| Deciders | Oxidized Core Team |

## Context

Minecraft uses several distinct coordinate systems that are related but not interchangeable.
Block positions are integer triples `(x, y, z)` where each unit is one block. Chunk positions
are `(x, z)` pairs where each unit is 16 blocks. Section positions are `(x, y, z)` triples
where each unit is a 16×16×16 section within a chunk column. Entity positions are double-
precision floating-point triples `(x, y, z)`. Region file coordinates are `(x, z)` pairs where
each unit is 32 chunks (512 blocks). These coordinate systems are used pervasively throughout
the codebase — block placement, entity movement, chunk loading, lighting, network packets, and
disk I/O all operate on coordinates.

Vanilla Java represents all integer coordinates as raw `int` values and all floating-point
coordinates as raw `double` values. `BlockPos` is a mutable class extending `Vec3i`, but its
coordinates are still just three ints. The network protocol packs block positions into a single
`i64` using a specific bit layout. Vanilla has historically suffered from coordinate confusion
bugs — passing a block X coordinate where a chunk X was expected, or forgetting to account for
the section Y offset. These bugs are subtle because all coordinate types are the same Java type
(`int` or `double`), so the compiler cannot catch them.

Rust's type system, specifically newtype wrappers and the trait system, allows us to make
coordinate confusion a compile-time error rather than a runtime bug. By giving each coordinate
system its own type, we ensure that a `BlockPos` cannot accidentally be used where a `ChunkPos`
is expected. Conversions between coordinate systems become explicit method calls, making the
transformation visible and reviewable in code.

## Decision Drivers

- **Type safety**: The primary goal — make it impossible to pass a block coordinate where a chunk
  coordinate is expected, or vice versa. The compiler should catch these mistakes.
- **Ergonomics**: Coordinate types are used everywhere. The API must be pleasant to use — not so
  verbose that it obscures the logic it appears in.
- **Zero-cost abstraction**: Newtype wrappers must compile down to raw integers/floats with no
  runtime overhead. No boxing, no indirection.
- **Protocol compatibility**: The packed `i64` format for block positions in network packets must
  be supported efficiently.
- **Mathematical operations**: Common operations (add, subtract, offset by direction, distance)
  must be supported on coordinate types.
- **Interoperability**: Conversions between coordinate systems (block → chunk, chunk → region,
  entity position → block position) must be correct and explicit.

## Considered Options

### Option 1: Raw i32/f64 Everywhere (Like Vanilla)

Use plain `i32` for integer coordinates and `f64` for entity positions. Functions take `(x, y, z)`
tuples. This is maximally simple and has zero API overhead, but provides no type safety. A
function `fn load_chunk(x: i32, z: i32)` can be called with block coordinates, and the compiler
will not complain. This is exactly the approach that causes coordinate confusion bugs. Rejected
because type safety is a primary driver.

### Option 2: Newtype Wrappers

Define distinct types for each coordinate system: `BlockPos`, `ChunkPos`, `SectionPos`, `Vec3`,
`RegionPos`. Each is a simple tuple struct wrapping the appropriate primitive types. Conversions
are explicit methods. Arithmetic operations are implemented via `std::ops` traits. This provides
full type safety with zero runtime cost (newtypes are erased at compilation). The downside is
verbosity — converting between types requires method calls — but this is a feature, not a bug:
it makes coordinate transformations visible.

### Option 3: Generic Coordinate With Phantom Type Marker

Define a single `Coord<T, S>` type where `T` is the scalar type (`i32` or `f64`) and `S` is a
phantom type marker (`struct Block; struct Chunk; struct Section;`). This reduces code
duplication (one impl block for all coordinate types) but obscures the different dimensionalities
— `BlockPos` is 3D, `ChunkPos` is 2D, `RegionPos` is 2D. Encoding dimensionality in a generic
framework adds complexity without clear benefit over dedicated types. The type signatures also
become noisy: `Coord<i32, Block>` vs. the cleaner `BlockPos`.

### Option 4: Unit-Tagged Dimensions (uom-style)

Use a dimensional analysis library to tag each coordinate axis with its unit (blocks, chunks,
sections, etc.). This is the most theoretically rigorous approach — the type system encodes not
just "this is a coordinate" but "this is a distance in chunks along the X axis". However, it
introduces a heavy type-level machinery that is overkill for a coordinate system with only five
unit types. The learning curve is steep, error messages are cryptic, and compile times increase.
The practical benefit over simpler newtypes is minimal.

## Decision

We adopt **newtype wrappers** for all coordinate systems. Each coordinate type is a distinct
Rust struct with explicit conversion methods between types.

### Core Types

```rust
/// A block position in world space. Y range: -2048..2047 (vanilla: -64..319 for overworld).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// A chunk column position (X, Z). Each chunk is 16×16 blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

/// A chunk section position (X, Y, Z). Each section is 16×16×16 blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SectionPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// An entity position in world space (double precision).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// A region file position (X, Z). Each region is 32×32 chunks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RegionPos {
    pub x: i32,
    pub z: i32,
}
```

Note that `ChunkPos` and `RegionPos` are 2D `(x, z)` — there is no Y component because chunks
and regions span the full vertical extent of the world. `SectionPos` is 3D because sections are
the sub-divisions of a chunk column along the Y axis.

### Conversions

Conversions between coordinate systems are explicit methods, not `From` trait implementations
(except for lossless, unambiguous conversions):

```rust
impl BlockPos {
    /// Get the chunk position containing this block.
    #[inline]
    pub fn chunk_pos(self) -> ChunkPos {
        ChunkPos {
            x: self.x >> 4,
            z: self.z >> 4,
        }
    }

    /// Get the section position containing this block.
    #[inline]
    pub fn section_pos(self) -> SectionPos {
        SectionPos {
            x: self.x >> 4,
            y: self.y >> 4,
            z: self.z >> 4,
        }
    }

    /// Get the position within the containing chunk section (0..16 for each axis).
    #[inline]
    pub fn chunk_local(self) -> (u8, u8, u8) {
        (
            (self.x & 0xF) as u8,
            (self.y & 0xF) as u8,
            (self.z & 0xF) as u8,
        )
    }

    /// Offset this position by a direction and distance.
    #[inline]
    pub fn offset(self, dir: Direction, distance: i32) -> BlockPos {
        BlockPos {
            x: self.x + dir.step_x() * distance,
            y: self.y + dir.step_y() * distance,
            z: self.z + dir.step_z() * distance,
        }
    }
}

impl ChunkPos {
    /// Get the region position containing this chunk.
    #[inline]
    pub fn region_pos(self) -> RegionPos {
        RegionPos {
            x: self.x >> 5,
            z: self.z >> 5,
        }
    }

    /// Get the position within the containing region (0..32 for each axis).
    #[inline]
    pub fn region_local(self) -> (u8, u8) {
        ((self.x & 0x1F) as u8, (self.z & 0x1F) as u8)
    }

    /// Get the block position of the chunk's minimum corner (lowest X, Z).
    #[inline]
    pub fn block_min(self) -> BlockPos {
        BlockPos { x: self.x << 4, y: 0, z: self.z << 4 }
    }
}

impl Vec3 {
    /// Truncate to block position (floor toward negative infinity).
    #[inline]
    pub fn block_pos(self) -> BlockPos {
        BlockPos {
            x: self.x.floor() as i32,
            y: self.y.floor() as i32,
            z: self.z.floor() as i32,
        }
    }
}
```

`From` traits are implemented only for safe, lossless, unambiguous conversions:

```rust
// BlockPos → SectionPos is lossless and unambiguous
impl From<BlockPos> for SectionPos { ... }

// Vec3 → BlockPos is NOT From because it's lossy (truncation)
// Use vec3.block_pos() instead
```

### Packed i64 Block Position (Network Protocol)

The network protocol packs block positions into a single `i64` with a specific bit layout:

```
Bits: [63..38] X (26 bits, signed) | [37..26] Z (26 bits, signed) | [25..0] Y (12 bits, signed)
```

Note the non-obvious ordering: X, then Z, then Y (not X, Y, Z).

```rust
impl BlockPos {
    /// Pack into the network protocol's i64 format.
    #[inline]
    pub fn to_packed(self) -> i64 {
        ((self.x as i64 & 0x3FFFFFF) << 38)
            | ((self.z as i64 & 0x3FFFFFF) << 12)
            | (self.y as i64 & 0xFFF)
    }

    /// Unpack from the network protocol's i64 format.
    #[inline]
    pub fn from_packed(packed: i64) -> BlockPos {
        let x = (packed >> 38) as i32;
        let z = ((packed >> 12) & 0x3FFFFFF) as i32;
        // Sign-extend the 12-bit Y value
        let y = ((packed << 52) >> 52) as i32;
        BlockPos { x, y, z }
    }
}
```

These methods are not `From`/`Into` implementations because the packed format is a protocol
concern, not a general-purpose conversion. Callsites must explicitly opt into packing.

### Section Index Math

In the overworld, Y ranges from -64 to 319, spanning sections -4 to 19 (24 sections). The
section index within a chunk column is:

```rust
impl SectionPos {
    /// Index of this section within a chunk column (0-based).
    /// For overworld: section Y=-4 → index 0, section Y=19 → index 23.
    #[inline]
    pub fn section_index(self, min_section_y: i32) -> usize {
        (self.y - min_section_y) as usize
    }
}

impl BlockPos {
    /// Section index within a chunk column for this block's Y coordinate.
    #[inline]
    pub fn section_index(self, min_section_y: i32) -> usize {
        ((self.y >> 4) - min_section_y) as usize
    }
}
```

The `min_section_y` parameter avoids hardcoding the overworld's `-4` — the Nether has different
limits, and custom dimensions may vary.

### AABB (Axis-Aligned Bounding Box)

Entity physics uses AABBs for collision detection:

```rust
/// An axis-aligned bounding box defined by two corners.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AABB {
    pub min_x: f64,
    pub min_y: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub max_z: f64,
}

impl AABB {
    pub fn new(min: Vec3, max: Vec3) -> Self { ... }
    pub fn intersects(&self, other: &AABB) -> bool { ... }
    pub fn contains(&self, point: Vec3) -> bool { ... }
    pub fn expand(&self, amount: Vec3) -> AABB { ... }
    pub fn inflate(&self, amount: f64) -> AABB { ... }
    pub fn move_by(&self, delta: Vec3) -> AABB { ... }
    pub fn center(&self) -> Vec3 { ... }
}
```

### Direction Enum

Cardinal and vertical directions are a first-class type:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    Down,   // -Y
    Up,     // +Y
    North,  // -Z
    South,  // +Z
    West,   // -X
    East,   // +X
}

impl Direction {
    pub fn step_x(self) -> i32 { ... }
    pub fn step_y(self) -> i32 { ... }
    pub fn step_z(self) -> i32 { ... }
    pub fn opposite(self) -> Direction { ... }
    pub fn axis(self) -> Axis { ... }
}
```

## Consequences

### Positive

- **Compile-time coordinate safety**: Passing a `BlockPos` where a `ChunkPos` is expected is a
  type error. This eliminates an entire class of bugs that vanilla has historically suffered from.
- **Self-documenting code**: Function signatures like `fn get_block(&self, pos: BlockPos)` and
  `fn load_chunk(&self, pos: ChunkPos)` make coordinate expectations explicit without comments.
- **Zero runtime cost**: All newtype wrappers are `#[repr(transparent)]` or simple structs that
  compile down to raw integers/floats. No boxing, no indirection, no allocation.
- **Explicit conversions**: `block_pos.chunk_pos()` is visible and searchable in code review.
  Coordinate transformations are never accidental.
- **Packed format is opt-in**: The `to_packed()`/`from_packed()` methods for network protocol
  encoding are explicit, preventing accidental use of packed format where unpacked is expected.

### Negative

- **Verbosity**: Converting between coordinate types requires method calls. Code that does a lot
  of coordinate math may feel heavier than raw integers. Mitigation: helper methods and operator
  overloading keep common patterns concise.
- **Learning curve**: New contributors must learn which coordinate type to use in each context.
  Mitigation: the type system guides them — the compiler will reject incorrect usage.

### Neutral

- **No generic over coordinate types**: We chose dedicated types over a generic `Coord<S>`. This
  means some code duplication (e.g. `Debug` and `Display` impls for each type), but the types
  are simple enough that this is minimal.
- **AABB uses f64 only**: There is no integer AABB type. Block-level bounding boxes are computed
  by converting to `AABB` with `.0` and `.0 + 1.0` for each axis. This matches vanilla's
  approach and avoids a second AABB type.

## Compliance

- **Conversion correctness tests**: For a range of coordinates, verify that
  `block.chunk_pos().block_min()` returns the chunk's minimum corner, and that
  `chunk.region_pos().region_local()` gives the correct offset within the region.
- **Packed round-trip tests**: For all valid Y values (-2048..2047) and a range of X/Z values,
  verify that `BlockPos::from_packed(pos.to_packed()) == pos`.
- **Section index tests**: Verify that `BlockPos::new(0, -64, 0).section_index(-4) == 0` and
  `BlockPos::new(0, 319, 0).section_index(-4) == 23` for the overworld.
- **Type safety enforcement**: Attempt to compile code that passes wrong coordinate types; verify
  it fails. (Documented as examples in unit test comments.)
- **AABB tests**: Verify intersection, containment, expansion, and movement with known inputs.

## Related ADRs

- **ADR-012** (Block State Representation): Block state lookups in chunk sections use chunk-local
  coordinates derived from `BlockPos.chunk_local()`.
- **ADR-014** (Chunk Storage): Chunk map is keyed by `ChunkPos`; section arrays are indexed by
  section index derived from `SectionPos`.
- **ADR-017** (Lighting Engine): Light propagation uses `BlockPos` offsets in all six directions
  via `Direction` and `BlockPos::offset()`.
- **ADR-015** (Disk I/O): Region files are located by `RegionPos` derived from `ChunkPos`.

## References

- [wiki.vg — Position](https://wiki.vg/Protocol#Position) — packed i64 bit layout
- [Minecraft Wiki — Coordinates](https://minecraft.wiki/w/Coordinates) — coordinate systems
- [Minecraft Wiki — Chunk Format](https://minecraft.wiki/w/Chunk_format) — section indexing
- [Minecraft Wiki — Region File Format](https://minecraft.wiki/w/Region_file_format) — region coordinates
- [Rust newtype pattern](https://doc.rust-lang.org/rust-by-example/generics/new_types.html)
