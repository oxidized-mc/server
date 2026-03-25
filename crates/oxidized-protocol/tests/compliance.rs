//! Protocol compliance tests with known test vectors from the Minecraft wiki.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use bytes::Bytes;

use oxidized_protocol::codec::Packet;
use oxidized_protocol::codec::varint::{
    VARINT_MAX_BYTES, VARLONG_MAX_BYTES, decode_varint, decode_varlong, encode_varint,
    encode_varlong,
};
use oxidized_protocol::packets::handshake::{ClientIntent, ClientIntentionPacket};

// ═══════════════════════════════════════════════════════════════════════════
// VarInt test vectors
// ═══════════════════════════════════════════════════════════════════════════

/// (value, expected wire bytes)
const VARINT_VECTORS: &[(i32, &[u8])] = &[
    (0, &[0x00]),
    (1, &[0x01]),
    (2, &[0x02]),
    (127, &[0x7f]),
    (128, &[0x80, 0x01]),
    (255, &[0xff, 0x01]),
    (25565, &[0xdd, 0xc7, 0x01]),
    (2_097_151, &[0xff, 0xff, 0x7f]),
    (2_147_483_647, &[0xff, 0xff, 0xff, 0xff, 0x07]),
    (-1, &[0xff, 0xff, 0xff, 0xff, 0x0f]),
    (-2_147_483_648, &[0x80, 0x80, 0x80, 0x80, 0x08]),
];

#[test]
fn test_varint_encode_vectors() {
    for &(value, expected) in VARINT_VECTORS {
        let mut buf = [0u8; VARINT_MAX_BYTES];
        let len = encode_varint(value, &mut buf);
        assert_eq!(
            &buf[..len],
            expected,
            "encode mismatch for value {value} (0x{value:08x})"
        );
    }
}

#[test]
fn test_varint_decode_vectors() {
    for &(expected_value, bytes) in VARINT_VECTORS {
        let (decoded, consumed) = decode_varint(bytes).unwrap();
        assert_eq!(
            decoded, expected_value,
            "decode mismatch for bytes {bytes:02x?}"
        );
        assert_eq!(
            consumed,
            bytes.len(),
            "consumed byte count mismatch for value {expected_value}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// VarLong test vectors
// ═══════════════════════════════════════════════════════════════════════════

/// (value, expected wire bytes)
const VARLONG_VECTORS: &[(i64, &[u8])] = &[
    (0, &[0x00]),
    (1, &[0x01]),
    (127, &[0x7f]),
    (128, &[0x80, 0x01]),
    (2_147_483_647, &[0xff, 0xff, 0xff, 0xff, 0x07]),
    (
        9_223_372_036_854_775_807,
        &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f],
    ),
    (
        -1,
        &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01],
    ),
    (
        -2_147_483_648,
        &[0x80, 0x80, 0x80, 0x80, 0xf8, 0xff, 0xff, 0xff, 0xff, 0x01],
    ),
    (
        -9_223_372_036_854_775_808,
        &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01],
    ),
];

#[test]
fn test_varlong_encode_vectors() {
    for &(value, expected) in VARLONG_VECTORS {
        let mut buf = [0u8; VARLONG_MAX_BYTES];
        let len = encode_varlong(value, &mut buf);
        assert_eq!(
            &buf[..len],
            expected,
            "encode mismatch for value {value} (0x{value:016x})"
        );
    }
}

#[test]
fn test_varlong_decode_vectors() {
    for &(expected_value, bytes) in VARLONG_VECTORS {
        let (decoded, consumed) = decode_varlong(bytes).unwrap();
        assert_eq!(
            decoded, expected_value,
            "decode mismatch for bytes {bytes:02x?}"
        );
        assert_eq!(
            consumed,
            bytes.len(),
            "consumed byte count mismatch for value {expected_value}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Handshake packet encoding compliance
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_handshake_status_packet_encoding() {
    let pkt = ClientIntentionPacket {
        protocol_version: 775,
        server_address: "localhost".to_string(),
        server_port: 25565,
        next_state: ClientIntent::Status,
    };
    let encoded = pkt.encode();
    let raw = encoded.freeze();

    // Decode the first field (protocol_version) as a VarInt and verify.
    let cursor = Bytes::from(raw.to_vec());
    let (proto, _) = decode_varint(&cursor).unwrap();
    assert_eq!(proto, 775, "protocol version should match");

    // Full roundtrip to verify structural correctness.
    let decoded = ClientIntentionPacket::decode(cursor.clone()).unwrap();
    assert_eq!(decoded.protocol_version, 775);
    assert_eq!(decoded.server_address, "localhost");
    assert_eq!(decoded.server_port, 25565);
    assert_eq!(decoded.next_state, ClientIntent::Status);

    // The last byte of the body should be 0x01 (Status intent).
    assert_eq!(
        *cursor.last().unwrap(),
        0x01,
        "last byte should be Status intent (1)"
    );
}
