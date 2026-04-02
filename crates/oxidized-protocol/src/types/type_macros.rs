//! Declarative macros to eliminate boilerplate across vector and coordinate types.
//!
//! See ADR-037 (Vector Type Macros) for design rationale.

/// Generate `Add`, `Sub`, and optionally `Neg` operator trait impls for a vector type.
///
/// Requires the type to have:
/// - `add_vec(self, rhs: Self) -> Self`
/// - `subtract_vec(self, rhs: Self) -> Self`
/// - `negate(self) -> Self` (only for the full form)
///
/// # Usage
///
/// ```ignore
/// impl_vector_ops!(Vec3);              // Add + Sub + Neg
/// impl_vector_ops!(Vec3i, no_neg);     // Add + Sub only
/// ```
macro_rules! impl_vector_ops {
    ($type:ty) => {
        impl_vector_ops!($type, no_neg);

        impl std::ops::Neg for $type {
            type Output = $type;

            fn neg(self) -> $type {
                self.negate()
            }
        }
    };
    ($type:ty, no_neg) => {
        impl std::ops::Add for $type {
            type Output = $type;

            fn add(self, rhs: $type) -> $type {
                self.add_vec(rhs)
            }
        }

        impl std::ops::Sub for $type {
            type Output = $type;

            fn sub(self, rhs: $type) -> $type {
                self.subtract_vec(rhs)
            }
        }
    };
}

/// Generate cardinal direction offset methods for a 3D position type.
///
/// Requires the type to have a `const fn offset(self, dx: i32, dy: i32, dz: i32) -> Self`
/// method. Generates `above`, `below`, `north`, `south`, `east`, and `west` as `const fn`.
macro_rules! impl_directional {
    ($type:ty) => {
        impl $type {
            /// Returns the position one block above (positive Y).
            pub const fn above(self) -> Self {
                self.offset(0, 1, 0)
            }

            /// Returns the position one block below (negative Y).
            pub const fn below(self) -> Self {
                self.offset(0, -1, 0)
            }

            /// Returns the position one block to the north (negative Z).
            pub const fn north(self) -> Self {
                self.offset(0, 0, -1)
            }

            /// Returns the position one block to the south (positive Z).
            pub const fn south(self) -> Self {
                self.offset(0, 0, 1)
            }

            /// Returns the position one block to the west (negative X).
            pub const fn west(self) -> Self {
                self.offset(-1, 0, 0)
            }

            /// Returns the position one block to the east (positive X).
            pub const fn east(self) -> Self {
                self.offset(1, 0, 0)
            }
        }
    };
}

/// Generate `get_axis` and `with_axis` methods for a 3D type with `x`, `y`, `z` fields.
///
/// Requires the type to have public `x`, `y`, `z` fields of the given scalar type
/// and to implement `Copy`.
macro_rules! impl_axis_accessor {
    ($type:ty, $scalar:ty) => {
        impl $type {
            /// Returns the component along the given axis.
            pub fn get_axis(self, axis: Axis) -> $scalar {
                match axis {
                    Axis::X => self.x,
                    Axis::Y => self.y,
                    Axis::Z => self.z,
                }
            }

            /// Returns a copy with the given axis component replaced.
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

/// Generate common boilerplate for protocol enums encoded as VarInt on the wire.
///
/// Generates: `id()`, `name()`, `by_id()`, `by_name()`, `read()`, `write()`, and `Display`.
/// Enum must use `#[repr(i32)]`. Unique methods (e.g., `GameType::is_creative()`) should
/// be added in a separate `impl` block.
///
/// # Usage
///
/// ```ignore
/// impl_protocol_enum! {
///     Difficulty {
///         Peaceful = 0 => "peaceful",
///         Easy     = 1 => "easy",
///         Normal   = 2 => "normal",
///         Hard     = 3 => "hard",
///     }
/// }
/// ```
macro_rules! impl_protocol_enum {
    ($enum_ty:ident { $($variant:ident = $id:literal => $name:literal),+ $(,)? }) => {
        impl $enum_ty {
            /// Returns the numeric ID of this variant.
            pub const fn id(self) -> i32 {
                self as i32
            }

            /// Returns the lowercase name of this variant.
            pub const fn name(self) -> &'static str {
                match self {
                    $( $enum_ty::$variant => $name, )+
                }
            }

            /// Looks up a variant by numeric ID.
            pub const fn by_id(id: i32) -> Option<$enum_ty> {
                match id {
                    $( $id => Some($enum_ty::$variant), )+
                    _ => None,
                }
            }

            /// Looks up a variant by lowercase name.
            pub fn by_name(name: &str) -> Option<$enum_ty> {
                match name {
                    $( $name => Some($enum_ty::$variant), )+
                    _ => None,
                }
            }

            /// Reads this type from a wire buffer as a VarInt.
            ///
            /// # Errors
            ///
            /// Returns [`TypeError`](crate::codec::types::TypeError) if the buffer is truncated or the value
            /// is out of range.
            pub fn read(buf: &mut ::bytes::Bytes) -> Result<Self, $crate::codec::types::TypeError> {
                let id = $crate::codec::varint::read_varint_buf(buf)?;
                $enum_ty::by_id(id).ok_or($crate::codec::types::TypeError::UnexpectedEof {
                    need: 1,
                    have: 0,
                })
            }

            /// Writes this type to a wire buffer as a VarInt.
            pub fn write(&self, buf: &mut ::bytes::BytesMut) {
                $crate::codec::varint::write_varint_buf(self.id(), buf);
            }
        }

        impl ::std::fmt::Display for $enum_ty {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.name())
            }
        }
    };
}
