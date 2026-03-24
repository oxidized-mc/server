//! Synchronised entity data system.
//!
//! Mirrors `net.minecraft.network.syncher.SynchedEntityData` and
//! `EntityDataSerializers` from vanilla. Each entity has a set of
//! indexed data slots that are tracked for changes and flushed to
//! clients via [`super::super::net`] packets.

use std::any::Any;

/// Wire type ID for each `EntityDataSerializer` in 26.1-pre-3.
///
/// Registration order from `EntityDataSerializers.java` static block.
/// The integer value is the serializer ID written on the wire.
///
/// # Wire Format
///
/// Each data value on the wire is encoded as:
/// `[u8 slot_id] [VarInt serializer_type] [codec-specific value]`
///
/// The list is terminated by `0xFF`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum DataSerializerType {
    /// `Byte` — single byte.
    Byte = 0,
    /// `Integer` — encoded as VarInt.
    Int = 1,
    /// `Long` — encoded as VarLong.
    Long = 2,
    /// `Float` — 4 bytes IEEE 754.
    Float = 3,
    /// `String` — VarInt-prefixed UTF-8.
    String = 4,
    /// `Component` — chat component (JSON text).
    Component = 5,
    /// `Optional<Component>` — boolean prefix + optional chat component.
    OptionalComponent = 6,
    /// `ItemStack` — full item stack encoding.
    ItemStack = 7,
    /// `Boolean` — single byte (0 or 1).
    Boolean = 8,
    /// `Rotations` — 3 × f32 (pitch, yaw, roll).
    Rotations = 9,
    /// `BlockPos` — packed i64 position.
    BlockPos = 10,
    /// `Optional<BlockPos>` — boolean prefix + optional packed position.
    OptionalBlockPos = 11,
    /// `Direction` — VarInt enum (0=Down, 1=Up, 2=North, 3=South, 4=West, 5=East).
    Direction = 12,
    /// `Optional<EntityReference<LivingEntity>>` — optional entity reference.
    OptionalLivingEntityReference = 13,
    /// `BlockState` — VarInt global block state ID.
    BlockState = 14,
    /// `Optional<BlockState>` — VarInt, 0 = absent.
    OptionalBlockState = 15,
    /// `ParticleOptions` — particle type + options.
    Particle = 16,
    /// `List<ParticleOptions>` — VarInt count + particles.
    Particles = 17,
    /// `VillagerData` — type VarInt + profession VarInt + level VarInt.
    VillagerData = 18,
    /// `OptionalInt` — VarInt, 0 = absent, else value + 1.
    OptionalUnsignedInt = 19,
    /// `Pose` — VarInt enum.
    Pose = 20,
    /// `Holder<CatVariant>` — registry holder reference.
    CatVariant = 21,
    /// `Holder<CatSoundVariant>` — registry holder reference.
    CatSoundVariant = 22,
    /// `Holder<CowVariant>` — registry holder reference.
    CowVariant = 23,
    /// `Holder<CowSoundVariant>` — registry holder reference.
    CowSoundVariant = 24,
    /// `Holder<WolfVariant>` — registry holder reference.
    WolfVariant = 25,
    /// `Holder<WolfSoundVariant>` — registry holder reference.
    WolfSoundVariant = 26,
    /// `Holder<FrogVariant>` — registry holder reference.
    FrogVariant = 27,
    /// `Holder<PigVariant>` — registry holder reference.
    PigVariant = 28,
    /// `Holder<PigSoundVariant>` — registry holder reference.
    PigSoundVariant = 29,
    /// `Holder<ChickenVariant>` — registry holder reference.
    ChickenVariant = 30,
    /// `Holder<ChickenSoundVariant>` — registry holder reference.
    ChickenSoundVariant = 31,
    /// `Holder<ZombieNautilusVariant>` — registry holder reference.
    ZombieNautilusVariant = 32,
    /// `Optional<GlobalPos>` — optional dimension + block position.
    OptionalGlobalPos = 33,
    /// `Holder<PaintingVariant>` — registry holder reference.
    PaintingVariant = 34,
    /// `Sniffer.State` — VarInt enum.
    SnifferState = 35,
    /// `Armadillo.ArmadilloState` — VarInt enum.
    ArmadilloState = 36,
    /// `CopperGolemState` — VarInt enum.
    CopperGolemState = 37,
    /// `WeatheringCopper.WeatherState` — VarInt enum.
    WeatheringCopperState = 38,
    /// `Vector3f` — 3 × f32.
    Vector3f = 39,
    /// `Quaternionf` — 4 × f32.
    Quaternionf = 40,
    /// `ResolvableProfile` — game profile reference.
    ResolvableProfile = 41,
    /// `HumanoidArm` — VarInt enum (0=Left, 1=Right).
    HumanoidArm = 42,
}

