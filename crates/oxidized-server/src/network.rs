//! TCP listener and per-connection handler for the Oxidized server.
//!
//! Binds to the configured address, accepts incoming connections, and
//! spawns a Tokio task per client. Handles the HANDSHAKING, STATUS,
//! LOGIN, and CONFIGURATION protocol states.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use oxidized_game::chunk::chunk_tracker::PlayerChunkTracker;
use oxidized_game::chunk::view_distance::spiral_chunks;
use oxidized_game::net::chunk_serializer::build_chunk_packet;
use oxidized_game::player::movement::validate_movement;
use oxidized_game::player::{
    GameMode, PlayerList, ServerPlayer, build_login_sequence, handle_accept_teleportation,
};
use oxidized_protocol::auth;
use oxidized_protocol::connection::{Connection, ConnectionError, ConnectionState, RawPacket};
use oxidized_protocol::crypto::{
    ServerKeyPair, generate_challenge, minecraft_digest, offline_uuid,
};
use oxidized_protocol::packets::configuration::{
    ClientInformation, ClientboundFinishConfigurationPacket, ClientboundRegistryDataPacket,
    ClientboundSelectKnownPacksPacket, ClientboundUpdateEnabledFeaturesPacket,
    ClientboundUpdateTagsPacket, KnownPack, RegistryEntry, ServerboundClientInformationPacket,
    ServerboundFinishConfigurationPacket, ServerboundSelectKnownPacksPacket,
};
use oxidized_protocol::packets::handshake::{ClientIntent, ClientIntentionPacket};
use oxidized_protocol::packets::login::clientbound_login_finished::ProfileProperty;
use oxidized_protocol::packets::login::{
    ClientboundDisconnectPacket, ClientboundHelloPacket, ClientboundLoginCompressionPacket,
    ClientboundLoginFinishedPacket, ServerboundHelloPacket, ServerboundKeyPacket,
    ServerboundLoginAcknowledgedPacket,
};
use oxidized_protocol::packets::play::{
    ClientboundChunkBatchFinishedPacket, ClientboundChunkBatchStartPacket,
    ClientboundForgetLevelChunkPacket, ClientboundGameEventPacket,
    ClientboundLevelChunkWithLightPacket, ClientboundPlayerPositionPacket,
    ClientboundSetChunkCacheCenterPacket, ClientboundSetChunkCacheRadiusPacket, GameEventType,
    PlayerCommandAction, RelativeFlags, ServerboundAcceptTeleportationPacket,
    ServerboundChunkBatchReceivedPacket, ServerboundMovePlayerPacket,
    ServerboundPlayerCommandPacket, ServerboundPlayerInputPacket,
};
use oxidized_protocol::packets::status::{
    ClientboundPongResponsePacket, ClientboundStatusResponsePacket, ServerboundPingRequestPacket,
    ServerboundStatusRequestPacket,
};
use oxidized_protocol::registry;
use oxidized_protocol::status::ServerStatus;
use oxidized_protocol::types::resource_location::ResourceLocation;
use oxidized_world::chunk::{ChunkPos, LevelChunk};
use oxidized_world::storage::PrimaryLevelData;
use parking_lot::RwLock;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

/// Shared state for login operations, passed to each connection handler.
pub struct LoginContext {
    /// Pre-built server status for the multiplayer list.
    pub server_status: Arc<ServerStatus>,
    /// RSA-1024 keypair used for encryption handshake.
    pub keypair: Arc<ServerKeyPair>,
    /// Whether the server authenticates players against Mojang session servers.
    pub online_mode: bool,
    /// Minimum packet size (in bytes) before compression is applied. `-1` disables compression.
    pub compression_threshold: i32,
    /// Whether to block connections through proxies by verifying the client IP.
    pub prevent_proxy_connections: bool,
    /// Reusable HTTP client for Mojang session server requests.
    pub http_client: reqwest::Client,
    /// Shared game server state for PLAY-state operations.
    pub server_ctx: Arc<ServerContext>,
}

/// Shared game server state accessible to all connection handlers.
///
/// Holds the player list, world metadata, and dimension registry needed
/// when a client transitions from CONFIGURATION to PLAY state.
pub struct ServerContext {
    /// Server-wide player roster (thread-safe via interior mutability).
    pub player_list: RwLock<PlayerList>,
    /// World metadata loaded from `level.dat` (or defaults for new worlds).
    pub level_data: PrimaryLevelData,
    /// All registered dimension identifiers (e.g., `minecraft:overworld`).
    pub dimensions: Vec<ResourceLocation>,
    /// Maximum view distance allowed by the server config (2–32 chunks).
    pub max_view_distance: i32,
    /// Maximum simulation distance allowed by the server config (2–32 chunks).
    pub max_simulation_distance: i32,
}

/// Starts the TCP listener and accepts connections until a shutdown signal
/// is received.
///
/// # Errors
///
/// Returns an error if the listener fails to bind to `addr`.
pub async fn listen(
    addr: SocketAddr,
    ctx: Arc<LoginContext>,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!(address = %addr, "Listening for connections");

    loop {
        tokio::select! {
            biased;

            _ = shutdown_rx.recv() => {
                info!("Shutdown signal received — stopping listener");
                break;
            }

            result = listener.accept() => {
                match result {
                    Ok((stream, peer_addr)) => {
                        info!(peer = %peer_addr, "New connection");
                        let ctx = Arc::clone(&ctx);
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, peer_addr, &ctx).await {
                                debug!(peer = %peer_addr, error = %e, "Connection closed");
                            }
                        });
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to accept connection");
                    }
                }
            }
        }
    }

    Ok(())
}

