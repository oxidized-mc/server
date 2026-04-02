//! `/say` and `/me` commands — broadcast messages via chat types.
//!
//! Vanilla uses `ChatType.SAY_COMMAND` (`chat.type.announcement`) for `/say`
//! and `ChatType.EMOTE_COMMAND` (`chat.type.emote`) for `/me`. We broadcast
//! via the server's chat channel.

use crate::commands::nodes::LiteralBuilderExt;
use crate::commands::source::CommandSourceStack;
use oxidized_chat::Component;
use oxidized_commands::argument_access::get_string;
use oxidized_commands::arguments::{ArgumentType, StringKind};
use oxidized_commands::context::CommandContext;
use oxidized_commands::dispatcher::CommandDispatcher;
use oxidized_commands::nodes::{argument, literal};

/// Registers the `/say` and `/me` commands.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("say")
            .description("Sends a message in chat to other players")
            .requires_op()
            .then(
                argument("message", ArgumentType::String(StringKind::GreedyPhrase)).executes(
                    |ctx: &CommandContext<CommandSourceStack>| {
                        let message = get_string(ctx, "message")?;
                        // Broadcast using SAY_COMMAND chat type format
                        ctx.source.send_translatable_success(
                            "chat.type.announcement",
                            vec![
                                Component::text(&ctx.source.display_name),
                                Component::text(message),
                            ],
                            true,
                        );
                        Ok(1)
                    },
                ),
            ),
    );

    d.register(
        literal("me")
            .description("Displays a message about yourself")
            .then(
                argument("action", ArgumentType::String(StringKind::GreedyPhrase)).executes(
                    |ctx: &CommandContext<CommandSourceStack>| {
                        let action = get_string(ctx, "action")?;
                        // Broadcast using EMOTE_COMMAND chat type format
                        ctx.source.send_translatable_success(
                            "chat.type.emote",
                            vec![
                                Component::text(&ctx.source.display_name),
                                Component::text(action),
                            ],
                            true,
                        );
                        Ok(1)
                    },
                ),
            ),
    );
}
