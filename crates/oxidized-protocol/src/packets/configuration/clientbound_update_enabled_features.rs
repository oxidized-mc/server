//! Clientbound update enabled features — the server tells the client which
//! game features are enabled (e.g. vanilla, trade_rebalance).
//!
//! Corresponds to `net.minecraft.network.protocol.configuration.ClientboundUpdateEnabledFeaturesPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::varint;
use crate::types::resource_location::ResourceLocation;

/// Clientbound packet `0x05` in the CONFIGURATION state — update enabled features.
///
/// Sent by the server to inform the client which feature flags are active.
/// Feature flags control experimental and optional game mechanics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundUpdateEnabledFeaturesPacket {
    /// The list of enabled feature identifiers (e.g. `minecraft:vanilla`).
    pub features: Vec<ResourceLocation>,
}

impl Packet for ClientboundUpdateEnabledFeaturesPacket {
    const PACKET_ID: i32 = 0x0c;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let count = varint::read_varint_buf(&mut data)?;
        if count < 0 {
            return Err(PacketDecodeError::InvalidData(format!(
                "negative feature count: {count}"
            )));
        }
        let mut features = Vec::with_capacity(count as usize);
        for _ in 0..count {
            features.push(ResourceLocation::read(&mut data)?);
        }
        Ok(Self { features })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        varint::write_varint_buf(self.features.len() as i32, &mut buf);
        for feature in &self.features {
            feature.write(&mut buf);
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
        let pkt = ClientboundUpdateEnabledFeaturesPacket { features: vec![] };
        let encoded = Packet::encode(&pkt);
        let decoded =
            <ClientboundUpdateEnabledFeaturesPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_single_feature() {
        let pkt = ClientboundUpdateEnabledFeaturesPacket {
            features: vec![ResourceLocation::new("minecraft", "vanilla").unwrap()],
        };
        let encoded = Packet::encode(&pkt);
        let decoded =
            <ClientboundUpdateEnabledFeaturesPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_multiple_features() {
        let pkt = ClientboundUpdateEnabledFeaturesPacket {
            features: vec![
                ResourceLocation::new("minecraft", "vanilla").unwrap(),
                ResourceLocation::new("minecraft", "trade_rebalance").unwrap(),
            ],
        };
        let encoded = Packet::encode(&pkt);
        let decoded =
            <ClientboundUpdateEnabledFeaturesPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_id() {
        assert_eq!(
            <ClientboundUpdateEnabledFeaturesPacket as Packet>::PACKET_ID,
            0x0c
        );
    }
}
