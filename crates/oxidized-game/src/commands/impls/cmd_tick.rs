//! `/tick` command — control the server tick rate.
//!
//! Subcommands:
//! - `/tick query` — show current TPS and frozen state
//! - `/tick rate <rate>` — change tick rate
//! - `/tick freeze` — pause game ticks
//! - `/tick unfreeze` — resume game ticks
//! - `/tick step [count]` — advance N ticks while frozen
//! - `/tick sprint <ticks>` — run as fast as possible for N ticks

use crate::commands::argument_access::{get_float, get_integer};
use crate::commands::arguments::ArgumentType;
use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/tick` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("tick")
            .description("Controls the server tick rate")
            .requires(|s: &CommandSourceStack| s.has_permission(3))
            // /tick query
            .then(
                literal("query").executes(|ctx: &CommandContext<CommandSourceStack>| {
                    let rate = ctx.source.server.tick_rate();
                    let frozen = ctx.source.server.is_tick_frozen();
                    let status = if frozen { "frozen" } else { "running" };
                    ctx.source.send_success(
                        &Component::text(format!("Tick rate: {rate:.1} TPS ({status})")),
                        false,
                    );
                    Ok(rate as i32)
                }),
            )
            // /tick rate <rate>
            .then(
                literal("rate").then(
                    argument(
                        "rate",
                        ArgumentType::Float {
                            min: Some(1.0),
                            max: Some(10000.0),
                        },
                    )
                    .executes(|ctx: &CommandContext<CommandSourceStack>| {
                        let rate = get_float(ctx, "rate")?;
                        ctx.source.server.set_tick_rate(rate);
                        ctx.source.send_success(
                            &Component::text(format!("Set tick rate to {rate:.1}")),
                            true,
                        );
                        Ok(rate as i32)
                    }),
                ),
            )
            // /tick freeze
            .then(
                literal("freeze").executes(|ctx: &CommandContext<CommandSourceStack>| {
                    ctx.source.server.set_tick_frozen(true);
                    ctx.source
                        .send_success(&Component::text("Game is now frozen"), true);
                    Ok(1)
                }),
            )
            // /tick unfreeze
            .then(
                literal("unfreeze").executes(|ctx: &CommandContext<CommandSourceStack>| {
                    ctx.source.server.set_tick_frozen(false);
                    ctx.source
                        .send_success(&Component::text("Game is no longer frozen"), true);
                    Ok(1)
                }),
            )
            // /tick step [count]
            .then(
                literal("step")
                    .executes(|ctx: &CommandContext<CommandSourceStack>| {
                        if !ctx.source.server.is_tick_frozen() {
                            ctx.source.send_failure(&Component::text(
                                "Cannot step — server is not frozen",
                            ));
                            return Ok(0);
                        }
                        ctx.source.server.tick_step(1);
                        ctx.source
                            .send_success(&Component::text("Stepping 1 tick"), true);
                        Ok(1)
                    })
                    .then(
                        argument(
                            "count",
                            ArgumentType::Integer {
                                min: Some(1),
                                max: Some(256),
                            },
                        )
                        .executes(
                            |ctx: &CommandContext<CommandSourceStack>| {
                                if !ctx.source.server.is_tick_frozen() {
                                    ctx.source.send_failure(&Component::text(
                                        "Cannot step — server is not frozen",
                                    ));
                                    return Ok(0);
                                }
                                let count = get_integer(ctx, "count")?;
                                ctx.source.server.tick_step(count as u32);
                                ctx.source.send_success(
                                    &Component::text(format!("Stepping {count} tick(s)")),
                                    true,
                                );
                                Ok(count)
                            },
                        ),
                    ),
            )
            // /tick sprint <ticks>
            .then(
                literal("sprint").then(
                    argument(
                        "ticks",
                        ArgumentType::Integer {
                            min: Some(1),
                            max: Some(72_000),
                        },
                    )
                    .executes(|ctx: &CommandContext<CommandSourceStack>| {
                        let ticks = get_integer(ctx, "ticks")?;
                        ctx.source.server.tick_sprint(ticks as u64);
                        ctx.source.send_success(
                            &Component::text(format!("Sprinting {ticks} tick(s)")),
                            true,
                        );
                        Ok(ticks)
                    }),
                ),
            ),
    );
}