/// Handles a single client connection through the protocol state machine.
///
/// Dispatches packets based on the current [`ConnectionState`]:
/// - **Handshaking** → parse [`ClientIntentionPacket`], transition state
/// - **Status** → respond with server status JSON and pong
/// - **Login** → authenticate, enable encryption/compression, finish login
async fn handle_connection(
    stream: tokio::net::TcpStream,
    addr: SocketAddr,
    ctx: &LoginContext,
) -> Result<(), ConnectionError> {
    let mut conn = Connection::new(stream, addr)?;
    debug!(
        peer = %addr,
        state = %conn.state,
        "Connection established",
    );

    loop {
        match conn.read_raw_packet().await {
            Ok(pkt) => {
                debug!(
                    peer = %addr,
                    state = %conn.state,
                    packet_id = format_args!("0x{:02X}", pkt.id),
                    size = pkt.data.len(),
                    "Received packet",
                );

                match conn.state {
                    ConnectionState::Handshaking => {
                        handle_handshake(&mut conn, pkt).await?;
                    },
                    ConnectionState::Status => {
                        let done = handle_status(&mut conn, pkt, &ctx.server_status).await?;
                        if done {
                            debug!(peer = %addr, "Status exchange complete");
                            return Ok(());
                        }
                    },
                    ConnectionState::Login => {
                        let profile = handle_login(&mut conn, pkt, ctx).await?;
                        // Login transitions to Configuration — handle it immediately
                        // (server drives the configuration flow, not client)
                        let client_info = handle_configuration(&mut conn, ctx).await?;
                        // Configuration transitions to Play — send login sequence
                        handle_play_entry(&mut conn, profile, client_info, ctx).await?;
                        info!(peer = %addr, "Player session ended");
                        return Ok(());
                    },
                    ConnectionState::Configuration | ConnectionState::Play => {
                        debug!(peer = %addr, state = %conn.state, "Unhandled state");
                        return Ok(());
                    },
                }
            },
            Err(ConnectionError::Io(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                debug!(peer = %addr, "Client disconnected");
                return Ok(());
            },
            Err(e) => {
                debug!(peer = %addr, error = %e, "Connection error");
                return Err(e);
            },
        }
    }
}

/// Processes the handshake packet and transitions to the requested state.
async fn handle_handshake(conn: &mut Connection, pkt: RawPacket) -> Result<(), ConnectionError> {
    if pkt.id != ClientIntentionPacket::PACKET_ID {
        warn!(
            peer = %conn.remote_addr(),
            packet_id = pkt.id,
            "Expected handshake packet (0x00)",
        );
        return Ok(());
    }

    let intention = ClientIntentionPacket::decode(pkt.data).map_err(|e| {
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })?;

    conn.protocol_version = intention.protocol_version;

    debug!(
        peer = %conn.remote_addr(),
        protocol_version = intention.protocol_version,
        server_address = %intention.server_address,
        server_port = intention.server_port,
        intent = ?intention.next_state,
        "Handshake received",
    );

    conn.state = match intention.next_state {
        ClientIntent::Status => ConnectionState::Status,
        ClientIntent::Login | ClientIntent::Transfer => ConnectionState::Login,
    };

    Ok(())
}

/// Handles the full login sequence for a single client connection.
///
/// Reads the initial [`ServerboundHelloPacket`], performs online-mode
/// authentication (encryption + Mojang session server) or offline-mode
/// UUID generation, optionally enables compression, sends the
/// [`ClientboundLoginFinishedPacket`], and transitions to
/// [`ConnectionState::Configuration`].
///
/// Returns the authenticated [`GameProfile`] for use in subsequent
/// protocol states.
///
/// # Errors
///
/// Returns a [`ConnectionError`] if any I/O, decoding, or authentication
/// step fails. On recoverable failures (bad name, failed auth) a
/// disconnect packet is sent before returning the error.
async fn handle_login(
    conn: &mut Connection,
    hello_pkt: RawPacket,
    ctx: &LoginContext,
) -> Result<auth::GameProfile, ConnectionError> {
    let addr = conn.remote_addr();

    // 1. Decode ServerboundHelloPacket (the first Login packet).
    if hello_pkt.id != ServerboundHelloPacket::PACKET_ID {
        warn!(peer = %addr, packet_id = hello_pkt.id, "Expected login hello packet (0x00)");
        return Err(disconnect_err(conn, "Unexpected packet during login").await);
    }

    let hello = ServerboundHelloPacket::decode(hello_pkt.data).map_err(|e| {
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })?;

    debug!(peer = %addr, name = %hello.name, profile_id = %hello.profile_id, "Login hello received");

    // 2. Validate player name length (1–16 characters).
    if hello.name.is_empty() || hello.name.len() > 16 {
        warn!(peer = %addr, name = %hello.name, "Invalid player name length");
        return Err(disconnect_err(conn, "Invalid player name").await);
    }

    // 3. Authenticate (online) or generate offline UUID.
    let profile = if ctx.online_mode {
        authenticate_online(conn, &hello, ctx).await?
    } else {
        let uuid = offline_uuid(&hello.name);
        debug!(peer = %addr, uuid = %uuid, name = %hello.name, "Offline-mode UUID generated");
        auth::GameProfile::new(uuid, hello.name.clone())
    };

    let uuid = profile.uuid().ok_or_else(|| {
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Profile has invalid UUID",
        ))
    })?;
    let username = profile.name().to_string();

    // 4. Enable compression if threshold >= 0.
    if ctx.compression_threshold >= 0 {
        let compression_pkt = ClientboundLoginCompressionPacket {
            threshold: ctx.compression_threshold,
        };
        let body = compression_pkt.encode();
        conn.send_raw(ClientboundLoginCompressionPacket::PACKET_ID, &body)
            .await?;
        conn.flush().await?;

        #[allow(clippy::cast_sign_loss)]
        conn.enable_compression(ctx.compression_threshold as usize);
        debug!(peer = %addr, threshold = ctx.compression_threshold, "Compression enabled");
    }

    // 5. Send LoginFinished packet.
    let properties: Vec<ProfileProperty> = profile
        .properties()
        .iter()
        .map(|p| ProfileProperty {
            name: p.name().to_string(),
            value: p.value().to_string(),
            signature: p.signature().map(String::from),
        })
        .collect();

    let finished = ClientboundLoginFinishedPacket {
        uuid,
        username: username.clone(),
        properties,
    };
    let body = finished.encode();
    conn.send_raw(ClientboundLoginFinishedPacket::PACKET_ID, &body)
        .await?;
    conn.flush().await?;

    debug!(peer = %addr, uuid = %uuid, name = %username, "Login finished sent");

    // 6. Wait for LoginAcknowledged.
    let ack_pkt = conn.read_raw_packet().await?;
    if ack_pkt.id != ServerboundLoginAcknowledgedPacket::PACKET_ID {
        warn!(peer = %addr, packet_id = ack_pkt.id, "Expected login acknowledged (0x03)");
        return Err(disconnect_err(conn, "Unexpected packet — expected login acknowledged").await);
    }
    let _ack = ServerboundLoginAcknowledgedPacket::decode(ack_pkt.data);

    // 7. Transition to Configuration state.
    conn.state = ConnectionState::Configuration;
    info!(peer = %addr, uuid = %uuid, name = %username, "Player login complete — entering configuration");

    Ok(profile)
}

