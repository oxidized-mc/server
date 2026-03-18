//! Serverbound select known packs — the client responds with which data
//! packs it already has cached.
//!
//! Corresponds to `net.minecraft.network.protocol.configuration.ServerboundSelectKnownPacksPacket`.

use bytes::{Bytes, BytesMut};
use thiserror::Error;

use crate::codec::varint;
use crate::packets::configuration::clientbound_select_known_packs::{KnownPack, KnownPacksError};

/// Errors from decoding a [`ServerboundSelectKnownPacksPacket`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ServerboundKnownPacksError {
    /// Inner decode failure.
    #[error("{0}")]
    Inner(#[from] KnownPacksError),

    /// Pack count exceeds the maximum of 64.
    #[error("too many packs: {0} (max 64)")]
    TooManyPacks(i32),
}

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

impl ServerboundSelectKnownPacksPacket {
    /// Packet ID in the CONFIGURATION state.
    pub const PACKET_ID: i32 = 0x07;

    /// Decodes from the raw packet body.
    ///
    /// # Errors
    ///
    /// Returns [`ServerboundKnownPacksError`] if the buffer is truncated,
    /// malformed, or the pack count exceeds 64.
    pub fn decode(mut data: Bytes) -> Result<Self, ServerboundKnownPacksError> {
        let count = varint::read_varint_buf(&mut data).map_err(KnownPacksError::from)?;
        if count < 0 {
            return Err(KnownPacksError::NegativeCount(count).into());
        }
        if count > MAX_KNOWN_PACKS {
            return Err(ServerboundKnownPacksError::TooManyPacks(count));
        }
        let mut packs = Vec::with_capacity(count as usize);
        for _ in 0..count {
            packs.push(KnownPack::read(&mut data)?);
        }
        Ok(Self { packs })
    }

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
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
        let pkt = ServerboundSelectKnownPacksPacket { packs: vec![] };
        let encoded = pkt.encode();
        let decoded = ServerboundSelectKnownPacksPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_single_pack() {
        let pkt = ServerboundSelectKnownPacksPacket {
            packs: vec![KnownPack {
                namespace: "minecraft".to_string(),
                id: "core".to_string(),
                version: "1.21.5".to_string(),
            }],
        };
        let encoded = pkt.encode();
        let decoded = ServerboundSelectKnownPacksPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_rejects_too_many_packs() {
        // Encode a packet with 65 packs
        let mut buf = BytesMut::new();
        varint::write_varint_buf(65, &mut buf);
        for _ in 0..65 {
            crate::codec::types::write_string(&mut buf, "minecraft");
            crate::codec::types::write_string(&mut buf, "core");
            crate::codec::types::write_string(&mut buf, "1.0");
        }
        let err = ServerboundSelectKnownPacksPacket::decode(buf.freeze()).unwrap_err();
        assert!(matches!(err, ServerboundKnownPacksError::TooManyPacks(65)));
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
        let encoded = pkt.encode();
        let decoded = ServerboundSelectKnownPacksPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.packs.len(), 64);
    }
}
