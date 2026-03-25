//! Block placement and item use handlers.
//!
//! Handles `UseItemOn` (block placement with face/cursor validation) and
//! `UseItem` (generic item use — currently stubbed).

use bytes::Bytes;
use tracing::debug;

use oxidized_game::player::GameMode;
use oxidized_protocol::chat::Component;
use oxidized_protocol::packets::play::{
    ClientboundOpenSignEditorPacket, ClientboundSetPlayerInventoryPacket,
    ServerboundUseItemOnPacket, ServerboundUseItemPacket,
};
use oxidized_protocol::types::BlockPos;
use oxidized_protocol::types::direction::{Axis, Direction};
use oxidized_world::chunk::level_chunk::OVERWORLD_MAX_Y;
use oxidized_world::registry::{BlockRegistry, BlockStateId, BlockTags};

use super::PlayContext;
use super::block_interaction::{
    MIN_BUILD_HEIGHT, broadcast_block_update, get_block, is_spawn_protected,
    is_within_build_height, is_within_reach, resync_block, send_ack, send_actionbar, set_block,
};
use crate::network::ConnectionError;
use crate::network::helpers::decode_packet;

/// Block name for air, used to avoid magic strings.
const AIR_BLOCK_NAME: &str = "minecraft:air";

/// Handles `ServerboundUseItemOnPacket` (0x42) — block placement / interaction.
///
/// Places the player's held block onto the targeted face. If the player is not
/// holding a placeable block, the packet is acknowledged but no block is placed.
pub async fn handle_use_item_on(
    play_ctx: &mut PlayContext<'_>,
    data: Bytes,
) -> Result<(), ConnectionError> {
    let pkt = decode_packet::<ServerboundUseItemOnPacket>(
        data,
        play_ctx.addr,
        play_ctx.player_name,
        "UseItemOn",
    )?;

    // Spectators cannot place or interact with blocks.
    if play_ctx.player.read().game_mode == GameMode::Spectator {
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    // Validate cursor position — vanilla clamps each axis to [0, 1] with
    // a small epsilon for floating-point tolerance. Values outside indicate
    // a modified client.
    let (cx, cy, cz) = (
        pkt.hit_result.cursor_x,
        pkt.hit_result.cursor_y,
        pkt.hit_result.cursor_z,
    );
    const CURSOR_EPSILON: f32 = 0.0000001;
    let valid_range = -CURSOR_EPSILON..=1.0 + CURSOR_EPSILON;
    if !valid_range.contains(&cx) || !valid_range.contains(&cy) || !valid_range.contains(&cz) {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            cursor_x = cx,
            cursor_y = cy,
            cursor_z = cz,
            "UseItemOn rejected: cursor out of range"
        );
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    // Validate reach distance.
    if !is_within_reach(play_ctx, pkt.hit_result.pos) {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            pos = ?pkt.hit_result.pos,
            "Block place rejected: out of reach"
        );
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    // Compute placement position: offset from clicked face.
    let place_pos = pkt.hit_result.pos.relative(pkt.hit_result.direction);

    // Reject placement outside valid build height.
    if !is_within_build_height(place_pos) {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            pos = ?place_pos,
            "Block place rejected: outside build height"
        );
        // Vanilla sends "Height limit for building is %s" on the actionbar.
        if place_pos.y > super::block_interaction::MAX_BUILD_HEIGHT {
            send_actionbar(
                play_ctx,
                Component::translatable(
                    "build.tooHigh".to_owned(),
                    vec![Component::text(OVERWORLD_MAX_Y.to_string())],
                ),
            )
            .await?;
        } else {
            send_actionbar(
                play_ctx,
                Component::translatable(
                    "build.tooLow".to_owned(),
                    vec![Component::text(MIN_BUILD_HEIGHT.to_string())],
                ),
            )
            .await?;
        }
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    // Reject placement inside the spawn protection zone.
    // Vanilla checks the *clicked* block position, not the computed placement
    // position, and resyncs both blocks to fully correct client predictions.
    let clicked_pos = pkt.hit_result.pos;
    if is_spawn_protected(play_ctx.server_ctx, clicked_pos) {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            pos = ?clicked_pos,
            "Block place rejected: spawn protection"
        );
        send_actionbar(
            play_ctx,
            Component::translatable(
                "build.spawn_protection".to_owned(),
                vec![Component::text(format!(
                    "{}, {}, {}",
                    clicked_pos.x, clicked_pos.y, clicked_pos.z
                ))],
            ),
        )
        .await?;
        resync_block(play_ctx, clicked_pos, Some(pkt.hit_result.direction)).await?;
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    // Get the held item name and check availability.
    let (held_item, game_mode, has_items, is_sneaking) = {
        let player = play_ctx.player.read();
        let selected = player.inventory.get_selected();
        let name = selected.item.0.clone();
        let gm = player.game_mode;
        let has = !selected.is_empty() && selected.count > 0;
        (name, gm, has, player.movement.is_sneaking)
    };

    // Vanilla: if the clicked block is interactable and the player is NOT
    // sneaking (or has empty hands), perform block interaction instead of
    // placement. Since we don't implement full interactions yet, we skip
    // placement to prevent placing blocks on crafting tables, chests, etc.
    let clicked_block = get_block(play_ctx.server_ctx, clicked_pos);
    let suppress_use = is_sneaking && has_items;
    if !suppress_use {
        if let Some(state) = clicked_block {
            if is_interactable_block(state) {
                send_ack(play_ctx, pkt.sequence).await?;
                return Ok(());
            }
        }
    }

    // In non-Creative modes, verify the player actually has items.
    if game_mode != GameMode::Creative && !has_items {
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    // Determine block state to place from the held item.
    let player_yaw = play_ctx.player.read().movement.yaw;
    let block_state_id = match held_item_to_block_state(
        &held_item,
        &play_ctx.server_ctx.world.block_registry,
        player_yaw,
        pkt.hit_result.direction,
        pkt.hit_result.cursor_y,
    ) {
        Some(id) => id,
        None => {
            // Not a placeable block — just acknowledge.
            send_ack(play_ctx, pkt.sequence).await?;
            return Ok(());
        },
    };

    // Allow placement if target is air or a replaceable block (tall grass, water, etc.)
    let existing = get_block(play_ctx.server_ctx, place_pos);
    let is_replaceable = match existing {
        Some(state) => is_replaceable_block(state),
        None => false,
    };
    if !is_replaceable {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            pos = ?place_pos,
            existing_state = ?existing,
            "Block place rejected: position not replaceable"
        );
        // Send the actual block state back to correct client prediction.
        resync_block(play_ctx, place_pos, None).await?;
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    // Reject placement if the block would intersect the placing player's
    // bounding box (vanilla: prevents suffocation by self-placement).
    if block_intersects_player(play_ctx, place_pos) {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            pos = ?place_pos,
            "Block place rejected: would intersect player"
        );
        resync_block(play_ctx, place_pos, None).await?;
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    // Set the block in chunk storage.
    let placed = set_block(play_ctx.server_ctx, place_pos, block_state_id);
    if !placed {
        debug!(
            peer = %play_ctx.addr,
            name = %play_ctx.player_name,
            pos = ?place_pos,
            "Block place failed: chunk not loaded"
        );
        // Re-sync both the placement target and the clicked face so the
        // client doesn't display a phantom block.
        resync_block(play_ctx, place_pos, None).await?;
        resync_block(play_ctx, pkt.hit_result.pos, Some(pkt.hit_result.direction)).await?;
        send_ack(play_ctx, pkt.sequence).await?;
        return Ok(());
    }

    debug!(
        peer = %play_ctx.addr,
        name = %play_ctx.player_name,
        pos = ?place_pos,
        block_state = block_state_id,
        "Block placed"
    );

    // Decrement item count in survival/adventure modes.
    if game_mode != GameMode::Creative {
        decrement_held_item(play_ctx).await?;
    }

    // Broadcast block change to all players (including the acting player).
    broadcast_block_update(play_ctx.server_ctx, place_pos, block_state_id as i32, None);

    // Place the complementary block for double-block items (doors, beds,
    // tall plants). The primary block was already placed above; this adds
    // the matching upper/lower or head/foot half.
    place_companion_block(play_ctx, &held_item, place_pos, block_state_id, player_yaw);

    // If the placed block is a sign, open the sign editor UI.
    if is_sign_block(&held_item) {
        let sign_editor = ClientboundOpenSignEditorPacket {
            pos: place_pos,
            is_front_text: true,
        };
        play_ctx.conn_handle.send_packet(&sign_editor).await?;
    }

    // Acknowledge the sequence.
    send_ack(play_ctx, pkt.sequence).await?;

    Ok(())
}

/// Handles `ServerboundUseItemPacket` (0x43) — generic item use.
///
/// Currently a no-op. Item use behaviors (eating, shooting bows, etc.)
/// require systems not yet implemented.
pub async fn handle_use_item(
    play_ctx: &mut PlayContext<'_>,
    data: Bytes,
) -> Result<(), ConnectionError> {
    let pkt = decode_packet::<ServerboundUseItemPacket>(
        data,
        play_ctx.addr,
        play_ctx.player_name,
        "UseItem",
    )?;

    debug!(
        peer = %play_ctx.addr,
        name = %play_ctx.player_name,
        hand = ?pkt.hand,
        sequence = pkt.sequence,
        "UseItem: not yet implemented"
    );

    send_ack(play_ctx, pkt.sequence).await?;
    Ok(())
}

/// Decrements the held item count by one and syncs the slot to the client.
///
/// Used after block placement in survival/adventure modes.
async fn decrement_held_item(
    play_ctx: &mut PlayContext<'_>,
) -> Result<(), ConnectionError> {
    let (slot_idx, updated) = {
        let mut player = play_ctx.player.write();
        let slot = player.inventory.selected_slot as usize;
        let stack = player.inventory.get_mut(slot);
        stack.count -= 1;
        if stack.count <= 0 {
            player.inventory.set(
                slot,
                oxidized_game::inventory::item_stack::ItemStack::empty(),
            );
        }
        (slot, player.inventory.get(slot).clone())
    };
    let slot_data = if updated.is_empty() {
        None
    } else {
        Some(super::inventory::item_stack_to_slot_data(&updated))
    };
    let sync_pkt = ClientboundSetPlayerInventoryPacket {
        slot: slot_idx as i32,
        contents: slot_data,
    };
    play_ctx.conn_handle.send_packet(&sync_pkt).await?;
    Ok(())
}

/// Places the complementary block for double-block items (doors, beds, tall
/// plants).
///
/// The primary block must already be placed in the world before calling this.
fn place_companion_block(
    play_ctx: &mut PlayContext<'_>,
    held_item: &str,
    place_pos: BlockPos,
    block_state_id: u32,
    player_yaw: f32,
) {
    if let Some((companion_pos, companion_state)) = double_block_companion(
        held_item,
        &play_ctx.server_ctx.world.block_registry,
        place_pos,
        block_state_id,
        player_yaw,
    ) {
        if !is_within_build_height(companion_pos) {
            return;
        }
        let companion_existing = get_block(play_ctx.server_ctx, companion_pos);
        let is_replaceable = companion_existing.is_some_and(is_replaceable_block);
        if is_replaceable {
            let ok = set_block(play_ctx.server_ctx, companion_pos, companion_state);
            if ok {
                broadcast_block_update(
                    play_ctx.server_ctx,
                    companion_pos,
                    companion_state as i32,
                    None,
                );
            }
        }
    }
}

/// Returns `true` if a block state is air or a vanilla-replaceable block.
///
/// Replaceable blocks can be overwritten by block placement without the player
/// needing to break them first (e.g., tall grass, water, snow layer, fire).
fn is_replaceable_block(state_id: u32) -> bool {
    BlockStateId(state_id as u16).is_replaceable()
}

/// Maps a held item name to a block state ID for placement, applying
/// directional and positional properties based on the placement context.
///
/// Uses the block registry's default state as a starting point, then adjusts:
/// - **`facing`**: horizontal direction opposite to the player's look direction
///   (vanilla convention: blocks face *toward* the player)
/// - **`axis`**: from the clicked block face (logs, pillars, basalt)
/// - **`half`**: top/bottom from cursor Y position (slabs, trapdoors)
/// - **`type`**: top/bottom for slabs using cursor Y
///
/// Items that are not placeable blocks return `None`.
fn held_item_to_block_state(
    item_name: &str,
    registry: &BlockRegistry,
    player_yaw: f32,
    clicked_face: Direction,
    cursor_y: f32,
) -> Option<u32> {
    if item_name.is_empty() || item_name == AIR_BLOCK_NAME {
        return None;
    }

    let mut state = registry.default_state(item_name)?;

    // Apply facing property: most directional blocks face opposite to
    // the player's look direction (so the front faces the player).
    let props = state.properties();
    let has_facing = props.iter().any(|&(name, _)| name == "facing");
    let has_axis = props.iter().any(|&(name, _)| name == "axis");
    let has_half = props.iter().any(|&(name, _)| name == "half");
    let has_type = props.iter().any(|&(name, _)| name == "type");

    if has_facing {
        let facing = facing_for_placement(item_name, player_yaw, clicked_face);
        if let Some(new_state) = state.with_property("facing", facing.name()) {
            state = new_state;
        }
    }

    if has_axis {
        let axis = axis_from_face(clicked_face);
        if let Some(new_state) = state.with_property("axis", axis.name()) {
            state = new_state;
        }
    }

    if has_half {
        let half = half_from_cursor(cursor_y, clicked_face);
        if let Some(new_state) = state.with_property("half", half) {
            state = new_state;
        }
    }

    // Slabs use "type" instead of "half" for top/bottom.
    if has_type && item_name.contains("slab") {
        let slab_type = slab_type_from_cursor(cursor_y, clicked_face);
        if let Some(new_state) = state.with_property("type", slab_type) {
            state = new_state;
        }
    }

    Some(u32::from(state.0))
}

/// Returns the block type index for a block/item name, used for tag lookups.
fn block_type_id_from_name(name: &str) -> Option<u16> {
    let def = BlockRegistry::new().get_block_def(name)?;
    Some(BlockStateId(def.default_state).data().block_type)
}

/// Determines the facing direction for block placement.
///
/// Most blocks face *opposite* to the player's look direction (toward the
/// player), matching vanilla `HorizontalDirectionalBlock.getStateForPlacement`.
/// Blocks like stairs, doors, and beds override this to face in the player's
/// look direction. Wall-mounted blocks use the clicked face.
fn facing_for_placement(item_name: &str, player_yaw: f32, clicked_face: Direction) -> Direction {
    // Wall-mounted blocks face the direction of the clicked face
    if is_wall_mountable(item_name) && clicked_face.is_horizontal() {
        return clicked_face;
    }

    let player_direction = Direction::from_y_rot(f64::from(player_yaw));

    // Blocks that face in the player's look direction (vanilla overrides
    // that use getHorizontalDirection() without getOpposite()).
    if is_player_direction_block(item_name) {
        return player_direction;
    }

    // Most directional blocks face opposite to the player's look direction
    // (toward the player), including pistons and observers.
    player_direction.opposite()
}

/// Returns `true` for blocks that mount on a wall surface rather than being
/// placed freestanding.
fn is_wall_mountable(item_name: &str) -> bool {
    let tags = BlockTags;
    block_type_id_from_name(item_name)
        .is_some_and(|id| tags.contains("oxidized:wall_mountable", id))
}

/// Returns `true` for blocks whose `facing` property should match the
/// player's look direction (not the opposite).
///
/// In vanilla, `HorizontalDirectionalBlock` uses
/// `getHorizontalDirection().getOpposite()` (toward the player), but many
/// subclasses override placement to use `getHorizontalDirection()` directly.
fn is_player_direction_block(item_name: &str) -> bool {
    let tags = BlockTags;
    block_type_id_from_name(item_name)
        .is_some_and(|id| tags.contains("oxidized:player_direction", id))
}

/// Maps a clicked face direction to the corresponding axis for log-type blocks.
fn axis_from_face(face: Direction) -> Axis {
    face.axis()
}

/// Determines the "half" property value (top/bottom) based on cursor position.
///
/// Vanilla rule: if the player clicks the top half of a face (cursor Y ≥ 0.5)
/// or clicks the bottom face of a block, the block is placed as "top" half.
fn half_from_cursor(cursor_y: f32, clicked_face: Direction) -> &'static str {
    if clicked_face == Direction::Down {
        "top"
    } else if clicked_face == Direction::Up {
        "bottom"
    } else if cursor_y >= 0.5 {
        "top"
    } else {
        "bottom"
    }
}

/// Determines the slab "type" property (top/bottom) based on cursor position.
///
/// Uses the same logic as `half_from_cursor` but with slab-specific values.
fn slab_type_from_cursor(cursor_y: f32, clicked_face: Direction) -> &'static str {
    if clicked_face == Direction::Down {
        "top"
    } else if clicked_face == Direction::Up {
        "bottom"
    } else if cursor_y >= 0.5 {
        "top"
    } else {
        "bottom"
    }
}

