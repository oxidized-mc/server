//! `/tp` and `/teleport` commands.
//!
//! TODO: Actual teleportation requires sending `ClientboundPlayerPositionPacket`
//! to the target player and updating their server-side position. This needs
//! per-player packet sending which is not yet available through `ServerHandle`.

use crate::commands::arguments::ArgumentType;
use crate::commands::context::{CommandContext, get_entity, get_vec3};
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/tp` and `/teleport` commands.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("tp")
            .description("Teleports entities")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            // /tp <location> (vec3)
            .then(argument("location", ArgumentType::Vec3).executes(
                |ctx: &CommandContext<CommandSourceStack>| {
                    let (x, y, z) = get_vec3(ctx, "location")?;
                    // TODO: Actually teleport the source player
                    ctx.source.send_success(
                        &Component::translatable(
                            "commands.teleport.success.location.single",
                            vec![
                                Component::text(&ctx.source.display_name),
                                Component::text(format!("{x:.2}")),
                                Component::text(format!("{y:.2}")),
                                Component::text(format!("{z:.2}")),
                            ],
                        ),
                        true,
                    );
                    Ok(1)
                },
            ))
            // /tp <destination> (entity selector)
            .then(
                argument(
                    "destination",
                    ArgumentType::Entity {
                        single: true,
                        player_only: false,
                    },
                )
                .executes(|ctx: &CommandContext<CommandSourceStack>| {
                    let dest = get_entity(ctx, "destination")?;
                    // TODO: Resolve entity selector, get target position, teleport
                    ctx.source.send_success(
                        &Component::translatable(
                            "commands.teleport.success.entity.single",
                            vec![
                                Component::text(&ctx.source.display_name),
                                Component::text(&dest.name),
                            ],
                        ),
                        true,
                    );
                    Ok(1)
                }),
            ),
    );

    // /teleport is an alias
    d.register(
        literal("teleport")
            .description("Teleports entities")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            .then(argument("location", ArgumentType::Vec3).executes(
                |ctx: &CommandContext<CommandSourceStack>| {
                    let (x, y, z) = get_vec3(ctx, "location")?;
                    // TODO: Actually teleport the source player
                    ctx.source.send_success(
                        &Component::translatable(
                            "commands.teleport.success.location.single",
                            vec![
                                Component::text(&ctx.source.display_name),
                                Component::text(format!("{x:.2}")),
                                Component::text(format!("{y:.2}")),
                                Component::text(format!("{z:.2}")),
                            ],
                        ),
                        true,
                    );
                    Ok(1)
                },
            ))
            .then(
                argument(
                    "destination",
                    ArgumentType::Entity {
                        single: true,
                        player_only: false,
                    },
                )
                .executes(|ctx: &CommandContext<CommandSourceStack>| {
                    let dest = get_entity(ctx, "destination")?;
                    // TODO: Resolve entity selector, get target position, teleport
                    ctx.source.send_success(
                        &Component::translatable(
                            "commands.teleport.success.entity.single",
                            vec![
                                Component::text(&ctx.source.display_name),
                                Component::text(&dest.name),
                            ],
                        ),
                        true,
                    );
                    Ok(1)
                }),
            ),
    );
}
