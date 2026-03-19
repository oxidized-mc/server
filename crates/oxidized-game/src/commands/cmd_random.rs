//! `/random` command — generate random values or manage sequences.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/random` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("random")
            .description("Generate random values or manage sequences")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /random
                ctx.source.send_failure(&Component::translatable(
                    "commands.help.failed",
                    vec![],
                ));
                ctx.source.send_failure(&Component::text(
                    "/random is not yet implemented",
                ));
                Ok(0)
            }),
    );
}
