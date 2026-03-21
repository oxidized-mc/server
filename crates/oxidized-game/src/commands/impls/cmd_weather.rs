//! `/weather` command — set the weather.
//!
//! Modifies weather state via `ServerHandle::set_weather`.

use crate::commands::CommandError;
use crate::commands::argument_access::get_time;
use crate::commands::arguments::ArgumentType;
use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/weather` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("weather")
            .description("Sets the weather")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            .then(
                literal("clear")
                    .executes(weather_fn("clear", "commands.weather.set.clear"))
                    .then(
                        argument("duration", ArgumentType::Time { min: 0 }).executes(
                            weather_with_duration_fn("clear", "commands.weather.set.clear"),
                        ),
                    ),
            )
            .then(
                literal("rain")
                    .executes(weather_fn("rain", "commands.weather.set.rain"))
                    .then(
                        argument("duration", ArgumentType::Time { min: 0 }).executes(
                            weather_with_duration_fn("rain", "commands.weather.set.rain"),
                        ),
                    ),
            )
            .then(
                literal("thunder")
                    .executes(weather_fn("thunder", "commands.weather.set.thunder"))
                    .then(
                        argument("duration", ArgumentType::Time { min: 0 }).executes(
                            weather_with_duration_fn("thunder", "commands.weather.set.thunder"),
                        ),
                    ),
            ),
    );
}

fn weather_fn(
    weather: &'static str,
    key: &'static str,
) -> impl Fn(&CommandContext<CommandSourceStack>) -> Result<i32, CommandError> + Send + Sync + 'static
{
    move |ctx| {
        ctx.source.server.set_weather(weather, None);
        ctx.source
            .send_success(&Component::translatable(key, vec![]), true);
        Ok(1)
    }
}

fn weather_with_duration_fn(
    weather: &'static str,
    key: &'static str,
) -> impl Fn(&CommandContext<CommandSourceStack>) -> Result<i32, CommandError> + Send + Sync + 'static
{
    move |ctx| {
        let duration = get_time(ctx, "duration")?;
        ctx.source.server.set_weather(weather, Some(duration));
        ctx.source
            .send_success(&Component::translatable(key, vec![]), true);
        Ok(1)
    }
}
