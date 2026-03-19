//! `/time` command — query or set the world time.
//!
//! Query subcommands use real data from `ServerHandle`. Set/add subcommands
//! send translatable feedback but cannot yet modify the world time.
//!
//! TODO: Modifying time requires wrapping `PrimaryLevelData` in a `RwLock`
//! and broadcasting time update packets to all connected clients.

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
            .description("Changes or queries the world's game time")
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
                            // TODO: Actually set the world time
                            ctx.source.send_success(
                                &Component::translatable(
                                    "commands.time.set",
                                    vec![Component::text(t.to_string())],
                                ),
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
                        // TODO: Actually add to the world time
                        ctx.source.send_success(
                            &Component::translatable(
                                "commands.time.set",
                                vec![Component::text(t.to_string())],
                            ),
                            true,
                        );
                        Ok(t)
                    },
                )),
            )
            // /time query ...
            .then(
                literal("query")
                    .then(literal("daytime").executes(
                        |ctx: &CommandContext<CommandSourceStack>| {
                            let day_time = ctx.source.server.day_time();
                            let display = (day_time % 24000) as i32;
                            ctx.source.send_success(
                                &Component::translatable(
                                    "commands.time.query",
                                    vec![Component::text(display.to_string())],
                                ),
                                false,
                            );
                            Ok(display)
                        },
                    ))
                    .then(literal("gametime").executes(
                        |ctx: &CommandContext<CommandSourceStack>| {
                            let game_time = ctx.source.server.game_time();
                            let display = (game_time % i64::from(i32::MAX)) as i32;
                            ctx.source.send_success(
                                &Component::translatable(
                                    "commands.time.query",
                                    vec![Component::text(display.to_string())],
                                ),
                                false,
                            );
                            Ok(display)
                        },
                    ))
                    .then(
                        literal("day").executes(|ctx: &CommandContext<CommandSourceStack>| {
                            let game_time = ctx.source.server.game_time();
                            let day = (game_time / 24000) as i32;
                            ctx.source.send_success(
                                &Component::translatable(
                                    "commands.time.query",
                                    vec![Component::text(day.to_string())],
                                ),
                                false,
                            );
                            Ok(day)
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
        // TODO: Actually set the world time
        ctx.source.send_success(
            &Component::translatable(
                "commands.time.set",
                vec![Component::text(ticks.to_string())],
            ),
            true,
        );
        Ok(ticks)
    }
}
