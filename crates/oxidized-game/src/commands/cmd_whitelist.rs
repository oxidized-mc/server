//! `/whitelist` command — manage the server whitelist.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/whitelist` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("whitelist")
            .description("Manage the server whitelist")
            .requires(|s: &CommandSourceStack| s.has_permission(3))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /whitelist
                ctx.source.send_failure(&Component::translatable(
                    "commands.help.failed",
                    vec![],
                ));
                ctx.source.send_failure(&Component::text(
                    "/whitelist is not yet implemented",
                ));
                Ok(0)
            }),
    );
}
