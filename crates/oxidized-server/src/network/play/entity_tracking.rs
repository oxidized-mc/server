//! Entity state tracking and metadata broadcast.
//!
//! Handles player commands (sprint, elytra) and input (sneak, sprint),
//! broadcasting entity metadata changes to other players.

use oxidized_game::entity::data_slots::{
    DATA_POSE, DATA_SHARED_FLAGS, FLAG_CROUCHING, FLAG_FALL_FLYING, FLAG_SPRINTING,
};
use oxidized_game::player::ServerPlayer;
use oxidized_protocol::codec::Packet;
use oxidized_protocol::packets::play::clientbound_set_entity_data::EntityDataEntry;
use oxidized_protocol::packets::play::{
    ClientboundSetEntityDataPacket, PlayerCommandAction, ServerboundPlayerCommandPacket,
    ServerboundPlayerInputPacket,
};
use tracing::debug;

use super::PlayContext;
use crate::network::helpers::decode_packet;
use crate::network::BroadcastMessage;

/// Builds the `DATA_SHARED_FLAGS` byte from the player's current state.
pub(super) fn build_shared_flags(player: &ServerPlayer) -> u8 {
    let mut flags: u8 = 0;
    if player.sneaking {
        flags |= 1 << FLAG_CROUCHING;
    }
    if player.sprinting {
        flags |= 1 << FLAG_SPRINTING;
    }
    if player.is_fall_flying {
        flags |= 1 << FLAG_FALL_FLYING;
    }
    flags
}

/// Broadcasts the current entity shared flags to all other players.
pub(super) fn broadcast_entity_flags(ctx: &PlayContext<'_>, entity_id: i32) {
    let flags = build_shared_flags(&ctx.player.read());
    let pkt = ClientboundSetEntityDataPacket::single_byte(entity_id, DATA_SHARED_FLAGS, flags);
    let encoded = pkt.encode();
    ctx.server_ctx.broadcast(BroadcastMessage {
        packet_id: ClientboundSetEntityDataPacket::PACKET_ID,
        data: encoded.freeze(),
        exclude_entity: Some(entity_id),
        target_entity: None,
    });
}

/// Handles player command packets (sprint, elytra, sleep, riding, etc.).
///
/// Note: sneaking is NOT handled here — in 26.1 it is sent via
/// `ServerboundPlayerInputPacket` bit flags (see [`handle_player_input`]).
pub(super) fn handle_player_command(ctx: &PlayContext<'_>, data: bytes::Bytes) {
    if let Ok(cmd) = decode_packet::<ServerboundPlayerCommandPacket>(
        data,
        ctx.addr,
        ctx.player_name,
        "PlayerCommand",
    ) {
        match cmd.action {
            PlayerCommandAction::StartSprinting => {
                let entity_id = {
                    let mut p = ctx.player.write();
                    p.sprinting = true;
                    p.entity_id
                };
                broadcast_entity_flags(ctx, entity_id);
                debug!(peer = %ctx.addr, name = %ctx.player_name, "Player started sprinting");
            },
            PlayerCommandAction::StopSprinting => {
                let entity_id = {
                    let mut p = ctx.player.write();
                    p.sprinting = false;
                    p.entity_id
                };
                broadcast_entity_flags(ctx, entity_id);
                debug!(peer = %ctx.addr, name = %ctx.player_name, "Player stopped sprinting");
            },
            PlayerCommandAction::StartFallFlying => {
                let entity_id = {
                    let mut p = ctx.player.write();
                    p.is_fall_flying = true;
                    p.entity_id
                };
                // Broadcast fall-flying flag.
                let flags = build_shared_flags(&ctx.player.read());
                let pkt = ClientboundSetEntityDataPacket::single_byte(
                    entity_id,
                    DATA_SHARED_FLAGS,
                    flags,
                );
                let encoded = pkt.encode();
                ctx.server_ctx.broadcast(BroadcastMessage {
                    packet_id: ClientboundSetEntityDataPacket::PACKET_ID,
                    data: encoded.freeze(),
                    exclude_entity: Some(entity_id),
                    target_entity: None,
                });
                debug!(peer = %ctx.addr, name = %ctx.player_name, "Player started elytra flight");
            },
            _ => {
                debug!(
                    peer = %ctx.addr,
                    name = %ctx.player_name,
                    action = ?cmd.action,
                    "Player command (unhandled action)",
                );
            },
        }
    }
}

/// Handles player input packets (shift, sprint flags).
///
/// When sneaking or sprinting state changes, broadcasts entity metadata
/// to all other players so they can see the pose change.
pub(super) fn handle_player_input(ctx: &PlayContext<'_>, data: bytes::Bytes) {
    if let Ok(input_pkt) = decode_packet::<ServerboundPlayerInputPacket>(
        data,
        ctx.addr,
        ctx.player_name,
        "PlayerInput",
    ) {
        let (old_sneaking, old_sprinting, entity_id) = {
            let p = ctx.player.read();
            (p.sneaking, p.sprinting, p.entity_id)
        };
        let new_sneaking = input_pkt.input.shift;
        let new_sprinting = input_pkt.input.sprint;

        // Update local state.
        {
            let mut p = ctx.player.write();
            p.sneaking = new_sneaking;
            p.sprinting = new_sprinting;
        }

        // Broadcast entity metadata if flags changed.
        if old_sneaking != new_sneaking || old_sprinting != new_sprinting {
            let flags = build_shared_flags(&ctx.player.read());

            // Pose: 5 = sneaking, 0 = standing.
            let pose: i32 = if new_sneaking { 5 } else { 0 };
            let mut pose_bytes = bytes::BytesMut::new();
            oxidized_protocol::codec::varint::write_varint_buf(pose, &mut pose_bytes);

            let pkt = ClientboundSetEntityDataPacket {
                entity_id,
                entries: vec![
                    EntityDataEntry {
                        slot: DATA_SHARED_FLAGS,
                        serializer_type: 0, // Byte
                        value_bytes: vec![flags],
                    },
                    EntityDataEntry {
                        slot: DATA_POSE,
                        serializer_type: 20, // Pose (VarInt enum)
                        value_bytes: pose_bytes.to_vec(),
                    },
                ],
            };
            let encoded = pkt.encode();
            ctx.server_ctx.broadcast(BroadcastMessage {
                packet_id: ClientboundSetEntityDataPacket::PACKET_ID,
                data: encoded.freeze(),
                exclude_entity: Some(entity_id),
                target_entity: None,
            });
        }
    }
}
