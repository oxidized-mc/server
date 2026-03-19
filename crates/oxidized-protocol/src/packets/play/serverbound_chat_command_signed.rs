//! ServerboundChatCommandSignedPacket (0x08) — client dispatches a signed command.
//!
//! Sent when the client clicks a `run_command` chat action or types a
//! command that includes signable arguments.  We only extract the
//! command string and skip the signature/last-seen fields because the
//! server does not enforce chat signing.

use bytes::Bytes;

use crate::codec::types;
use crate::packets::play::PlayPacketError;

/// 0x08 — Signed chat command.
///
/// Wire format:
/// ```text
/// String   command
/// Instant  timeStamp      (i64 millis)
/// Long     salt
/// VarInt   signatureCount
///   for each: String argName + 256-byte signature
/// VarInt   lastSeenOffset
/// BitSet   lastSeenAcknowledged (fixed 3 bytes)
/// Byte     checksum
/// ```
///
/// We only need the `command` field — the rest is consumed and discarded.
#[derive(Debug, Clone)]
pub struct ServerboundChatCommandSignedPacket {
    /// The command text without the leading `/`.
    pub command: String,
}

impl ServerboundChatCommandSignedPacket {
    /// Packet ID in the PLAY state serverbound registry.
    pub const PACKET_ID: i32 = 0x08;

    /// Decodes the packet from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer is malformed.
    pub fn decode(mut data: Bytes) -> Result<Self, PlayPacketError> {
        let command = types::read_string(&mut data, 32767)?;
        // Skip remaining fields (timestamp, salt, signatures, last-seen, checksum).
        // We intentionally do not validate them — mirrors vanilla offline-mode behavior
        // where signature enforcement is disabled.
        Ok(Self { command })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use bytes::BufMut;

    use super::*;
    use crate::codec::varint;

    #[test]
    fn test_packet_id() {
        assert_eq!(ServerboundChatCommandSignedPacket::PACKET_ID, 0x08);
    }

    #[test]
    fn test_decode_extracts_command() {
        // Build a minimal signed command packet:
        // command string + timestamp(i64) + salt(i64) + sig_count(varint 0)
        // + offset(varint 0) + acknowledged(3 bytes) + checksum(1 byte)
        let mut buf = bytes::BytesMut::new();
        types::write_string(&mut buf, "help 2");
        buf.put_i64(1234567890); // timestamp
        buf.put_i64(0); // salt
        varint::write_varint_buf(0, &mut buf); // 0 signatures
        varint::write_varint_buf(0, &mut buf); // last seen offset
        buf.put_bytes(0, 3); // acknowledged bitset (3 bytes)
        buf.put_u8(0); // checksum

        let pkt = ServerboundChatCommandSignedPacket::decode(buf.freeze()).unwrap();
        assert_eq!(pkt.command, "help 2");
    }

    #[test]
    fn test_decode_with_signatures() {
        let mut buf = bytes::BytesMut::new();
        types::write_string(&mut buf, "msg Alice hi");
        buf.put_i64(0); // timestamp
        buf.put_i64(0); // salt
        varint::write_varint_buf(1, &mut buf); // 1 signature
        types::write_string(&mut buf, "message"); // arg name
        buf.put_bytes(0xAB, 256); // 256-byte signature
        varint::write_varint_buf(0, &mut buf); // last seen offset
        buf.put_bytes(0, 3); // acknowledged
        buf.put_u8(0); // checksum

        let pkt = ServerboundChatCommandSignedPacket::decode(buf.freeze()).unwrap();
        assert_eq!(pkt.command, "msg Alice hi");
    }
}
