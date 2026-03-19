//! `/scoreboard` command — manage scoreboards and objectives.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/scoreboard` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("scoreboard")
            .description("Manage scoreboards and objectives")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /scoreboard
                ctx.source
                    .send_failure(&Component::translatable("commands.help.failed", vec![]));
                ctx.source
                    .send_failure(&Component::text("/scoreboard is not yet implemented"));
                Ok(0)
            }),
    );
}
