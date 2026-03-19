//! `/me` command — send an action message.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/me` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("me")
            .description("Send an action message")
            .requires(|s: &CommandSourceStack| s.has_permission(0))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /me
                ctx.source
                    .send_failure(&Component::translatable("commands.help.failed", vec![]));
                ctx.source
                    .send_failure(&Component::text("/me is not yet implemented"));
                Ok(0)
            }),
    );
}
