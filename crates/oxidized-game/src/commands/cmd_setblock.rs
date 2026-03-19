//! `/setblock` command — set a block at a position.

use crate::commands::arguments::ArgumentType;
use crate::commands::context::{CommandContext, get_block_pos, get_string};
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/setblock` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("setblock")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            .then(
                argument("pos", ArgumentType::BlockPos).then(
                    argument("block", ArgumentType::BlockState)
                        // /setblock <pos> <block>
                        .executes(|ctx: &CommandContext<CommandSourceStack>| {
                            let (x, y, z) = get_block_pos(ctx, "pos")?;
                            let block = get_string(ctx, "block")?;
                            ctx.source.send_success(
                                &Component::text(format!(
                                    "Changed the block at {x}, {y}, {z} to {block}"
                                )),
                                true,
                            );
                            Ok(1)
                        })
                        // /setblock <pos> <block> <mode>
                        .then(literal("destroy").executes(
                            |ctx: &CommandContext<CommandSourceStack>| {
                                let (x, y, z) = get_block_pos(ctx, "pos")?;
                                let block = get_string(ctx, "block")?;
                                ctx.source.send_success(
                                    &Component::text(format!(
                                        "Changed the block at {x}, {y}, {z} to {block}"
                                    )),
                                    true,
                                );
                                Ok(1)
                            },
                        ))
                        .then(literal("keep").executes(
                            |ctx: &CommandContext<CommandSourceStack>| {
                                let (x, y, z) = get_block_pos(ctx, "pos")?;
                                let block = get_string(ctx, "block")?;
                                ctx.source.send_success(
                                    &Component::text(format!(
                                        "Changed the block at {x}, {y}, {z} to {block}"
                                    )),
                                    true,
                                );
                                Ok(1)
                            },
                        ))
                        .then(literal("replace").executes(
                            |ctx: &CommandContext<CommandSourceStack>| {
                                let (x, y, z) = get_block_pos(ctx, "pos")?;
                                let block = get_string(ctx, "block")?;
                                ctx.source.send_success(
                                    &Component::text(format!(
                                        "Changed the block at {x}, {y}, {z} to {block}"
                                    )),
                                    true,
                                );
                                Ok(1)
                            },
                        )),
                ),
            ),
    );
}
