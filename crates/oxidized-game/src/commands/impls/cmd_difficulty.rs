//! `/difficulty` command — query or set the world difficulty.
//!
//! Query reads the actual difficulty from `ServerHandle`. Set subcommands
//! send translatable feedback but cannot yet modify the difficulty.
//!
//! TODO: Modifying difficulty requires wrapping `PrimaryLevelData` in a
//! `RwLock` and sending `ClientboundChangeDifficultyPacket` to all clients.

use crate::commands::nodes::LiteralBuilderExt;
use crate::commands::source::CommandSourceStack;
use oxidized_chat::Component;
use oxidized_commands::CommandError;
use oxidized_commands::context::CommandContext;
use oxidized_commands::dispatcher::CommandDispatcher;
use oxidized_commands::nodes::literal;

/// Map difficulty ID to vanilla translation key.
fn difficulty_key(id: i32) -> &'static str {
    match id {
        0 => "options.difficulty.peaceful",
        1 => "options.difficulty.easy",
        2 => "options.difficulty.normal",
        3 => "options.difficulty.hard",
        _ => "options.difficulty.normal",
    }
}

/// Registers the `/difficulty` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("difficulty")
            .description("Sets the difficulty level")
            .requires_op()
            // /difficulty — query current difficulty
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                let diff = ctx.source.server.difficulty();
                ctx.source.send_translatable_success(
                    "commands.difficulty.query",
                    vec![Component::translatable(difficulty_key(diff), vec![])],
                    false,
                );
                Ok(diff)
            })
            .then(literal("peaceful").executes(difficulty_fn(0)))
            .then(literal("easy").executes(difficulty_fn(1)))
            .then(literal("normal").executes(difficulty_fn(2)))
            .then(literal("hard").executes(difficulty_fn(3))),
    );
}

fn difficulty_fn(
    level: i32,
) -> impl Fn(&CommandContext<CommandSourceStack>) -> Result<i32, CommandError> + Send + Sync + 'static
{
    move |ctx| {
        let current = ctx.source.server.difficulty();
        if current == level {
            ctx.source.send_translatable_failure(
                "commands.difficulty.failure",
                vec![Component::translatable(difficulty_key(level), vec![])],
            );
            return Ok(0);
        }
        // TODO: Actually change the difficulty — requires wrapping
        // `PrimaryLevelData` in a `RwLock` and broadcasting
        // `ClientboundChangeDifficultyPacket` to all clients.
        Err(CommandError::NotImplemented("difficulty set".into()))
    }
}
