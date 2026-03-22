//! Serverbound client information packet (play state).
//!
//! Sent when the client changes settings (render distance, language,
//! skin parts, etc.) during the PLAY state. Same wire format as the
//! configuration-state packet but with a different packet ID.
//!
//! Corresponds to `net.minecraft.network.protocol.common.ServerboundClientInformationPacket`
//! in the PLAY protocol.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::packets::configuration::ClientInformation;

/// Serverbound client information packet in PLAY state.
///
/// Wraps [`ClientInformation`] with the play-state packet ID (`0x0E`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundClientInformationPlayPacket {
    /// The client's current settings.
    pub information: ClientInformation,
}

impl Packet for ServerboundClientInformationPlayPacket {
    const PACKET_ID: i32 = 0x0E;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let information = ClientInformation::read(&mut data)?;
        Ok(Self { information })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        self.information.write(&mut buf);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_default() {
        let pkt = ServerboundClientInformationPlayPacket {
            information: ClientInformation::create_default(),
        };
        let encoded = pkt.encode();
        let decoded = ServerboundClientInformationPlayPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_id() {
        assert_eq!(ServerboundClientInformationPlayPacket::PACKET_ID, 0x0E);
    }

    #[test]
    fn test_decode_empty_buffer() {
        let data = Bytes::new();
        assert!(ServerboundClientInformationPlayPacket::decode(data).is_err());
    }
}
