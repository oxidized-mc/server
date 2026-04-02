//! `/time` command — query or set the world time.
//!
//! Query subcommands use real data from `ServerHandle`. Set/add subcommands
//! actually modify the world time via `ServerHandle::set_day_time` /
//! `add_day_time`.

use crate::commands::CommandError;
use crate::commands::argument_access::get_time;
use crate::commands::arguments::ArgumentType;
use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::nodes::LiteralBuilderExt;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;
use oxidized_protocol::constants::{
    DAY_START_TICKS, MIDNIGHT_TICKS, NIGHT_START_TICKS, NOON_TICKS, TICKS_PER_GAME_DAY,
};

/// Registers the `/time` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("time")
            .description("Changes or queries the world's game time")
            .requires_op()
            // /time set ...
            .then(
                literal("set")
                    .then(literal("day").executes(set_time_fn(DAY_START_TICKS as i32)))
                    .then(literal("noon").executes(set_time_fn(NOON_TICKS as i32)))
                    .then(literal("night").executes(set_time_fn(NIGHT_START_TICKS as i32)))
                    .then(literal("midnight").executes(set_time_fn(MIDNIGHT_TICKS as i32)))
                    .then(argument("time", ArgumentType::Time { min: 0 }).executes(
                        |ctx: &CommandContext<CommandSourceStack>| {
                            let t = get_time(ctx, "time")?;
                            ctx.source.server.set_day_time(i64::from(t));
                            ctx.source.send_translatable_success(
                                "commands.time.set",
                                vec![Component::text(t.to_string())],
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
                        ctx.source.server.add_day_time(i64::from(t));
                        ctx.source.send_translatable_success(
                            "commands.time.set",
                            vec![Component::text(t.to_string())],
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
                            let display = (day_time % TICKS_PER_GAME_DAY as i64) as i32;
                            ctx.source.send_translatable_success(
                                "commands.time.query",
                                vec![Component::text(display.to_string())],
                                false,
                            );
                            Ok(display)
                        },
                    ))
                    .then(literal("gametime").executes(
                        |ctx: &CommandContext<CommandSourceStack>| {
                            let game_time = ctx.source.server.game_time();
                            let display = (game_time % i64::from(i32::MAX)) as i32;
                            ctx.source.send_translatable_success(
                                "commands.time.query",
                                vec![Component::text(display.to_string())],
                                false,
                            );
                            Ok(display)
                        },
                    ))
                    .then(
                        literal("day").executes(|ctx: &CommandContext<CommandSourceStack>| {
                            let game_time = ctx.source.server.game_time();
                            let day = (game_time / TICKS_PER_GAME_DAY as i64) as i32;
                            ctx.source.send_translatable_success(
                                "commands.time.query",
                                vec![Component::text(day.to_string())],
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
        ctx.source.server.set_day_time(i64::from(ticks));
        ctx.source.send_translatable_success(
            "commands.time.set",
            vec![Component::text(ticks.to_string())],
            true,
        );
        Ok(ticks)
    }
}
