//! TCP listener and per-connection handler for the Oxidized server.
//!
//! Binds to the configured address, accepts incoming connections, and
//! spawns a Tokio task per client. Handles the HANDSHAKING, STATUS,
//! LOGIN, and CONFIGURATION protocol states.

use std::net::SocketAddr;
use std::sync::Arc;

use oxidized_protocol::auth;
use oxidized_protocol::connection::{Connection, ConnectionError, ConnectionState, RawPacket};
use oxidized_protocol::crypto::{
    generate_challenge, minecraft_digest, offline_uuid, ServerKeyPair,
};
use oxidized_protocol::packets::configuration::{
    ClientInformation, ClientboundFinishConfigurationPacket, ClientboundRegistryDataPacket,
    ClientboundSelectKnownPacksPacket, ClientboundUpdateEnabledFeaturesPacket,
    ClientboundUpdateTagsPacket, KnownPack, RegistryEntry,
    ServerboundClientInformationPacket, ServerboundFinishConfigurationPacket,
    ServerboundSelectKnownPacksPacket,
};
use oxidized_protocol::packets::handshake::{ClientIntent, ClientIntentionPacket};
use oxidized_protocol::packets::login::clientbound_login_finished::ProfileProperty;
use oxidized_protocol::packets::login::{
    ClientboundDisconnectPacket, ClientboundHelloPacket, ClientboundLoginCompressionPacket,
    ClientboundLoginFinishedPacket, ServerboundHelloPacket, ServerboundKeyPacket,
    ServerboundLoginAcknowledgedPacket,
};
use oxidized_protocol::packets::status::{
    ClientboundPongResponsePacket, ClientboundStatusResponsePacket, ServerboundPingRequestPacket,
    ServerboundStatusRequestPacket,
};
use oxidized_protocol::registry;
use oxidized_protocol::status::ServerStatus;
use oxidized_protocol::types::resource_location::ResourceLocation;
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
                        handle_login(&mut conn, pkt, ctx).await?;
                        // Login transitions to Configuration — handle it immediately
                        // (server drives the configuration flow, not client)
                        handle_configuration(&mut conn, ctx).await?;
                        info!(peer = %addr, "Configuration complete — entering play");
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
/// # Errors
///
/// Returns a [`ConnectionError`] if any I/O, decoding, or authentication
/// step fails. On recoverable failures (bad name, failed auth) a
/// disconnect packet is sent before returning the error.
async fn handle_login(
    conn: &mut Connection,
    hello_pkt: RawPacket,
    ctx: &LoginContext,
) -> Result<(), ConnectionError> {
    let addr = conn.remote_addr();

    // 1. Decode ServerboundHelloPacket (the first Login packet).
    if hello_pkt.id != ServerboundHelloPacket::PACKET_ID {
        warn!(peer = %addr, packet_id = hello_pkt.id, "Expected login hello packet (0x00)");
        return disconnect(conn, "Unexpected packet during login").await;
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
        return disconnect(conn, "Invalid player name").await;
    }

    // 3. Authenticate (online) or generate offline UUID.
    let (uuid, username, properties) = if ctx.online_mode {
        authenticate_online(conn, &hello, ctx).await?
    } else {
        let uuid = offline_uuid(&hello.name);
        debug!(peer = %addr, uuid = %uuid, name = %hello.name, "Offline-mode UUID generated");
        (uuid, hello.name.clone(), Vec::new())
    };

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
        return disconnect(conn, "Unexpected packet — expected login acknowledged").await;
    }
    let _ack = ServerboundLoginAcknowledgedPacket::decode(ack_pkt.data);

    // 7. Transition to Configuration state.
    conn.state = ConnectionState::Configuration;
    info!(peer = %addr, uuid = %uuid, name = %username, "Player login complete — entering configuration");

    Ok(())
}

/// Performs online-mode authentication: encryption handshake, shared secret
/// exchange, and Mojang session server verification.
///
/// Returns the authenticated player's UUID, username, and profile properties.
///
/// # Errors
///
/// Returns a [`ConnectionError`] if encryption setup, challenge verification,
/// or session server authentication fails.
async fn authenticate_online(
    conn: &mut Connection,
    hello: &ServerboundHelloPacket,
    ctx: &LoginContext,
) -> Result<(uuid::Uuid, String, Vec<ProfileProperty>), ConnectionError> {
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

    // i. Extract profile data.
    let uuid = profile.uuid().ok_or_else(|| {
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Mojang returned invalid UUID",
        ))
    })?;

    let properties = profile
        .properties()
        .iter()
        .map(|p| ProfileProperty {
            name: p.name().to_string(),
            value: p.value().to_string(),
            signature: p.signature().map(String::from),
        })
        .collect();

    info!(peer = %addr, uuid = %uuid, name = %profile.name(), "Player authenticated");

    Ok((uuid, profile.name().to_string(), properties))
}

/// Handles the CONFIGURATION state — sends registry data, tags, features,
/// and transitions the client to PLAY.
///
/// The configuration flow is server-driven:
/// 1. Send `ClientboundSelectKnownPacksPacket`
/// 2. Receive `ServerboundSelectKnownPacksPacket`
/// 3. Send `ClientboundRegistryDataPacket` × N (one per synchronized registry)
/// 4. Send `ClientboundUpdateTagsPacket` (empty — no block/item registries yet)
/// 5. Send `ClientboundUpdateEnabledFeaturesPacket` (vanilla features)
/// 6. Send `ClientboundFinishConfigurationPacket`
/// 7. Receive `ServerboundFinishConfigurationPacket`
/// 8. Transition to PLAY state
///
/// # Errors
///
/// Returns a [`ConnectionError`] if any I/O, decoding, or protocol step fails.
async fn handle_configuration(
    conn: &mut Connection,
    _ctx: &LoginContext,
) -> Result<(), ConnectionError> {
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
    //    The client may send ClientInformation (0x00) before responding.
    loop {
        let pkt = conn.read_raw_packet().await?;
        match pkt.id {
            ServerboundClientInformationPacket::PACKET_ID => {
                let info_pkt = ServerboundClientInformationPacket::decode(pkt.data)
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
                    "Received client information",
                );
                client_info = Some(info_pkt.information);
            }
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
            }
            _ => {
                warn!(peer = %addr, id = pkt.id, "Unexpected packet during configuration");
                return disconnect(conn, "Unexpected packet during configuration").await;
            }
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

    // 4. Send empty tags (full tag support requires block/item registries)
    let tags_packet = ClientboundUpdateTagsPacket { tags: vec![] };
    conn.send_raw(
        ClientboundUpdateTagsPacket::PACKET_ID,
        &tags_packet.encode(),
    )
    .await?;
    conn.flush().await?;
    debug!(peer = %addr, "Sent empty tags");

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
    //    The client may send ClientInformation again if settings changed.
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
            }
            ServerboundFinishConfigurationPacket::PACKET_ID => {
                break;
            }
            _ => {
                warn!(peer = %addr, id = finish_pkt.id, "Expected FinishConfiguration");
                return disconnect(
                    conn,
                    "Unexpected packet — expected finish configuration",
                )
                .await;
            }
        }
    }

    // Use client_info (or defaults) for this session
    let _client_info = client_info.unwrap_or_else(ClientInformation::create_default);

    // 8. Transition to Play
    conn.state = ConnectionState::Play;
    info!(peer = %addr, "Configuration complete — client entering PLAY state");

    Ok(())
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
