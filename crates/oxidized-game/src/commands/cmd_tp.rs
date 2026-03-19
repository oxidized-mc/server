//! `/tp` and `/teleport` commands.

use crate::commands::arguments::ArgumentType;
use crate::commands::context::{CommandContext, get_string, get_vec3};
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/tp` and `/teleport` commands.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("tp")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            // /tp <location> (vec3)
            .then(argument("location", ArgumentType::Vec3).executes(
                |ctx: &CommandContext<CommandSourceStack>| {
                    let (x, y, z) = get_vec3(ctx, "location")?;
                    ctx.source.send_success(
                        &Component::text(format!(
                            "Teleported {} to {x:.2}, {y:.2}, {z:.2}",
                            ctx.source.display_name
                        )),
                        true,
                    );
                    Ok(1)
                },
            ))
            // /tp <destination> (entity selector — parsed as string for now)
            .then(
                argument(
                    "destination",
                    ArgumentType::Entity {
                        single: true,
                        player_only: false,
                    },
                )
                .executes(|ctx: &CommandContext<CommandSourceStack>| {
                    let dest = get_string(ctx, "destination")?;
                    ctx.source.send_success(
                        &Component::text(format!(
                            "Teleported {} to {dest}",
                            ctx.source.display_name
                        )),
                        true,
                    );
                    Ok(1)
                }),
            ),
    );

    // /teleport is an alias — register as a separate literal with same structure
    d.register(
        literal("teleport")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            .then(argument("location", ArgumentType::Vec3).executes(
                |ctx: &CommandContext<CommandSourceStack>| {
                    let (x, y, z) = get_vec3(ctx, "location")?;
                    ctx.source.send_success(
                        &Component::text(format!(
                            "Teleported {} to {x:.2}, {y:.2}, {z:.2}",
                            ctx.source.display_name
                        )),
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
                    let dest = get_string(ctx, "destination")?;
                    ctx.source.send_success(
                        &Component::text(format!(
                            "Teleported {} to {dest}",
                            ctx.source.display_name
                        )),
                        true,
                    );
                    Ok(1)
                }),
            ),
    );
}
