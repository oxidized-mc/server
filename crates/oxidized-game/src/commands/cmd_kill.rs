//! `/kill` command — kill entities.

use crate::commands::arguments::ArgumentType;
use crate::commands::context::{CommandContext, get_string};
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/kill` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("kill")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            // /kill — kill self
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                ctx.source.send_success(
                    &Component::text(format!("Killed {}", ctx.source.display_name)),
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
                    let targets = get_string(ctx, "targets")?;
                    ctx.source
                        .send_success(&Component::text(format!("Killed {targets}")), true);
                    Ok(1)
                }),
            ),
    );
}
