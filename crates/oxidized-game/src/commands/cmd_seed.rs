//! `/seed` command — show the world seed.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/seed` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("seed")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                let seed = ctx.source.server.seed();
                ctx.source
                    .send_success(&Component::text(format!("Seed: [{seed}]")), false);
                Ok(seed as i32)
            }),
    );
}
