//! Status state handler.
//!
//! Responds to server list ping requests with the server's current status
//! (player count, version, MOTD) and echoes pong packets.

use oxidized_codec::Packet;
use oxidized_protocol::packets::status::{
    ClientboundPongResponsePacket, ClientboundStatusResponsePacket, ServerboundPingRequestPacket,
    ServerboundStatusRequestPacket,
};
use oxidized_protocol::status::ServerStatus;
use oxidized_transport::connection::{Connection, ConnectionError, RawPacket};
use tracing::{debug, warn};

use super::LoginContext;
use super::helpers::decode_packet;

/// Processes STATUS state packets.
///
/// Returns `true` when the exchange is complete (after sending the pong),
/// signaling the connection should close.
///
/// `has_requested_status` tracks whether a status request has already been
/// received on this connection. Vanilla disconnects on a duplicate request.
pub async fn handle_status(
    conn: &mut Connection,
    pkt: RawPacket,
    ctx: &LoginContext,
    has_requested_status: &mut bool,
) -> Result<bool, ConnectionError> {
    match pkt.id {
        ServerboundStatusRequestPacket::PACKET_ID => {
            let _request = ServerboundStatusRequestPacket::decode(pkt.data);

            if *has_requested_status {
                debug!(
                    peer = %conn.remote_addr(),
                    "Duplicate status request — disconnecting",
                );
                return Err(ConnectionError::Io(std::io::Error::other(
                    "duplicate status request",
                )));
            }
            *has_requested_status = true;

            // Build the status dynamically from live server state so
            // player count and sample are always up to date.
            let server_ctx = &ctx.server_ctx;
            let (online, sample) = {
                let player_list = server_ctx.network.player_list.read();
                let count = player_list.player_count() as u32;
                let entries: Vec<_> = player_list
                    .iter()
                    .take(12) // Vanilla caps the sample at 12
                    .map(|p| {
                        let p = p.read();
                        oxidized_protocol::status::PlayerSample {
                            name: p.name.clone(),
                            id: p.uuid,
                        }
                    })
                    .collect();
                (count, entries)
            };

            let base = &*ctx.server_status;
            let live_status = ServerStatus {
                version: base.version.clone(),
                players: oxidized_protocol::status::StatusPlayers {
                    max: server_ctx.network.max_players as u32,
                    online,
                    sample,
                },
                description: base.description.clone(),
                favicon: base.favicon.clone(),
                is_secure_chat_enforced: base.is_secure_chat_enforced,
            };

            let response = ClientboundStatusResponsePacket {
                status_json: live_status
                    .to_json()
                    .map_err(|e| ConnectionError::Io(std::io::Error::other(e.to_string())))?,
            };
            conn.send_packet(&response).await?;

            debug!(
                peer = %conn.remote_addr(),
                online = online,
                json_len = response.status_json.len(),
                "Sent status response",
            );
            Ok(false)
        },
        ServerboundPingRequestPacket::PACKET_ID => {
            let ping: ServerboundPingRequestPacket =
                decode_packet(pkt.data, conn.remote_addr(), "", "Ping")?;

            let pong = ClientboundPongResponsePacket { time: ping.time };
            conn.send_packet(&pong).await?;

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
