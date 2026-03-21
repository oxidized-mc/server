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
            .executes(execute_list)
            .then(literal("uuids").executes(execute_list_uuids)),
    );
}

/// Executes `/list` — shows online player count and names.
fn execute_list(ctx: &CommandContext<CommandSourceStack>) -> Result<i32, crate::commands::CommandError> {
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
}

/// Executes `/list uuids` — shows online players with their UUIDs.
fn execute_list_uuids(
    ctx: &CommandContext<CommandSourceStack>,
) -> Result<i32, crate::commands::CommandError> {
    let names = ctx.source.server.online_player_names();
    let count = names.len();
    let max = ctx.source.server.max_players();

    let entries: Vec<String> = names
        .iter()
        .map(|name| {
            if let Some(uuid) = ctx.source.server.find_player_uuid(name) {
                format!("{name} ({uuid})")
            } else {
                name.clone()
            }
        })
        .collect();
    let player_list = entries.join(", ");

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
}
