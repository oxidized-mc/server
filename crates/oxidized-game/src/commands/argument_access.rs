//! Typed argument getters for extracting parsed values from a command context.

use crate::commands::CommandError;
use crate::commands::context::{ArgumentResult, CommandContext};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::types::game_type::GameType;

/// Looks up a parsed argument by name.
fn get_arg_result<'a, S>(
    ctx: &'a CommandContext<S>,
    name: &str,
) -> Result<&'a ArgumentResult, CommandError> {
    ctx.arguments
        .get(name)
        .map(|a| &a.result)
        .ok_or_else(|| CommandError::Parse(format!("No argument named '{name}'")))
}

/// Extracts a typed value from a named argument using an extractor closure.
fn get_typed<S, T>(
    ctx: &CommandContext<S>,
    name: &str,
    type_name: &str,
    extract: impl FnOnce(&ArgumentResult) -> Option<T>,
) -> Result<T, CommandError> {
    extract(get_arg_result(ctx, name)?)
        .ok_or_else(|| CommandError::Parse(format!("Argument '{name}' is not {type_name}")))
}

/// Gets an integer argument by name.
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if no argument named `name` exists or
/// the argument is not an integer.
pub fn get_integer<S>(ctx: &CommandContext<S>, name: &str) -> Result<i32, CommandError> {
    get_typed(ctx, name, "an integer", |r| match r {
        ArgumentResult::Integer(v) => Some(*v),
        _ => None,
    })
}

/// Gets a long argument by name.
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if no argument named `name` exists or
/// the argument is not a long.
pub fn get_long<S>(ctx: &CommandContext<S>, name: &str) -> Result<i64, CommandError> {
    get_typed(ctx, name, "a long", |r| match r {
        ArgumentResult::Long(v) => Some(*v),
        _ => None,
    })
}

/// Gets a float argument by name.
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if no argument named `name` exists or
/// the argument is not a float.
pub fn get_float<S>(ctx: &CommandContext<S>, name: &str) -> Result<f32, CommandError> {
    get_typed(ctx, name, "a float", |r| match r {
        ArgumentResult::Float(v) => Some(*v),
        _ => None,
    })
}

/// Gets a double argument by name.
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if no argument named `name` exists or
/// the argument is not a double.
pub fn get_double<S>(ctx: &CommandContext<S>, name: &str) -> Result<f64, CommandError> {
    get_typed(ctx, name, "a double", |r| match r {
        ArgumentResult::Double(v) => Some(*v),
        _ => None,
    })
}

/// Gets a boolean argument by name.
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if no argument named `name` exists or
/// the argument is not a boolean.
pub fn get_bool<S>(ctx: &CommandContext<S>, name: &str) -> Result<bool, CommandError> {
    get_typed(ctx, name, "a boolean", |r| match r {
        ArgumentResult::Bool(v) => Some(*v),
        _ => None,
    })
}

/// Gets a string argument by name.
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if no argument named `name` exists or
/// the argument is not a string.
pub fn get_string<'a, S>(ctx: &'a CommandContext<S>, name: &str) -> Result<&'a str, CommandError> {
    match get_arg_result(ctx, name)? {
        ArgumentResult::String(v) => Ok(v.as_str()),
        _ => Err(CommandError::Parse(format!(
            "Argument '{name}' is not a string"
        ))),
    }
}

/// Gets a gamemode argument by name.
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if no argument named `name` exists or
/// the argument is not a game mode.
pub fn get_gamemode<S>(ctx: &CommandContext<S>, name: &str) -> Result<GameType, CommandError> {
    get_typed(ctx, name, "a game mode", |r| match r {
        ArgumentResult::Gamemode(gm) => Some(*gm),
        _ => None,
    })
}

/// Gets a time argument by name (in ticks).
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if no argument named `name` exists or
/// the argument is not a time or integer value.
pub fn get_time<S>(ctx: &CommandContext<S>, name: &str) -> Result<i32, CommandError> {
    match get_arg_result(ctx, name)? {
        // Accept both Time and raw Integer as ticks
        ArgumentResult::Time(v) | ArgumentResult::Integer(v) => Ok(*v),
        _ => Err(CommandError::Parse(format!(
            "Argument '{name}' is not a time value"
        ))),
    }
}

/// Gets a block position argument by name. Resolves relative/local coordinates
/// using the source position when the context source is [`CommandSourceStack`].
pub fn get_block_pos(
    ctx: &CommandContext<CommandSourceStack>,
    name: &str,
) -> Result<(i32, i32, i32), CommandError> {
    match get_arg_result(ctx, name)? {
        ArgumentResult::BlockPos(x, y, z) => Ok((*x, *y, *z)),
        ArgumentResult::Coordinates(coords) => {
            Ok(coords.resolve_block_pos(ctx.source.position, ctx.source.rotation))
        },
        _ => Err(CommandError::Parse(format!(
            "Argument '{name}' is not a block position"
        ))),
    }
}

/// Gets resolved entity targets from a string argument, handling selectors.
pub fn get_entities(
    ctx: &CommandContext<CommandSourceStack>,
    name: &str,
) -> Result<Vec<crate::commands::selector::SelectorTarget>, CommandError> {
    let input = get_string(ctx, name)?;
    crate::commands::selector::resolve_entities(input, &ctx.source)
}

/// Gets a single entity target from a string argument.
pub fn get_entity(
    ctx: &CommandContext<CommandSourceStack>,
    name: &str,
) -> Result<crate::commands::selector::SelectorTarget, CommandError> {
    let targets = get_entities(ctx, name)?;
    if targets.len() != 1 {
        return Err(CommandError::Parse(format!(
            "Expected exactly one entity, got {}",
            targets.len()
        )));
    }
    let mut iter = targets.into_iter();
    iter.next()
        .ok_or_else(|| CommandError::Parse("Expected exactly one entity, got 0".to_string()))
}

/// Gets a player name from a string argument (for `GameProfile` args).
pub fn get_game_profile(
    ctx: &CommandContext<CommandSourceStack>,
    name: &str,
) -> Result<crate::commands::selector::SelectorTarget, CommandError> {
    get_entity(ctx, name)
}

/// Gets a vec3 argument by name. Resolves relative/local coordinates
/// using the source position when the context source is [`CommandSourceStack`].
pub fn get_vec3(
    ctx: &CommandContext<CommandSourceStack>,
    name: &str,
) -> Result<(f64, f64, f64), CommandError> {
    match get_arg_result(ctx, name)? {
        ArgumentResult::Vec3(x, y, z) => Ok((*x, *y, *z)),
        ArgumentResult::Coordinates(coords) => {
            Ok(coords.resolve(ctx.source.position, ctx.source.rotation))
        },
        _ => Err(CommandError::Parse(format!(
            "Argument '{name}' is not a vec3"
        ))),
    }
}

/// Resolves an entity selector argument and invokes `action` for each target.
///
/// Returns the number of targets that were successfully processed. The loop
/// exits early if `action` returns an error.
///
/// # Errors
///
/// Returns [`CommandError`] if the selector cannot be resolved or if `action`
/// returns an error for any target.
pub fn for_each_target<F>(
    ctx: &CommandContext<CommandSourceStack>,
    selector_name: &str,
    mut action: F,
) -> Result<i32, CommandError>
where
    F: FnMut(
        &CommandSourceStack,
        &crate::commands::selector::SelectorTarget,
    ) -> Result<(), CommandError>,
{
    let targets = get_entities(ctx, selector_name)?;
    let mut count = 0;
    for target in &targets {
        action(&ctx.source, target)?;
        count += 1;
    }
    Ok(count)
}
