//! Clientbound finish configuration — signals the client to transition from
//! CONFIGURATION state to PLAY state.
//!
//! Corresponds to `net.minecraft.network.protocol.configuration.ClientboundFinishConfigurationPacket`.

impl_empty_packet!(
    ClientboundFinishConfigurationPacket,
    0x03,
    "Clientbound packet `0x03` in the CONFIGURATION state — finish configuration."
);

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use oxidized_codec::Packet;

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundFinishConfigurationPacket;
        let encoded = Packet::encode(&pkt);
        assert!(encoded.is_empty());
        let decoded =
            <ClientboundFinishConfigurationPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ClientboundFinishConfigurationPacket, 0x03);
    }
}
