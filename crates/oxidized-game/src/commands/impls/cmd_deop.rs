//! `/deop` command — revoke operator status from a player.

use crate::commands::argument_access::get_game_profile;
use crate::commands::arguments::ArgumentType;
use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/deop` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("deop")
            .description("Revoke operator status")
            .requires_op_level(3)
            .then(argument("targets", ArgumentType::GameProfile).executes(
                |ctx: &CommandContext<CommandSourceStack>| {
                    let target = get_game_profile(ctx, "targets")?;
                    if ctx.source.server.deop_player(target.uuid) {
                        ctx.source.send_translatable_success(
                            "commands.deop.success",
                            vec![Component::text(&target.name)],
                            true,
                        );
                        Ok(1)
                    } else {
                        ctx.source
                            .send_failure(&Component::translatable("commands.deop.failed", vec![]));
                        Ok(0)
                    }
                },
            )),
    );
}
