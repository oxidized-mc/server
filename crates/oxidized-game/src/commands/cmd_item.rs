//! `/item` command — manipulate items in inventories.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/item` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("item")
            .description("Manipulate items in inventories")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /item
                ctx.source
                    .send_failure(&Component::translatable("commands.help.failed", vec![]));
                ctx.source
                    .send_failure(&Component::text("/item is not yet implemented"));
                Ok(0)
            }),
    );
}
