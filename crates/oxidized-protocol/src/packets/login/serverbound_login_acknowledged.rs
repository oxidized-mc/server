//! Serverbound login acknowledged — the client confirms it received the login
//! success and is ready to transition to the CONFIGURATION state.
//!
//! Corresponds to `net.minecraft.network.protocol.login.ServerboundLoginAcknowledgedPacket`.

impl_empty_packet!(
    ServerboundLoginAcknowledgedPacket,
    0x03,
    "Serverbound packet `0x03` in the LOGIN state — login acknowledged."
);

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use oxidized_codec::Packet;

    #[test]
    fn test_roundtrip() {
        let pkt = ServerboundLoginAcknowledgedPacket;
        let encoded = pkt.encode();
        let decoded = ServerboundLoginAcknowledgedPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ServerboundLoginAcknowledgedPacket, 0x03);
    }
}
