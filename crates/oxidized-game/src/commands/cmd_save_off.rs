//! `/save-off` command — disable automatic saving.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/save-off` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("save-off")
            .description("Disable automatic saving")
            .requires(|s: &CommandSourceStack| s.has_permission(4))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /save-off
                ctx.source
                    .send_failure(&Component::translatable("commands.help.failed", vec![]));
                ctx.source
                    .send_failure(&Component::text("/save-off is not yet implemented"));
                Ok(0)
            }),
    );
}
