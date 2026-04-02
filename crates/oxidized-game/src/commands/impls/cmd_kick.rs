//! `/kick` command — disconnect a player.
//!
//! Uses `ServerHandle::kick_player()` which currently logs but doesn't
//! disconnect. Full implementation needs a dedicated kick channel.
//!
//! TODO: Wire `kick_player()` to actually disconnect the player's TCP
//! connection with a disconnect packet.

use crate::commands::argument_access::get_entities;
use crate::commands::nodes::LiteralBuilderExt;
use crate::commands::source::CommandSourceStack;
use oxidized_chat::Component;
use oxidized_commands::argument_access::get_string;
use oxidized_commands::arguments::{ArgumentType, StringKind};
use oxidized_commands::context::CommandContext;
use oxidized_commands::dispatcher::CommandDispatcher;
use oxidized_commands::nodes::{argument, literal};

/// Registers the `/kick` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("kick")
            .description("Kicks a player off the server")
            .requires_op_level(3)
            .then(
                argument(
                    "targets",
                    ArgumentType::Entity {
                        single: false,
                        player_only: true,
                    },
                )
                // /kick <targets>
                .executes(|ctx: &CommandContext<CommandSourceStack>| {
                    kick_targets(ctx, "Kicked by an operator")
                })
                // /kick <targets> <reason>
                .then(
                    argument("reason", ArgumentType::String(StringKind::GreedyPhrase)).executes(
                        |ctx: &CommandContext<CommandSourceStack>| {
                            let reason = get_string(ctx, "reason")?;
                            kick_targets(ctx, reason)
                        },
                    ),
                ),
            ),
    );
}

/// Kicks all resolved targets with the given reason message.
fn kick_targets(
    ctx: &CommandContext<CommandSourceStack>,
    reason: &str,
) -> Result<i32, oxidized_commands::CommandError> {
    let targets = get_entities(ctx, "targets")?;
    for target in &targets {
        let kicked = ctx.source.server.kick_player(&target.name, reason);
        if kicked {
            ctx.source.send_translatable_success(
                "commands.kick.success",
                vec![Component::text(&target.name), Component::text(reason)],
                true,
            );
        } else {
            ctx.source
                .send_translatable_failure("argument.entity.notfound.player", vec![]);
        }
    }
    Ok(1)
}
