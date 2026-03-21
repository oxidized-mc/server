//! Clientbound select known packs — the server asks the client which data
//! packs it already has cached.
//!
//! Corresponds to `net.minecraft.network.protocol.configuration.ClientboundSelectKnownPacksPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;
use crate::codec::varint;

/// A known data pack identifier used during configuration negotiation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownPack {
    /// The pack's namespace (e.g. `"minecraft"`).
    pub namespace: String,
    /// The pack's identifier (e.g. `"core"`).
    pub id: String,
    /// The pack's version string.
    pub version: String,
}

/// Maximum allowed string length for pack fields.
const MAX_PACK_STRING: usize = 32767;

impl KnownPack {
    /// Reads a `KnownPack` from a wire buffer.
    pub(crate) fn read(buf: &mut Bytes) -> Result<Self, PacketDecodeError> {
        let namespace = types::read_string(buf, MAX_PACK_STRING)?;
        let id = types::read_string(buf, MAX_PACK_STRING)?;
        let version = types::read_string(buf, MAX_PACK_STRING)?;
        Ok(Self {
            namespace,
            id,
            version,
        })
    }

    /// Writes this `KnownPack` to a wire buffer.
    pub(crate) fn write(&self, buf: &mut BytesMut) {
        types::write_string(buf, &self.namespace);
        types::write_string(buf, &self.id);
        types::write_string(buf, &self.version);
    }
}

/// Clientbound packet `0x04` in the CONFIGURATION state — select known packs.
///
/// The server sends a list of known data packs; the client responds with
/// which ones it already has cached via
/// [`ServerboundSelectKnownPacksPacket`](super::ServerboundSelectKnownPacksPacket).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundSelectKnownPacksPacket {
    /// The list of known packs the server is offering.
    pub packs: Vec<KnownPack>,
}

impl Packet for ClientboundSelectKnownPacksPacket {
    const PACKET_ID: i32 = 0x0e;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let count = varint::read_varint_buf(&mut data)?;
        if count < 0 {
            return Err(PacketDecodeError::InvalidData(format!(
                "negative pack count: {count}"
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
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        varint::write_varint_buf(self.packs.len() as i32, &mut buf);
        for pack in &self.packs {
            pack.write(&mut buf);
        }
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_empty() {
        let pkt = ClientboundSelectKnownPacksPacket { packs: vec![] };
        let encoded = Packet::encode(&pkt);
        let decoded =
            <ClientboundSelectKnownPacksPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_single_pack() {
        let pkt = ClientboundSelectKnownPacksPacket {
            packs: vec![KnownPack {
                namespace: "minecraft".to_string(),
                id: "core".to_string(),
                version: "1.21.5".to_string(),
            }],
        };
        let encoded = Packet::encode(&pkt);
        let decoded =
            <ClientboundSelectKnownPacksPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_multiple_packs() {
        let pkt = ClientboundSelectKnownPacksPacket {
            packs: vec![
                KnownPack {
                    namespace: "minecraft".to_string(),
                    id: "core".to_string(),
                    version: "1.21.5".to_string(),
                },
                KnownPack {
                    namespace: "minecraft".to_string(),
                    id: "trade_rebalance".to_string(),
                    version: "1.21.5".to_string(),
                },
            ],
        };
        let encoded = Packet::encode(&pkt);
        let decoded =
            <ClientboundSelectKnownPacksPacket as Packet>::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_id() {
        assert_eq!(
            <ClientboundSelectKnownPacksPacket as Packet>::PACKET_ID,
            0x0e
        );
    }
}
