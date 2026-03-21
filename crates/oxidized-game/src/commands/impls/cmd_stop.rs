//! `/stop` command — shuts down the server.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/stop` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("stop")
            .description("Stops the server")
            .requires(|s: &CommandSourceStack| s.has_permission(4))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                ctx.source.send_success(
                    &Component::translatable("commands.stop.stopping", vec![]),
                    true,
                );
                ctx.source.server.request_shutdown();
                Ok(1)
            }),
    );
}
