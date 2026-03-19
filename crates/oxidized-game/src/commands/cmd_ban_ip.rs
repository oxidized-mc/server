//! `/ban-ip` command — ban an ip address from the server.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/ban-ip` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("ban-ip")
            .description("Ban an IP address from the server")
            .requires(|s: &CommandSourceStack| s.has_permission(3))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /ban-ip
                ctx.source
                    .send_failure(&Component::translatable("commands.help.failed", vec![]));
                ctx.source
                    .send_failure(&Component::text("/ban-ip is not yet implemented"));
                Ok(0)
            }),
    );
}