impl DataSerializerType {
    /// Total number of registered serializer types.
    pub const COUNT: u32 = 43;

    /// Converts a wire ID to a serializer type, returning `None` for
    /// unknown IDs.
    pub fn from_id(id: u32) -> Option<Self> {
        match id {
            0 => Some(Self::Byte),
            1 => Some(Self::Int),
            2 => Some(Self::Long),
            3 => Some(Self::Float),
            4 => Some(Self::String),
            5 => Some(Self::Component),
            6 => Some(Self::OptionalComponent),
            7 => Some(Self::ItemStack),
            8 => Some(Self::Boolean),
            9 => Some(Self::Rotations),
            10 => Some(Self::BlockPos),
            11 => Some(Self::OptionalBlockPos),
            12 => Some(Self::Direction),
            13 => Some(Self::OptionalLivingEntityReference),
            14 => Some(Self::BlockState),
            15 => Some(Self::OptionalBlockState),
            16 => Some(Self::Particle),
            17 => Some(Self::Particles),
            18 => Some(Self::VillagerData),
            19 => Some(Self::OptionalUnsignedInt),
            20 => Some(Self::Pose),
            21 => Some(Self::CatVariant),
            22 => Some(Self::CatSoundVariant),
            23 => Some(Self::CowVariant),
            24 => Some(Self::CowSoundVariant),
            25 => Some(Self::WolfVariant),
            26 => Some(Self::WolfSoundVariant),
            27 => Some(Self::FrogVariant),
            28 => Some(Self::PigVariant),
            29 => Some(Self::PigSoundVariant),
            30 => Some(Self::ChickenVariant),
            31 => Some(Self::ChickenSoundVariant),
            32 => Some(Self::ZombieNautilusVariant),
            33 => Some(Self::OptionalGlobalPos),
            34 => Some(Self::PaintingVariant),
            35 => Some(Self::SnifferState),
            36 => Some(Self::ArmadilloState),
            37 => Some(Self::CopperGolemState),
            38 => Some(Self::WeatheringCopperState),
            39 => Some(Self::Vector3f),
            40 => Some(Self::Quaternionf),
            41 => Some(Self::ResolvableProfile),
            42 => Some(Self::HumanoidArm),
            _ => None,
        }
    }

    /// Returns the wire ID for this serializer type.
    pub fn id(self) -> u32 {
        self as u32
    }
}

/// A single data slot entry in [`SynchedEntityData`].
pub struct DataItem {
    /// The wire serializer type for this slot.
    pub serializer_type: DataSerializerType,
    /// The current value, type-erased.
    value: Box<dyn Any + Send + Sync>,
    /// Whether this slot has been modified since last pack.
    dirty: bool,
}

/// Runtime store for per-entity synchronised data slots.
///
/// Each entity defines its data slots at construction time via
/// [`define()`](Self::define). Systems modify values via
/// [`set()`](Self::set), which marks slots as dirty. The network sync
/// system calls [`pack_dirty()`](Self::pack_dirty) to collect changed
/// values for transmission.
///
/// Mirrors `SynchedEntityData` in vanilla.
pub struct SynchedEntityData {
    items: Vec<Option<DataItem>>,
    is_dirty: bool,
}

