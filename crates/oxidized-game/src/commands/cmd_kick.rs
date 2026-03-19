//! `/kick` command — disconnect a player.

use crate::commands::arguments::{ArgumentType, StringKind};
use crate::commands::context::{CommandContext, get_string};
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/kick` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("kick")
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
                    let targets = get_string(ctx, "targets")?;
                    ctx.source
                        .send_success(&Component::text(format!("Kicked {targets}")), true);
                    Ok(1)
                })
                // /kick <targets> <reason>
                .then(
                    argument("reason", ArgumentType::String(StringKind::GreedyPhrase)).executes(
                        |ctx: &CommandContext<CommandSourceStack>| {
                            let targets = get_string(ctx, "targets")?;
                            let reason = get_string(ctx, "reason")?;
                            ctx.source.send_success(
                                &Component::text(format!("Kicked {targets}: {reason}")),
                                true,
                            );
                            Ok(1)
                        },
                    ),
                ),
            ),
    );
}
