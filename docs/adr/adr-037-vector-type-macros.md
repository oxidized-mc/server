# ADR-037: Coordinate & Vector Type Macros

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-19 |
| Phases | R1 (Refactoring), P07 (retrofit) |
| Deciders | Oxidized Core Team |

## Context

The `oxidized-protocol/src/types/` module contains 6 coordinate/vector types that share
significant boilerplate:

| Type | File | LOC | Shared patterns |
|------|------|-----|-----------------|
| `Vec3` | vec3.rs | 543 | Add, Sub, Neg, axis accessors, Display |
| `Vec3i` | vec3i.rs | 430 | Add, Sub, Neg, axis accessors, Display, directional |
| `BlockPos` | block_pos.rs | 513 | Directional (above/below/north/south/east/west), axis accessors |
| `SectionPos` | section_pos.rs | 403 | Directional, axis accessors, conversions |
| `ChunkPos` | chunk_pos.rs | 355 | Basic ops |
| `Direction` | direction.rs | 680 | Axis-based dispatch |

The duplicated patterns total ~200 lines across these files:

1. **Operator overloading** (Add, Sub, Neg): Each type implements 3 traits with identical
   delegation to named methods. ~20 lines per type × 2 types (Vec3, Vec3i) = 40 lines.

2. **Directional accessors**: `above()`, `below()`, `north()`, `south()`, `east()`,
   `west()` — identical structure calling `offset()` with hardcoded direction values.
   ~18 lines per type × 3 types (BlockPos, Vec3i, SectionPos) = 54 lines.

3. **Axis accessors**: `get_axis(Axis) -> T` and `with_axis(Axis, T) -> Self` — 3-arm
   match on Axis enum, identical structure. ~12 lines per type × 4 types = 48 lines.

4. **Wire format read/write**: `read(buf: &mut Bytes) -> Result<Self>` and
   `write(&self, buf: &mut BytesMut)` — similar structure. In `codec/types.rs`, 11
   read/write pairs for primitive types total ~150 lines of identical boilerplate.

Additionally, `codec/varint.rs` has `encode_varint`/`encode_varlong` and
`decode_varint`/`decode_varlong` as near-identical function pairs differing only in the
integer type (i32 vs i64) — 67 lines of duplication.

This duplication is a maintenance hazard: a bug fix in one type's `north()` method must be
manually replicated in two other files. It also inflates file sizes, making the types
appear more complex than they are.

## Decision Drivers

- **DRY**: identical logic should be written once and generated for each type
- **Correctness**: a fix to shared logic should automatically apply to all types
- **Readability**: macro invocations should be clear about what they generate
- **Debuggability**: macro-generated code should be inspectable via `cargo expand`
- **No proc-macro overhead**: prefer declarative `macro_rules!` for simple patterns
- **Consistency**: all types in the module should use the same patterns

## Considered Options

### Option 1: No macros — accept the duplication

