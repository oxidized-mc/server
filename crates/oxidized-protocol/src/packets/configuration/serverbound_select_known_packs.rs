//! Serverbound select known packs — the client responds with which data
//! packs it already has cached.
//!
//! Corresponds to `net.minecraft.network.protocol.configuration.ServerboundSelectKnownPacksPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::varint;
use crate::packets::configuration::clientbound_select_known_packs::KnownPack;

/// Maximum number of known packs the client may send.
const MAX_KNOWN_PACKS: i32 = 64;

/// Serverbound packet `0x02` in the CONFIGURATION state — select known packs.
///
/// The client sends back the subset of packs (from the server's offer) that
/// it already has cached. The server can then skip sending full registry data
/// for those packs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundSelectKnownPacksPacket {
    /// The packs the client has cached (max 64).
    pub packs: Vec<KnownPack>,
}

impl Packet for ServerboundSelectKnownPacksPacket {
    const PACKET_ID: i32 = 0x07;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let count = varint::read_varint_buf(&mut data)?;
        if count < 0 {
            return Err(PacketDecodeError::InvalidData(format!(
                "negative pack count: {count}"
            )));
        }
        if count > MAX_KNOWN_PACKS {
            return Err(PacketDecodeError::InvalidData(format!(
                "too many packs: {count} (max 64)"
            )));
        }
        let mut packs = Vec::with_capacity(count as usize);
        for _ in 0..count {
            packs.push(KnownPack::read(&mut data)?);
        }
        Ok(Self { packs })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        crate::codec::types::write_list(&mut buf, &self.packs, |b, p| p.write(b));
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_empty() {
        assert_packet_roundtrip!(ServerboundSelectKnownPacksPacket { packs: vec![] });
    }

    #[test]
    fn test_roundtrip_single_pack() {
        assert_packet_roundtrip!(ServerboundSelectKnownPacksPacket {
            packs: vec![KnownPack {
                namespace: "minecraft".to_string(),
                id: "core".to_string(),
                version: "1.21.5".to_string(),
            }],
        });
    }

    #[test]
    fn test_rejects_too_many_packs() {
        let mut buf = BytesMut::new();
        varint::write_varint_buf(65, &mut buf);
        for _ in 0..65 {
            crate::codec::types::write_string(&mut buf, "minecraft");
            crate::codec::types::write_string(&mut buf, "core");
            crate::codec::types::write_string(&mut buf, "1.0");
        }
        let err = <ServerboundSelectKnownPacksPacket as Packet>::decode(buf.freeze()).unwrap_err();
        assert!(
            matches!(err, PacketDecodeError::InvalidData(ref msg) if msg.contains("too many packs"))
        );
    }

    #[test]
    fn test_accepts_max_packs() {
        let packs: Vec<KnownPack> = (0..64)
            .map(|i| KnownPack {
                namespace: "minecraft".to_string(),
                id: format!("pack{i}"),
                version: "1.0".to_string(),
            })
            .collect();
        let pkt = ServerboundSelectKnownPacksPacket { packs };
        let encoded = Packet::encode(&pkt);
        let decoded =
            <ServerboundSelectKnownPacksPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.packs.len(), 64);
    }

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ServerboundSelectKnownPacksPacket, 0x07);
    }
}
