//! `/list` command — list online players.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/list` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("list")
            .description("Lists players on the server")
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                let names = ctx.source.server.online_player_names();
                let count = names.len();
                let max = ctx.source.server.max_players();
                let player_list = if names.is_empty() {
                    String::new()
                } else {
                    names.join(", ")
                };
                ctx.source.send_success(
                    &Component::translatable(
                        "commands.list.players",
                        vec![
                            Component::text(count.to_string()),
                            Component::text(max.to_string()),
                            Component::text(player_list),
                        ],
                    ),
                    false,
                );
                Ok(count as i32)
            }),
    );
}