/// Returns `true` if the item name represents a sign block.
fn is_sign_block(item_name: &str) -> bool {
    let tags = BlockTags;
    block_type_id_from_name(item_name).is_some_and(|id| tags.contains("minecraft:all_signs", id))
}

/// Computes the companion block position and state for double-block items.
///
/// Returns `Some((pos, state_id))` for the second half of:
/// - **Doors**: upper half at y+1 (half=upper, same facing)
/// - **Beds**: head part at facing direction offset (part=head, same facing)
/// - **Tall plants**: upper half at y+1 (half=upper)
///
/// Returns `None` for non-double-block items.
fn double_block_companion(
    item_name: &str,
    _registry: &BlockRegistry,
    primary_pos: BlockPos,
    primary_state_id: u32,
    _player_yaw: f32,
) -> Option<(BlockPos, u32)> {
    let primary = BlockStateId(primary_state_id as u16);

    if is_door_block(item_name) {
        // Doors: primary = lower half, companion = upper half at y+1.
        let upper = primary.with_property("half", "upper")?;
        Some((primary_pos.above(), u32::from(upper.0)))
    } else if is_bed_block(item_name) {
        // Beds: primary = foot, companion = head in the facing direction.
        // The foot's `facing` property already points from foot→head (set by
        // `facing_for_placement` to the player's look direction).
        let facing_name = primary
            .properties()
            .iter()
            .find(|&&(k, _)| k == "facing")
            .map(|&(_, v)| v)?;
        let facing = direction_from_name(facing_name)?;
        let head_pos = primary_pos.relative(facing);
        let head = primary.with_property("part", "head")?;
        Some((head_pos, u32::from(head.0)))
    } else if is_tall_plant(item_name) {
        // Tall plants: primary = lower, companion = upper at y+1.
        // Tall plants use "half" with values "upper"/"lower".
        let upper = primary.with_property("half", "upper")?;
        Some((primary_pos.above(), u32::from(upper.0)))
    } else {
        None
    }
}

