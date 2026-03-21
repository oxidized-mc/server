//! Serverbound finish configuration — the client acknowledges it has received
//! all configuration data and is ready for the PLAY state.
//!
//! Corresponds to `net.minecraft.network.protocol.configuration.ServerboundFinishConfigurationPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;

/// Serverbound packet `0x01` in the CONFIGURATION state — finish configuration.
///
/// This is an empty packet with no fields. The client sends it after receiving
/// [`ClientboundFinishConfigurationPacket`](super::ClientboundFinishConfigurationPacket)
/// to confirm it is ready to transition to the PLAY state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundFinishConfigurationPacket;

impl Packet for ServerboundFinishConfigurationPacket {
    const PACKET_ID: i32 = 0x03;

    fn decode(_data: Bytes) -> Result<Self, PacketDecodeError> {
        Ok(Self)
    }

    fn encode(&self) -> BytesMut {
        BytesMut::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

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
        assert_eq!(
            <ServerboundFinishConfigurationPacket as Packet>::PACKET_ID,
            0x03
        );
    }
}
