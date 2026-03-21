//! `/give` command — give items to players.
//!
//! TODO: Requires inventory system (player inventory component in ECS),
//! item stack creation from registry ID, and `ClientboundContainerSetSlotPacket`
//! to sync the slot to the client.

use crate::commands::arguments::ArgumentType;
use crate::commands::argument_access::{get_entities, get_integer, get_string};
use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/give` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("give")
            .description("Gives an item to a player")
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
                        .executes(give_exec)
                        // /give <targets> <item> <count>
                        .then(
                            argument(
                                "count",
                                ArgumentType::Integer {
                                    min: Some(1),
                                    max: Some(2_147_483_647),
                                },
                            )
                            .executes(give_exec),
                        ),
                ),
            ),
    );
}

fn give_exec(
    ctx: &CommandContext<CommandSourceStack>,
) -> Result<i32, crate::commands::CommandError> {
    let targets = get_entities(ctx, "targets")?;
    let item = get_string(ctx, "item")?;
    let count = get_integer(ctx, "count").unwrap_or(1);
    // TODO: Actually give items to target players
    for target in &targets {
        ctx.source.send_success(
            &Component::translatable(
                "commands.give.success.single",
                vec![
                    Component::text(count.to_string()),
                    Component::text(format!("[{item}]")),
                    Component::text(&target.name),
                ],
            ),
            true,
        );
    }
    Ok(count)
}
