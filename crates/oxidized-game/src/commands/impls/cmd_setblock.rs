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
                        .executes(setblock_replace) // default = replace
                        .then(literal("destroy").executes(setblock_destroy))
                        .then(literal("keep").executes(setblock_keep))
                        .then(literal("replace").executes(setblock_replace)),
                ),
            ),
    );
}

/// Resolves block name from command context, prefixing `minecraft:` if needed.
fn resolve_block_name(
    ctx: &CommandContext<CommandSourceStack>,
) -> Result<String, crate::commands::CommandError> {
    let block = get_string(ctx, "block")?;
    Ok(if block.contains(':') {
        block.to_string()
    } else {
        format!("minecraft:{block}")
    })
}

/// Sends a success message for setblock.
fn send_setblock_success(ctx: &CommandContext<CommandSourceStack>, x: i32, y: i32, z: i32) {
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
}

/// Sends the standard setblock failure message.
fn send_setblock_failure(ctx: &CommandContext<CommandSourceStack>) {
    ctx.source
        .send_failure(&Component::translatable("commands.setblock.failed", vec![]));
}

/// `/setblock <pos> <block>` and `/setblock <pos> <block> replace`
fn setblock_replace(
    ctx: &CommandContext<CommandSourceStack>,
) -> Result<i32, crate::commands::CommandError> {
    let (x, y, z) = get_block_pos(ctx, "pos")?;
    let block_name = resolve_block_name(ctx)?;

    if !ctx.source.server.set_block(x, y, z, &block_name) {
        send_setblock_failure(ctx);
        return Ok(0);
    }

    send_setblock_success(ctx, x, y, z);
    Ok(1)
}

/// `/setblock <pos> <block> keep` — only sets if the target is air.
fn setblock_keep(
    ctx: &CommandContext<CommandSourceStack>,
) -> Result<i32, crate::commands::CommandError> {
    let (x, y, z) = get_block_pos(ctx, "pos")?;
    let block_name = resolve_block_name(ctx)?;

    if let Some(existing) = ctx.source.server.get_block(x, y, z) {
        if existing != "minecraft:air" {
            send_setblock_failure(ctx);
            return Ok(0);
        }
    }

    if !ctx.source.server.set_block(x, y, z, &block_name) {
        send_setblock_failure(ctx);
        return Ok(0);
    }

    send_setblock_success(ctx, x, y, z);
    Ok(1)
}

/// `/setblock <pos> <block> destroy` — destroys old block first (air), then places.
fn setblock_destroy(
    ctx: &CommandContext<CommandSourceStack>,
) -> Result<i32, crate::commands::CommandError> {
    let (x, y, z) = get_block_pos(ctx, "pos")?;
    let block_name = resolve_block_name(ctx)?;

    // Destroy the existing block first (set to air).
    // TODO: When item drops are implemented, drop items here.
    ctx.source.server.set_block(x, y, z, "minecraft:air");

    if block_name == "minecraft:air" {
        send_setblock_success(ctx, x, y, z);
        return Ok(1);
    }

    if !ctx.source.server.set_block(x, y, z, &block_name) {
        send_setblock_failure(ctx);
        return Ok(0);
    }

    send_setblock_success(ctx, x, y, z);
    Ok(1)
}
