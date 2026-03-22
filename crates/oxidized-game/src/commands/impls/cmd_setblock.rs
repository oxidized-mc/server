//! `/setblock` command — set a block at a position.

use crate::commands::argument_access::{get_block_pos, get_string};
use crate::commands::arguments::ArgumentType;
use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/setblock` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("setblock")
            .description("Changes a block to another block")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            .then(
                argument("pos", ArgumentType::BlockPos).then(
                    argument("block", ArgumentType::BlockState)
                        // /setblock <pos> <block>
                        .executes(setblock_exec)
                        // /setblock <pos> <block> <mode>
                        .then(literal("destroy").executes(setblock_exec))
                        .then(literal("keep").executes(setblock_exec))
                        .then(literal("replace").executes(setblock_exec)),
                ),
            ),
    );
}

fn setblock_exec(
    ctx: &CommandContext<CommandSourceStack>,
) -> Result<i32, crate::commands::CommandError> {
    let (x, y, z) = get_block_pos(ctx, "pos")?;
    let block = get_string(ctx, "block")?;

    let block_name = if block.contains(':') {
        block.to_string()
    } else {
        format!("minecraft:{block}")
    };

    if !ctx.source.server.set_block(x, y, z, &block_name) {
        ctx.source
            .send_failure(&Component::translatable("commands.setblock.failed", vec![]));
        return Ok(0);
    }

    ctx.source.send_success(
        &Component::translatable(
            "commands.setblock.success",
            vec![
                Component::text(x.to_string()),
                Component::text(y.to_string()),
                Component::text(z.to_string()),
            ],
        ),
        true,
    );
    Ok(1)
}