/// Returns `true` if the item is a door block (any wood type or iron).
fn is_door_block(item_name: &str) -> bool {
    let tags = BlockTags;
    block_type_id_from_name(item_name).is_some_and(|id| tags.contains("minecraft:doors", id))
}

/// Returns `true` if the item is a bed block (any color).
fn is_bed_block(item_name: &str) -> bool {
    let tags = BlockTags;
    block_type_id_from_name(item_name).is_some_and(|id| tags.contains("minecraft:beds", id))
}

/// Returns `true` if the item is a tall double-height plant.
fn is_tall_plant(item_name: &str) -> bool {
    let tags = BlockTags;
    block_type_id_from_name(item_name).is_some_and(|id| tags.contains("oxidized:tall_plants", id))
}

/// Computes the companion break position for double-block items.
///
/// When breaking one half of a double block, the other half must also be
/// removed. Returns `Some(pos)` of the companion block to remove.
pub(super) fn double_block_break_companion(
    block_name: &str,
    state_id: u32,
    pos: BlockPos,
) -> Option<BlockPos> {
    let state = BlockStateId(state_id as u16);
    let props = state.properties();

    if is_door_block(block_name) || is_tall_plant(block_name) {
        // Doors and tall plants use "half": "upper"/"lower"
        let half = props.iter().find(|&&(k, _)| k == "half").map(|&(_, v)| v)?;
        match half {
            "upper" => Some(pos.below()),
            "lower" => Some(pos.above()),
            _ => None,
        }
    } else if is_bed_block(block_name) {
        // Beds use "part": "head"/"foot" and "facing" for direction
        let part = props.iter().find(|&&(k, _)| k == "part").map(|&(_, v)| v)?;
        let facing_name = props
            .iter()
            .find(|&&(k, _)| k == "facing")
            .map(|&(_, v)| v)?;
        let facing = direction_from_name(facing_name)?;
        match part {
            "head" => Some(pos.relative(facing.opposite())),
            "foot" => Some(pos.relative(facing)),
            _ => None,
        }
    } else {
        None
    }
}

