//! `/give` command — give items to players.

use crate::commands::arguments::ArgumentType;
use crate::commands::context::{CommandContext, get_integer, get_string};
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/give` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("give")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            .then(
                argument(
                    "targets",
                    ArgumentType::Entity {
                        single: false,
                        player_only: true,
                    },
                )
                .then(
                    argument("item", ArgumentType::ItemStack)
                        // /give <targets> <item>
                        .executes(|ctx: &CommandContext<CommandSourceStack>| {
                            let targets = get_string(ctx, "targets")?;
                            let item = get_string(ctx, "item")?;
                            ctx.source.send_success(
                                &Component::text(format!("Gave 1 [{item}] to {targets}")),
                                true,
                            );
                            Ok(1)
                        })
                        // /give <targets> <item> <count>
                        .then(
                            argument(
                                "count",
                                ArgumentType::Integer {
                                    min: Some(1),
                                    max: Some(2_147_483_647),
                                },
                            )
                            .executes(
                                |ctx: &CommandContext<CommandSourceStack>| {
                                    let targets = get_string(ctx, "targets")?;
                                    let item = get_string(ctx, "item")?;
                                    let count = get_integer(ctx, "count")?;
                                    ctx.source.send_success(
                                        &Component::text(format!(
                                            "Gave {count} [{item}] to {targets}"
                                        )),
                                        true,
                                    );
                                    Ok(count)
                                },
                            ),
                        ),
                ),
            ),
    );
}