/// Performs online-mode authentication: encryption handshake, shared secret
/// exchange, and Mojang session server verification.
///
/// Returns the authenticated [`GameProfile`] from Mojang's session server.
///
/// # Errors
///
/// Returns a [`ConnectionError`] if encryption setup, challenge verification,
/// or session server authentication fails.
async fn authenticate_online(
    conn: &mut Connection,
    hello: &ServerboundHelloPacket,
    ctx: &LoginContext,
) -> Result<auth::GameProfile, ConnectionError> {
    let addr = conn.remote_addr();

    // a. Generate 4-byte challenge.
    let challenge = generate_challenge();

    // b. Send ClientboundHelloPacket with encryption request.
    let hello_response = ClientboundHelloPacket {
        server_id: String::new(),
        public_key: ctx.keypair.public_key_der().to_vec(),
        challenge: challenge.to_vec(),
        should_authenticate: true,
    };
    let body = hello_response.encode();
    conn.send_raw(ClientboundHelloPacket::PACKET_ID, &body)
        .await?;
    conn.flush().await?;

    debug!(peer = %addr, "Encryption request sent");

    // c. Read ServerboundKeyPacket.
    let key_pkt = conn.read_raw_packet().await?;
    if key_pkt.id != ServerboundKeyPacket::PACKET_ID {
        warn!(peer = %addr, packet_id = key_pkt.id, "Expected key response (0x01)");
        return Err(disconnect_err(conn, "Unexpected packet — expected encryption response").await);
    }

    let key = ServerboundKeyPacket::decode(key_pkt.data).map_err(|e| {
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })?;

    // d. Decrypt shared secret and challenge.
    let shared_secret = ctx
        .keypair
        .decrypt_shared_secret(&key.key_bytes)
        .map_err(|e| {
            ConnectionError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to decrypt shared secret: {e}"),
            ))
        })?;

    let decrypted_challenge = ctx.keypair.decrypt(&key.encrypted_challenge).map_err(|e| {
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Failed to decrypt challenge: {e}"),
        ))
    })?;

    // e. Verify the decrypted challenge matches the original.
    if decrypted_challenge != challenge {
        warn!(peer = %addr, "Challenge verification failed");
        return Err(disconnect_err(conn, "Challenge verification failed").await);
    }

    // f. Enable encryption on the connection.
    conn.enable_encryption(&shared_secret);
    debug!(peer = %addr, "Encryption enabled");

    // g. Compute server hash for session verification.
    let server_hash = minecraft_digest("", &shared_secret, ctx.keypair.public_key_der());

    // h. Authenticate with Mojang session servers.
    let client_ip = if ctx.prevent_proxy_connections {
        Some(addr.ip().to_string())
    } else {
        None
    };

    let profile = auth::has_joined(
        &ctx.http_client,
        &hello.name,
        &server_hash,
        client_ip.as_deref(),
    )
    .await
    .map_err(|e| {
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("Authentication failed: {e}"),
        ))
    });

    let profile = match profile {
        Ok(p) => p,
        Err(e) => {
            let _ = disconnect(conn, "Failed to verify username!").await;
            return Err(e);
        },
    };

    // i. Return the authenticated profile.
    info!(peer = %addr, name = %profile.name(), "Player authenticated");

    Ok(profile)
}