/// Parses a direction name string into a `Direction`.
fn direction_from_name(name: &str) -> Option<Direction> {
    match name {
        "north" => Some(Direction::North),
        "south" => Some(Direction::South),
        "east" => Some(Direction::East),
        "west" => Some(Direction::West),
        "up" => Some(Direction::Up),
        "down" => Some(Direction::Down),
        _ => None,
    }
}

/// Returns `true` if the block state represents an interactable block.
///
/// Interactable blocks (crafting table, chest, bed, etc.) have a primary
/// "use" action when right-clicked. Vanilla processes `state.useItemOn()`
/// before item placement; if the block is interactable and the player is
/// not sneaking, placement is suppressed.
///
/// Uses the `IS_INTERACTABLE` flag for most blocks. Beds are a special case:
/// vanilla data does not flag them as interactable (sleeping is conditional),
/// but placement must still be suppressed.
fn is_interactable_block(state_id: u32) -> bool {
    let bsid = BlockStateId(state_id as u16);
    bsid.is_interactable() || BlockTags.contains("minecraft:beds", bsid.data().block_type)
}

/// Player half-width (vanilla: 0.3 from center).
const PLAYER_HALF_WIDTH: f64 = 0.3;
/// Standing player height (vanilla: 1.8).
const PLAYER_STANDING_HEIGHT: f64 = 1.8;
/// Sneaking player height (vanilla: 1.5).
const PLAYER_SNEAKING_HEIGHT: f64 = 1.5;

