//! `/op` command — grant operator status to a player.

use crate::commands::argument_access::get_game_profile;
use crate::commands::nodes::LiteralBuilderExt;
use crate::commands::source::CommandSourceStack;
use oxidized_chat::Component;
use oxidized_commands::arguments::ArgumentType;
use oxidized_commands::context::CommandContext;
use oxidized_commands::dispatcher::CommandDispatcher;
use oxidized_commands::nodes::{argument, literal};

/// Registers the `/op` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("op")
            .description("Grant operator status")
            .requires_op_level(3)
            .then(argument("targets", ArgumentType::GameProfile).executes(
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
                        ctx.source
                            .send_failure(&Component::translatable("commands.op.failed", vec![]));
                        Ok(0)
                    }
                },
            )),
    );
}
