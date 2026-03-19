//! `/publish` command — open the server to lan.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/publish` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("publish")
            .description("Open the server to LAN")
            .requires(|s: &CommandSourceStack| s.has_permission(4))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /publish
                ctx.source
                    .send_failure(&Component::translatable("commands.help.failed", vec![]));
                ctx.source
                    .send_failure(&Component::text("/publish is not yet implemented"));
                Ok(0)
            }),
    );
}
