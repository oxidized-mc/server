//! Clientbound section blocks update packet.
//!
//! Batch block update for all changed blocks within a single 16×16×16 chunk section.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundSectionBlocksUpdatePacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::varint;
use crate::types::SectionPos;

/// A single block change within a chunk section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionBlockUpdate {
    /// Section-local X (0–15).
    pub local_x: u8,
    /// Section-local Y (0–15).
    pub local_y: u8,
    /// Section-local Z (0–15).
    pub local_z: u8,
    /// New block state ID.
    pub block_state: i32,
}

/// Batch block update for a 16×16×16 chunk section.
///
/// Each entry is encoded as a VarLong:
/// `(block_state_id << 12) | (local_x << 8) | (local_z << 4) | local_y`.
///
/// Wire format: `section_pos: Long | count: VarInt | entries: VarLong[]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundSectionBlocksUpdatePacket {
    /// The section position (chunk section coordinates).
    pub section_pos: SectionPos,
    /// Individual block changes within this section.
    pub updates: Vec<SectionBlockUpdate>,
}

impl Packet for ClientboundSectionBlocksUpdatePacket {
    const PACKET_ID: i32 = 0x54;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        if data.remaining() < 8 {
            return Err(PacketDecodeError::InvalidData(
                "Missing section position".into(),
            ));
        }
        let packed_section = data.get_i64();
        let section_pos = SectionPos::from_long(packed_section);

        let count = varint::read_varint_buf(&mut data)? as usize;
        let mut updates = Vec::with_capacity(count);
        for _ in 0..count {
            let encoded = read_varlong(&mut data)?;
            let block_state = (encoded >> 12) as i32;
            let local_x = ((encoded >> 8) & 0xF) as u8;
            let local_z = ((encoded >> 4) & 0xF) as u8;
            let local_y = (encoded & 0xF) as u8;
            updates.push(SectionBlockUpdate {
                local_x,
                local_y,
                local_z,
                block_state,
            });
        }

        Ok(Self {
            section_pos,
            updates,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(8 + 5 + self.updates.len() * 10);
        buf.put_i64(self.section_pos.as_long());
        varint::write_varint_buf(self.updates.len() as i32, &mut buf);
        for update in &self.updates {
            let encoded = ((update.block_state as i64) << 12)
                | ((update.local_x as i64) << 8)
                | ((update.local_z as i64) << 4)
                | (update.local_y as i64);
            write_varlong(encoded, &mut buf);
        }
        buf
    }
}

/// Reads a VarLong (variable-length i64, up to 10 bytes).
fn read_varlong(buf: &mut Bytes) -> Result<i64, PacketDecodeError> {
    let mut value: i64 = 0;
    let mut shift: u32 = 0;
    loop {
        if buf.remaining() < 1 {
            return Err(PacketDecodeError::InvalidData("VarLong truncated".into()));
        }
        let byte = buf.get_u8();
        value |= ((byte & 0x7F) as i64) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
        if shift >= 70 {
            return Err(PacketDecodeError::InvalidData("VarLong too long".into()));
        }
    }
}

/// Writes a VarLong (variable-length i64).
fn write_varlong(value: i64, buf: &mut BytesMut) {
    let mut uval = value as u64;
    loop {
        if uval & !0x7F == 0 {
            buf.put_u8(uval as u8);
            return;
        }
        buf.put_u8((uval as u8 & 0x7F) | 0x80);
        uval >>= 7;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_single_update() {
        let pkt = ClientboundSectionBlocksUpdatePacket {
            section_pos: SectionPos::new(0, 4, 0),
            updates: vec![SectionBlockUpdate {
                local_x: 5,
                local_y: 10,
                local_z: 3,
                block_state: 1,
            }],
        };
        let encoded = pkt.encode();
        let decoded = ClientboundSectionBlocksUpdatePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.section_pos, pkt.section_pos);
        assert_eq!(decoded.updates.len(), 1);
        assert_eq!(decoded.updates[0].local_x, 5);
        assert_eq!(decoded.updates[0].local_y, 10);
        assert_eq!(decoded.updates[0].local_z, 3);
        assert_eq!(decoded.updates[0].block_state, 1);
    }

    #[test]
    fn test_roundtrip_multiple_updates() {
        let pkt = ClientboundSectionBlocksUpdatePacket {
            section_pos: SectionPos::new(1, 5, -1),
            updates: vec![
                SectionBlockUpdate {
                    local_x: 0,
                    local_y: 0,
                    local_z: 0,
                    block_state: 0,
                },
                SectionBlockUpdate {
                    local_x: 15,
                    local_y: 15,
                    local_z: 15,
                    block_state: 29872,
                },
                SectionBlockUpdate {
                    local_x: 8,
                    local_y: 4,
                    local_z: 12,
                    block_state: 1,
                },
            ],
        };
        let encoded = pkt.encode();
        let decoded = ClientboundSectionBlocksUpdatePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.updates.len(), 3);
        assert_eq!(decoded.updates[1].local_x, 15);
        assert_eq!(decoded.updates[1].block_state, 29872);
    }

    #[test]
    fn test_empty_updates() {
        let pkt = ClientboundSectionBlocksUpdatePacket {
            section_pos: SectionPos::new(0, 0, 0),
            updates: vec![],
        };
        let encoded = pkt.encode();
        let decoded = ClientboundSectionBlocksUpdatePacket::decode(encoded.freeze()).unwrap();
        assert!(decoded.updates.is_empty());
    }
}
