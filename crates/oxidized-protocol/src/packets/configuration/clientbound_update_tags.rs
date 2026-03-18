//! Clientbound update tags — the server sends tag registries to the client.
//! Tags group related registry entries (e.g. "minecraft:logs" contains all
//! log block IDs).
//!
//! This packet is used in both CONFIGURATION and PLAY states.
//!
//! Corresponds to `net.minecraft.network.protocol.common.ClientboundUpdateTagsPacket`.

use bytes::{Bytes, BytesMut};
use thiserror::Error;

use crate::codec::varint::{self, VarIntError};
use crate::types::resource_location::{ResourceLocation, ResourceLocationError};

/// Errors from decoding a [`ClientboundUpdateTagsPacket`].
#[derive(Debug, Error)]
pub enum UpdateTagsError {
    /// VarInt decode failure.
    #[error("varint error: {0}")]
    VarInt(#[from] VarIntError),

    /// Invalid resource location.
    #[error("resource location error: {0}")]
    ResourceLocation(#[from] ResourceLocationError),

    /// Negative count for a collection.
    #[error("negative count: {0}")]
    NegativeCount(i32),
}

/// A single tag entry within a tag registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagEntry {
    /// The tag name (e.g. `minecraft:logs`).
    pub name: ResourceLocation,
    /// Registry element IDs that belong to this tag.
    pub entries: Vec<i32>,
}

/// A tag registry — groups tags under a specific registry type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagRegistry {
    /// The registry identifier (e.g. `minecraft:block`).
    pub registry: ResourceLocation,
    /// The tags in this registry.
    pub tags: Vec<TagEntry>,
}

/// Clientbound update tags packet — sent during CONFIGURATION and PLAY states.
///
/// Contains all tag registries (blocks, items, fluids, etc.) with their
/// tag entries. Each tag maps a name to a list of registry element IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundUpdateTagsPacket {
    /// The tag registries.
    pub tags: Vec<TagRegistry>,
}

impl ClientboundUpdateTagsPacket {
    /// Packet ID in the CONFIGURATION state.
    pub const PACKET_ID: i32 = 0x03;

    /// Decodes from the raw packet body.
    ///
    /// # Errors
    ///
    /// Returns [`UpdateTagsError`] if the buffer is truncated or malformed.
    pub fn decode(mut data: Bytes) -> Result<Self, UpdateTagsError> {
        let registry_count = varint::read_varint_buf(&mut data)?;
        if registry_count < 0 {
            return Err(UpdateTagsError::NegativeCount(registry_count));
        }

        let mut tags = Vec::with_capacity(registry_count as usize);
        for _ in 0..registry_count {
            let registry = ResourceLocation::read(&mut data)?;

            let tag_count = varint::read_varint_buf(&mut data)?;
            if tag_count < 0 {
                return Err(UpdateTagsError::NegativeCount(tag_count));
            }

            let mut tag_entries = Vec::with_capacity(tag_count as usize);
            for _ in 0..tag_count {
                let name = ResourceLocation::read(&mut data)?;

                let entry_count = varint::read_varint_buf(&mut data)?;
                if entry_count < 0 {
                    return Err(UpdateTagsError::NegativeCount(entry_count));
                }

                let mut entries = Vec::with_capacity(entry_count as usize);
                for _ in 0..entry_count {
                    entries.push(varint::read_varint_buf(&mut data)?);
                }

                tag_entries.push(TagEntry { name, entries });
            }

            tags.push(TagRegistry {
                registry,
                tags: tag_entries,
            });
        }

        Ok(Self { tags })
    }

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();

        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        varint::write_varint_buf(self.tags.len() as i32, &mut buf);

        for registry in &self.tags {
            registry.registry.write(&mut buf);

            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            varint::write_varint_buf(registry.tags.len() as i32, &mut buf);

            for tag in &registry.tags {
                tag.name.write(&mut buf);

                #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                varint::write_varint_buf(tag.entries.len() as i32, &mut buf);

                for &entry_id in &tag.entries {
                    varint::write_varint_buf(entry_id, &mut buf);
                }
            }
        }

        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_empty() {
        let pkt = ClientboundUpdateTagsPacket { tags: vec![] };
        let encoded = pkt.encode();
        let decoded = ClientboundUpdateTagsPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_single_registry_empty_tags() {
        let pkt = ClientboundUpdateTagsPacket {
            tags: vec![TagRegistry {
                registry: ResourceLocation::new("minecraft", "block").unwrap(),
                tags: vec![],
            }],
        };
        let encoded = pkt.encode();
        let decoded = ClientboundUpdateTagsPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_with_tag_entries() {
        let pkt = ClientboundUpdateTagsPacket {
            tags: vec![TagRegistry {
                registry: ResourceLocation::new("minecraft", "block").unwrap(),
                tags: vec![
                    TagEntry {
                        name: ResourceLocation::new("minecraft", "logs").unwrap(),
                        entries: vec![10, 11, 12, 13],
                    },
                    TagEntry {
                        name: ResourceLocation::new("minecraft", "planks").unwrap(),
                        entries: vec![20, 21, 22, 23, 24, 25],
                    },
                ],
            }],
        };
        let encoded = pkt.encode();
        let decoded = ClientboundUpdateTagsPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_multiple_registries() {
        let pkt = ClientboundUpdateTagsPacket {
            tags: vec![
                TagRegistry {
                    registry: ResourceLocation::new("minecraft", "block").unwrap(),
                    tags: vec![TagEntry {
                        name: ResourceLocation::new("minecraft", "wool").unwrap(),
                        entries: vec![100, 101],
                    }],
                },
                TagRegistry {
                    registry: ResourceLocation::new("minecraft", "item").unwrap(),
                    tags: vec![TagEntry {
                        name: ResourceLocation::new("minecraft", "wool").unwrap(),
                        entries: vec![200, 201],
                    }],
                },
                TagRegistry {
                    registry: ResourceLocation::new("minecraft", "fluid").unwrap(),
                    tags: vec![TagEntry {
                        name: ResourceLocation::new("minecraft", "water").unwrap(),
                        entries: vec![1, 2],
                    }],
                },
            ],
        };
        let encoded = pkt.encode();
        let decoded = ClientboundUpdateTagsPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_tag_with_empty_entries() {
        let pkt = ClientboundUpdateTagsPacket {
            tags: vec![TagRegistry {
                registry: ResourceLocation::new("minecraft", "block").unwrap(),
                tags: vec![TagEntry {
                    name: ResourceLocation::new("minecraft", "empty_tag").unwrap(),
                    entries: vec![],
                }],
            }],
        };
        let encoded = pkt.encode();
        let decoded = ClientboundUpdateTagsPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }
}
