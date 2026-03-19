//! `/weather` command — set the weather.
//!
//! TODO: Modifying weather requires wrapping `PrimaryLevelData` in a `RwLock`
//! and sending `ClientboundGameEventPacket` (rain/thunder changes) to clients.

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
            .description("Sets the weather")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            .then(
                literal("clear")
                    .executes(weather_fn("commands.weather.set.clear"))
                    .then(
                        argument("duration", ArgumentType::Time { min: 0 })
                            .executes(weather_with_duration_fn("commands.weather.set.clear")),
                    ),
            )
            .then(
                literal("rain")
                    .executes(weather_fn("commands.weather.set.rain"))
                    .then(
                        argument("duration", ArgumentType::Time { min: 0 })
                            .executes(weather_with_duration_fn("commands.weather.set.rain")),
                    ),
            )
            .then(
                literal("thunder")
                    .executes(weather_fn("commands.weather.set.thunder"))
                    .then(
                        argument("duration", ArgumentType::Time { min: 0 })
                            .executes(weather_with_duration_fn("commands.weather.set.thunder")),
                    ),
            ),
    );
}

fn weather_fn(
    key: &'static str,
) -> impl Fn(&CommandContext<CommandSourceStack>) -> Result<i32, CommandError> + Send + Sync + 'static
{
    move |ctx| {
        // TODO: Actually change the weather
        ctx.source
            .send_success(&Component::translatable(key, vec![]), true);
        Ok(1)
    }
}

fn weather_with_duration_fn(
    key: &'static str,
) -> impl Fn(&CommandContext<CommandSourceStack>) -> Result<i32, CommandError> + Send + Sync + 'static
{
    move |ctx| {
        let _duration = get_time(ctx, "duration")?;
        // TODO: Actually change the weather with duration
        ctx.source
            .send_success(&Component::translatable(key, vec![]), true);
        Ok(1)
    }
}
