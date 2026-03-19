//! `/trigger` command — modify a trigger scoreboard objective.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/trigger` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("trigger")
            .description("Modify a trigger scoreboard objective")
            .requires(|s: &CommandSourceStack| s.has_permission(0))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /trigger
                ctx.source.send_failure(&Component::translatable(
                    "commands.help.failed",
                    vec![],
                ));
                ctx.source.send_failure(&Component::text(
                    "/trigger is not yet implemented",
                ));
                Ok(0)
            }),
    );
}
