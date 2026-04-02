//! Clientbound set default spawn position packet.
//!
//! Tells the client the world spawn point (compass target).
//!
//! In vanilla 26.1, this sends `LevelData.RespawnData`:
//! `GlobalPos(dimension + BlockPos) + yaw + pitch`.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundSetDefaultSpawnPositionPacket`.

use bytes::{BufMut, Bytes, BytesMut};

use oxidized_codec::types;
use oxidized_mc_types::resource_location::ResourceLocation;

use oxidized_codec::Packet;
use oxidized_codec::packet::PacketDecodeError;

/// Sets the world spawn position (compass target).
///
/// Wire format: `dimension: ResourceLocation | pos: i64 (packed BlockPos) | yaw: f32 | pitch: f32`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundSetDefaultSpawnPositionPacket {
    /// The dimension containing the spawn point.
    pub dimension: ResourceLocation,
    /// Packed block position (see `BlockPos::as_long`).
    pub pos: i64,
    /// Spawn yaw angle.
    pub yaw: f32,
    /// Spawn pitch angle.
    pub pitch: f32,
}

impl Packet for ClientboundSetDefaultSpawnPositionPacket {
    const PACKET_ID: i32 = 0x61;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let dimension = ResourceLocation::read(&mut data)?;
        let pos = types::read_i64(&mut data)?;
        let yaw = types::read_f32(&mut data)?;
        let pitch = types::read_f32(&mut data)?;
        Ok(Self {
            dimension,
            pos,
            yaw,
            pitch,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(32);
        self.dimension.write(&mut buf);
        types::write_i64(&mut buf, self.pos);
        buf.put_f32(self.yaw);
        buf.put_f32(self.pitch);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use oxidized_mc_types::block_pos::BlockPos;

    #[test]
    fn test_roundtrip() {
        let spawn = BlockPos::new(100, 64, -200);
        let pkt = ClientboundSetDefaultSpawnPositionPacket {
            dimension: ResourceLocation::minecraft("overworld"),
            pos: spawn.as_long(),
            yaw: 90.0,
            pitch: 0.0,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundSetDefaultSpawnPositionPacket::decode(encoded.freeze()).unwrap();

        let pos = BlockPos::from_long(decoded.pos);
        assert_eq!(pos.x, 100);
        assert_eq!(pos.y, 64);
        assert_eq!(pos.z, -200);
        assert!((decoded.yaw - 90.0).abs() < f32::EPSILON);
        assert!((decoded.pitch).abs() < f32::EPSILON);
    }
}
