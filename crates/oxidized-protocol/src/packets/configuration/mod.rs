//! Configuration state packets (State 4).
//!
//! These packets are exchanged after login is acknowledged and before play
//! begins. The server sends registry data, known packs, enabled features,
//! tags, and finally a finish-configuration signal.

pub mod clientbound_finish_configuration;
pub mod clientbound_registry_data;
pub mod clientbound_select_known_packs;
pub mod clientbound_update_enabled_features;
pub mod clientbound_update_tags;
pub mod serverbound_client_information;
pub mod serverbound_finish_configuration;
pub mod serverbound_select_known_packs;

pub use clientbound_finish_configuration::ClientboundFinishConfigurationPacket;
pub use clientbound_registry_data::{ClientboundRegistryDataPacket, RegistryEntry};
pub use clientbound_select_known_packs::{ClientboundSelectKnownPacksPacket, KnownPack};
pub use clientbound_update_enabled_features::ClientboundUpdateEnabledFeaturesPacket;
pub use clientbound_update_tags::{ClientboundUpdateTagsPacket, TagEntry, TagRegistry};
pub use serverbound_client_information::{ClientInformation, ServerboundClientInformationPacket};
pub use serverbound_finish_configuration::ServerboundFinishConfigurationPacket;
pub use serverbound_select_known_packs::ServerboundSelectKnownPacksPacket;
