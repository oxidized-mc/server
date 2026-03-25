//! Clientbound select known packs — the server asks the client which data
//! packs it already has cached.
//!
//! Corresponds to `net.minecraft.network.protocol.configuration.ClientboundSelectKnownPacksPacket`.

use bytes::{Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;

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
        let packs = types::read_list(&mut data, KnownPack::read)?;
        Ok(Self { packs })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        types::write_list(&mut buf, &self.packs, |b, p| p.write(b));
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_empty() {
        assert_packet_roundtrip!(ClientboundSelectKnownPacksPacket { packs: vec![] });
    }

    #[test]
    fn test_roundtrip_single_pack() {
        assert_packet_roundtrip!(ClientboundSelectKnownPacksPacket {
            packs: vec![KnownPack {
                namespace: "minecraft".to_string(),
                id: "core".to_string(),
                version: "1.21.5".to_string(),
            }],
        });
    }

    #[test]
    fn test_roundtrip_multiple_packs() {
        assert_packet_roundtrip!(ClientboundSelectKnownPacksPacket {
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
        });
    }

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ClientboundSelectKnownPacksPacket, 0x0e);
    }
}
