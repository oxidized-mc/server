//! Login state handler.
//!
//! Reads the initial [`ServerboundHelloPacket`], performs online-mode
//! authentication (encryption + Mojang session server) or offline-mode
//! UUID generation, optionally enables compression, and transitions
//! to [`ConnectionState::Configuration`].

use oxidized_protocol::auth;
use oxidized_protocol::connection::{Connection, ConnectionError, ConnectionState, RawPacket};
use oxidized_protocol::crypto::{generate_challenge, minecraft_digest, offline_uuid};
use oxidized_protocol::packets::login::clientbound_login_finished::ProfileProperty;
use oxidized_protocol::packets::login::{
    ClientboundHelloPacket, ClientboundLoginCompressionPacket, ClientboundLoginFinishedPacket,
    ServerboundHelloPacket, ServerboundKeyPacket, ServerboundLoginAcknowledgedPacket,
};
use tracing::{debug, info, warn};

use super::LoginContext;
use super::helpers::{decode_packet, disconnect, disconnect_err};

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
pub async fn handle_login(
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

    let hello: ServerboundHelloPacket =
        decode_packet(hello_pkt.data, addr, "", "LoginHello")?;

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
        conn.send_packet(&compression_pkt).await?;

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
    conn.send_packet(&finished).await?;

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
    conn.send_packet(&hello_response).await?;

    debug!(peer = %addr, "Encryption request sent");

    // c. Read ServerboundKeyPacket.
    let key_pkt = conn.read_raw_packet().await?;
    if key_pkt.id != ServerboundKeyPacket::PACKET_ID {
        warn!(peer = %addr, packet_id = key_pkt.id, "Expected key response (0x01)");
        return Err(disconnect_err(conn, "Unexpected packet — expected encryption response").await);
    }

    let key: ServerboundKeyPacket =
        decode_packet(key_pkt.data, addr, "", "KeyResponse")?;

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
    info!(peer = %addr, uuid = %profile.uuid().unwrap_or_default(), name = %profile.name(), "Player authenticated");

    Ok(profile)
}
