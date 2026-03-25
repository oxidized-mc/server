//! Clientbound initialize world border packet.
//!
//! Sends the complete world border state to a joining player.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundInitializeBorderPacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::{types, varint};

/// Initialises the client's world border with full state.
///
/// Wire format:
/// ```text
/// new_center_x: f64 | new_center_z: f64 | old_size: f64 | new_size: f64
/// | lerp_time: VarLong | new_absolute_max_size: VarInt
/// | warning_blocks: VarInt | warning_time: VarInt
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundInitializeBorderPacket {
    /// New center X coordinate.
    pub new_center_x: f64,
    /// New center Z coordinate.
    pub new_center_z: f64,
    /// Old border diameter in blocks.
    pub old_size: f64,
    /// New border diameter in blocks.
    pub new_size: f64,
    /// Time in milliseconds to lerp from old size to new size (0 = instant).
    pub lerp_time: i64,
    /// Maximum border size the server allows.
    pub new_absolute_max_size: i32,
    /// Warning distance in blocks from the border edge.
    pub warning_blocks: i32,
    /// Warning time in seconds before border starts shrinking.
    pub warning_time: i32,
}

/// Writes a VarLong to a buffer (unsigned right-shift to handle negatives).
fn write_varlong(buf: &mut BytesMut, value: i64) {
    let mut v = value as u64;
    loop {
        let byte = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 {
            buf.put_u8(byte);
            break;
        }
        buf.put_u8(byte | 0x80);
    }
}

/// Reads a VarLong from a buffer.
fn read_varlong(data: &mut Bytes) -> Result<i64, PacketDecodeError> {
    let mut result: i64 = 0;
    let mut shift = 0u32;
    loop {
        if !data.has_remaining() {
            return Err(PacketDecodeError::InvalidData(
                "unexpected end of VarLong".into(),
            ));
        }
        let byte = data.get_u8();
        result |= ((byte & 0x7F) as i64) << shift;
        if byte & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift >= 70 {
            return Err(PacketDecodeError::InvalidData(
                "VarLong exceeds 10 bytes".into(),
            ));
        }
    }
}

impl Packet for ClientboundInitializeBorderPacket {
    const PACKET_ID: i32 = 0x2B;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        types::ensure_remaining(&data, 32, "InitializeBorderPacket")?;
        let new_center_x = data.get_f64();
        let new_center_z = data.get_f64();
        let old_size = data.get_f64();
        let new_size = data.get_f64();
        let lerp_time = read_varlong(&mut data)?;
        let new_absolute_max_size = varint::read_varint_buf(&mut data)?;
        let warning_blocks = varint::read_varint_buf(&mut data)?;
        let warning_time = varint::read_varint_buf(&mut data)?;

        Ok(Self {
            new_center_x,
            new_center_z,
            old_size,
            new_size,
            lerp_time,
            new_absolute_max_size,
            warning_blocks,
            warning_time,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(64);
        buf.put_f64(self.new_center_x);
        buf.put_f64(self.new_center_z);
        buf.put_f64(self.old_size);
        buf.put_f64(self.new_size);
        write_varlong(&mut buf, self.lerp_time);
        varint::write_varint_buf(self.new_absolute_max_size, &mut buf);
        varint::write_varint_buf(self.warning_blocks, &mut buf);
        varint::write_varint_buf(self.warning_time, &mut buf);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_defaults() {
        let pkt = ClientboundInitializeBorderPacket {
            new_center_x: 0.0,
            new_center_z: 0.0,
            old_size: 59_999_968.0,
            new_size: 59_999_968.0,
            lerp_time: 0,
            new_absolute_max_size: 29_999_984,
            warning_blocks: 5,
            warning_time: 15,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundInitializeBorderPacket::decode(encoded.freeze()).unwrap();
        assert!((decoded.new_center_x - 0.0).abs() < f64::EPSILON);
        assert!((decoded.new_center_z - 0.0).abs() < f64::EPSILON);
        assert!((decoded.old_size - 59_999_968.0).abs() < f64::EPSILON);
        assert!((decoded.new_size - 59_999_968.0).abs() < f64::EPSILON);
        assert_eq!(decoded.lerp_time, 0);
        assert_eq!(decoded.new_absolute_max_size, 29_999_984);
        assert_eq!(decoded.warning_blocks, 5);
        assert_eq!(decoded.warning_time, 15);
    }

    #[test]
    fn test_roundtrip_shrinking_border() {
        let pkt = ClientboundInitializeBorderPacket {
            new_center_x: 100.5,
            new_center_z: -200.75,
            old_size: 1000.0,
            new_size: 500.0,
            lerp_time: 60_000,
            new_absolute_max_size: 29_999_984,
            warning_blocks: 10,
            warning_time: 30,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundInitializeBorderPacket::decode(encoded.freeze()).unwrap();
        assert!((decoded.new_center_x - 100.5).abs() < f64::EPSILON);
        assert!((decoded.new_center_z - (-200.75)).abs() < f64::EPSILON);
        assert!((decoded.old_size - 1000.0).abs() < f64::EPSILON);
        assert!((decoded.new_size - 500.0).abs() < f64::EPSILON);
        assert_eq!(decoded.lerp_time, 60_000);
        assert_eq!(decoded.warning_blocks, 10);
        assert_eq!(decoded.warning_time, 30);
    }

    #[test]
    fn test_varlong_negative_roundtrip() {
        let pkt = ClientboundInitializeBorderPacket {
            new_center_x: 0.0,
            new_center_z: 0.0,
            old_size: 100.0,
            new_size: 100.0,
            lerp_time: -1, // negative VarLong
            new_absolute_max_size: 29_999_984,
            warning_blocks: 5,
            warning_time: 15,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundInitializeBorderPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.lerp_time, -1);
    }
}
