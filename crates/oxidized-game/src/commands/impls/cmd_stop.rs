//! `/stop` command — shuts down the server.

use crate::commands::nodes::LiteralBuilderExt;
use crate::commands::source::CommandSourceStack;
use oxidized_commands::context::CommandContext;
use oxidized_commands::dispatcher::CommandDispatcher;
use oxidized_commands::nodes::literal;

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
