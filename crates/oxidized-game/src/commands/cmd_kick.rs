//! `/kick` command — disconnect a player.
//!
//! Uses `ServerHandle::kick_player()` which currently logs but doesn't
//! disconnect. Full implementation needs a dedicated kick channel.
//!
//! TODO: Wire `kick_player()` to actually disconnect the player's TCP
//! connection with a disconnect packet.

use crate::commands::arguments::{ArgumentType, StringKind};
use crate::commands::argument_access::{get_entities, get_string};
use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/kick` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("kick")
            .description("Kicks a player off the server")
            .requires(|s: &CommandSourceStack| s.has_permission(3))
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
                    let targets = get_entities(ctx, "targets")?;
                    for target in &targets {
                        let kicked = ctx
                            .source
                            .server
                            .kick_player(&target.name, "Kicked by an operator");
                        if kicked {
                            ctx.source.send_success(
                                &Component::translatable(
                                    "commands.kick.success",
                                    vec![
                                        Component::text(&target.name),
                                        Component::text("Kicked by an operator"),
                                    ],
                                ),
                                true,
                            );
                        } else {
                            ctx.source.send_failure(&Component::translatable(
                                "argument.entity.notfound.player",
                                vec![],
                            ));
                        }
                    }
                    Ok(1)
                })
                // /kick <targets> <reason>
                .then(
                    argument("reason", ArgumentType::String(StringKind::GreedyPhrase)).executes(
                        |ctx: &CommandContext<CommandSourceStack>| {
                            let targets = get_entities(ctx, "targets")?;
                            let reason = get_string(ctx, "reason")?;
                            for target in &targets {
                                let kicked = ctx.source.server.kick_player(&target.name, reason);
                                if kicked {
                                    ctx.source.send_success(
                                        &Component::translatable(
                                            "commands.kick.success",
                                            vec![
                                                Component::text(&target.name),
                                                Component::text(reason),
                                            ],
                                        ),
                                        true,
                                    );
                                } else {
                                    ctx.source.send_failure(&Component::translatable(
                                        "argument.entity.notfound.player",
                                        vec![],
                                    ));
                                }
                            }
                            Ok(1)
                        },
                    ),
                ),
            ),
    );
}
