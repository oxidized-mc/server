//! Serverbound status request packet — the client asks for server status.
//!
//! This is an empty packet (no fields). It corresponds to
//! `net.minecraft.network.protocol.status.ServerboundStatusRequestPacket`.

impl_empty_packet!(
    ServerboundStatusRequestPacket,
    0x00,
    "Serverbound packet `0x00` in the STATUS state — requests the server status JSON."
);

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use bytes::Bytes;

    use super::*;
    use crate::codec::Packet;

    #[test]
    fn test_decode_empty_body() {
        let pkt = <ServerboundStatusRequestPacket as Packet>::decode(Bytes::new()).unwrap();
        assert_eq!(pkt, ServerboundStatusRequestPacket);
    }

    #[test]
    fn test_packet_trait_decode() {
        let pkt = <ServerboundStatusRequestPacket as Packet>::decode(Bytes::new()).unwrap();
        assert_eq!(pkt, ServerboundStatusRequestPacket);
    }

    #[test]
    fn test_packet_trait_encode_empty() {
        let pkt = ServerboundStatusRequestPacket;
        let encoded = Packet::encode(&pkt);
        assert!(encoded.is_empty());
    }

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = ServerboundStatusRequestPacket;
        let encoded = Packet::encode(&pkt);
        let decoded = <ServerboundStatusRequestPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(<ServerboundStatusRequestPacket as Packet>::PACKET_ID, 0x00);
    }
}
