//! `/transfer` command — transfer players to another server.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/transfer` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("transfer")
            .description("Transfer players to another server")
            .requires(|s: &CommandSourceStack| s.has_permission(3))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /transfer
                ctx.source.send_failure(&Component::translatable(
                    "commands.help.failed",
                    vec![],
                ));
                ctx.source.send_failure(&Component::text(
                    "/transfer is not yet implemented",
                ));
                Ok(0)
            }),
    );
}
