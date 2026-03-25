//! `/op` command — grant operator status to a player.

use crate::commands::argument_access::get_game_profile;
use crate::commands::arguments::ArgumentType;
use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/op` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("op")
            .description("Grant operator status")
            .requires(|s: &CommandSourceStack| s.has_permission(3))
            .then(
                argument("targets", ArgumentType::GameProfile).executes(
                    |ctx: &CommandContext<CommandSourceStack>| {
                        let target = get_game_profile(ctx, "targets")?;
                        if ctx.source.server.op_player(target.uuid, &target.name) {
                            ctx.source.send_translatable_success(
                                "commands.op.success",
                                vec![Component::text(&target.name)],
                                true,
                            );
                            Ok(1)
                        } else {
                            ctx.source.send_failure(&Component::translatable(
                                "commands.op.failed",
                                vec![],
                            ));
                            Ok(0)
                        }
                    },
                ),
            ),
    );
}
