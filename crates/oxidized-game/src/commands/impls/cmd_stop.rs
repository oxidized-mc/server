//! `/stop` command — shuts down the server.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;

/// Registers the `/stop` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("stop")
            .description("Stops the server")
            .requires_op_level(4)
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                ctx.source
                    .send_translatable_success("commands.stop.stopping", vec![], true);
                ctx.source.server.request_shutdown();
                Ok(1)
            }),
    );
}
