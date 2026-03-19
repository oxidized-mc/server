//! Declarative macros to eliminate boilerplate across vector and coordinate types.
//!
//! See [ADR-037](../../../../docs/adr/adr-037-vector-type-macros.md) for design rationale.

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