impl SynchedEntityData {
    /// Creates an empty data store.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            is_dirty: false,
        }
    }

    /// Defines a new data slot with the given serializer type and default
    /// value.
    ///
    /// Must be called during entity construction. Slots are indexed by
    /// `slot` (0–254). Defining a slot that already exists overwrites it.
    pub fn define<T: Any + Clone + Send + Sync>(
        &mut self,
        slot: u8,
        serializer: DataSerializerType,
        default: T,
    ) {
        let idx = slot as usize;
        if self.items.len() <= idx {
            self.items.resize_with(idx + 1, || None);
        }
        self.items[idx] = Some(DataItem {
            serializer_type: serializer,
            value: Box::new(default),
            dirty: false,
        });
    }

    /// Gets the value of a data slot.
    ///
    /// # Panics
    ///
    /// Panics if the slot is undefined or the type `T` doesn't match.
    #[allow(clippy::expect_used)]
    pub fn get<T: Any + Clone>(&self, slot: u8) -> T {
        let item = self.items[slot as usize]
            .as_ref()
            .expect("undefined data slot");
        item.value
            .downcast_ref::<T>()
            .expect("type mismatch in SynchedEntityData::get")
            .clone()
    }

    /// Sets a data slot value, marking it dirty if the value changed.
    ///
    /// If the new value equals the current value, the slot is not marked
    /// dirty (avoids unnecessary network traffic).
    ///
    /// # Panics
    ///
    /// Panics if the slot is undefined.
    #[allow(clippy::expect_used)]
    pub fn set<T: Any + PartialEq + Clone + Send + Sync>(&mut self, slot: u8, value: T) {
        let item = self.items[slot as usize]
            .as_mut()
            .expect("undefined data slot");
        if let Some(existing) = item.value.downcast_ref::<T>() {
            if existing == &value {
                return;
            }
        }
        item.value = Box::new(value);
        item.dirty = true;
        self.is_dirty = true;
    }

    /// Returns `true` if any slot has been modified since the last
    /// [`pack_dirty()`](Self::pack_dirty) call.
    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    /// Collects all dirty slots and resets their dirty flags.
    ///
    /// Returns a list of `(slot_index, serializer_type)` pairs for
    /// each modified slot. The caller is responsible for serializing
    /// the actual values.
    pub fn pack_dirty(&mut self) -> Vec<DirtyDataValue<'_>> {
        if !self.is_dirty {
            return Vec::new();
        }
        self.is_dirty = false;
        self.items
            .iter_mut()
            .enumerate()
            .filter_map(|(i, maybe)| {
                let item = maybe.as_mut()?;
                if item.dirty {
                    item.dirty = false;
                    Some(DirtyDataValue {
                        slot: i as u8,
                        serializer_type: item.serializer_type,
                        value: &item.value,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Collects all defined slots (for initial entity spawn).
    ///
    /// Unlike [`Self::pack_dirty`], this returns every defined slot regardless
    /// of dirty state. Used when sending the full entity data on spawn.
    pub fn pack_all(&self) -> Vec<DirtyDataValue<'_>> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(i, maybe)| {
                let item = maybe.as_ref()?;
                Some(DirtyDataValue {
                    slot: i as u8,
                    serializer_type: item.serializer_type,
                    value: &item.value,
                })
            })
            .collect()
    }

    /// Returns the number of defined slots.
    pub fn len(&self) -> usize {
        self.items.iter().filter(|i| i.is_some()).count()
    }

    /// Returns `true` if no slots are defined.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for SynchedEntityData {
    fn default() -> Self {
        Self::new()
    }
}

/// A reference to a dirty data slot value, collected by
/// [`SynchedEntityData::pack_dirty()`].
pub struct DirtyDataValue<'a> {
    /// Slot index (0–254).
    pub slot: u8,
    /// Wire serializer type.
    pub serializer_type: DataSerializerType,
    /// Reference to the current value (type-erased).
    pub value: &'a Box<dyn Any + Send + Sync>,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_serializer_type_from_id_valid() {
        assert_eq!(
            DataSerializerType::from_id(0),
            Some(DataSerializerType::Byte)
        );
        assert_eq!(
            DataSerializerType::from_id(8),
            Some(DataSerializerType::Boolean)
        );
        assert_eq!(
            DataSerializerType::from_id(42),
            Some(DataSerializerType::HumanoidArm)
        );
    }

    #[test]
    fn test_serializer_type_from_id_invalid() {
        assert_eq!(DataSerializerType::from_id(43), None);
        assert_eq!(DataSerializerType::from_id(255), None);
    }

    #[test]
    fn test_serializer_type_roundtrip() {
        for id in 0..DataSerializerType::COUNT {
            let ty = DataSerializerType::from_id(id).unwrap();
            assert_eq!(ty.id(), id);
        }
    }

    #[test]
    fn test_synched_data_bool_roundtrip() {
        let mut data = SynchedEntityData::new();
        data.define(4, DataSerializerType::Boolean, false);
        assert!(!data.get::<bool>(4));
        data.set(4, true);
        assert!(data.get::<bool>(4));
    }

    #[test]
    fn test_synched_data_byte_roundtrip() {
        let mut data = SynchedEntityData::new();
        data.define(0, DataSerializerType::Byte, 0u8);
        assert_eq!(data.get::<u8>(0), 0);
        data.set(0u8, 42u8);
        assert_eq!(data.get::<u8>(0), 42);
    }

    #[test]
    fn test_synched_data_int_roundtrip() {
        let mut data = SynchedEntityData::new();
        data.define(1, DataSerializerType::Int, 300i32);
        assert_eq!(data.get::<i32>(1), 300);
        data.set(1u8, 200i32);
        assert_eq!(data.get::<i32>(1), 200);
    }

    #[test]
    fn test_no_dirty_on_same_value() {
        let mut data = SynchedEntityData::new();
        data.define(4, DataSerializerType::Boolean, false);
        data.set(4u8, false);
        assert!(!data.is_dirty());
    }

    #[test]
    fn test_dirty_on_changed_value() {
        let mut data = SynchedEntityData::new();
        data.define(4, DataSerializerType::Boolean, false);
        data.set(4u8, true);
        assert!(data.is_dirty());
    }

    #[test]
    fn test_pack_dirty_collects_changed() {
        let mut data = SynchedEntityData::new();
        data.define(0, DataSerializerType::Byte, 0u8);
        data.define(1, DataSerializerType::Int, 300i32);
        data.set(0u8, 4u8);
        let dirty = data.pack_dirty();
        assert_eq!(dirty.len(), 1);
        assert_eq!(dirty[0].slot, 0);
        assert_eq!(dirty[0].serializer_type, DataSerializerType::Byte);
    }

    #[test]
    fn test_pack_dirty_resets_flags() {
        let mut data = SynchedEntityData::new();
        data.define(0, DataSerializerType::Byte, 0u8);
        data.set(0u8, 4u8);
        let _ = data.pack_dirty();
        assert!(!data.is_dirty());
        let dirty2 = data.pack_dirty();
        assert!(dirty2.is_empty());
    }

    #[test]
    fn test_pack_all_returns_everything() {
        let mut data = SynchedEntityData::new();
        data.define(0, DataSerializerType::Byte, 0u8);
        data.define(1, DataSerializerType::Int, 300i32);
        data.define(4, DataSerializerType::Boolean, false);
        let all = data.pack_all();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_len_and_is_empty() {
        let mut data = SynchedEntityData::new();
        assert!(data.is_empty());
        assert_eq!(data.len(), 0);
        data.define(0, DataSerializerType::Byte, 0u8);
        assert!(!data.is_empty());
        assert_eq!(data.len(), 1);
    }

    #[test]
    fn test_sparse_slots() {
        let mut data = SynchedEntityData::new();
        data.define(0, DataSerializerType::Byte, 0u8);
        data.define(7, DataSerializerType::Int, 0i32);
        assert_eq!(data.len(), 2);
        assert_eq!(data.get::<i32>(7), 0);
    }
}
