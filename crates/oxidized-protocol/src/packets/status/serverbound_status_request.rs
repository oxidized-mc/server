//! Serverbound status request packet — the client asks for server status.
//!
//! This is an empty packet (no fields). It corresponds to
//! `net.minecraft.network.protocol.status.ServerboundStatusRequestPacket`.

use bytes::Bytes;

/// Serverbound packet `0x00` in the STATUS state — requests the server status JSON.
///
/// This packet has no fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerboundStatusRequestPacket;

impl ServerboundStatusRequestPacket {
    /// Packet ID in the STATUS state.
    pub const PACKET_ID: i32 = 0x00;

    /// Decodes from raw packet body (expected to be empty).
    pub fn decode(_data: Bytes) -> Self {
        Self
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_empty_body() {
        let pkt = ServerboundStatusRequestPacket::decode(Bytes::new());
        assert_eq!(pkt, ServerboundStatusRequestPacket);
    }
}
