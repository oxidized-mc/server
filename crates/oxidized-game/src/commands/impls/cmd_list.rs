//! `/list` command — list online players.

use crate::commands::source::CommandSourceStack;
use oxidized_chat::Component;
use oxidized_commands::context::CommandContext;
use oxidized_commands::dispatcher::CommandDispatcher;
use oxidized_commands::nodes::literal;

/// Registers the `/list` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("list")
            .description("Lists players on the server")
            .executes(|ctx| execute_list_impl(ctx, false))
            .then(literal("uuids").executes(|ctx| execute_list_impl(ctx, true))),
    );
}

/// Shared implementation for `/list` and `/list uuids`.
fn execute_list_impl(
    ctx: &CommandContext<CommandSourceStack>,
    include_uuids: bool,
) -> Result<i32, oxidized_commands::CommandError> {
    let names = ctx.source.server.online_player_names();
    let count = names.len();
    let max = ctx.source.server.max_players();

    let player_list = if include_uuids {
        names
            .iter()
            .map(|name| {
                if let Some(uuid) = ctx.source.server.find_player_uuid(name) {
                    format!("{name} ({uuid})")
                } else {
                    name.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    } else if names.is_empty() {
        String::new()
    } else {
        names.join(", ")
    };

    ctx.source.send_translatable_success(
        "commands.list.players",
        vec![
            Component::text(count.to_string()),
            Component::text(max.to_string()),
            Component::text(player_list),
        ],
        false,
    );
    Ok(count as i32)
}