/// Handles the CONFIGURATION state — sends registry data, tags, features,
/// and transitions the client to PLAY.
///
/// The configuration flow is server-driven:
/// 1. Send `ClientboundSelectKnownPacksPacket`
/// 2. Receive `ServerboundSelectKnownPacksPacket`
/// 3. Send `ClientboundRegistryDataPacket` × N (one per synchronized registry)
/// 4. Send `ClientboundUpdateTagsPacket` (all tag registries with entries)
/// 5. Send `ClientboundUpdateEnabledFeaturesPacket` (vanilla features)
/// 6. Send `ClientboundFinishConfigurationPacket`
/// 7. Receive `ServerboundFinishConfigurationPacket`
/// 8. Transition to PLAY state
///
/// Returns the [`ClientInformation`] received from the client (language,
/// view distance, etc.) for use in player setup.
///
/// # Errors
///
/// Returns a [`ConnectionError`] if any I/O, decoding, or protocol step fails.
async fn handle_configuration(
    conn: &mut Connection,
    _ctx: &LoginContext,
) -> Result<ClientInformation, ConnectionError> {
    let addr = conn.remote_addr();
    let mut client_info: Option<ClientInformation> = None;

    // 1. Send SelectKnownPacks — we claim to have the vanilla core pack
    let known_packs = ClientboundSelectKnownPacksPacket {
        packs: vec![KnownPack {
            namespace: "minecraft".to_string(),
            id: "core".to_string(),
            version: "1.21.6".to_string(),
        }],
    };
    conn.send_raw(
        ClientboundSelectKnownPacksPacket::PACKET_ID,
        &known_packs.encode(),
    )
    .await?;
    conn.flush().await?;
    debug!(peer = %addr, "Sent SelectKnownPacks");

    // 2. Receive serverbound packets until we get SelectKnownPacks.
    //    The client may send ClientInformation (0x00) or CustomPayload
    //    (0x02, e.g. minecraft:brand) before responding.
    const SB_CUSTOM_PAYLOAD: i32 = 0x02;
    loop {
        let pkt = conn.read_raw_packet().await?;
        match pkt.id {
            ServerboundClientInformationPacket::PACKET_ID => {
                let info_pkt =
                    ServerboundClientInformationPacket::decode(pkt.data).map_err(|e| {
                        ConnectionError::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            e.to_string(),
                        ))
                    })?;
                debug!(
                    peer = %addr,
                    language = %info_pkt.information.language,
                    view_distance = info_pkt.information.view_distance,
                    "Received client information",
                );
                client_info = Some(info_pkt.information);
            },
            SB_CUSTOM_PAYLOAD => {
                debug!(peer = %addr, "Received custom payload (ignored)");
            },
            ServerboundSelectKnownPacksPacket::PACKET_ID => {
                let _client_packs =
                    ServerboundSelectKnownPacksPacket::decode(pkt.data).map_err(|e| {
                        ConnectionError::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            e.to_string(),
                        ))
                    })?;
                debug!(peer = %addr, "Received client known packs response");
                break;
            },
            _ => {
                warn!(peer = %addr, id = pkt.id, "Unexpected packet during configuration");
                return Err(disconnect_err(conn, "Unexpected packet during configuration").await);
            },
        }
    }

    // 3. Send all synchronized registries (full data, ignoring known-pack
    //    optimisation for now)
    for registry_name in registry::SYNCHRONIZED_REGISTRIES {
        let entries = registry::get_registry_entries(registry_name).map_err(|e| {
            ConnectionError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            ))
        })?;

        let registry_loc = ResourceLocation::from_string(registry_name).map_err(|e| {
            ConnectionError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            ))
        })?;

        let reg_entries: Vec<RegistryEntry> = entries
            .into_iter()
            .map(|(name, compound)| {
                let id = ResourceLocation::from_string(&name).map_err(|e| {
                    ConnectionError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        e.to_string(),
                    ))
                })?;
                Ok(RegistryEntry {
                    id,
                    data: Some(compound),
                })
            })
            .collect::<Result<Vec<_>, ConnectionError>>()?;

        let packet = ClientboundRegistryDataPacket {
            registry: registry_loc,
            entries: reg_entries,
        };

        let body = packet.encode();
        conn.send_raw(ClientboundRegistryDataPacket::PACKET_ID, &body)
            .await?;
    }
    conn.flush().await?;
    debug!(
        peer = %addr,
        count = registry::SYNCHRONIZED_REGISTRIES.len(),
        "Sent all registry data",
    );

    // 4. Send tags (block, item, fluid, entity_type, enchantment, etc.)
    let tags_packet = registry::build_tags_packet();
    let tag_count = tags_packet.tags.len();
    conn.send_raw(
        ClientboundUpdateTagsPacket::PACKET_ID,
        &tags_packet.encode(),
    )
    .await?;
    conn.flush().await?;
    debug!(peer = %addr, registries = tag_count, "Sent tags");

    // 5. Send enabled features (vanilla feature set)
    let vanilla_feature = ResourceLocation::from_string("minecraft:vanilla").map_err(|e| {
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })?;
    let features_packet = ClientboundUpdateEnabledFeaturesPacket {
        features: vec![vanilla_feature],
    };
    conn.send_raw(
        ClientboundUpdateEnabledFeaturesPacket::PACKET_ID,
        &features_packet.encode(),
    )
    .await?;
    conn.flush().await?;
    debug!(peer = %addr, "Sent enabled features");

    // 6. Send FinishConfiguration
    let finish = ClientboundFinishConfigurationPacket;
    conn.send_raw(
        ClientboundFinishConfigurationPacket::PACKET_ID,
        &finish.encode(),
    )
    .await?;
    conn.flush().await?;
    debug!(peer = %addr, "Sent finish configuration");

    // 7. Wait for client FinishConfiguration acknowledgement.
    //    The client may send ClientInformation or CustomPayload again.
    loop {
        let finish_pkt = conn.read_raw_packet().await?;
        match finish_pkt.id {
            ServerboundClientInformationPacket::PACKET_ID => {
                let info_pkt = ServerboundClientInformationPacket::decode(finish_pkt.data)
                    .map_err(|e| {
                        ConnectionError::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            e.to_string(),
                        ))
                    })?;
                debug!(
                    peer = %addr,
                    language = %info_pkt.information.language,
                    view_distance = info_pkt.information.view_distance,
                    "Received updated client information",
                );
                client_info = Some(info_pkt.information);
            },
            SB_CUSTOM_PAYLOAD => {
                debug!(peer = %addr, "Received custom payload (ignored)");
            },
            ServerboundFinishConfigurationPacket::PACKET_ID => {
                break;
            },
            _ => {
                warn!(peer = %addr, id = finish_pkt.id, "Expected FinishConfiguration");
                return Err(disconnect_err(
                    conn,
                    "Unexpected packet — expected finish configuration",
                )
                .await);
            },
        }
    }

    // Use client_info (or defaults) for this session
    let client_info = client_info.unwrap_or_else(ClientInformation::create_default);

    // 8. Transition to Play
    conn.state = ConnectionState::Play;
    info!(peer = %addr, "Configuration complete — client entering PLAY state");

    Ok(client_info)
}

