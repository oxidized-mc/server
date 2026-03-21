//! Chat message and chat command handling.
//!
//! Processes incoming chat messages with rate limiting and color code
//! support, and dispatches `/commands` through the Brigadier framework.

use std::sync::Arc;

use oxidized_protocol::chat::Component;
use oxidized_protocol::codec::Packet;
use oxidized_protocol::connection::{Connection, ConnectionError};
use oxidized_protocol::packets::play::ClientboundSystemChatPacket;
use tracing::{debug, info, warn};

use super::PlayContext;
use super::commands::make_command_source_for_player;
use crate::network::{ChatBroadcastMessage, ServerContext};

/// Handles an incoming chat message from a player.
pub async fn handle_chat(ctx: &mut PlayContext<'_>, message: &str) -> Result<(), ConnectionError> {
    if !ctx.rate_limiter.try_acquire(std::time::Instant::now()) {
        warn!(peer = %ctx.addr, name = %ctx.player_name, "Chat rate-limited");
        let kick_msg = ClientboundSystemChatPacket {
            content: Component::text("You are sending messages too quickly"),
            overlay: false,
        };
        let _ = ctx.conn.send_packet(&kick_msg).await;
        return Ok(());
    }

    let message_component = match ctx.server_ctx.color_char {
        Some(ch) => Component::from_legacy_with_char(message, ch),
        None => Component::from_legacy(message),
    };
    let decorated = Component::translatable(
        "chat.type.text".to_string(),
        vec![Component::text(ctx.player_name), message_component],
    );
    let sys_pkt = ClientboundSystemChatPacket {
        content: decorated,
        overlay: false,
    };
    let encoded = sys_pkt.encode();
    let broadcast_msg = ChatBroadcastMessage {
        packet_id: ClientboundSystemChatPacket::PACKET_ID,
        data: encoded.freeze(),
    };
    let _ = ctx.server_ctx.chat_tx.send(broadcast_msg);
    info!(
        peer = %ctx.addr,
        player = %ctx.player_name,
        message = %message,
        "Chat message",
    );

    Ok(())
}

/// Handles a `/command` from a player using the Brigadier dispatcher.
pub async fn handle_chat_command(
    conn: &mut Connection,
    command: &str,
    player_name: &str,
    player_uuid: uuid::Uuid,
    player_pos: (f64, f64, f64),
    player_rot: (f32, f32),
    permission_level: u32,
    server_ctx: &Arc<ServerContext>,
) {
    // Collect feedback messages via a channel so they go ONLY to the
    // executing player, not broadcast to everyone.
    let (feedback_tx, feedback_rx) = std::sync::mpsc::channel::<Component>();
    let source = make_command_source_for_player(
        player_name,
        player_uuid,
        player_pos,
        player_rot,
        permission_level,
        feedback_tx,
        server_ctx,
    );

    let result = server_ctx.commands.dispatch(command, source);

    // Drain all feedback messages and send to this player only.
    while let Ok(component) = feedback_rx.try_recv() {
        let pkt = ClientboundSystemChatPacket {
            content: component,
            overlay: false,
        };
        let _ = conn
            .send_raw(ClientboundSystemChatPacket::PACKET_ID, &pkt.encode())
            .await;
    }
    let _ = conn.flush().await;

    match result {
        Ok(r) => {
            debug!(
                player = %player_name,
                command = %command,
                result = r,
                "Command executed",
            );
        },
        Err(e) => {
            let err_msg = ClientboundSystemChatPacket {
                content: Component::text(format!("{e}")),
                overlay: false,
            };
            let _ = conn.send_packet(&err_msg).await;
            debug!(player = %player_name, command = %command, error = %e, "Command failed");
        },
    }
}
