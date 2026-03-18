//! Clientbound set simulation distance packet.
//!
//! Tells the client the server's simulation distance.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundSetSimulationDistancePacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::varint;

/// Sets the simulation distance for the client.
///
/// Wire format: `simulation_distance: VarInt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundSetSimulationDistancePacket {
    /// Simulation distance in chunks.
    pub simulation_distance: i32,
}

impl ClientboundSetSimulationDistancePacket {
    /// Packet ID in the PLAY state.
    pub const PACKET_ID: i32 = 0x6F; // 111

    /// Decodes from the raw packet body.
    pub fn decode(mut data: Bytes) -> Result<Self, varint::VarIntError> {
        let simulation_distance = varint::read_varint_buf(&mut data)?;
        Ok(Self { simulation_distance })
    }

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(5);
        varint::write_varint_buf(self.simulation_distance, &mut buf);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let pkt = ClientboundSetSimulationDistancePacket { simulation_distance: 10 };
        let encoded = pkt.encode();
        let decoded = ClientboundSetSimulationDistancePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.simulation_distance, 10);
    }
}
