//! Incremental light update for a single chunk column.
//!
//! Sent when block changes cause light to propagate or retract. Uses the
//! same [`LightUpdateData`] encoding as the full chunk packet but without
//! the chunk block data.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundLightUpdatePacket`.

use bytes::{Bytes, BytesMut};

use super::clientbound_level_chunk_with_light::LightUpdateData;
use oxidized_codec::Packet;
use oxidized_codec::packet::PacketDecodeError;
use oxidized_codec::varint;

/// Sends updated sky and/or block light data for a single chunk column.
///
/// Wire format: `chunk_x: VarInt | chunk_z: VarInt | light_data: LightUpdateData`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundLightUpdatePacket {
    /// Chunk X coordinate.
    pub chunk_x: i32,
    /// Chunk Z coordinate.
    pub chunk_z: i32,
    /// Light data for changed sections.
    pub light_data: LightUpdateData,
}

impl Packet for ClientboundLightUpdatePacket {
    const PACKET_ID: i32 = 0x30;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let chunk_x = varint::read_varint_buf(&mut data)?;
        let chunk_z = varint::read_varint_buf(&mut data)?;
        let light_data = LightUpdateData::read_from(&mut data)?;
        Ok(Self {
            chunk_x,
            chunk_z,
            light_data,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(128);
        varint::write_varint_buf(self.chunk_x, &mut buf);
        varint::write_varint_buf(self.chunk_z, &mut buf);
        self.light_data.write_to(&mut buf);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_empty() {
        let pkt = ClientboundLightUpdatePacket {
            chunk_x: 5,
            chunk_z: -3,
            light_data: LightUpdateData::empty(),
        };
        let encoded = pkt.encode();
        let decoded = ClientboundLightUpdatePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_roundtrip_with_data() {
        let pkt = ClientboundLightUpdatePacket {
            chunk_x: 0,
            chunk_z: 0,
            light_data: LightUpdateData {
                sky_y_mask: vec![1],
                block_y_mask: vec![2],
                empty_sky_y_mask: vec![4],
                empty_block_y_mask: vec![8],
                sky_updates: vec![vec![0xAB; 2048]],
                block_updates: vec![vec![0xCD; 2048]],
            },
        };
        let encoded = pkt.encode();
        let decoded = ClientboundLightUpdatePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }
}
