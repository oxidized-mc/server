//! `/tp` and `/teleport` commands.
//!
//! Vanilla branches:
//! - `/tp <location>` — teleport self to coordinates
//! - `/tp <destination>` — teleport self to entity
//! - `/tp <targets> <location>` — teleport targets to coordinates
//! - `/tp <targets> <location> <rotation>` — teleport with rotation
//! - `/tp <targets> <location> facing <facingLocation>` — face location
//! - `/tp <targets> <location> facing entity <facingEntity>` — face entity
//! - `/tp <targets> <location> facing entity <facingEntity> <anchor>` — face w/ anchor
//! - `/tp <targets> <destination>` — teleport targets to entity
//!
//! TODO: Actual teleportation requires sending `ClientboundPlayerPositionPacket`.

use crate::commands::arguments::ArgumentType;
use crate::commands::context::{
    get_entity, get_vec3, CommandContext,
};
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use crate::commands::CommandError;
use oxidized_protocol::chat::Component;

/// Registers the `/tp` and `/teleport` commands with all vanilla branches.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(build_tp_tree("tp"));
    d.register(build_tp_tree("teleport"));
}

/// Builds the command tree for either `/tp` or `/teleport`.
fn build_tp_tree(name: &'static str) -> crate::commands::nodes::LiteralBuilder<CommandSourceStack> {
    literal(name)
        .description("Teleports entities")
        .requires(|s: &CommandSourceStack| s.has_permission(2))
        // /tp <location> — teleport self to coordinates
        .then(argument("location", ArgumentType::Vec3).executes(exec_tp_location))
        // /tp <destination> — teleport self to entity
        .then(
            argument(
                "destination",
                ArgumentType::Entity {
                    single: true,
                    player_only: false,
                },
            )
            .executes(exec_tp_destination)
            // /tp <targets> <location> [facing|rotation]
            .then(
                argument("location", ArgumentType::Vec3)
                    // /tp <targets> <location>
                    .executes(exec_tp_targets_location)
                    // /tp <targets> <location> <rotation>
                    .then(
                        argument("rotation", ArgumentType::Rotation)
                            .executes(exec_tp_targets_location_rotation),
                    )
                    // /tp <targets> <location> facing <facingLocation>
                    .then(
                        literal("facing")
                            .then(
                                argument("facingLocation", ArgumentType::Vec3)
                                    .executes(exec_tp_targets_location_facing_pos),
                            )
                            .then(
                                literal("entity")
                                    .then(
                                        argument(
                                            "facingEntity",
                                            ArgumentType::Entity {
                                                single: true,
                                                player_only: false,
                                            },
                                        )
                                        // /tp <targets> <location> facing entity <entity>
                                        .executes(exec_tp_targets_location_facing_entity)
                                        // /tp <targets> <location> facing entity <entity> <anchor>
                                        .then(
                                            argument("anchor", ArgumentType::EntityAnchor)
                                                .executes(
                                                    exec_tp_targets_location_facing_entity_anchor,
                                                ),
                                        ),
                                    ),
                            ),
                    ),
            )
            // /tp <targets> <destination>
            .then(
                argument(
                    "dest",
                    ArgumentType::Entity {
                        single: true,
                        player_only: false,
                    },
                )
                .executes(exec_tp_targets_destination),
            ),
        )
}

// ── Execution helpers ──────────────────────────────────────────────

/// `/tp <location>` — teleport source to coordinates.
fn exec_tp_location(ctx: &CommandContext<CommandSourceStack>) -> Result<i32, CommandError> {
    let (x, y, z) = get_vec3(ctx, "location")?;
    ctx.source.send_success(
        &tp_location_msg(&ctx.source.display_name, x, y, z),
        true,
    );
    Ok(1)
}

/// `/tp <destination>` — teleport source to entity.
fn exec_tp_destination(ctx: &CommandContext<CommandSourceStack>) -> Result<i32, CommandError> {
    let dest = get_entity(ctx, "destination")?;
    ctx.source.send_success(
        &tp_entity_msg(&ctx.source.display_name, &dest.name),
        true,
    );
    Ok(1)
}

/// `/tp <targets> <location>` — teleport targets to coordinates.
fn exec_tp_targets_location(
    ctx: &CommandContext<CommandSourceStack>,
) -> Result<i32, CommandError> {
    let target = get_entity(ctx, "destination")?;
    let (x, y, z) = get_vec3(ctx, "location")?;
    ctx.source.send_success(
        &tp_location_msg(&target.name, x, y, z),
        true,
    );
    Ok(1)
}

/// `/tp <targets> <location> <rotation>` — teleport with explicit rotation.
fn exec_tp_targets_location_rotation(
    ctx: &CommandContext<CommandSourceStack>,
) -> Result<i32, CommandError> {
    let target = get_entity(ctx, "destination")?;
    let (x, y, z) = get_vec3(ctx, "location")?;
    // TODO: apply rotation from "rotation" argument
    ctx.source.send_success(
        &tp_location_msg(&target.name, x, y, z),
        true,
    );
    Ok(1)
}

/// `/tp <targets> <location> facing <facingLocation>`
fn exec_tp_targets_location_facing_pos(
    ctx: &CommandContext<CommandSourceStack>,
) -> Result<i32, CommandError> {
    let target = get_entity(ctx, "destination")?;
    let (x, y, z) = get_vec3(ctx, "location")?;
    // TODO: compute rotation from facing position
    ctx.source.send_success(
        &tp_location_msg(&target.name, x, y, z),
        true,
    );
    Ok(1)
}

/// `/tp <targets> <location> facing entity <entity>`
fn exec_tp_targets_location_facing_entity(
    ctx: &CommandContext<CommandSourceStack>,
) -> Result<i32, CommandError> {
    let target = get_entity(ctx, "destination")?;
    let (x, y, z) = get_vec3(ctx, "location")?;
    // TODO: compute rotation from facing entity position
    ctx.source.send_success(
        &tp_location_msg(&target.name, x, y, z),
        true,
    );
    Ok(1)
}

/// `/tp <targets> <location> facing entity <entity> <anchor>`
fn exec_tp_targets_location_facing_entity_anchor(
    ctx: &CommandContext<CommandSourceStack>,
) -> Result<i32, CommandError> {
    let target = get_entity(ctx, "destination")?;
    let (x, y, z) = get_vec3(ctx, "location")?;
    // TODO: compute rotation from facing entity position + anchor
    ctx.source.send_success(
        &tp_location_msg(&target.name, x, y, z),
        true,
    );
    Ok(1)
}

/// `/tp <targets> <destination>` — teleport targets to entity.
fn exec_tp_targets_destination(
    ctx: &CommandContext<CommandSourceStack>,
) -> Result<i32, CommandError> {
    let target = get_entity(ctx, "destination")?;
    let dest = get_entity(ctx, "dest")?;
    ctx.source.send_success(
        &tp_entity_msg(&target.name, &dest.name),
        true,
    );
    Ok(1)
}

// ── Message builders ───────────────────────────────────────────────

/// Success message for location-based teleport.
fn tp_location_msg(target: &str, x: f64, y: f64, z: f64) -> Component {
    Component::translatable(
        "commands.teleport.success.location.single",
        vec![
            Component::text(target),
            Component::text(format!("{x:.2}")),
            Component::text(format!("{y:.2}")),
            Component::text(format!("{z:.2}")),
        ],
    )
}

/// Success message for entity-to-entity teleport.
fn tp_entity_msg(target: &str, destination: &str) -> Component {
    Component::translatable(
        "commands.teleport.success.entity.single",
        vec![
            Component::text(target),
            Component::text(destination),
        ],
    )
}
