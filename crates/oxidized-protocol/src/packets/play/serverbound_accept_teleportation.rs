//! Serverbound accept teleportation packet.
//!
//! Sent by the client to confirm a server-initiated teleport.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ServerboundAcceptTeleportationPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::varint;

/// Client confirmation of a server-initiated teleport.
///
/// Wire format: `teleport_id: VarInt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundAcceptTeleportationPacket {
    /// The teleport ID from the corresponding position packet.
    pub teleport_id: i32,
}

impl ServerboundAcceptTeleportationPacket {
    /// Packet ID in the PLAY state.
    pub const PACKET_ID: i32 = 0x00;

    /// Decodes from the raw packet body.
    pub fn decode(mut data: Bytes) -> Result<Self, varint::VarIntError> {
        let teleport_id = varint::read_varint_buf(&mut data)?;
        Ok(Self { teleport_id })
    }

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(5);
        varint::write_varint_buf(self.teleport_id, &mut buf);
        buf
    }
}

impl Packet for ServerboundAcceptTeleportationPacket {
    const PACKET_ID: i32 = 0x00;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let teleport_id = varint::read_varint_buf(&mut data)?;
        Ok(Self { teleport_id })
    }

    fn encode(&self) -> BytesMut {
        self.encode()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ServerboundAcceptTeleportationPacket { teleport_id: 42 };
        let encoded = pkt.encode();
        let decoded = ServerboundAcceptTeleportationPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.teleport_id, 42);
    }

    #[test]
    fn test_zero_id() {
        let pkt = ServerboundAcceptTeleportationPacket { teleport_id: 0 };
        let encoded = pkt.encode();
        let decoded = ServerboundAcceptTeleportationPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.teleport_id, 0);
    }

    #[test]
    fn test_packet_trait_roundtrip() {
        let pkt = ServerboundAcceptTeleportationPacket { teleport_id: 42 };
        let encoded = Packet::encode(&pkt);
        let decoded =
            <ServerboundAcceptTeleportationPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_trait_id() {
        assert_eq!(
            <ServerboundAcceptTeleportationPacket as Packet>::PACKET_ID,
            0x00
        );
    }
}
