//! `/setblock` command — set a block at a position.
//!
//! TODO: Actually setting blocks requires `ServerLevel` access from commands
//! and broadcasting block change packets to nearby clients.

use crate::commands::arguments::ArgumentType;
use crate::commands::argument_access::{get_block_pos, get_string};
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
    let _block = get_string(ctx, "block")?;
    // TODO: Actually set the block via ServerLevel
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
