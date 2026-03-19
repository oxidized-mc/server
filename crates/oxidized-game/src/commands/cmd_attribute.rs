//! `/attribute` command — query or modify entity attributes.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/attribute` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("attribute")
            .description("Query or modify entity attributes")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /attribute
                ctx.source.send_failure(&Component::translatable(
                    "commands.help.failed",
                    vec![],
                ));
                ctx.source.send_failure(&Component::text(
                    "/attribute is not yet implemented",
                ));
                Ok(0)
            }),
    );
}
