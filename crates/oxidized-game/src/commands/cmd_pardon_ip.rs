//! `/pardon-ip` command — remove an ip from the ban list.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/pardon-ip` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("pardon-ip")
            .description("Remove an IP from the ban list")
            .requires(|s: &CommandSourceStack| s.has_permission(3))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /pardon-ip
                ctx.source
                    .send_failure(&Component::translatable("commands.help.failed", vec![]));
                ctx.source
                    .send_failure(&Component::text("/pardon-ip is not yet implemented"));
                Ok(0)
            }),
    );
}
