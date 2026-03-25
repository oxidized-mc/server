//! Clientbound update enabled features — the server tells the client which
//! game features are enabled (e.g. vanilla, trade_rebalance).
//!
//! Corresponds to `net.minecraft.network.protocol.configuration.ClientboundUpdateEnabledFeaturesPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;
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
        let features =
            types::read_list(&mut data, |d| ResourceLocation::read(d).map_err(Into::into))?;
        Ok(Self { features })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        types::write_list(&mut buf, &self.features, |b, f| f.write(b));
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_empty() {
        assert_packet_roundtrip!(ClientboundUpdateEnabledFeaturesPacket { features: vec![] });
    }

    #[test]
    fn test_roundtrip_single_feature() {
        assert_packet_roundtrip!(ClientboundUpdateEnabledFeaturesPacket {
            features: vec![ResourceLocation::new("minecraft", "vanilla").unwrap()],
        });
    }

    #[test]
    fn test_roundtrip_multiple_features() {
        assert_packet_roundtrip!(ClientboundUpdateEnabledFeaturesPacket {
            features: vec![
                ResourceLocation::new("minecraft", "vanilla").unwrap(),
                ResourceLocation::new("minecraft", "trade_rebalance").unwrap(),
            ],
        });
    }

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ClientboundUpdateEnabledFeaturesPacket, 0x0c);
    }
}
