//! `/teammsg` command — send a message to team members.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/teammsg` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("teammsg")
            .description("Send a message to team members")
            .requires(|s: &CommandSourceStack| s.has_permission(0))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /teammsg
                ctx.source.send_failure(&Component::translatable(
                    "commands.help.failed",
                    vec![],
                ));
                ctx.source.send_failure(&Component::text(
                    "/teammsg is not yet implemented",
                ));
                Ok(0)
            }),
    );
    d.register(
        literal("tm")
            .description("Send a message to team members")
            .requires(|s: &CommandSourceStack| s.has_permission(0))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /tm
                ctx.source.send_failure(&Component::translatable(
                    "commands.help.failed",
                    vec![],
                ));
                ctx.source.send_failure(&Component::text(
                    "/tm is not yet implemented",
                ));
                Ok(0)
            }),
    );
}
