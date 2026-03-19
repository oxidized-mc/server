//! `/kill` command — kill entities.
//!
//! TODO: Actually killing entities requires health/damage system and death
//! event handling. Needs `ServerHandle::kill_entity()` or similar.

use crate::commands::arguments::ArgumentType;
use crate::commands::context::{CommandContext, get_entities};
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/kill` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("kill")
            .description("Kills entities")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            // /kill — kill self
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Actually kill the source player
                ctx.source.send_success(
                    &Component::translatable(
                        "commands.kill.success.single",
                        vec![Component::text(&ctx.source.display_name)],
                    ),
                    true,
                );
                Ok(1)
            })
            // /kill <targets>
            .then(
                argument(
                    "targets",
                    ArgumentType::Entity {
                        single: false,
                        player_only: false,
                    },
                )
                .executes(|ctx: &CommandContext<CommandSourceStack>| {
                    let targets = get_entities(ctx, "targets")?;
                    // TODO: Resolve entity selector and kill matching entities
                    for target in &targets {
                        ctx.source.send_success(
                            &Component::translatable(
                                "commands.kill.success.single",
                                vec![Component::text(&target.name)],
                            ),
                            true,
                        );
                    }
                    Ok(targets.len() as i32)
                }),
            ),
    );
}
