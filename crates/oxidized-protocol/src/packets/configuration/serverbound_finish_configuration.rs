//! Serverbound finish configuration — the client acknowledges it has received
//! all configuration data and is ready for the PLAY state.
//!
//! Corresponds to `net.minecraft.network.protocol.configuration.ServerboundFinishConfigurationPacket`.

impl_empty_packet!(
    ServerboundFinishConfigurationPacket,
    0x03,
    "Serverbound packet `0x03` in the CONFIGURATION state — finish configuration."
);

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::codec::Packet;

    #[test]
    fn test_roundtrip() {
        let pkt = ServerboundFinishConfigurationPacket;
        let encoded = Packet::encode(&pkt);
        assert!(encoded.is_empty());
        let decoded =
            <ServerboundFinishConfigurationPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ServerboundFinishConfigurationPacket, 0x03);
    }
}
