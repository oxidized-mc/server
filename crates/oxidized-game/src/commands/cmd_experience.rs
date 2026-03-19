//! `/experience` command — add or query player experience.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/experience` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("experience")
            .description("Add or query player experience")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /experience
                ctx.source.send_failure(&Component::translatable(
                    "commands.help.failed",
                    vec![],
                ));
                ctx.source.send_failure(&Component::text(
                    "/experience is not yet implemented",
                ));
                Ok(0)
            }),
    );
    d.register(
        literal("xp")
            .description("Add or query player experience")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /xp
                ctx.source.send_failure(&Component::translatable(
                    "commands.help.failed",
                    vec![],
                ));
                ctx.source.send_failure(&Component::text(
                    "/xp is not yet implemented",
                ));
                Ok(0)
            }),
    );
}
