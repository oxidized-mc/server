//! Chat message and chat command handling.
//!
//! Processes incoming chat messages with rate limiting, validation, and
//! color code support, and dispatches `/commands` through the Brigadier
//! framework.

use std::sync::Arc;

use oxidized_chat::Component;
use oxidized_codec::Packet;
use oxidized_protocol::packets::play::ClientboundSystemChatPacket;
use oxidized_protocol::transport::connection::ConnectionError;
use oxidized_protocol::transport::handle::ConnectionHandle;
use tracing::{debug, info, warn};

use super::PlayContext;
use super::commands::make_command_source_for_player;
use crate::network::helpers::disconnect_handle;
use crate::network::{BroadcastMessage, ServerContext};

/// Maximum number of characters in a single chat message (vanilla
/// `SharedConstants.MAX_CHAT_LENGTH`).
pub const MAX_CHAT_LENGTH: usize = 256;

/// Returns `true` if `ch` is allowed in a chat message.
///
/// Mirrors vanilla `StringUtil.isAllowedChatCharacter`: rejects the
/// section sign (§, U+00A7), DEL (0x7F), and all control characters
/// below U+0020 **except** horizontal-tab (0x09) and line-feed (0x0A).
pub fn is_allowed_chat_character(ch: char) -> bool {
    if ch == '\u{00A7}' || ch == '\u{007F}' {
        return false;
    }
    // Allow printable characters (>= space) plus \t and \n.
    ch >= ' ' || ch == '\t' || ch == '\n'
}

/// Returns `true` if `message` contains any character that is not
/// allowed in chat (see [`is_allowed_chat_character`]).
pub fn is_chat_message_illegal(message: &str) -> bool {
    message.chars().any(|ch| !is_allowed_chat_character(ch))
}

/// Handles an incoming chat message from a player.
///
/// # Errors
///
/// Returns [`ConnectionError`] if the message is illegal or exceeds
/// the length limit, causing the player to be disconnected.
pub async fn handle_chat(ctx: &mut PlayContext<'_>, message: &str) -> Result<(), ConnectionError> {
    // Empty messages are silently ignored (vanilla behaviour).
    if message.is_empty() {
        return Ok(());
    }

    // Reject messages that exceed the vanilla length cap.
    if message.len() > MAX_CHAT_LENGTH {
        warn!(
            peer = %ctx.addr,
            name = %ctx.player_name,
            len = message.len(),
            "Chat message too long — disconnecting",
        );
        return Err(disconnect_handle(ctx.conn_handle, "chat_validation_failed").await);
    }

    // Reject messages containing illegal characters.
    if is_chat_message_illegal(message) {
        warn!(
            peer = %ctx.addr,
            name = %ctx.player_name,
            "Illegal characters in chat message — disconnecting",
        );
        return Err(disconnect_handle(
            ctx.conn_handle,
            "multiplayer.disconnect.illegal_characters",
        )
        .await);
    }

    if !ctx.rate_limiter.try_acquire() {
        warn!(peer = %ctx.addr, name = %ctx.player_name, "Chat rate-limited — disconnecting");
        return Err(disconnect_handle(ctx.conn_handle, "disconnect.spam").await);
    }

    let message_component = match ctx.server_ctx.settings.color_char {
        Some(ch) => Component::from_legacy_with_char(message, ch),
        None => Component::from_legacy(message),
    };
    let decorated = Component::translatable(
        "chat.type.text".to_string(),
        vec![Component::text(ctx.player_name), message_component],
    );
    let sys_pkt = ClientboundSystemChatPacket {
        content: decorated,
        is_overlay: false,
    };
    let encoded = sys_pkt.encode();
    let broadcast_msg = BroadcastMessage {
        packet_id: ClientboundSystemChatPacket::PACKET_ID,
        data: encoded.freeze(),
        exclude_entity: None,
        target_entity: None,
    };
    ctx.server_ctx.broadcast(broadcast_msg);
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
    conn_handle: &ConnectionHandle,
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
            is_overlay: false,
        };
        let _ = conn_handle
            .send_raw(
                ClientboundSystemChatPacket::PACKET_ID,
                pkt.encode().freeze(),
            )
            .await;
    }

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
                is_overlay: false,
            };
            let _ = conn_handle.send_packet(&err_msg).await;
            debug!(player = %player_name, command = %command, error = %e, "Command failed");
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ── is_allowed_chat_character ──────────────────────────────────

    #[test]
    fn test_allowed_char_printable_ascii() {
        for ch in ' '..='~' {
            assert!(
                is_allowed_chat_character(ch),
                "expected '{ch}' to be allowed"
            );
        }
    }

    #[test]
    fn test_allowed_char_tab_and_newline() {
        assert!(is_allowed_chat_character('\t'));
        assert!(is_allowed_chat_character('\n'));
    }

    #[test]
    fn test_allowed_char_rejects_null() {
        assert!(!is_allowed_chat_character('\0'));
    }

    #[test]
    fn test_allowed_char_rejects_control_chars() {
        for code in (0x01..=0x08).chain(0x0B..=0x1F) {
            let ch = char::from(code);
            assert!(
                !is_allowed_chat_character(ch),
                "expected 0x{code:02X} to be rejected",
            );
        }
    }

    #[test]
    fn test_allowed_char_rejects_del() {
        assert!(!is_allowed_chat_character('\u{7F}'));
    }

    #[test]
    fn test_allowed_char_rejects_section_sign() {
        assert!(!is_allowed_chat_character('\u{00A7}'));
    }

    #[test]
    fn test_allowed_char_unicode_above_a7() {
        assert!(is_allowed_chat_character('\u{00A8}'));
        assert!(is_allowed_chat_character('é'));
        assert!(is_allowed_chat_character('日'));
    }

    // ── is_chat_message_illegal ───────────────────────────────────

    #[test]
    fn test_illegal_message_empty() {
        assert!(!is_chat_message_illegal(""));
    }

    #[test]
    fn test_illegal_message_normal_text() {
        assert!(!is_chat_message_illegal("Hello, world!"));
    }

    #[test]
    fn test_illegal_message_with_null() {
        assert!(is_chat_message_illegal("hello\0world"));
    }

    #[test]
    fn test_illegal_message_with_bel() {
        assert!(is_chat_message_illegal("ding\x07dong"));
    }

    #[test]
    fn test_illegal_message_with_section_sign() {
        assert!(is_chat_message_illegal("color §c code"));
    }

    #[test]
    fn test_illegal_message_tab_allowed() {
        assert!(!is_chat_message_illegal("col1\tcol2"));
    }

    #[test]
    fn test_illegal_message_newline_allowed() {
        assert!(!is_chat_message_illegal("line1\nline2"));
    }

    // ── MAX_CHAT_LENGTH ───────────────────────────────────────────

    #[test]
    fn test_max_chat_length_boundary() {
        let at_limit = "a".repeat(MAX_CHAT_LENGTH);
        let over_limit = "a".repeat(MAX_CHAT_LENGTH + 1);
        assert_eq!(at_limit.len(), 256);
        assert!(at_limit.len() <= MAX_CHAT_LENGTH);
        assert!(over_limit.len() > MAX_CHAT_LENGTH);
    }
}
