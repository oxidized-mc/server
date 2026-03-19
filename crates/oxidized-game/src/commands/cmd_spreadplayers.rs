//! `/spreadplayers` command — spread players around a point.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/spreadplayers` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("spreadplayers")
            .description("Spread players around a point")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /spreadplayers
                ctx.source
                    .send_failure(&Component::translatable("commands.help.failed", vec![]));
                ctx.source
                    .send_failure(&Component::text("/spreadplayers is not yet implemented"));
                Ok(0)
            }),
    );
}
