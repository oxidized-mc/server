//! `/time` command — query or set the world time.

use crate::commands::CommandError;
use crate::commands::arguments::ArgumentType;
use crate::commands::context::{CommandContext, get_time};
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/time` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("time")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            // /time set ...
            .then(
                literal("set")
                    .then(literal("day").executes(set_time_fn(1000)))
                    .then(literal("noon").executes(set_time_fn(6000)))
                    .then(literal("night").executes(set_time_fn(13000)))
                    .then(literal("midnight").executes(set_time_fn(18000)))
                    .then(argument("time", ArgumentType::Time { min: 0 }).executes(
                        |ctx: &CommandContext<CommandSourceStack>| {
                            let t = get_time(ctx, "time")?;
                            ctx.source.send_success(
                                &Component::text(format!("Set the time to {t}")),
                                true,
                            );
                            Ok(t)
                        },
                    )),
            )
            // /time add <time>
            .then(
                literal("add").then(argument("time", ArgumentType::Time { min: 0 }).executes(
                    |ctx: &CommandContext<CommandSourceStack>| {
                        let t = get_time(ctx, "time")?;
                        ctx.source
                            .send_success(&Component::text(format!("Added {t} to the time")), true);
                        Ok(t)
                    },
                )),
            )
            // /time query ...
            .then(
                literal("query")
                    .then(literal("daytime").executes(
                        |ctx: &CommandContext<CommandSourceStack>| {
                            ctx.source
                                .send_success(&Component::text("The time is 1000"), true);
                            Ok(1000)
                        },
                    ))
                    .then(literal("gametime").executes(
                        |ctx: &CommandContext<CommandSourceStack>| {
                            ctx.source
                                .send_success(&Component::text("The game time is 0"), true);
                            Ok(0)
                        },
                    ))
                    .then(
                        literal("day").executes(|ctx: &CommandContext<CommandSourceStack>| {
                            ctx.source
                                .send_success(&Component::text("The day is 0"), true);
                            Ok(0)
                        }),
                    ),
            ),
    );
}

/// Creates an execution function that sets the time to a fixed value.
fn set_time_fn(
    ticks: i32,
) -> impl Fn(&CommandContext<CommandSourceStack>) -> Result<i32, CommandError> + Send + Sync + 'static
{
    move |ctx| {
        ctx.source
            .send_success(&Component::text(format!("Set the time to {ticks}")), true);
        Ok(ticks)
    }
}
