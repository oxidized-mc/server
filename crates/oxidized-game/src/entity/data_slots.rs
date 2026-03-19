//! Base entity data slot index constants.
//!
//! These match the `defineId()` calls in `Entity.java` for the base
//! `Entity` class. Subclasses define additional slots starting from
//! higher indices.
//!
//! # Wire Format
//!
//! Each slot is identified by a `u8` index (0–254). Slot 255 (0xFF)
//! is reserved as the end-of-data marker in
//! `ClientboundSetEntityDataPacket`.

use super::synched_data::DataSerializerType;

/// Slot 0: Shared flags byte field.
///
/// | Bit | Meaning |
/// |-----|---------|
/// | 0 | On fire |
/// | 1 | Crouching / sneaking |
/// | 2 | (unused, was riding) |
/// | 3 | Sprinting |
/// | 4 | Swimming |
/// | 5 | Invisible |
/// | 6 | Glowing |
/// | 7 | Fall flying (elytra) |
///
/// Serializer: [`DataSerializerType::Byte`]
pub const DATA_SHARED_FLAGS: u8 = 0;

/// Slot 1: Air supply ticks remaining.
///
/// Maximum is 300 (15 seconds). Decreases while underwater.
///
/// Serializer: [`DataSerializerType::Int`]
pub const DATA_AIR_SUPPLY: u8 = 1;

/// Slot 2: Custom name (optional text component).
///
/// Serializer: [`DataSerializerType::OptionalComponent`]
pub const DATA_CUSTOM_NAME: u8 = 2;

/// Slot 3: Whether the custom name is always visible.
///
/// Serializer: [`DataSerializerType::Boolean`]
pub const DATA_CUSTOM_NAME_VISIBLE: u8 = 3;

/// Slot 4: Whether the entity is silent (no sound).
///
/// Serializer: [`DataSerializerType::Boolean`]
pub const DATA_SILENT: u8 = 4;

/// Slot 5: Whether the entity has no gravity.
///
/// Serializer: [`DataSerializerType::Boolean`]
pub const DATA_NO_GRAVITY: u8 = 5;

/// Slot 6: Entity pose.
///
/// | Value | Pose |
/// |-------|------|
/// | 0 | Standing |
/// | 1 | Fall flying |
/// | 2 | Sleeping |
/// | 3 | Swimming |
/// | 4 | Spin attack |
/// | 5 | Sneaking |
/// | 6 | Long jumping |
/// | 7 | Dying |
/// | 8 | Croaking |
/// | 9 | Using tongue |
/// | 10 | Sitting |
/// | 11 | Roaring |
/// | 12 | Sniffing |
/// | 13 | Emerging |
/// | 14 | Digging |
/// | 15 | Sliding |
/// | 16 | Shooting |
/// | 17 | Inhaling |
///
/// Serializer: [`DataSerializerType::Pose`]
pub const DATA_POSE: u8 = 6;

/// Slot 7: Freeze ticks (used by Powder Snow).
///
/// Serializer: [`DataSerializerType::Int`]
pub const DATA_TICKS_FROZEN: u8 = 7;

// --- Shared flag bit indices ---

/// Bit 0 of [`DATA_SHARED_FLAGS`]: entity is on fire.
pub const FLAG_ON_FIRE: u8 = 0;
/// Bit 1 of [`DATA_SHARED_FLAGS`]: entity is crouching / sneaking.
pub const FLAG_CROUCHING: u8 = 1;
/// Bit 3 of [`DATA_SHARED_FLAGS`]: entity is sprinting.
pub const FLAG_SPRINTING: u8 = 3;
/// Bit 4 of [`DATA_SHARED_FLAGS`]: entity is swimming.
pub const FLAG_SWIMMING: u8 = 4;
/// Bit 5 of [`DATA_SHARED_FLAGS`]: entity is invisible.
pub const FLAG_INVISIBLE: u8 = 5;
/// Bit 6 of [`DATA_SHARED_FLAGS`]: entity is glowing.
pub const FLAG_GLOWING: u8 = 6;
/// Bit 7 of [`DATA_SHARED_FLAGS`]: entity is fall-flying (elytra).
pub const FLAG_FALL_FLYING: u8 = 7;

/// Returns the expected serializer type for each base entity slot.
///
/// Used for validation during entity construction.
pub fn base_entity_serializer(slot: u8) -> Option<DataSerializerType> {
    match slot {
        DATA_SHARED_FLAGS => Some(DataSerializerType::Byte),
        DATA_AIR_SUPPLY => Some(DataSerializerType::Int),
        DATA_CUSTOM_NAME => Some(DataSerializerType::OptionalComponent),
        DATA_CUSTOM_NAME_VISIBLE => Some(DataSerializerType::Boolean),
        DATA_SILENT => Some(DataSerializerType::Boolean),
        DATA_NO_GRAVITY => Some(DataSerializerType::Boolean),
        DATA_POSE => Some(DataSerializerType::Pose),
        DATA_TICKS_FROZEN => Some(DataSerializerType::Int),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_slot_indices_are_sequential() {
        assert_eq!(DATA_SHARED_FLAGS, 0);
        assert_eq!(DATA_AIR_SUPPLY, 1);
        assert_eq!(DATA_CUSTOM_NAME, 2);
        assert_eq!(DATA_CUSTOM_NAME_VISIBLE, 3);
        assert_eq!(DATA_SILENT, 4);
        assert_eq!(DATA_NO_GRAVITY, 5);
        assert_eq!(DATA_POSE, 6);
        assert_eq!(DATA_TICKS_FROZEN, 7);
    }

    #[test]
    fn test_base_entity_serializer_mapping() {
        assert_eq!(
            base_entity_serializer(DATA_SHARED_FLAGS),
            Some(DataSerializerType::Byte)
        );
        assert_eq!(
            base_entity_serializer(DATA_AIR_SUPPLY),
            Some(DataSerializerType::Int)
        );
        assert_eq!(
            base_entity_serializer(DATA_POSE),
            Some(DataSerializerType::Pose)
        );
        assert_eq!(base_entity_serializer(8), None);
    }

    #[test]
    fn test_flag_bits_dont_overlap() {
        let flags = [
            FLAG_ON_FIRE,
            FLAG_CROUCHING,
            FLAG_SPRINTING,
            FLAG_SWIMMING,
            FLAG_INVISIBLE,
            FLAG_GLOWING,
            FLAG_FALL_FLYING,
        ];
        for (i, &a) in flags.iter().enumerate() {
            for &b in &flags[i + 1..] {
                assert_ne!(a, b, "Flag bits must not overlap");
            }
        }
    }
}
