//! Clientbound registry data — the server sends a full registry (e.g.
//! dimension types, biomes) to the client during configuration.
//!
//! Corresponds to `net.minecraft.network.protocol.configuration.ClientboundRegistryDataPacket`.

use bytes::{Buf, Bytes, BytesMut};

use oxidized_codec::Packet;
use oxidized_codec::packet::PacketDecodeError;
use oxidized_codec::types;
use oxidized_codec::varint;
use oxidized_mc_types::resource_location::ResourceLocation;

/// A single entry in a registry data packet.
#[derive(Debug, Clone, PartialEq)]
pub struct RegistryEntry {
    /// The entry's identifier (e.g. `minecraft:overworld`).
    pub id: ResourceLocation,
    /// Optional NBT data for this entry. `None` if the client already has the
    /// data via a matching known pack.
    pub data: Option<oxidized_nbt::NbtCompound>,
}

/// Clientbound packet `0x02` in the CONFIGURATION state — registry data.
///
/// Sends a complete registry (e.g. `minecraft:dimension_type`,
/// `minecraft:worldgen/biome`) to the client. Each registry is sent as a
/// separate packet.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundRegistryDataPacket {
    /// The registry identifier (e.g. `minecraft:dimension_type`).
    pub registry: ResourceLocation,
    /// The entries in this registry.
    pub entries: Vec<RegistryEntry>,
}

impl Packet for ClientboundRegistryDataPacket {
    const PACKET_ID: i32 = 0x07;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let registry = ResourceLocation::read(&mut data)?;

        let count = varint::read_varint_buf(&mut data)?;
        if count < 0 {
            return Err(PacketDecodeError::InvalidData(format!(
                "negative entry count: {count}"
            )));
        }

        let mut entries = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let id = ResourceLocation::read(&mut data)?;
            let has_data = types::read_bool(&mut data)?;
            let nbt_data = if has_data {
                let mut reader = data.reader();
                let mut acc = oxidized_nbt::NbtAccounter::default_quota();
                let compound = oxidized_nbt::read_network_nbt(&mut reader, &mut acc)?;
                data = reader.into_inner();
                Some(compound)
            } else {
                None
            };
            entries.push(RegistryEntry { id, data: nbt_data });
        }

        Ok(Self { registry, entries })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        self.registry.write(&mut buf);

        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        varint::write_varint_buf(self.entries.len() as i32, &mut buf);

        for entry in &self.entries {
            entry.id.write(&mut buf);
            types::write_bool(&mut buf, entry.data.is_some());
            if let Some(compound) = &entry.data {
                let mut nbt_buf = Vec::new();
                // NBT write cannot fail for valid compounds written to a Vec.
                #[allow(clippy::expect_used)]
                oxidized_nbt::write_network_nbt(&mut nbt_buf, compound)
                    .expect("NBT write to Vec should not fail");
                buf.extend_from_slice(&nbt_buf);
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
    fn test_roundtrip_empty_entries() {
        assert_packet_roundtrip!(ClientboundRegistryDataPacket {
            registry: ResourceLocation::new("minecraft", "dimension_type").unwrap(),
            entries: vec![],
        });
    }

    #[test]
    fn test_roundtrip_entry_without_data() {
        assert_packet_roundtrip!(ClientboundRegistryDataPacket {
            registry: ResourceLocation::new("minecraft", "worldgen/biome").unwrap(),
            entries: vec![RegistryEntry {
                id: ResourceLocation::new("minecraft", "plains").unwrap(),
                data: None,
            }],
        });
    }

    #[test]
    fn test_roundtrip_entry_with_data() {
        let mut compound = oxidized_nbt::NbtCompound::new();
        compound.put_string("name", "overworld");
        compound.put_int("id", 0);

        let pkt = ClientboundRegistryDataPacket {
            registry: ResourceLocation::new("minecraft", "dimension_type").unwrap(),
            entries: vec![RegistryEntry {
                id: ResourceLocation::new("minecraft", "overworld").unwrap(),
                data: Some(compound),
            }],
        };
        let encoded = Packet::encode(&pkt);
        let decoded = <ClientboundRegistryDataPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_mixed_entries() {
        let mut compound = oxidized_nbt::NbtCompound::new();
        compound.put_byte("has_skylight", 1);

        let pkt = ClientboundRegistryDataPacket {
            registry: ResourceLocation::new("minecraft", "dimension_type").unwrap(),
            entries: vec![
                RegistryEntry {
                    id: ResourceLocation::new("minecraft", "overworld").unwrap(),
                    data: Some(compound),
                },
                RegistryEntry {
                    id: ResourceLocation::new("minecraft", "the_nether").unwrap(),
                    data: None,
                },
                RegistryEntry {
                    id: ResourceLocation::new("minecraft", "the_end").unwrap(),
                    data: None,
                },
            ],
        };
        let encoded = Packet::encode(&pkt);
        let decoded = <ClientboundRegistryDataPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ClientboundRegistryDataPacket, 0x07);
    }
}
