//! `/msg` command — send a private message.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/msg` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("msg")
            .description("Send a private message")
            .requires(|s: &CommandSourceStack| s.has_permission(0))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /msg
                ctx.source
                    .send_failure(&Component::translatable("commands.help.failed", vec![]));
                ctx.source
                    .send_failure(&Component::text("/msg is not yet implemented"));
                Ok(0)
            }),
    );
    d.register(
        literal("tell")
            .description("Send a private message")
            .requires(|s: &CommandSourceStack| s.has_permission(0))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /tell
                ctx.source
                    .send_failure(&Component::translatable("commands.help.failed", vec![]));
                ctx.source
                    .send_failure(&Component::text("/tell is not yet implemented"));
                Ok(0)
            }),
    );
    d.register(
        literal("w")
            .description("Send a private message")
            .requires(|s: &CommandSourceStack| s.has_permission(0))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /w
                ctx.source
                    .send_failure(&Component::translatable("commands.help.failed", vec![]));
                ctx.source
                    .send_failure(&Component::text("/w is not yet implemented"));
                Ok(0)
            }),
    );
}