/// Handles the PLAY-state login sequence for a newly joined player.
///
/// Creates a [`ServerPlayer`], loads saved player data (if available),
/// builds the 8-packet login sequence via [`build_login_sequence`], and
/// sends them to the client. Then enters a minimal read loop to process
/// incoming PLAY packets (currently only handles teleport confirmations).
///
/// # Errors
///
/// Returns a [`ConnectionError`] if any I/O or protocol step fails.
async fn handle_play_entry(
    conn: &mut Connection,
    profile: auth::GameProfile,
    client_info: ClientInformation,
    ctx: &LoginContext,
) -> Result<(), ConnectionError> {
    let addr = conn.remote_addr();
    let server_ctx = &ctx.server_ctx;

    let uuid = profile.uuid().ok_or_else(|| {
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Profile has invalid UUID",
        ))
    })?;

    // Create ServerPlayer with entity ID from the player list.
    let entity_id = server_ctx.player_list.read().next_entity_id();
    let game_mode = GameMode::from_id(server_ctx.level_data.game_type);
    let dimension = ResourceLocation::from_string("minecraft:overworld").map_err(|e| {
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })?;

    let mut player = ServerPlayer::new(entity_id, profile, dimension, game_mode);

    // Apply client preferences, capped to server maximums.
    player.view_distance =
        i32::from(client_info.view_distance).min(server_ctx.max_view_distance);
    player.simulation_distance =
        i32::from(client_info.view_distance).min(server_ctx.max_simulation_distance);

    // Try to load saved player data from playerdata/<uuid>.dat.
    let playerdata_path = format!("world/playerdata/{uuid}.dat");
    if Path::new(&playerdata_path).exists() {
        match oxidized_nbt::read_file(Path::new(&playerdata_path)) {
            Ok(nbt) => {
                player.load_from_nbt(&nbt);
                debug!(peer = %addr, uuid = %uuid, "Loaded player data from disk");
            },
            Err(e) => {
                warn!(peer = %addr, uuid = %uuid, error = %e, "Failed to load player data — using defaults");
            },
        }
    } else {
        // New player — spawn at world spawn.
        let (sx, sy, sz) = server_ctx.level_data.spawn_pos();
        player.pos =
            oxidized_protocol::types::Vec3::new(f64::from(sx), f64::from(sy), f64::from(sz));
        debug!(peer = %addr, uuid = %uuid, "New player — spawning at world spawn");
    }

    // Assign a teleport ID for the initial position packet.
    let teleport_id = player.next_teleport_id();
    player.pending_teleports.push_back(teleport_id);

    let player_name = player.name.clone();
    let player_view_distance = player.view_distance;
    let player_chunk_x = player.pos.x.floor() as i32;
    let player_chunk_z = player.pos.z.floor() as i32;

    // Build the 8-packet login sequence.
    let dimension_type_id = 0; // overworld = 0 in registry order
    let packets = {
        let player_list = server_ctx.player_list.read();
        build_login_sequence(
            &player,
            teleport_id,
            &server_ctx.level_data,
            &player_list,
            &server_ctx.dimensions,
            dimension_type_id,
        )
    };

    // Send all login packets before adding player to the list.
    // This prevents a "ghost" player entry if sending fails.
    for pkt in &packets {
        conn.send_raw(pkt.id, &pkt.body).await?;
    }
    conn.flush().await?;

    // Send chunk cache radius to the client.
    let cache_radius = ClientboundSetChunkCacheRadiusPacket {
        radius: player_view_distance,
    };
    conn.send_raw(
        ClientboundSetChunkCacheRadiusPacket::PACKET_ID,
        &cache_radius.encode(),
    )
    .await?;

    // Send initial chunk batch — empty air chunks in a spiral pattern.
    // Real chunk loading (from disk/worldgen) comes in later phases.
    let chunk_center = ChunkPos::from_block(player_chunk_x, player_chunk_z);
    let chunk_count = send_initial_chunks(conn, chunk_center, player_view_distance).await?;

    // Signal the client that initial chunks have been sent. Without this
    // event the client stays on the "Loading Terrain" screen indefinitely.
    let chunks_load_start = ClientboundGameEventPacket {
        event: GameEventType::LevelChunksLoadStart,
        param: 0.0,
    };
    conn.send_raw(
        ClientboundGameEventPacket::PACKET_ID,
        &chunks_load_start.encode(),
    )
    .await?;
    conn.flush().await?;

    info!(
        peer = %addr,
        uuid = %uuid,
        chunks = chunk_count,
        "Initial chunk batch sent",
    );

    // Add player to the server player list (only after successful send).
    let player_arc = server_ctx.player_list.write().add(player);

    // Track which chunks the player has loaded so we can send/forget on boundary crossings.
    let initial_chunk = ChunkPos::from_block(player_chunk_x, player_chunk_z);
    let mut chunk_tracker = PlayerChunkTracker::new(initial_chunk, player_view_distance);

    info!(
        peer = %addr,
        uuid = %uuid,
        name = %player_name,
        entity_id = entity_id,
        packets = packets.len(),
        "PLAY login sequence sent",
    );

    // PLAY read loop — handles movement, teleport confirmations, chunk batch
    // rate feedback, sprint/sneak state, and logs other packets.
    loop {
        match conn.read_raw_packet().await {
            Ok(pkt) => {
                if pkt.id == ServerboundAcceptTeleportationPacket::PACKET_ID {
                    match ServerboundAcceptTeleportationPacket::decode(pkt.data) {
                        Ok(ack) => {
                            let mut p = player_arc.write();
                            let accepted = handle_accept_teleportation(&mut p, ack.teleport_id);
                            debug!(
                                peer = %addr,
                                teleport_id = ack.teleport_id,
                                accepted = accepted,
                                pending = p.pending_teleports.len(),
                                "Teleport confirmation",
                            );
                        },
                        Err(e) => {
                            debug!(peer = %addr, error = %e, "Failed to decode teleport ack");
                        },
                    }
                } else if pkt.id == ServerboundChunkBatchReceivedPacket::PACKET_ID {
                    match ServerboundChunkBatchReceivedPacket::decode(pkt.data) {
                        Ok(batch_ack) => {
                            let rate = batch_ack.desired_chunks_per_tick;
                            if rate.is_finite() && rate > 0.0 {
                                player_arc.write().chunk_send_rate = rate.clamp(0.1, 100.0);
                                debug!(peer = %addr, rate, "Chunk batch rate update");
                            } else {
                                debug!(
                                    peer = %addr,
                                    invalid_rate = rate,
                                    "Ignored invalid chunk send rate",
                                );
                            }
                        },
                        Err(e) => {
                            debug!(peer = %addr, error = %e, "Failed to decode chunk batch ack");
                        },
                    }
                } else if pkt.id == ServerboundMovePlayerPacket::PACKET_ID_POS
                    || pkt.id == ServerboundMovePlayerPacket::PACKET_ID_POS_ROT
                    || pkt.id == ServerboundMovePlayerPacket::PACKET_ID_ROT
                    || pkt.id == ServerboundMovePlayerPacket::PACKET_ID_STATUS_ONLY
                {
                    let move_pkt = match pkt.id {
                        ServerboundMovePlayerPacket::PACKET_ID_POS => {
                            ServerboundMovePlayerPacket::decode_pos(pkt.data)
                        },
                        ServerboundMovePlayerPacket::PACKET_ID_POS_ROT => {
                            ServerboundMovePlayerPacket::decode_pos_rot(pkt.data)
                        },
                        ServerboundMovePlayerPacket::PACKET_ID_ROT => {
                            ServerboundMovePlayerPacket::decode_rot(pkt.data)
                        },
                        _ => ServerboundMovePlayerPacket::decode_status_only(pkt.data),
                    };

                    match move_pkt {
                        Ok(move_pkt) => {
                            if move_pkt.contains_invalid_values() {
                                debug!(peer = %addr, "Movement packet contains invalid values");
                                continue;
                            }

                            let result = {
                                let p = player_arc.read();
                                validate_movement(
                                    p.pos,
                                    p.yaw,
                                    p.pitch,
                                    move_pkt.x,
                                    move_pkt.y,
                                    move_pkt.z,
                                    move_pkt.yaw,
                                    move_pkt.pitch,
                                )
                            };

                            if result.needs_correction {
                                let correction = {
                                    let mut p = player_arc.write();
                                    let teleport_id = p.next_teleport_id();
                                    p.pending_teleports.push_back(teleport_id);
                                    ClientboundPlayerPositionPacket {
                                        teleport_id,
                                        x: p.pos.x,
                                        y: p.pos.y,
                                        z: p.pos.z,
                                        dx: 0.0,
                                        dy: 0.0,
                                        dz: 0.0,
                                        yaw: p.yaw,
                                        pitch: p.pitch,
                                        relative_flags: RelativeFlags::empty(),
                                    }
                                };
                                conn.send_raw(
                                    ClientboundPlayerPositionPacket::PACKET_ID,
                                    &correction.encode(),
                                )
                                .await?;
                                conn.flush().await?;
                                debug!(peer = %addr, "Position correction sent");
                            } else {
                                {
                                    let mut p = player_arc.write();
                                    p.pos = result.new_pos;
                                    p.yaw = result.new_yaw;
                                    p.pitch = result.new_pitch;
                                    p.on_ground = move_pkt.on_ground;
                                }

                                // Check if player crossed a chunk boundary
                                if move_pkt.has_pos() {
                                    let new_chunk = ChunkPos::from_block(
                                        result.new_pos.x.floor() as i32,
                                        result.new_pos.z.floor() as i32,
                                    );
                                    let (to_load, to_unload) =
                                        chunk_tracker.update_center(new_chunk);

                                    if !to_load.is_empty() || !to_unload.is_empty() {
                                        let center_pkt = ClientboundSetChunkCacheCenterPacket {
                                            chunk_x: new_chunk.x,
                                            chunk_z: new_chunk.z,
                                        };
                                        conn.send_raw(
                                            ClientboundSetChunkCacheCenterPacket::PACKET_ID,
                                            &center_pkt.encode(),
                                        )
                                        .await?;

                                        for pos in &to_unload {
                                            let forget = ClientboundForgetLevelChunkPacket {
                                                chunk_x: pos.x,
                                                chunk_z: pos.z,
                                            };
                                            conn.send_raw(
                                                ClientboundForgetLevelChunkPacket::PACKET_ID,
                                                &forget.encode(),
                                            )
                                            .await?;
                                        }

                                        if !to_load.is_empty() {
                                            conn.send_raw(
                                                ClientboundChunkBatchStartPacket::PACKET_ID,
                                                &ClientboundChunkBatchStartPacket.encode(),
                                            )
                                            .await?;

                                            for pos in &to_load {
                                                let chunk = LevelChunk::new(*pos);
                                                let chunk_pkt = build_chunk_packet(&chunk);
                                                conn.send_raw(
                                                    ClientboundLevelChunkWithLightPacket::PACKET_ID,
                                                    &chunk_pkt.encode(),
                                                )
                                                .await?;
                                            }

                                            let batch_finished =
                                                ClientboundChunkBatchFinishedPacket {
                                                    batch_size: to_load.len() as i32,
                                                };
                                            conn.send_raw(
                                                ClientboundChunkBatchFinishedPacket::PACKET_ID,
                                                &batch_finished.encode(),
                                            )
                                            .await?;
                                        }

                                        conn.flush().await?;

                                        debug!(
                                            peer = %addr,
                                            loaded = to_load.len(),
                                            unloaded = to_unload.len(),
                                            center_x = new_chunk.x,
                                            center_z = new_chunk.z,
                                            "Chunk boundary crossed",
                                        );
                                    }
                                }
                            }
                        },
                        Err(e) => {
                            debug!(peer = %addr, error = %e, "Failed to decode movement packet");
                        },
                    }
                } else if pkt.id == ServerboundPlayerCommandPacket::PACKET_ID {
                    match ServerboundPlayerCommandPacket::decode(pkt.data) {
                        Ok(cmd) => match cmd.action {
                            PlayerCommandAction::StartSprinting => {
                                player_arc.write().sprinting = true;
                                debug!(peer = %addr, "Player started sprinting");
                            },
                            PlayerCommandAction::StopSprinting => {
                                player_arc.write().sprinting = false;
                                debug!(peer = %addr, "Player stopped sprinting");
                            },
                            _ => {
                                debug!(
                                    peer = %addr,
                                    action = ?cmd.action,
                                    "Player command (unhandled action)",
                                );
                            },
                        },
                        Err(e) => {
                            debug!(peer = %addr, error = %e, "Failed to decode player command");
                        },
                    }
                } else if pkt.id == ServerboundPlayerInputPacket::PACKET_ID {
                    match ServerboundPlayerInputPacket::decode(pkt.data) {
                        Ok(input_pkt) => {
                            let mut p = player_arc.write();
                            p.sneaking = input_pkt.input.shift;
                            p.sprinting = input_pkt.input.sprint;
                        },
                        Err(e) => {
                            debug!(peer = %addr, error = %e, "Failed to decode player input");
                        },
                    }
                } else {
                    debug!(
                        peer = %addr,
                        packet_id = format_args!("0x{:02X}", pkt.id),
                        size = pkt.data.len(),
                        "PLAY packet (unhandled)",
                    );
                }
            },
            Err(ConnectionError::Io(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                info!(peer = %addr, name = %player_name, "Player disconnected");
                break;
            },
            Err(e) => {
                debug!(peer = %addr, error = %e, "PLAY connection error");
                break;
            },
        }
    }

    // Clean up — remove player from the list.
    server_ctx.player_list.write().remove(&uuid);
    info!(peer = %addr, name = %player_name, "Player removed from player list");

    Ok(())
}

