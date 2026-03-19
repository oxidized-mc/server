//! Integration tests for the packet encode/decode pipeline.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use bytes::Bytes;
use uuid::Uuid;

use oxidized_protocol::codec::types;
use oxidized_protocol::packets::handshake::{ClientIntent, ClientIntentionPacket};
use oxidized_protocol::packets::login::clientbound_login_finished::ProfileProperty;
use oxidized_protocol::packets::login::{
    ClientboundHelloPacket, ClientboundLoginCompressionPacket, ClientboundLoginFinishedPacket,
    ServerboundHelloPacket, ServerboundKeyPacket,
};
use oxidized_protocol::packets::status::{
    ClientboundStatusResponsePacket, ServerboundPingRequestPacket,
};

// ---------------------------------------------------------------------------
// 1. ClientIntentionPacket roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_client_intention_encode_decode_roundtrip() {
    let pkt = ClientIntentionPacket {
        protocol_version: 1_073_742_124,
        server_address: "localhost".to_string(),
        server_port: 25565,
        next_state: ClientIntent::Login,
    };
    let encoded = pkt.encode();
    let decoded = ClientIntentionPacket::decode(encoded.freeze()).unwrap();
    assert_eq!(decoded.protocol_version, 1_073_742_124);
    assert_eq!(decoded.server_address, "localhost");
    assert_eq!(decoded.server_port, 25565);
    assert_eq!(decoded.next_state, ClientIntent::Login);
}

// ---------------------------------------------------------------------------
// 2. ClientboundStatusResponsePacket encode
// ---------------------------------------------------------------------------

#[test]
fn test_status_response_encode() {
    let json = r#"{"version":{"name":"26.1","protocol":1073742124}}"#;
    let pkt = ClientboundStatusResponsePacket {
        status_json: json.to_string(),
    };
    let encoded = pkt.encode();
    // The output should be a VarInt-length-prefixed UTF-8 string.
    let mut data = Bytes::from(encoded.to_vec());
    let decoded_str = types::read_string(&mut data, 32767).unwrap();
    assert_eq!(decoded_str, json);
    assert_eq!(data.len(), 0, "no trailing bytes");
}

// ---------------------------------------------------------------------------
// 3. ServerboundPingRequestPacket roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_ping_pong_roundtrip() {
    let pkt = ServerboundPingRequestPacket {
        time: 1_718_000_000_000,
    };
    let encoded = pkt.encode();
    let decoded = ServerboundPingRequestPacket::decode(encoded.freeze()).unwrap();
    assert_eq!(decoded.time, 1_718_000_000_000);
}

// ---------------------------------------------------------------------------
// 4. ClientboundHelloPacket roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_login_hello_encode_decode() {
    let pkt = ClientboundHelloPacket {
        server_id: "oxidized".to_string(),
        public_key: vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04],
        challenge: vec![0xCA, 0xFE, 0xBA, 0xBE],
        should_authenticate: true,
    };
    let encoded = pkt.encode();
    let decoded = ClientboundHelloPacket::decode(encoded.freeze()).unwrap();
    assert_eq!(decoded.server_id, "oxidized");
    assert_eq!(
        decoded.public_key,
        [0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04]
    );
    assert_eq!(decoded.challenge, [0xCA, 0xFE, 0xBA, 0xBE]);
    assert!(decoded.should_authenticate);
}

// ---------------------------------------------------------------------------
// 5. ClientboundLoginCompressionPacket roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_login_compression_encode_decode() {
    let pkt = ClientboundLoginCompressionPacket { threshold: 256 };
    let encoded = pkt.encode();
    let decoded = ClientboundLoginCompressionPacket::decode(encoded.freeze()).unwrap();
    assert_eq!(decoded.threshold, 256);
}

// ---------------------------------------------------------------------------
// 6. ClientboundLoginFinishedPacket roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_login_finished_encode_decode() {
    let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let pkt = ClientboundLoginFinishedPacket {
        uuid,
        username: "TestPlayer".to_string(),
        properties: vec![],
    };
    let encoded = pkt.encode();
    let decoded = ClientboundLoginFinishedPacket::decode(encoded.freeze()).unwrap();
    assert_eq!(decoded.uuid, uuid);
    assert_eq!(decoded.username, "TestPlayer");
    assert!(decoded.properties.is_empty());
}

#[test]
fn test_login_finished_with_properties() {
    let uuid = Uuid::parse_str("069a79f4-44e9-4726-a5be-fca90e38aaf5").unwrap();
    let pkt = ClientboundLoginFinishedPacket {
        uuid,
        username: "Notch".to_string(),
        properties: vec![ProfileProperty {
            name: "textures".to_string(),
            value: "base64data==".to_string(),
            signature: Some("sigdata==".to_string()),
        }],
    };
    let encoded = pkt.encode();
    let decoded = ClientboundLoginFinishedPacket::decode(encoded.freeze()).unwrap();
    assert_eq!(decoded.uuid, uuid);
    assert_eq!(decoded.username, "Notch");
    assert_eq!(decoded.properties.len(), 1);
    assert_eq!(decoded.properties[0].name, "textures");
    assert_eq!(decoded.properties[0].value, "base64data==");
    assert_eq!(
        decoded.properties[0].signature.as_deref(),
        Some("sigdata==")
    );
}

// ---------------------------------------------------------------------------
// 7. ServerboundHelloPacket roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_serverbound_hello_encode_decode() {
    let uuid = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
    let pkt = ServerboundHelloPacket {
        name: "Steve".to_string(),
        profile_id: uuid,
    };
    let encoded = pkt.encode();
    let decoded = ServerboundHelloPacket::decode(encoded.freeze()).unwrap();
    assert_eq!(decoded.name, "Steve");
    assert_eq!(decoded.profile_id, uuid);
}

// ---------------------------------------------------------------------------
// 8. ServerboundKeyPacket roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_serverbound_key_encode_decode() {
    let pkt = ServerboundKeyPacket {
        key_bytes: vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08],
        encrypted_challenge: vec![0xAA, 0xBB, 0xCC, 0xDD],
    };
    let encoded = pkt.encode();
    let decoded = ServerboundKeyPacket::decode(encoded.freeze()).unwrap();
    assert_eq!(
        decoded.key_bytes,
        [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
    );
    assert_eq!(decoded.encrypted_challenge, [0xAA, 0xBB, 0xCC, 0xDD]);
}
