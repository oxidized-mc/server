//! `/save-all` command — save the server to disk.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/save-all` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("save-all")
            .description("Save the server to disk")
            .requires(|s: &CommandSourceStack| s.has_permission(4))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /save-all
                ctx.source.send_failure(&Component::translatable(
                    "commands.help.failed",
                    vec![],
                ));
                ctx.source.send_failure(&Component::text(
                    "/save-all is not yet implemented",
                ));
                Ok(0)
            }),
    );
}
