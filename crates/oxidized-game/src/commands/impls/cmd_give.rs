//! `/give` command — give items to players.
//!
//! TODO: Requires inventory system (player inventory component in ECS),
//! item stack creation from registry ID, and `ClientboundContainerSetSlotPacket`
//! to sync the slot to the client.

use crate::commands::arguments::ArgumentType;
use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;

/// Registers the `/give` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("give")
            .description("Gives an item to a player")
            .requires_op()
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
    _ctx: &CommandContext<CommandSourceStack>,
) -> Result<i32, crate::commands::CommandError> {
    // TODO: Actually give items to target players — requires inventory
    // system (player inventory component in ECS), item stack creation
    // from registry ID, and ClientboundContainerSetSlotPacket.
    Err(crate::commands::CommandError::NotImplemented("give".into()))
}
