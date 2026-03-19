//! `/locate` command — locate structures, biomes, or pois.
//!
//! TODO: Full implementation requires additional game systems.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/locate` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("locate")
            .description("Locate structures, biomes, or POIs")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                // TODO: Implement /locate
                ctx.source.send_failure(&Component::translatable(
                    "commands.help.failed",
                    vec![],
                ));
                ctx.source.send_failure(&Component::text(
                    "/locate is not yet implemented",
                ));
                Ok(0)
            }),
    );
}
