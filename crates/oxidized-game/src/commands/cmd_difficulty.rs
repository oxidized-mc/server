//! `/difficulty` command — set the world difficulty.

use crate::commands::CommandError;
use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/difficulty` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("difficulty")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            // /difficulty — query current difficulty
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                ctx.source.send_success(
                    &Component::translatable("commands.difficulty.query", vec![]),
                    false,
                );
                Ok(0)
            })
            .then(literal("peaceful").executes(difficulty_fn("peaceful")))
            .then(literal("easy").executes(difficulty_fn("easy")))
            .then(literal("normal").executes(difficulty_fn("normal")))
            .then(literal("hard").executes(difficulty_fn("hard"))),
    );
}

fn difficulty_fn(
    name: &'static str,
) -> impl Fn(&CommandContext<CommandSourceStack>) -> Result<i32, CommandError> + Send + Sync + 'static
{
    move |ctx| {
        ctx.source.send_success(
            &Component::text(format!("The difficulty has been set to {name}")),
            true,
        );
        Ok(1)
    }
}