/// Returns `true` if placing a full block at `pos` would intersect the
/// placing player's axis-aligned bounding box.
fn block_intersects_player(play_ctx: &PlayContext<'_>, pos: BlockPos) -> bool {
    let player = play_ctx.player.read();
    let px = player.movement.pos.x;
    let py = player.movement.pos.y;
    let pz = player.movement.pos.z;
    let height = if player.movement.is_sneaking {
        PLAYER_SNEAKING_HEIGHT
    } else {
        PLAYER_STANDING_HEIGHT
    };

    // Player AABB: (px - 0.3, py, pz - 0.3) to (px + 0.3, py + height, pz + 0.3)
    // Block AABB:  (bx, by, bz) to (bx + 1, by + 1, bz + 1)
    let bx = pos.x as f64;
    let by = pos.y as f64;
    let bz = pos.z as f64;

    let player_min_x = px - PLAYER_HALF_WIDTH;
    let player_max_x = px + PLAYER_HALF_WIDTH;
    let player_min_y = py;
    let player_max_y = py + height;
    let player_min_z = pz - PLAYER_HALF_WIDTH;
    let player_max_z = pz + PLAYER_HALF_WIDTH;

    // AABB overlap test: overlap on all three axes means intersection.
    player_min_x < bx + 1.0
        && player_max_x > bx
        && player_min_y < by + 1.0
        && player_max_y > by
        && player_min_z < bz + 1.0
        && player_max_z > bz
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use oxidized_world::registry::BlockRegistry;

    // ── is_replaceable_block ────────────────────────────────────────────

    #[test]
    fn test_replaceable_air_is_replaceable() {
        let reg = BlockRegistry::new();
        let air = reg.default_state("minecraft:air").unwrap();
        assert!(is_replaceable_block(u32::from(air.0)));
    }

    #[test]
    fn test_replaceable_cave_air() {
        let reg = BlockRegistry::new();
        let state = reg.default_state("minecraft:cave_air").unwrap();
        assert!(is_replaceable_block(u32::from(state.0)));
    }

    #[test]
    fn test_replaceable_water() {
        let reg = BlockRegistry::new();
        let state = reg.default_state("minecraft:water").unwrap();
        assert!(is_replaceable_block(u32::from(state.0)));
    }

    #[test]
    fn test_replaceable_tall_grass() {
        let reg = BlockRegistry::new();
        let state = reg.default_state("minecraft:tall_grass").unwrap();
        assert!(is_replaceable_block(u32::from(state.0)));
    }

    #[test]
    fn test_replaceable_fire() {
        let reg = BlockRegistry::new();
        let state = reg.default_state("minecraft:fire").unwrap();
        assert!(is_replaceable_block(u32::from(state.0)));
    }

    #[test]
    fn test_replaceable_snow() {
        let reg = BlockRegistry::new();
        let state = reg.default_state("minecraft:snow").unwrap();
        assert!(is_replaceable_block(u32::from(state.0)));
    }

    #[test]
    fn test_replaceable_vine() {
        let reg = BlockRegistry::new();
        let state = reg.default_state("minecraft:vine").unwrap();
        assert!(is_replaceable_block(u32::from(state.0)));
    }

    #[test]
    fn test_replaceable_stone_is_not_replaceable() {
        let reg = BlockRegistry::new();
        let state = reg.default_state("minecraft:stone").unwrap();
        assert!(!is_replaceable_block(u32::from(state.0)));
    }

    #[test]
    fn test_replaceable_oak_planks_is_not_replaceable() {
        let reg = BlockRegistry::new();
        let state = reg.default_state("minecraft:oak_planks").unwrap();
        assert!(!is_replaceable_block(u32::from(state.0)));
    }

    // ── is_interactable_block ───────────────────────────────────────────

    #[test]
    fn test_interactable_crafting_table() {
        let reg = BlockRegistry::new();
        let state = reg.default_state("minecraft:crafting_table").unwrap();
        assert!(is_interactable_block(u32::from(state.0)));
    }

    #[test]
    fn test_interactable_chest() {
        let reg = BlockRegistry::new();
        let state = reg.default_state("minecraft:chest").unwrap();
        assert!(is_interactable_block(u32::from(state.0)));
    }

    #[test]
    fn test_interactable_lever() {
        let reg = BlockRegistry::new();
        let state = reg.default_state("minecraft:lever").unwrap();
        assert!(is_interactable_block(u32::from(state.0)));
    }

    #[test]
    fn test_interactable_oak_door() {
        let reg = BlockRegistry::new();
        let state = reg.default_state("minecraft:oak_door").unwrap();
        assert!(is_interactable_block(u32::from(state.0)));
    }

    #[test]
    fn test_interactable_white_bed() {
        let reg = BlockRegistry::new();
        let state = reg.default_state("minecraft:white_bed").unwrap();
        assert!(is_interactable_block(u32::from(state.0)));
    }

    #[test]
    fn test_interactable_oak_button() {
        let reg = BlockRegistry::new();
        let state = reg.default_state("minecraft:oak_button").unwrap();
        assert!(is_interactable_block(u32::from(state.0)));
    }

    #[test]
    fn test_interactable_stone_is_not_interactable() {
        let reg = BlockRegistry::new();
        let state = reg.default_state("minecraft:stone").unwrap();
        assert!(!is_interactable_block(u32::from(state.0)));
    }

    #[test]
    fn test_interactable_air_is_not_interactable() {
        let reg = BlockRegistry::new();
        let state = reg.default_state("minecraft:air").unwrap();
        assert!(!is_interactable_block(u32::from(state.0)));
    }

    // ── is_wall_mountable ───────────────────────────────────────────────

    #[test]
    fn test_wall_mountable_oak_button() {
        assert!(is_wall_mountable("minecraft:oak_button"));
    }

    #[test]
    fn test_wall_mountable_lever() {
        assert!(is_wall_mountable("minecraft:lever"));
    }

    #[test]
    fn test_wall_mountable_wall_torch() {
        assert!(is_wall_mountable("minecraft:wall_torch"));
    }

    #[test]
    fn test_wall_mountable_oak_wall_sign() {
        assert!(is_wall_mountable("minecraft:oak_wall_sign"));
    }

    #[test]
    fn test_wall_mountable_stone_is_not() {
        assert!(!is_wall_mountable("minecraft:stone"));
    }

    #[test]
    fn test_wall_mountable_oak_door_is_not() {
        assert!(!is_wall_mountable("minecraft:oak_door"));
    }

    // ── is_player_direction_block ───────────────────────────────────────

    #[test]
    fn test_player_direction_oak_door() {
        assert!(is_player_direction_block("minecraft:oak_door"));
    }

    #[test]
    fn test_player_direction_white_bed() {
        assert!(is_player_direction_block("minecraft:white_bed"));
    }

    #[test]
    fn test_player_direction_oak_stairs() {
        assert!(is_player_direction_block("minecraft:oak_stairs"));
    }

    #[test]
    fn test_player_direction_oak_fence_gate() {
        assert!(is_player_direction_block("minecraft:oak_fence_gate"));
    }

    #[test]
    fn test_player_direction_oak_trapdoor() {
        assert!(is_player_direction_block("minecraft:oak_trapdoor"));
    }

    #[test]
    fn test_player_direction_repeater() {
        assert!(is_player_direction_block("minecraft:repeater"));
    }

    #[test]
    fn test_player_direction_comparator() {
        assert!(is_player_direction_block("minecraft:comparator"));
    }

    #[test]
    fn test_player_direction_stone_is_not() {
        assert!(!is_player_direction_block("minecraft:stone"));
    }

    // ── is_sign_block ───────────────────────────────────────────────────

    #[test]
    fn test_sign_block_oak_sign() {
        assert!(is_sign_block("minecraft:oak_sign"));
    }

    #[test]
    fn test_sign_block_oak_wall_sign() {
        assert!(is_sign_block("minecraft:oak_wall_sign"));
    }

    #[test]
    fn test_sign_block_spruce_sign() {
        assert!(is_sign_block("minecraft:spruce_sign"));
    }

    #[test]
    fn test_sign_block_stone_is_not() {
        assert!(!is_sign_block("minecraft:stone"));
    }

    // ── is_door_block ───────────────────────────────────────────────────

    #[test]
    fn test_door_block_oak_door() {
        assert!(is_door_block("minecraft:oak_door"));
    }

    #[test]
    fn test_door_block_iron_door() {
        assert!(is_door_block("minecraft:iron_door"));
    }

    #[test]
    fn test_door_block_spruce_door() {
        assert!(is_door_block("minecraft:spruce_door"));
    }

    #[test]
    fn test_door_block_stone_is_not() {
        assert!(!is_door_block("minecraft:stone"));
    }

    // ── is_bed_block ────────────────────────────────────────────────────

    #[test]
    fn test_bed_block_white_bed() {
        assert!(is_bed_block("minecraft:white_bed"));
    }

    #[test]
    fn test_bed_block_red_bed() {
        assert!(is_bed_block("minecraft:red_bed"));
    }

    #[test]
    fn test_bed_block_stone_is_not() {
        assert!(!is_bed_block("minecraft:stone"));
    }

    // ── is_tall_plant ───────────────────────────────────────────────────

    #[test]
    fn test_tall_plant_sunflower() {
        assert!(is_tall_plant("minecraft:sunflower"));
    }

    #[test]
    fn test_tall_plant_lilac() {
        assert!(is_tall_plant("minecraft:lilac"));
    }

    #[test]
    fn test_tall_plant_rose_bush() {
        assert!(is_tall_plant("minecraft:rose_bush"));
    }

    #[test]
    fn test_tall_plant_peony() {
        assert!(is_tall_plant("minecraft:peony"));
    }

    #[test]
    fn test_tall_plant_tall_grass() {
        assert!(is_tall_plant("minecraft:tall_grass"));
    }

    #[test]
    fn test_tall_plant_large_fern() {
        assert!(is_tall_plant("minecraft:large_fern"));
    }

    #[test]
    fn test_tall_plant_pitcher_plant() {
        assert!(is_tall_plant("minecraft:pitcher_plant"));
    }

    #[test]
    fn test_tall_plant_tall_seagrass() {
        assert!(is_tall_plant("minecraft:tall_seagrass"));
    }

    #[test]
    fn test_tall_plant_stone_is_not() {
        assert!(!is_tall_plant("minecraft:stone"));
    }

    #[test]
    fn test_tall_plant_short_grass_is_not() {
        assert!(!is_tall_plant("minecraft:short_grass"));
    }

    // ── block_type_id_from_name ─────────────────────────────────────────

    #[test]
    fn test_block_type_id_from_name_known() {
        assert!(block_type_id_from_name("minecraft:stone").is_some());
    }

    #[test]
    fn test_block_type_id_from_name_unknown() {
        assert!(block_type_id_from_name("minecraft:nonexistent").is_none());
    }
}