The duplication is isolated and stable (operator semantics don't change). Explicit code is
easier to read than macro-generated code. However, the duplication will grow as we add more
types (e.g., `EntityPos`, `RegionPos`) in later phases.

### Option 2: Declarative macros (`macro_rules!`) for each pattern

Create 4-5 small `macro_rules!` macros, each handling one pattern (operators, directional
accessors, axis accessors, wire primitives). Each macro is simple (10-20 lines of macro
code) and generates well-understood boilerplate. Invocations are one-liners in each type's
file.

### Option 3: Proc macros in `oxidized-macros`

Create derive macros like `#[derive(VectorOps)]` and `#[derive(Directional)]`. More
powerful but requires compile-time proc-macro infrastructure, is harder to debug, and is
overkill for simple trait impls.

### Option 4: Trait with blanket implementations

Define traits like `VectorOps` with a blanket `impl<T: VectorOps> Add for T`. This is
elegant but runs into Rust's orphan rules — we can't implement `std::ops::Add` via a
blanket impl on types in our crate without a local trait as intermediary, which adds
complexity.

## Decision

**We adopt declarative macros (Option 2)** for all four patterns. The macros live alongside
the types they generate code for, either in a shared `type_macros.rs` or at the top of the
module that uses them.

### Macro Definitions

#### 1. `impl_vector_ops!` — Arithmetic operator traits

```rust
/// Generate Add, Sub, Neg operator trait impls for a vector type.
///
/// Requires the type to have:
/// - `add_vec(self, rhs: Self) -> Self`
/// - `subtract(self, rhs: Self) -> Self`
/// - `negate(self) -> Self` (for Neg)
macro_rules! impl_vector_ops {
    ($type:ty) => {
        impl std::ops::Add for $type {
            type Output = $type;
            fn add(self, rhs: $type) -> $type {
                self.add_vec(rhs)
            }
        }

        impl std::ops::Sub for $type {
            type Output = $type;
            fn sub(self, rhs: $type) -> $type {
                self.subtract(rhs)
            }
        }

        impl std::ops::Neg for $type {
            type Output = $type;
            fn neg(self) -> $type {
                self.negate()
            }
        }
    };
}
```

**Usage**: `impl_vector_ops!(Vec3);` — replaces 18 lines with 1.

#### 2. `impl_directional!` — Cardinal direction accessors

```rust
/// Generate above/below/north/south/east/west offset methods.
///
/// Requires the type to have `offset(dx, dy, dz) -> Self` method
/// where dx/dy/dz are the type's coordinate scalar (i32 or i64).
macro_rules! impl_directional {
    ($type:ty, $scalar:ty) => {
        impl $type {
            /// Returns the position one block above (y + 1).
            pub fn above(self) -> Self { self.offset(0, 1, 0) }
            /// Returns the position one block below (y - 1).
            pub fn below(self) -> Self { self.offset(0, -1, 0) }
            /// Returns the position one block north (z - 1).
            pub fn north(self) -> Self { self.offset(0, 0, -1) }
            /// Returns the position one block south (z + 1).
            pub fn south(self) -> Self { self.offset(0, 0, 1) }
            /// Returns the position one block west (x - 1).
            pub fn west(self) -> Self { self.offset(-1, 0, 0) }
            /// Returns the position one block east (x + 1).
            pub fn east(self) -> Self { self.offset(1, 0, 0) }
        }
    };
}
```

**Usage**: `impl_directional!(BlockPos, i32);` — replaces 18 lines with 1.

#### 3. `impl_axis_accessor!` — Axis-based get/set

```rust
/// Generate get_axis/with_axis methods for a 3D type.
///
/// Requires the type to have `x`, `y`, `z` fields of the given scalar type.
macro_rules! impl_axis_accessor {
    ($type:ty, $scalar:ty) => {
        impl $type {
            /// Get the component along the given axis.
            pub fn get_axis(self, axis: Axis) -> $scalar {
                match axis {
                    Axis::X => self.x,
                    Axis::Y => self.y,
                    Axis::Z => self.z,
                }
            }

            /// Return a copy with the given axis component replaced.
            pub fn with_axis(self, axis: Axis, value: $scalar) -> Self {
                match axis {
                    Axis::X => Self { x: value, ..self },
                    Axis::Y => Self { y: value, ..self },
                    Axis::Z => Self { z: value, ..self },
                }
            }
        }
    };
}
```

**Usage**: `impl_axis_accessor!(Vec3, f64);` — replaces 12 lines with 1.

#### 4. `impl_wire_primitive!` — Protocol read/write pairs

```rust
/// Generate read/write functions for a primitive wire type.
macro_rules! impl_wire_primitive {
    ($name:ident, $type:ty, $get:ident, $put:ident, $size:expr) => {
        pub fn $name(buf: &mut Bytes) -> Result<$type, TypeError> {
            if buf.remaining() < $size {
                return Err(TypeError::InsufficientData {
                    expected: $size,
                    actual: buf.remaining(),
                });
            }
            Ok(buf.$get())
        }

        paste::paste! {
            pub fn [<write_ $name>](buf: &mut BytesMut, value: $type) {
                buf.$put(value);
            }
        }
    };
}
```

**Usage**: Replaces 11 read/write pairs (~150 lines) with 11 one-line invocations.

#### 5. VarInt/VarLong unification

Instead of a macro, use a generic approach with a `VarEncoding` trait:

```rust
pub trait VarEncoding: Sized {
    const MAX_BYTES: usize;
    fn from_u64(v: u64) -> Self;
    fn to_u64(self) -> u64;
}

impl VarEncoding for i32 { /* ... */ }
impl VarEncoding for i64 { /* ... */ }

pub fn encode_var<T: VarEncoding>(value: T, buf: &mut [u8]) -> usize { /* shared */ }
pub fn decode_var<T: VarEncoding>(buf: &[u8]) -> Result<(T, usize), VarIntError> { /* shared */ }
```

This eliminates 67 lines of duplication between encode_varint/encode_varlong and
decode_varint/decode_varlong.

### Macro Placement

Macros are defined in a `type_macros.rs` file within `oxidized-protocol/src/types/` and
imported by each type file via `use super::type_macros::*` or `#[macro_use]`.

For `impl_wire_primitive!`, it lives in `oxidized-protocol/src/codec/` next to `types.rs`.

## Consequences

### Positive

- **DRY**: ~200 lines of duplication eliminated; single source of truth for each pattern
- **Correctness**: a bug fix in a macro automatically fixes all types using it
- **Consistency**: all vector types have identical operator semantics by construction
- **Extensibility**: adding a new coordinate type (e.g., `EntityPos`) requires one macro
  invocation instead of copying 40+ lines of boilerplate
- **Clarity**: macro invocations serve as documentation ("this type supports directional ops")

### Negative

- **Macro complexity**: developers must understand `macro_rules!` syntax to modify shared
  behavior — though the macros are simple (10-20 lines each)
- **IDE support**: some IDEs have limited go-to-definition for macro-generated methods
  (mitigated by `cargo expand`)
- **Error messages**: compile errors in macro-generated code point to the macro definition,
  not the invocation site — though the simple patterns make this rare

### Neutral

- **No runtime cost**: all macros are fully expanded at compile time
- **No new dependencies**: `macro_rules!` is built into Rust; `paste` crate (for
  identifier concatenation) is already in the workspace for other macros
- **The existing named methods (`add_vec`, `subtract`, `offset`) remain** — the macros
  delegate to them, so direct calls still work

## Compliance

- **New vector/coordinate types**: Must use `impl_vector_ops!` and `impl_directional!`
  where applicable, rather than hand-writing operator traits
- **Existing types**: Must be migrated during the R1 refactoring phase
- **Macro modifications**: Changes to macro definitions require running the full test suite
  for all types (`cargo test -p oxidized-protocol`)
- **Documentation**: Each macro must have a doc comment explaining its requirements
  (which methods/fields the type must have)
- **Inspectability**: `cargo expand -p oxidized-protocol` must produce readable expanded code

## Related ADRs

- [ADR-013: Type-Safe Coordinate System](adr-013-coordinate-types.md) — defines the types
  that these macros generate code for
- [ADR-007: Packet Codec Framework](adr-007-packet-codec.md) — wire format read/write is
  part of the codec framework
- [ADR-035: Module Structure & File Size Policy](adr-035-module-structure.md) — macros
  reduce file sizes to stay within the policy guidelines

## References

- [The Little Book of Rust Macros](https://danielkeep.github.io/tlborm/book/)
- [`macro_rules!` reference](https://doc.rust-lang.org/reference/macros-by-example.html)
- [`paste` crate](https://docs.rs/paste/latest/paste/) — for identifier concatenation
- [`cargo expand`](https://github.com/dtolnay/cargo-expand) — for inspecting macro output
