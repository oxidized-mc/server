//! `/jfr` command — start or stop jfr profiling.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/jfr` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("jfr")
            .description("Start or stop JFR profiling")
            .requires(|s: &CommandSourceStack| s.has_permission(4))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /jfr
                ctx.source.send_failure(&Component::translatable(
                    "commands.help.failed",
                    vec![],
                ));
                ctx.source.send_failure(&Component::text(
                    "/jfr is not yet implemented",
                ));
                Ok(0)
            }),
    );
}
