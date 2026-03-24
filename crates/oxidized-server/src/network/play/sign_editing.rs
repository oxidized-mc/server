//! Sign editing handler (stub).
//!
//! Sign editing requires block entities, which are not yet implemented.
//! Validates that the player is within editing range.

use bytes::Bytes;
use tracing::{debug, warn};

use oxidized_protocol::packets::play::ServerboundSignUpdatePacket;

use super::PlayContext;
use super::block_interaction::player_distance_to_block_sq;
use crate::network::helpers::decode_packet;
use crate::network::ConnectionError;

/// Maximum distance from a sign the player can edit (squared).
const MAX_SIGN_EDIT_DISTANCE_SQ: f64 = 8.0 * 8.0;

/// Handles `ServerboundSignUpdatePacket` (0x3D) — stub.
///
/// Sign editing requires block entities, which are not yet implemented.
/// Validates that the player is within editing range.
pub async fn handle_sign_update(
    play_ctx: &mut PlayContext<'_>,
    data: Bytes,
) -> Result<(), ConnectionError> {
    let pkt = decode_packet::<ServerboundSignUpdatePacket>(
        data,
        play_ctx.addr,
        play_ctx.player_name,
        "SignUpdate",
    )?;

    if player_distance_to_block_sq(play_ctx, pkt.pos) > MAX_SIGN_EDIT_DISTANCE_SQ {
        warn!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            pos = ?pkt.pos,
            "SignUpdate rejected: too far from sign"
        );
        return Ok(());
    }

    debug!(
        peer = %play_ctx.addr,
        name = %play_ctx.player_name,
        pos = ?pkt.pos,
        "SignUpdate: block entities not yet implemented"
    );

    Ok(())
}
