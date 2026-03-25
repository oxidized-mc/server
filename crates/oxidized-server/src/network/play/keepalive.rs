//! Keepalive packet handling.
//!
//! Sends periodic pings and validates responses. Computes latency as an
//! exponential moving average matching vanilla behavior.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use oxidized_game::player::ServerPlayer;
use parking_lot::RwLock;
use tracing::{debug, warn};

use crate::network::helpers::decode_packet;
use oxidized_protocol::packets::play::ServerboundKeepAlivePacket;

/// Result of processing a keepalive response.
pub(super) enum KeepaliveResult {
    /// Valid response — contains (uuid, latency) for broadcast.
    Ok(uuid::Uuid, i32),
    /// Decode error — ignore silently.
    DecodeError,
    /// Wrong challenge ID or not pending — vanilla disconnects.
    Mismatch,
}

/// Handles a keepalive response from the client.
///
/// Computes latency as an exponential moving average:
/// `latency = (old * 3 + sample) / 4` (matching vanilla).
///
/// Returns [`KeepaliveResult`] so the caller can broadcast latency updates
/// or disconnect on mismatch (matching vanilla behavior).
pub(super) fn handle_keepalive(
    data: bytes::Bytes,
    addr: SocketAddr,
    player_name: &str,
    keepalive_pending: &mut bool,
    keepalive_challenge: i64,
    keepalive_sent_at: &Instant,
    player: &Arc<RwLock<ServerPlayer>>,
) -> KeepaliveResult {
    let ka = match decode_packet::<ServerboundKeepAlivePacket>(data, addr, player_name, "KeepAlive")
    {
        Ok(pkt) => pkt,
        Err(_) => return KeepaliveResult::DecodeError,
    };

    if *keepalive_pending && ka.id == keepalive_challenge {
        *keepalive_pending = false;
        let sample = keepalive_sent_at.elapsed().as_millis() as i32;
        let mut p = player.write();
        p.connection.latency = (p.connection.latency * 3 + sample) / 4;
        let latency = p.connection.latency;
        let uuid = p.uuid;
        debug!(peer = %addr, name = %player_name, latency_ms = latency, "Keepalive response");
        KeepaliveResult::Ok(uuid, latency)
    } else {
        warn!(
            peer = %addr,
            name = %player_name,
            expected = keepalive_challenge,
            got = ka.id,
            pending = *keepalive_pending,
            "Keepalive mismatch — disconnecting",
        );
        KeepaliveResult::Mismatch
    }
}