/// Sends the initial chunk batch for a player joining the world.
///
/// Creates empty air chunks in a spiral pattern around the player and sends
/// them wrapped in `ChunkBatchStart` / `ChunkBatchFinished` framing.
///
/// Real chunk loading from disk or worldgen is not yet implemented — this
/// sends purely air so the client has valid chunk data and renders the world.
///
/// Returns the number of chunks sent.
async fn send_initial_chunks(
    conn: &mut Connection,
    center: ChunkPos,
    view_distance: i32,
) -> Result<i32, ConnectionError> {
    // Start the chunk batch.
    conn.send_raw(
        ClientboundChunkBatchStartPacket::PACKET_ID,
        &ClientboundChunkBatchStartPacket.encode(),
    )
    .await?;

    let mut count: i32 = 0;
    for chunk_pos in spiral_chunks(center, view_distance) {
        let chunk = LevelChunk::new(chunk_pos);
        let pkt = build_chunk_packet(&chunk);
        conn.send_raw(
            ClientboundLevelChunkWithLightPacket::PACKET_ID,
            &pkt.encode(),
        )
        .await?;
        count += 1;
    }

    // Finish the chunk batch.
    let finished = ClientboundChunkBatchFinishedPacket { batch_size: count };
    conn.send_raw(
        ClientboundChunkBatchFinishedPacket::PACKET_ID,
        &finished.encode(),
    )
    .await?;
    conn.flush().await?;

    Ok(count)
}

