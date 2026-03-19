//! `/list` command — list online players.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/list` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("list").executes(|ctx: &CommandContext<CommandSourceStack>| {
            let names = ctx.source.server.online_player_names();
            let count = names.len();
            let max = ctx.source.server.max_players();
            if names.is_empty() {
                ctx.source.send_success(
                    &Component::text(format!(
                        "There are {count} of a max of {max} players online"
                    )),
                    false,
                );
            } else {
                let list = names.join(", ");
                ctx.source.send_success(
                    &Component::text(format!(
                        "There are {count} of a max of {max} players online: {list}"
                    )),
                    false,
                );
            }
            Ok(count as i32)
        }),
    );
}
