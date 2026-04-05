//! Generic roundtrip test helper and comprehensive packet roundtrip tests.
//!
//! This module provides [`assert_roundtrip`], a generic helper that verifies
//! any [`Packet`] implementation correctly survives an encode → decode cycle.
//! Every packet type in the protocol is tested here through the unified
//! [`Packet`] trait, validating that the trait implementations are correct.
//!
//! See the unified `Packet` trait design for details on the encode/decode contract.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use oxidized_codec::Packet;

// ═══════════════════════════════════════════════════════════════════════════
// Generic roundtrip helper
// ═══════════════════════════════════════════════════════════════════════════

/// Asserts that a packet survives an encode → decode roundtrip via the
/// [`Packet`] trait, producing an identical value.
///
/// # Panics
///
/// Panics if decoding fails or if the decoded packet differs from the original.
fn assert_roundtrip<P: Packet + PartialEq + std::fmt::Debug>(pkt: &P) {
    let encoded = pkt.encode();
    let decoded =
        P::decode(encoded.freeze()).expect("decode should succeed for a packet we just encoded");
    assert_eq!(
        pkt,
        &decoded,
        "roundtrip mismatch for {} (packet ID 0x{:02X})",
        std::any::type_name::<P>(),
        P::PACKET_ID,
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Handshake state
// ═══════════════════════════════════════════════════════════════════════════

use oxidized_protocol::packets::handshake::{ClientIntent, ClientIntentionPacket};

#[test]
fn roundtrip_client_intention_status() {
    assert_roundtrip(&ClientIntentionPacket {
        protocol_version: 775,
        server_address: "localhost".to_string(),
        server_port: 25565,
        next_state: ClientIntent::Status,
    });
}

#[test]
fn roundtrip_client_intention_login() {
    assert_roundtrip(&ClientIntentionPacket {
        protocol_version: 775,
        server_address: "mc.example.com".to_string(),
        server_port: 19132,
        next_state: ClientIntent::Login,
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// Status state
// ═══════════════════════════════════════════════════════════════════════════

use oxidized_protocol::packets::status::{
    ClientboundPongResponsePacket, ClientboundStatusResponsePacket, ServerboundPingRequestPacket,
    ServerboundStatusRequestPacket,
};

#[test]
fn roundtrip_status_request() {
    assert_roundtrip(&ServerboundStatusRequestPacket);
}

#[test]
fn roundtrip_status_response() {
    assert_roundtrip(&ClientboundStatusResponsePacket {
        status_json: r#"{"version":{"name":"26.1","protocol":775}}"#.to_string(),
    });
}

#[test]
fn roundtrip_ping_request() {
    assert_roundtrip(&ServerboundPingRequestPacket {
        time: 1_719_000_000_000,
    });
}

#[test]
fn roundtrip_pong_response() {
    assert_roundtrip(&ClientboundPongResponsePacket {
        time: 1_719_000_000_000,
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// Login state
// ═══════════════════════════════════════════════════════════════════════════

use oxidized_protocol::packets::login::clientbound_login_finished::ProfileProperty;
use oxidized_protocol::packets::login::{
    ClientboundDisconnectPacket as LoginDisconnectPacket, ClientboundHelloPacket,
    ClientboundLoginCompressionPacket, ClientboundLoginFinishedPacket, ServerboundHelloPacket,
    ServerboundKeyPacket, ServerboundLoginAcknowledgedPacket,
};
use uuid::Uuid;

#[test]
fn roundtrip_serverbound_hello() {
    assert_roundtrip(&ServerboundHelloPacket {
        name: "Steve".to_string(),
        profile_id: Uuid::from_u128(0x550e8400_e29b_41d4_a716_446655440000),
    });
}

#[test]
fn roundtrip_clientbound_hello() {
    assert_roundtrip(&ClientboundHelloPacket {
        server_id: "oxidized".to_string(),
        public_key: vec![0xDE, 0xAD, 0xBE, 0xEF],
        challenge: vec![0xCA, 0xFE, 0xBA, 0xBE],
        is_authenticating: true,
    });
}

#[test]
fn roundtrip_serverbound_key() {
    assert_roundtrip(&ServerboundKeyPacket {
        key_bytes: vec![0x01, 0x02, 0x03, 0x04],
        encrypted_challenge: vec![0xAA, 0xBB, 0xCC, 0xDD],
    });
}

#[test]
fn roundtrip_login_compression() {
    assert_roundtrip(&ClientboundLoginCompressionPacket { threshold: 256 });
}

#[test]
fn roundtrip_login_finished_no_properties() {
    assert_roundtrip(&ClientboundLoginFinishedPacket {
        uuid: Uuid::from_u128(0x550e8400_e29b_41d4_a716_446655440000),
        username: "TestPlayer".to_string(),
        properties: vec![],
    });
}

#[test]
fn roundtrip_login_finished_with_properties() {
    assert_roundtrip(&ClientboundLoginFinishedPacket {
        uuid: Uuid::from_u128(0x069a79f4_44e9_4726_a5be_fca90e38aaf5),
        username: "Notch".to_string(),
        properties: vec![ProfileProperty {
            name: "textures".to_string(),
            value: "base64data==".to_string(),
            signature: Some("sigdata==".to_string()),
        }],
    });
}

#[test]
fn roundtrip_login_disconnect() {
    assert_roundtrip(&LoginDisconnectPacket {
        reason: r#"{"text":"Server is full"}"#.to_string(),
    });
}

#[test]
fn roundtrip_login_acknowledged() {
    assert_roundtrip(&ServerboundLoginAcknowledgedPacket);
}

// ═══════════════════════════════════════════════════════════════════════════
// Configuration state
// ═══════════════════════════════════════════════════════════════════════════

use oxidized_mc_types::resource_location::ResourceLocation;
use oxidized_protocol::packets::configuration::{
    ClientInformation, ClientboundFinishConfigurationPacket, ClientboundRegistryDataPacket,
    ClientboundSelectKnownPacksPacket, ClientboundUpdateEnabledFeaturesPacket,
    ClientboundUpdateTagsPacket, KnownPack, RegistryEntry, ServerboundClientInformationPacket,
    ServerboundFinishConfigurationPacket, ServerboundSelectKnownPacksPacket, TagEntry, TagRegistry,
};

#[test]
fn roundtrip_finish_configuration_clientbound() {
    assert_roundtrip(&ClientboundFinishConfigurationPacket);
}

#[test]
fn roundtrip_finish_configuration_serverbound() {
    assert_roundtrip(&ServerboundFinishConfigurationPacket);
}

#[test]
fn roundtrip_registry_data() {
    assert_roundtrip(&ClientboundRegistryDataPacket {
        registry: ResourceLocation::new("minecraft", "dimension_type").unwrap(),
        entries: vec![RegistryEntry {
            id: ResourceLocation::new("minecraft", "overworld").unwrap(),
            data: None,
        }],
    });
}

#[test]
fn roundtrip_select_known_packs_clientbound() {
    assert_roundtrip(&ClientboundSelectKnownPacksPacket {
        packs: vec![KnownPack {
            namespace: "minecraft".to_string(),
            id: "core".to_string(),
            version: "1.21".to_string(),
        }],
    });
}

#[test]
fn roundtrip_select_known_packs_serverbound() {
    assert_roundtrip(&ServerboundSelectKnownPacksPacket {
        packs: vec![KnownPack {
            namespace: "minecraft".to_string(),
            id: "core".to_string(),
            version: "1.21".to_string(),
        }],
    });
}

#[test]
fn roundtrip_update_enabled_features() {
    assert_roundtrip(&ClientboundUpdateEnabledFeaturesPacket {
        features: vec![
            ResourceLocation::new("minecraft", "vanilla").unwrap(),
            ResourceLocation::new("minecraft", "bundle").unwrap(),
        ],
    });
}

#[test]
fn roundtrip_update_tags() {
    assert_roundtrip(&ClientboundUpdateTagsPacket {
        tags: vec![TagRegistry {
            registry: ResourceLocation::new("minecraft", "block").unwrap(),
            tags: vec![TagEntry {
                name: ResourceLocation::new("minecraft", "planks").unwrap(),
                entries: vec![1, 2, 3],
            }],
        }],
    });
}

#[test]
fn roundtrip_client_information() {
    assert_roundtrip(&ServerboundClientInformationPacket {
        information: ClientInformation::create_default(),
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// Play state — simple packets
// ═══════════════════════════════════════════════════════════════════════════

use oxidized_protocol::packets::play::{
    ClientboundChunkBatchFinishedPacket, ClientboundChunkBatchStartPacket,
    ClientboundForgetLevelChunkPacket, ClientboundKeepAlivePacket, ClientboundRemoveEntitiesPacket,
    ClientboundRotateHeadPacket, ClientboundSetChunkCacheCenterPacket,
    ClientboundSetChunkCacheRadiusPacket, ServerboundAcceptTeleportationPacket,
    ServerboundChatAckPacket, ServerboundChatCommandPacket, ServerboundChatCommandSignedPacket,
    ServerboundChunkBatchReceivedPacket, ServerboundCommandSuggestionPacket,
    ServerboundKeepAlivePacket,
};

#[test]
fn roundtrip_keep_alive_clientbound() {
    assert_roundtrip(&ClientboundKeepAlivePacket { id: 12345 });
}

#[test]
fn roundtrip_keep_alive_serverbound() {
    assert_roundtrip(&ServerboundKeepAlivePacket { id: 12345 });
}

#[test]
fn roundtrip_chunk_batch_start() {
    assert_roundtrip(&ClientboundChunkBatchStartPacket);
}

#[test]
fn roundtrip_chunk_batch_finished() {
    assert_roundtrip(&ClientboundChunkBatchFinishedPacket { batch_size: 16 });
}

#[test]
fn roundtrip_chunk_batch_received() {
    assert_roundtrip(&ServerboundChunkBatchReceivedPacket {
        desired_chunks_per_tick: 5.0,
    });
}

#[test]
fn roundtrip_forget_level_chunk() {
    assert_roundtrip(&ClientboundForgetLevelChunkPacket {
        chunk_x: -3,
        chunk_z: 7,
    });
}

#[test]
fn roundtrip_set_chunk_cache_center() {
    assert_roundtrip(&ClientboundSetChunkCacheCenterPacket {
        chunk_x: 0,
        chunk_z: 0,
    });
}

#[test]
fn roundtrip_set_chunk_cache_radius() {
    assert_roundtrip(&ClientboundSetChunkCacheRadiusPacket { radius: 12 });
}

#[test]
fn roundtrip_accept_teleportation() {
    assert_roundtrip(&ServerboundAcceptTeleportationPacket { teleport_id: 42 });
}

#[test]
fn roundtrip_remove_entities() {
    assert_roundtrip(&ClientboundRemoveEntitiesPacket {
        entity_ids: vec![1, 2, 3, 100],
    });
}

#[test]
fn roundtrip_rotate_head() {
    assert_roundtrip(&ClientboundRotateHeadPacket {
        entity_id: 7,
        head_yaw: 128,
    });
}

#[test]
fn roundtrip_chat_ack() {
    assert_roundtrip(&ServerboundChatAckPacket { offset: 5 });
}

#[test]
fn roundtrip_chat_command() {
    assert_roundtrip(&ServerboundChatCommandPacket {
        command: "gamemode creative".to_string(),
    });
}

#[test]
fn roundtrip_chat_command_signed() {
    assert_roundtrip(&ServerboundChatCommandSignedPacket {
        command: "tp @s 0 64 0".to_string(),
    });
}

#[test]
fn roundtrip_command_suggestion() {
    assert_roundtrip(&ServerboundCommandSuggestionPacket {
        id: 1,
        command: "game".to_string(),
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// Play state — movement & entity packets
// ═══════════════════════════════════════════════════════════════════════════

use oxidized_protocol::packets::play::{
    ClientboundMoveEntityPosPacket, ClientboundMoveEntityPosRotPacket,
    ClientboundMoveEntityRotPacket, PlayerCommandAction, PlayerInput,
    ServerboundPlayerCommandPacket, ServerboundPlayerInputPacket,
};

#[test]
fn roundtrip_move_entity_pos() {
    assert_roundtrip(&ClientboundMoveEntityPosPacket {
        entity_id: 42,
        dx: 100,
        dy: -50,
        dz: 200,
        is_on_ground: true,
    });
}

#[test]
fn roundtrip_move_entity_pos_rot() {
    assert_roundtrip(&ClientboundMoveEntityPosRotPacket {
        entity_id: 42,
        dx: 100,
        dy: -50,
        dz: 200,
        yaw: 128,
        pitch: 64,
        is_on_ground: false,
    });
}

#[test]
fn roundtrip_move_entity_rot() {
    assert_roundtrip(&ClientboundMoveEntityRotPacket {
        entity_id: 42,
        yaw: 200,
        pitch: 30,
        is_on_ground: true,
    });
}

#[test]
fn roundtrip_player_command() {
    assert_roundtrip(&ServerboundPlayerCommandPacket {
        entity_id: 99,
        action: PlayerCommandAction::StartSprinting,
        data: 0,
    });
}

#[test]
fn roundtrip_player_input() {
    assert_roundtrip(&ServerboundPlayerInputPacket {
        input: PlayerInput {
            is_forward: true,
            is_backward: false,
            is_left: false,
            is_right: true,
            is_jumping: true,
            is_shifting: false,
            is_sprinting: true,
        },
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// Play state — complex packets
// ═══════════════════════════════════════════════════════════════════════════

use oxidized_protocol::packets::play::{
    ClientboundSetEntityDataPacket, ClientboundSetSimulationDistancePacket,
};

#[test]
fn roundtrip_set_simulation_distance() {
    assert_roundtrip(&ClientboundSetSimulationDistancePacket {
        simulation_distance: 12,
    });
}

#[test]
fn roundtrip_set_entity_data_single_byte() {
    assert_roundtrip(&ClientboundSetEntityDataPacket::single_byte(42, 0, 0x20));
}

#[test]
fn roundtrip_delete_chat_by_index() {
    use oxidized_protocol::packets::play::ClientboundDeleteChatPacket;
    assert_roundtrip(&ClientboundDeleteChatPacket {
        packed_message_id: 5,
        full_signature: None,
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// Play state — chat packets
// ═══════════════════════════════════════════════════════════════════════════

use oxidized_chat::Component;
use oxidized_protocol::packets::play::{
    ClientboundCommandSuggestionsPacket, ClientboundSystemChatPacket, SuggestionEntry,
};

#[test]
fn roundtrip_system_chat() {
    assert_roundtrip(&ClientboundSystemChatPacket {
        content: Component::text("Hello, world!"),
        is_overlay: false,
    });
}

#[test]
fn roundtrip_system_chat_overlay() {
    assert_roundtrip(&ClientboundSystemChatPacket {
        content: Component::text("Action bar text"),
        is_overlay: true,
    });
}

#[test]
fn roundtrip_command_suggestions_empty() {
    assert_roundtrip(&ClientboundCommandSuggestionsPacket {
        id: 1,
        start: 0,
        length: 4,
        suggestions: vec![],
    });
}

#[test]
fn roundtrip_command_suggestions_with_entries() {
    assert_roundtrip(&ClientboundCommandSuggestionsPacket {
        id: 1,
        start: 1,
        length: 8,
        suggestions: vec![
            SuggestionEntry {
                text: "gamemode".to_string(),
                tooltip: None,
            },
            SuggestionEntry {
                text: "gamerule".to_string(),
                tooltip: Some(Component::text("Change game rules")),
            },
        ],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// Proptest — generic Packet trait roundtrip strategies
// ═══════════════════════════════════════════════════════════════════════════

use proptest::prelude::*;

/// Strategy for finite f32 values (no NaN/Inf — required for PartialEq).
fn finite_f32() -> impl Strategy<Value = f32> {
    prop::num::f32::NORMAL
        | prop::num::f32::POSITIVE
        | prop::num::f32::NEGATIVE
        | prop::num::f32::ZERO
}

proptest! {
    /// Generic roundtrip via `Packet` trait for `ServerboundPingRequestPacket`.
    #[test]
    fn proptest_ping_request_trait_roundtrip(time: i64) {
        let pkt = ServerboundPingRequestPacket { time };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip via `Packet` trait for `ClientboundPongResponsePacket`.
    #[test]
    fn proptest_pong_response_trait_roundtrip(time: i64) {
        let pkt = ClientboundPongResponsePacket { time };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip via `Packet` trait for `ClientboundKeepAlivePacket`.
    #[test]
    fn proptest_keepalive_cb_trait_roundtrip(id: i64) {
        let pkt = ClientboundKeepAlivePacket { id };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip via `Packet` trait for `ServerboundKeepAlivePacket`.
    #[test]
    fn proptest_keepalive_sb_trait_roundtrip(id: i64) {
        let pkt = ServerboundKeepAlivePacket { id };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip via `Packet` trait for `ClientboundChunkBatchFinishedPacket`.
    #[test]
    fn proptest_chunk_batch_finished_trait_roundtrip(batch_size: i32) {
        let pkt = ClientboundChunkBatchFinishedPacket { batch_size };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ClientboundForgetLevelChunkPacket` via `Packet` trait.
    #[test]
    fn proptest_forget_level_chunk_trait_roundtrip(chunk_x: i32, chunk_z: i32) {
        let pkt = ClientboundForgetLevelChunkPacket { chunk_x, chunk_z };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ClientboundSetChunkCacheCenterPacket` via `Packet` trait.
    #[test]
    fn proptest_set_chunk_cache_center_trait_roundtrip(chunk_x: i32, chunk_z: i32) {
        let pkt = ClientboundSetChunkCacheCenterPacket { chunk_x, chunk_z };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ClientboundSetChunkCacheRadiusPacket` via `Packet` trait.
    #[test]
    fn proptest_set_chunk_cache_radius_trait_roundtrip(radius: i32) {
        let pkt = ClientboundSetChunkCacheRadiusPacket { radius };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ClientboundSetSimulationDistancePacket` via `Packet` trait.
    #[test]
    fn proptest_set_simulation_distance_trait_roundtrip(distance: i32) {
        let pkt = ClientboundSetSimulationDistancePacket { simulation_distance: distance };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ServerboundAcceptTeleportationPacket` via `Packet` trait.
    #[test]
    fn proptest_accept_teleportation_trait_roundtrip(id: i32) {
        let pkt = ServerboundAcceptTeleportationPacket { teleport_id: id };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ServerboundChatAckPacket` via `Packet` trait.
    #[test]
    fn proptest_chat_ack_trait_roundtrip(offset: i32) {
        let pkt = ServerboundChatAckPacket { offset };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ClientboundRemoveEntitiesPacket` via `Packet` trait.
    #[test]
    fn proptest_remove_entities_trait_roundtrip(
        ids in prop::collection::vec(any::<i32>(), 0..20),
    ) {
        let pkt = ClientboundRemoveEntitiesPacket { entity_ids: ids };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ClientboundRotateHeadPacket` via `Packet` trait.
    #[test]
    fn proptest_rotate_head_trait_roundtrip(entity_id: i32, head_yaw: u8) {
        let pkt = ClientboundRotateHeadPacket { entity_id, head_yaw };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ClientboundMoveEntityPosPacket` via `Packet` trait.
    #[test]
    fn proptest_move_entity_pos_trait_roundtrip(
        entity_id: i32, dx: i16, dy: i16, dz: i16, is_on_ground: bool,
    ) {
        let pkt = ClientboundMoveEntityPosPacket { entity_id, dx, dy, dz, is_on_ground };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ClientboundMoveEntityPosRotPacket` via `Packet` trait.
    #[test]
    fn proptest_move_entity_pos_rot_trait_roundtrip(
        entity_id: i32, dx: i16, dy: i16, dz: i16,
        yaw: u8, pitch: u8, is_on_ground: bool,
    ) {
        let pkt = ClientboundMoveEntityPosRotPacket {
            entity_id, dx, dy, dz, yaw, pitch, is_on_ground,
        };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ClientboundMoveEntityRotPacket` via `Packet` trait.
    #[test]
    fn proptest_move_entity_rot_trait_roundtrip(
        entity_id: i32, yaw: u8, pitch: u8, is_on_ground: bool,
    ) {
        let pkt = ClientboundMoveEntityRotPacket { entity_id, yaw, pitch, is_on_ground };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ServerboundPlayerCommandPacket` via `Packet` trait.
    #[test]
    fn proptest_player_command_trait_roundtrip(
        entity_id: i32,
        action_id in 0i32..=6,
        data: i32,
    ) {
        let action = PlayerCommandAction::from_id(action_id).unwrap();
        let pkt = ServerboundPlayerCommandPacket { entity_id, action, data };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ServerboundPlayerInputPacket` via `Packet` trait.
    #[test]
    fn proptest_player_input_trait_roundtrip(flags in 0u8..=0x7F) {
        let pkt = ServerboundPlayerInputPacket {
            input: PlayerInput::from_byte(flags),
        };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ClientboundLoginCompressionPacket` via `Packet` trait.
    #[test]
    fn proptest_login_compression_trait_roundtrip(threshold: i32) {
        let pkt = ClientboundLoginCompressionPacket { threshold };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ServerboundChatCommandPacket` with variable-length strings.
    #[test]
    fn proptest_chat_command_trait_roundtrip(cmd in "[a-zA-Z0-9 _/]{1,100}") {
        let pkt = ServerboundChatCommandPacket { command: cmd };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ServerboundCommandSuggestionPacket` via `Packet` trait.
    #[test]
    fn proptest_command_suggestion_trait_roundtrip(
        id: i32,
        cmd in "[a-zA-Z0-9]{1,50}",
    ) {
        let pkt = ServerboundCommandSuggestionPacket { id, command: cmd };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ClientboundGameEventPacket` via `Packet` trait.
    #[test]
    fn proptest_game_event_trait_roundtrip(
        event_id in 0u8..=13,
        param in finite_f32(),
    ) {
        use oxidized_protocol::packets::play::{ClientboundGameEventPacket, GameEventType};
        let event = GameEventType::from_id(event_id).unwrap();
        let pkt = ClientboundGameEventPacket { event, param };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ClientboundSetEntityDataPacket` single-byte via `Packet` trait.
    #[test]
    fn proptest_set_entity_data_trait_roundtrip(
        entity_id: i32, slot in 0u8..=254, value: u8,
    ) {
        let pkt = ClientboundSetEntityDataPacket::single_byte(entity_id, slot, value);
        assert_roundtrip(&pkt);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Proptest — string-containing packets via Packet trait
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    /// Generic roundtrip for `ClientIntentionPacket` with random addresses.
    #[test]
    fn proptest_client_intention_trait_roundtrip(
        addr in "[a-z0-9.]{1,50}",
        port: u16,
        intent in 1i32..=2,
    ) {
        let next_state = if intent == 1 {
            ClientIntent::Status
        } else {
            ClientIntent::Login
        };
        let pkt = ClientIntentionPacket {
            protocol_version: 775,
            server_address: addr,
            server_port: port,
            next_state,
        };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ServerboundHelloPacket` with random names.
    #[test]
    fn proptest_serverbound_hello_trait_roundtrip(
        name in "[a-zA-Z0-9_]{3,16}",
        uuid_bytes in prop::array::uniform16(any::<u8>()),
    ) {
        let pkt = ServerboundHelloPacket {
            name,
            profile_id: Uuid::from_bytes(uuid_bytes),
        };
        assert_roundtrip(&pkt);
    }

    /// Generic roundtrip for `ClientboundStatusResponsePacket` with random JSON.
    #[test]
    fn proptest_status_response_trait_roundtrip(json in "[a-zA-Z0-9{}:,\"]{1,200}") {
        let pkt = ClientboundStatusResponsePacket { status_json: json };
        assert_roundtrip(&pkt);
    }
}
