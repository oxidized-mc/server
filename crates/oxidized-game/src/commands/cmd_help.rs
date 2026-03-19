//! `/help` command — list available commands.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/help` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("help").executes(|ctx: &CommandContext<CommandSourceStack>| {
            ctx.source
                .send_message(&Component::text("--- Showing help page 1 of 1 ---"));
            ctx.source
                .send_message(&Component::text("/help - Shows this list"));
            ctx.source
                .send_message(&Component::text("/list - Lists online players"));
            ctx.source
                .send_message(&Component::text("/stop - Stops the server"));
            Ok(1)
        }),
    );
}
