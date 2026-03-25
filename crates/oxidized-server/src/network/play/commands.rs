//! Command-related play handlers.
//!
//! Provides tab-completion (command suggestions) and the helper functions
//! for building [`CommandSourceStack`] instances.

use std::sync::Arc;

use oxidized_game::commands::source::{CommandSourceKind, CommandSourceStack};
use oxidized_game::player::ServerPlayer;
use oxidized_protocol::chat::Component;
use oxidized_protocol::connection::ConnectionError;
use oxidized_protocol::packets::play::{
    ClientboundCommandSuggestionsPacket, ClientboundCommandsPacket,
    ServerboundCommandSuggestionPacket,
};
use tracing::debug;

use super::PlayContext;
use crate::network::ServerContext;
use crate::network::helpers::decode_packet;

/// Maximum number of tab-completion suggestions sent to the client
/// (vanilla `ServerGamePacketListenerImpl.MAX_COMMAND_SUGGESTIONS`).
pub const MAX_COMMAND_SUGGESTIONS: usize = 1000;

/// Handles a tab-completion request from the client.
pub async fn handle_command_suggestion(
    ctx: &mut PlayContext<'_>,
    data: bytes::Bytes,
) -> Result<(), ConnectionError> {
    let suggestion_pkt: ServerboundCommandSuggestionPacket =
        decode_packet(data, ctx.addr, ctx.player_name, "CommandSuggestion")?;

    let (pos, rot) = {
        let p = ctx.player.read();
        (
            (p.movement.pos.x, p.movement.pos.y, p.movement.pos.z),
            (p.movement.yaw, p.movement.pitch),
        )
    };
    let (feedback_tx, _) = std::sync::mpsc::channel::<Component>();
    let perm_level = ctx
        .server_ctx
        .ops
        .get_permission_level(&ctx.player_uuid)
        .clamp(0, 4) as u32;
    let source = make_command_source_for_player(
        ctx.player_name,
        ctx.player_uuid,
        pos,
        rot,
        perm_level,
        feedback_tx,
        ctx.server_ctx,
    );

    // Client sends command with leading `/` — strip it
    // but track the offset so we return correct ranges.
    let raw = &suggestion_pkt.command;
    let (input, prefix_len) = if let Some(stripped) = raw.strip_prefix('/') {
        (stripped, 1usize)
    } else {
        (raw.as_str(), 0usize)
    };
    let mut suggestions = ctx.server_ctx.commands.completions(input, &source);

    // Vanilla caps suggestions at MAX_COMMAND_SUGGESTIONS (1000).
    suggestions.truncate(MAX_COMMAND_SUGGESTIONS);

    // Compute the range from suggestions (all share the
    // same range for a single completion position).
    let (start, length) = if let Some(first) = suggestions.first() {
        let s = first.range.start + prefix_len;
        let l = first.range.len();
        (s as i32, l as i32)
    } else {
        (0, 0)
    };

    let response = ClientboundCommandSuggestionsPacket {
        id: suggestion_pkt.id,
        start,
        length,
        suggestions: suggestions
            .into_iter()
            .map(|s| {
                oxidized_protocol::packets::play::clientbound_command_suggestions::SuggestionEntry {
                    text: s.text,
                    tooltip: s.tooltip,
                }
            })
            .collect(),
    };
    let _ = ctx.conn_handle.send_packet(&response).await;

    Ok(())
}

/// Builds a [`CommandSourceStack`] for use during login (command tree
/// serialization). Uses a no-op feedback sender since no packets need
/// to be sent at this point.
pub fn make_command_source(
    player_name: &str,
    uuid: uuid::Uuid,
    player: &ServerPlayer,
    server_ctx: &Arc<ServerContext>,
) -> CommandSourceStack {
    let perm_level = server_ctx.ops.get_permission_level(&uuid).clamp(0, 4) as u32;
    CommandSourceStack {
        source: CommandSourceKind::Player {
            name: player_name.to_string(),
            uuid,
        },
        position: (
            player.movement.pos.x,
            player.movement.pos.y,
            player.movement.pos.z,
        ),
        rotation: (player.movement.yaw, player.movement.pitch),
        permission_level: perm_level,
        display_name: player_name.to_string(),
        server: server_ctx.clone(),
        feedback_sender: Arc::new(|_| {}),
        is_silent: false,
    }
}

/// Builds a [`CommandSourceStack`] for a player executing a command.
/// The feedback sender writes system chat messages directly to the player's
/// connection.
pub fn make_command_source_for_player(
    player_name: &str,
    uuid: uuid::Uuid,
    pos: (f64, f64, f64),
    rot: (f32, f32),
    permission_level: u32,
    feedback_tx: std::sync::mpsc::Sender<Component>,
    server_ctx: &Arc<ServerContext>,
) -> CommandSourceStack {
    let name = player_name.to_string();
    CommandSourceStack {
        source: CommandSourceKind::Player {
            name: player_name.to_string(),
            uuid,
        },
        position: pos,
        rotation: rot,
        permission_level,
        display_name: player_name.to_string(),
        server: server_ctx.clone(),
        feedback_sender: Arc::new(move |component: &Component| {
            let _ = feedback_tx.send(component.clone());
            debug!(player = %name, "Command feedback queued");
        }),
        is_silent: false,
    }
}

/// Converts the game crate's [`CommandTreeData`](oxidized_game::commands::CommandTreeData) into a protocol-level
/// [`ClientboundCommandsPacket`].
pub fn commands_packet_from_tree(
    tree: &oxidized_game::commands::CommandTreeData,
) -> ClientboundCommandsPacket {
    let nodes = tree
        .nodes
        .iter()
        .map(|n| {
            use oxidized_protocol::packets::play::clientbound_commands::CommandNodeData;
            CommandNodeData {
                flags: n.flags,
                children: n.children.clone(),
                redirect_node: n.redirect_node,
                name: n.name.clone(),
                parser_id: n.parser.as_ref().map(|p| p.parser_id),
                parser_properties: n.parser.as_ref().map(|p| p.properties.clone()),
                suggestions_type: n.suggestions_type.clone(),
            }
        })
        .collect();
    ClientboundCommandsPacket {
        nodes,
        root_index: tree.root_index,
    }
}
