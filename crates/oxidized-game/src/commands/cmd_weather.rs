//! `/weather` command — set the weather.

use crate::commands::CommandError;
use crate::commands::arguments::ArgumentType;
use crate::commands::context::{CommandContext, get_time};
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/weather` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("weather")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            .then(
                literal("clear").executes(weather_fn("clear")).then(
                    argument("duration", ArgumentType::Time { min: 0 })
                        .executes(weather_with_duration_fn("clear")),
                ),
            )
            .then(
                literal("rain").executes(weather_fn("rain")).then(
                    argument("duration", ArgumentType::Time { min: 0 })
                        .executes(weather_with_duration_fn("rain")),
                ),
            )
            .then(
                literal("thunder").executes(weather_fn("thunder")).then(
                    argument("duration", ArgumentType::Time { min: 0 })
                        .executes(weather_with_duration_fn("thunder")),
                ),
            ),
    );
}

fn weather_fn(
    kind: &'static str,
) -> impl Fn(&CommandContext<CommandSourceStack>) -> Result<i32, CommandError> + Send + Sync + 'static
{
    move |ctx| {
        ctx.source
            .send_success(&Component::text(format!("Set the weather to {kind}")), true);
        Ok(1)
    }
}

fn weather_with_duration_fn(
    kind: &'static str,
) -> impl Fn(&CommandContext<CommandSourceStack>) -> Result<i32, CommandError> + Send + Sync + 'static
{
    move |ctx| {
        let duration = get_time(ctx, "duration")?;
        ctx.source.send_success(
            &Component::text(format!("Set the weather to {kind} for {duration} ticks")),
            true,
        );
        Ok(duration)
    }
}
