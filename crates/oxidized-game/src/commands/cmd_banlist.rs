//! `/banlist` command — list banned players and ips.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/banlist` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("banlist")
            .description("List banned players and IPs")
            .requires(|s: &CommandSourceStack| s.has_permission(3))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /banlist
                ctx.source.send_failure(&Component::translatable(
                    "commands.help.failed",
                    vec![],
                ));
                ctx.source.send_failure(&Component::text(
                    "/banlist is not yet implemented",
                ));
                Ok(0)
            }),
    );
}