/// Sends a disconnect packet to the client and returns a corresponding
/// [`ConnectionError`].
async fn disconnect(conn: &mut Connection, reason: &str) -> Result<(), ConnectionError> {
    let pkt = ClientboundDisconnectPacket {
        reason: reason.to_string(),
    };
    let body = pkt.encode();
    let _ = conn
        .send_raw(ClientboundDisconnectPacket::PACKET_ID, &body)
        .await;
    let _ = conn.flush().await;
    Err(ConnectionError::Io(std::io::Error::new(
        std::io::ErrorKind::ConnectionAborted,
        reason.to_string(),
    )))
}

/// Sends a disconnect packet and returns the [`ConnectionError`] directly
/// (for use in expressions where the caller builds its own `Err`).
async fn disconnect_err(conn: &mut Connection, reason: &str) -> ConnectionError {
    let pkt = ClientboundDisconnectPacket {
        reason: reason.to_string(),
    };
    let body = pkt.encode();
    let _ = conn
        .send_raw(ClientboundDisconnectPacket::PACKET_ID, &body)
        .await;
    let _ = conn.flush().await;
    ConnectionError::Io(std::io::Error::new(
        std::io::ErrorKind::ConnectionAborted,
        reason.to_string(),
    ))
}

/// Processes STATUS state packets. Returns `true` when the exchange is
/// complete (after sending the pong), signaling the connection should close.
async fn handle_status(
    conn: &mut Connection,
    pkt: RawPacket,
    server_status: &ServerStatus,
) -> Result<bool, ConnectionError> {
    match pkt.id {
        ServerboundStatusRequestPacket::PACKET_ID => {
            let _request = ServerboundStatusRequestPacket::decode(pkt.data);

            let response = ClientboundStatusResponsePacket {
                status_json: server_status
                    .to_json()
                    .map_err(|e| ConnectionError::Io(std::io::Error::other(e.to_string())))?,
            };
            let body = response.encode();
            conn.send_raw(ClientboundStatusResponsePacket::PACKET_ID, &body)
                .await?;
            conn.flush().await?;

            debug!(
                peer = %conn.remote_addr(),
                json_len = response.status_json.len(),
                "Sent status response",
            );
            Ok(false)
        },
        ServerboundPingRequestPacket::PACKET_ID => {
            let ping = ServerboundPingRequestPacket::decode(pkt.data).map_err(|e| {
                ConnectionError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.to_string(),
                ))
            })?;

            let pong = ClientboundPongResponsePacket { time: ping.time };
            let body = pong.encode();
            conn.send_raw(ClientboundPongResponsePacket::PACKET_ID, &body)
                .await?;
            conn.flush().await?;

            debug!(
                peer = %conn.remote_addr(),
                time = ping.time,
                "Sent pong response",
            );
            Ok(true) // Close after pong (vanilla behavior)
        },
        unknown => {
            warn!(
                peer = %conn.remote_addr(),
                packet_id = unknown,
                "Unknown status packet",
            );
            Ok(false)
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use bytes::BytesMut;
    use oxidized_protocol::codec::{frame, varint};
    use oxidized_protocol::constants;
    use oxidized_protocol::crypto::ServerKeyPair;
    use oxidized_protocol::status::{Component, StatusPlayers, StatusVersion};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    fn test_server_status() -> ServerStatus {
        ServerStatus {
            version: StatusVersion {
                name: constants::VERSION_NAME.to_string(),
                protocol: constants::PROTOCOL_VERSION,
            },
            players: StatusPlayers {
                max: 20,
                online: 0,
                sample: Vec::new(),
            },
            description: Component::text("Test Server"),
            favicon: None,
            enforces_secure_chat: false,
        }
    }

    fn test_login_context() -> Arc<LoginContext> {
        Arc::new(LoginContext {
            server_status: Arc::new(test_server_status()),
            keypair: Arc::new(ServerKeyPair::generate().unwrap()),
            online_mode: false,
            compression_threshold: -1,
            prevent_proxy_connections: false,
            http_client: reqwest::Client::new(),
            server_ctx: Arc::new(ServerContext {
                player_list: RwLock::new(PlayerList::new(20)),
                level_data: PrimaryLevelData::from_nbt(&oxidized_nbt::NbtCompound::new()).unwrap(),
                dimensions: vec![ResourceLocation::from_string("minecraft:overworld").unwrap()],
                max_view_distance: 10,
                max_simulation_distance: 10,
            }),
        })
    }

    /// Sends a framed packet (VarInt length + VarInt packet_id + body) over a raw stream.
    async fn send_packet(stream: &mut TcpStream, packet_id: i32, body: &[u8]) {
        let mut inner = BytesMut::new();
        varint::write_varint_buf(packet_id, &mut inner);
        inner.extend_from_slice(body);
        frame::write_frame(stream, &inner).await.unwrap();
        stream.flush().await.unwrap();
    }

    /// Reads one framed packet and returns (packet_id, body).
    async fn read_packet(stream: &mut TcpStream) -> (i32, bytes::Bytes) {
        let frame_data =
            frame::read_frame(stream, oxidized_protocol::codec::frame::MAX_PACKET_SIZE)
                .await
                .unwrap();
        let mut buf = frame_data;
        let id = varint::read_varint_buf(&mut buf).unwrap();
        (id, buf)
    }

    #[tokio::test]
    async fn test_full_status_exchange() {
        let ctx = test_login_context();
        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        // Bind to a random port
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let tcp_listener = TcpListener::bind(addr).await.unwrap();
        let bound_addr = tcp_listener.local_addr().unwrap();
        drop(tcp_listener);

        let shutdown_rx = shutdown_tx.subscribe();
        let ctx_clone = Arc::clone(&ctx);
        let server = tokio::spawn(async move {
            listen(bound_addr, ctx_clone, shutdown_rx).await.unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let mut client = TcpStream::connect(bound_addr).await.unwrap();

        // 1. Send handshake (intent = Status)
        let handshake = ClientIntentionPacket {
            protocol_version: constants::PROTOCOL_VERSION,
            server_address: "localhost".to_string(),
            server_port: 25565,
            next_state: ClientIntent::Status,
        };
        let handshake_body = handshake.encode();
        send_packet(
            &mut client,
            ClientIntentionPacket::PACKET_ID,
            &handshake_body,
        )
        .await;

        // 2. Send status request (empty body)
        send_packet(&mut client, ServerboundStatusRequestPacket::PACKET_ID, &[]).await;

        // 3. Read status response
        let (resp_id, resp_body) = read_packet(&mut client).await;
        assert_eq!(resp_id, ClientboundStatusResponsePacket::PACKET_ID);

        // Parse the response JSON string
        use oxidized_protocol::codec::types;
        let mut resp_data = resp_body;
        let json_str = types::read_string(&mut resp_data, 32767).unwrap();
        let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(json["version"]["protocol"], constants::PROTOCOL_VERSION);
        assert_eq!(json["description"]["text"], "Test Server");
        assert_eq!(json["players"]["max"], 20);

        // 4. Send ping request
        let ping_time: i64 = 1_719_000_000_000;
        let mut ping_body = BytesMut::new();
        oxidized_protocol::codec::types::write_i64(&mut ping_body, ping_time);
        send_packet(
            &mut client,
            ServerboundPingRequestPacket::PACKET_ID,
            &ping_body,
        )
        .await;

        // 5. Read pong response
        let (pong_id, pong_body) = read_packet(&mut client).await;
        assert_eq!(pong_id, ClientboundPongResponsePacket::PACKET_ID);
        let mut pong_data = pong_body;
        let echoed_time = types::read_i64(&mut pong_data).unwrap();
        assert_eq!(echoed_time, ping_time);

        // 6. Server should have closed our connection after pong
        // Reading should return EOF
        let mut eof_buf = [0u8; 1];
        let read_result = client.read(&mut eof_buf).await.unwrap();
        assert_eq!(read_result, 0, "expected EOF after pong");

        // Clean up
        let _ = shutdown_tx.send(());
        let _ = server.await;
    }

    #[tokio::test]
    async fn test_protocol_mismatch_still_responds() {
        let ctx = test_login_context();
        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let tcp_listener = TcpListener::bind(addr).await.unwrap();
        let bound_addr = tcp_listener.local_addr().unwrap();
        drop(tcp_listener);

        let shutdown_rx = shutdown_tx.subscribe();
        let ctx_clone = Arc::clone(&ctx);
        let server = tokio::spawn(async move {
            listen(bound_addr, ctx_clone, shutdown_rx).await.unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let mut client = TcpStream::connect(bound_addr).await.unwrap();

        // Send handshake with WRONG protocol version
        let handshake = ClientIntentionPacket {
            protocol_version: 999,
            server_address: "localhost".to_string(),
            server_port: 25565,
            next_state: ClientIntent::Status,
        };
        let handshake_body = handshake.encode();
        send_packet(
            &mut client,
            ClientIntentionPacket::PACKET_ID,
            &handshake_body,
        )
        .await;

        // Send status request
        send_packet(&mut client, ServerboundStatusRequestPacket::PACKET_ID, &[]).await;

        // Should still get a valid response (vanilla behavior)
        let (resp_id, resp_body) = read_packet(&mut client).await;
        assert_eq!(resp_id, ClientboundStatusResponsePacket::PACKET_ID);

        use oxidized_protocol::codec::types;
        let mut resp_data = resp_body;
        let json_str = types::read_string(&mut resp_data, 32767).unwrap();
        let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(json["version"]["protocol"], constants::PROTOCOL_VERSION);

        let _ = shutdown_tx.send(());
        let _ = server.await;
    }

    #[tokio::test]
    async fn test_listener_graceful_shutdown() {
        let ctx = test_login_context();
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let tcp_listener = TcpListener::bind(addr).await.unwrap();
        let bound_addr = tcp_listener.local_addr().unwrap();
        drop(tcp_listener);

        let shutdown_rx = shutdown_tx.subscribe();
        let server = tokio::spawn(async move {
            listen(bound_addr, ctx, shutdown_rx).await.unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let _ = shutdown_tx.send(());

        let result = tokio::time::timeout(tokio::time::Duration::from_secs(2), server).await;
        assert!(result.is_ok(), "server should shut down within 2 seconds");
    }
}
