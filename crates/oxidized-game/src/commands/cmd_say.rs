//! `/say` command — broadcast a message via DisguisedChat.

use crate::commands::arguments::{ArgumentType, StringKind};
use crate::commands::context::{CommandContext, get_string};
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/say` and `/me` commands.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("say")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            .then(
                argument("message", ArgumentType::String(StringKind::GreedyPhrase)).executes(
                    |ctx: &CommandContext<CommandSourceStack>| {
                        let message = get_string(ctx, "message")?;
                        ctx.source.send_success(
                            &Component::text(format!("[{}] {message}", ctx.source.display_name)),
                            true,
                        );
                        Ok(1)
                    },
                ),
            ),
    );

    d.register(literal("me").then(
        argument("action", ArgumentType::String(StringKind::GreedyPhrase)).executes(
            |ctx: &CommandContext<CommandSourceStack>| {
                let action = get_string(ctx, "action")?;
                ctx.source.send_success(
                    &Component::text(format!("* {} {action}", ctx.source.display_name)),
                    true,
                );
                Ok(1)
            },
        ),
    ));
}
