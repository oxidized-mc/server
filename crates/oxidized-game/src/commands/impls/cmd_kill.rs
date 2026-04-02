//! `/kill` command — kill entities.
//!
//! TODO: Actually killing entities requires health/damage system and death
//! event handling. Needs `ServerHandle::kill_entity()` or similar.

use crate::commands::CommandError;
use crate::commands::arguments::ArgumentType;
use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::nodes::LiteralBuilderExt;
use crate::commands::source::CommandSourceStack;

/// Registers the `/kill` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("kill")
            .description("Kills entities")
            .requires_op()
            // /kill — kill self
            .executes(|_ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Actually kill the source player — requires
                // health/damage system and death event handling.
                Err(CommandError::NotImplemented("kill".into()))
            })
            // /kill <targets>
            .then(
                argument(
                    "targets",
                    ArgumentType::Entity {
                        single: false,
                        player_only: false,
                    },
                )
                .executes(|_ctx: &CommandContext<CommandSourceStack>| {
                    // TODO: Resolve entity selector and kill matching entities
                    Err(CommandError::NotImplemented("kill".into()))
                }),
            ),
    );
}
