//! Command parsing context, parsed arguments, and string reader.

use crate::commands::CommandError;
use crate::commands::arguments::{ArgumentType, StringKind};
use crate::commands::nodes::CommandFn;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;
use oxidized_protocol::types::game_type::GameType;
use std::collections::HashMap;

/// A parsed command ready for execution.
pub struct CommandContext<S> {
    /// The source that invoked the command.
    pub source: S,
    /// The full input string.
    pub input: String,
    /// Parsed arguments keyed by name.
    pub arguments: HashMap<String, ParsedArgument>,
    /// The command function to execute (if any).
    pub command: Option<CommandFn<S>>,
}

/// A single parsed argument with its position and value.
#[derive(Debug, Clone)]
pub struct ParsedArgument {
    /// Range in the input string.
    pub range: StringRange,
    /// The parsed value.
    pub result: ArgumentResult,
}

/// Typed result of parsing an argument.
#[derive(Debug, Clone, PartialEq)]
pub enum ArgumentResult {
    /// A boolean value.
    Bool(bool),
    /// A 32-bit integer.
    Integer(i32),
    /// A 64-bit integer.
    Long(i64),
    /// A 32-bit float.
    Float(f32),
    /// A 64-bit float.
    Double(f64),
    /// A string.
    String(String),
    /// An (x, y, z) integer position.
    BlockPos(i32, i32, i32),
    /// An (x, y, z) double-precision position.
    Vec3(f64, f64, f64),
    /// A game mode.
    Gamemode(GameType),
    /// A resource location string.
    ResourceLocation(String),
    /// A UUID.
    Uuid(uuid::Uuid),
    /// A time value in ticks.
    Time(i32),
}

/// A range within the input string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StringRange {
    /// Start index (inclusive).
    pub start: usize,
    /// End index (exclusive).
    pub end: usize,
}

impl StringRange {
    /// Creates a new range.
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Returns the length of the range.
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Returns `true` if the range is empty.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// A tab-completion suggestion.
#[derive(Debug, Clone)]
pub struct Suggestion {
    /// Range in the input string to replace.
    pub range: StringRange,
    /// The suggested text.
    pub text: String,
    /// An optional tooltip shown to the player.
    pub tooltip: Option<Component>,
}

/// Results from parsing input against the command graph.
pub struct ParseResults<S> {
    /// The built context (with parsed args and command fn).
    pub context: CommandContext<S>,
    /// How far we parsed successfully.
    pub cursor: usize,
}

/// A simple cursor-based string reader for argument parsing.
pub struct StringReader<'a> {
    input: &'a str,
    cursor: usize,
}

impl<'a> StringReader<'a> {
    /// Creates a new reader at the given start position.
    pub fn new(input: &'a str, cursor: usize) -> Self {
        Self { input, cursor }
    }

    /// Returns the current cursor position.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Returns the remaining unparsed input.
    pub fn remaining(&self) -> &str {
        &self.input[self.cursor..]
    }

    /// Returns `true` if there is more input.
    pub fn can_read(&self) -> bool {
        self.cursor < self.input.len()
    }

    /// Peeks at the next character without consuming.
    pub fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    /// Skips whitespace.
    pub fn skip_whitespace(&mut self) {
        while self.can_read() && self.peek() == Some(' ') {
            self.cursor += 1;
        }
    }

    /// Reads a single word (no spaces).
    pub fn read_word(&mut self) -> &str {
        let start = self.cursor;
        while self.can_read() && self.peek() != Some(' ') {
            self.cursor += 1;
        }
        &self.input[start..self.cursor]
    }

    /// Reads an integer.
    pub fn read_integer(&mut self) -> Result<i32, CommandError> {
        let word = self.read_word();
        word.parse::<i32>()
            .map_err(|_| CommandError::Parse(format!("Expected integer, got '{word}'")))
    }

    /// Reads a long.
    pub fn read_long(&mut self) -> Result<i64, CommandError> {
        let word = self.read_word();
        word.parse::<i64>()
            .map_err(|_| CommandError::Parse(format!("Expected long, got '{word}'")))
    }

    /// Reads a float.
    pub fn read_float(&mut self) -> Result<f32, CommandError> {
        let word = self.read_word();
        word.parse::<f32>()
            .map_err(|_| CommandError::Parse(format!("Expected float, got '{word}'")))
    }

    /// Reads a double.
    pub fn read_double(&mut self) -> Result<f64, CommandError> {
        let word = self.read_word();
        word.parse::<f64>()
            .map_err(|_| CommandError::Parse(format!("Expected double, got '{word}'")))
    }

    /// Reads a boolean.
    pub fn read_bool(&mut self) -> Result<bool, CommandError> {
        let word = self.read_word();
        match word {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(CommandError::Parse(format!(
                "Expected 'true' or 'false', got '{word}'"
            ))),
        }
    }

    /// Reads a string according to the given kind.
    pub fn read_string(&mut self, kind: StringKind) -> String {
        match kind {
            StringKind::SingleWord => self.read_word().to_string(),
            StringKind::GreedyPhrase => {
                let rest = self.remaining().to_string();
                self.cursor = self.input.len();
                rest
            },
            StringKind::QuotablePhrase => {
                if self.peek() == Some('"') {
                    self.cursor += 1; // skip opening quote
                    let mut result = String::new();
                    while self.can_read() {
                        let ch = self.input.as_bytes()[self.cursor] as char;
                        if ch == '\\' && self.cursor + 1 < self.input.len() {
                            let next = self.input.as_bytes()[self.cursor + 1] as char;
                            if next == '"' || next == '\\' {
                                result.push(next);
                                self.cursor += 2;
                                continue;
                            }
                        }
                        if ch == '"' {
                            break;
                        }
                        result.push(ch);
                        self.cursor += 1;
                    }
                    if self.can_read() {
                        self.cursor += 1; // skip closing quote
                    }
                    result
                } else {
                    self.read_word().to_string()
                }
            },
        }
    }
}

/// Parses an argument value from a `StringReader` given the argument type.
pub fn parse_argument(
    reader: &mut StringReader<'_>,
    arg_type: &ArgumentType,
) -> Result<ArgumentResult, CommandError> {
    match arg_type {
        ArgumentType::Bool => Ok(ArgumentResult::Bool(reader.read_bool()?)),
        ArgumentType::Integer { min, max } => {
            let v = reader.read_integer()?;
            if let Some(lo) = min {
                if v < *lo {
                    return Err(CommandError::Parse(format!(
                        "Integer must not be less than {lo}, found {v}"
                    )));
                }
            }
            if let Some(hi) = max {
                if v > *hi {
                    return Err(CommandError::Parse(format!(
                        "Integer must not be more than {hi}, found {v}"
                    )));
                }
            }
            Ok(ArgumentResult::Integer(v))
        },
        ArgumentType::Long { min, max } => {
            let v = reader.read_long()?;
            if let Some(lo) = min {
                if v < *lo {
                    return Err(CommandError::Parse(format!(
                        "Long must not be less than {lo}, found {v}"
                    )));
                }
            }
            if let Some(hi) = max {
                if v > *hi {
                    return Err(CommandError::Parse(format!(
                        "Long must not be more than {hi}, found {v}"
                    )));
                }
            }
            Ok(ArgumentResult::Long(v))
        },
        ArgumentType::Float { min, max } => {
            let v = reader.read_float()?;
            if let Some(lo) = min {
                if v < *lo {
                    return Err(CommandError::Parse(format!(
                        "Float must not be less than {lo}, found {v}"
                    )));
                }
            }
            if let Some(hi) = max {
                if v > *hi {
                    return Err(CommandError::Parse(format!(
                        "Float must not be more than {hi}, found {v}"
                    )));
                }
            }
            Ok(ArgumentResult::Float(v))
        },
        ArgumentType::Double { min, max } => {
            let v = reader.read_double()?;
            if let Some(lo) = min {
                if v < *lo {
                    return Err(CommandError::Parse(format!(
                        "Double must not be less than {lo}, found {v}"
                    )));
                }
            }
            if let Some(hi) = max {
                if v > *hi {
                    return Err(CommandError::Parse(format!(
                        "Double must not be more than {hi}, found {v}"
                    )));
                }
            }
            Ok(ArgumentResult::Double(v))
        },
        ArgumentType::String(kind) => Ok(ArgumentResult::String(reader.read_string(*kind))),
        ArgumentType::Entity { .. } | ArgumentType::GameProfile | ArgumentType::Message => {
            // Entity selectors / game profiles / messages are complex —
            // read a word/greedy phrase as a raw string for now.
            // Full entity selector parsing comes in a later phase.
            Ok(ArgumentResult::String(reader.read_word().to_string()))
        },
        ArgumentType::BlockPos => {
            let x = reader.read_integer()?;
            reader.skip_whitespace();
            let y = reader.read_integer()?;
            reader.skip_whitespace();
            let z = reader.read_integer()?;
            Ok(ArgumentResult::BlockPos(x, y, z))
        },
        ArgumentType::Vec3 => {
            let x = reader.read_double()?;
            reader.skip_whitespace();
            let y = reader.read_double()?;
            reader.skip_whitespace();
            let z = reader.read_double()?;
            Ok(ArgumentResult::Vec3(x, y, z))
        },
        ArgumentType::Vec2 => {
            let x = reader.read_double()?;
            reader.skip_whitespace();
            let z = reader.read_double()?;
            Ok(ArgumentResult::Vec3(x, 0.0, z))
        },
        ArgumentType::Gamemode => {
            let word = reader.read_word();
            match GameType::by_name(word) {
                Some(gm) => Ok(ArgumentResult::Gamemode(gm)),
                None => Err(CommandError::Parse(format!("Unknown game mode: '{word}'"))),
            }
        },
        ArgumentType::ResourceLocation
        | ArgumentType::Dimension
        | ArgumentType::Function
        | ArgumentType::ResourceOrTag { .. }
        | ArgumentType::ResourceOrTagKey { .. }
        | ArgumentType::Resource { .. }
        | ArgumentType::ResourceKey { .. }
        | ArgumentType::ResourceSelector { .. }
        | ArgumentType::LootTable
        | ArgumentType::LootPredicate
        | ArgumentType::LootModifier
        | ArgumentType::Dialog => Ok(ArgumentResult::String(reader.read_word().to_string())),
        ArgumentType::Time { min } => {
            let word = reader.read_word();
            let (num_str, multiplier) = if let Some(stripped) = word.strip_suffix('d') {
                (stripped, 24000)
            } else if let Some(stripped) = word.strip_suffix('s') {
                (stripped, 20)
            } else if let Some(stripped) = word.strip_suffix('t') {
                (stripped, 1)
            } else {
                (word, 1) // default: ticks
            };
            let v: i32 = num_str
                .parse()
                .map_err(|_| CommandError::Parse(format!("Expected time value, got '{word}'")))?;
            let ticks = v
                .checked_mul(multiplier)
                .ok_or_else(|| CommandError::Parse(format!("Time value too large: '{word}'")))?;
            if ticks < *min {
                return Err(CommandError::Parse(format!(
                    "Time must not be less than {min} ticks, found {ticks}"
                )));
            }
            Ok(ArgumentResult::Time(ticks))
        },
        ArgumentType::Uuid => {
            let word = reader.read_word();
            let uuid = uuid::Uuid::parse_str(word)
                .map_err(|_| CommandError::Parse(format!("Invalid UUID: '{word}'")))?;
            Ok(ArgumentResult::Uuid(uuid))
        },
        // All remaining types: parse as a single word string for now.
        _ => Ok(ArgumentResult::String(reader.read_word().to_string())),
    }
}

// ── Typed argument getters ──────────────────────────────────────────

/// Gets an integer argument by name.
pub fn get_integer<S>(ctx: &CommandContext<S>, name: &str) -> Result<i32, CommandError> {
    match ctx
        .arguments
        .get(name)
        .ok_or_else(|| CommandError::Parse(format!("No argument named '{name}'")))?
        .result
    {
        ArgumentResult::Integer(v) => Ok(v),
        _ => Err(CommandError::Parse(format!(
            "Argument '{name}' is not an integer"
        ))),
    }
}

/// Gets a long argument by name.
pub fn get_long<S>(ctx: &CommandContext<S>, name: &str) -> Result<i64, CommandError> {
    match ctx
        .arguments
        .get(name)
        .ok_or_else(|| CommandError::Parse(format!("No argument named '{name}'")))?
        .result
    {
        ArgumentResult::Long(v) => Ok(v),
        _ => Err(CommandError::Parse(format!(
            "Argument '{name}' is not a long"
        ))),
    }
}

/// Gets a float argument by name.
pub fn get_float<S>(ctx: &CommandContext<S>, name: &str) -> Result<f32, CommandError> {
    match ctx
        .arguments
        .get(name)
        .ok_or_else(|| CommandError::Parse(format!("No argument named '{name}'")))?
        .result
    {
        ArgumentResult::Float(v) => Ok(v),
        _ => Err(CommandError::Parse(format!(
            "Argument '{name}' is not a float"
        ))),
    }
}

/// Gets a double argument by name.
pub fn get_double<S>(ctx: &CommandContext<S>, name: &str) -> Result<f64, CommandError> {
    match ctx
        .arguments
        .get(name)
        .ok_or_else(|| CommandError::Parse(format!("No argument named '{name}'")))?
        .result
    {
        ArgumentResult::Double(v) => Ok(v),
        _ => Err(CommandError::Parse(format!(
            "Argument '{name}' is not a double"
        ))),
    }
}

/// Gets a boolean argument by name.
pub fn get_bool<S>(ctx: &CommandContext<S>, name: &str) -> Result<bool, CommandError> {
    match ctx
        .arguments
        .get(name)
        .ok_or_else(|| CommandError::Parse(format!("No argument named '{name}'")))?
        .result
    {
        ArgumentResult::Bool(v) => Ok(v),
        _ => Err(CommandError::Parse(format!(
            "Argument '{name}' is not a boolean"
        ))),
    }
}

/// Gets a string argument by name.
pub fn get_string<'a, S>(ctx: &'a CommandContext<S>, name: &str) -> Result<&'a str, CommandError> {
    match &ctx
        .arguments
        .get(name)
        .ok_or_else(|| CommandError::Parse(format!("No argument named '{name}'")))?
        .result
    {
        ArgumentResult::String(v) => Ok(v.as_str()),
        _ => Err(CommandError::Parse(format!(
            "Argument '{name}' is not a string"
        ))),
    }
}

/// Gets a gamemode argument by name.
pub fn get_gamemode<S>(ctx: &CommandContext<S>, name: &str) -> Result<GameType, CommandError> {
    match ctx
        .arguments
        .get(name)
        .ok_or_else(|| CommandError::Parse(format!("No argument named '{name}'")))?
        .result
    {
        ArgumentResult::Gamemode(gm) => Ok(gm),
        _ => Err(CommandError::Parse(format!(
            "Argument '{name}' is not a game mode"
        ))),
    }
}

/// Gets a time argument by name (in ticks).
pub fn get_time<S>(ctx: &CommandContext<S>, name: &str) -> Result<i32, CommandError> {
    match ctx
        .arguments
        .get(name)
        .ok_or_else(|| CommandError::Parse(format!("No argument named '{name}'")))?
        .result
    {
        ArgumentResult::Time(v) => Ok(v),
        // Also accept raw integers as ticks
        ArgumentResult::Integer(v) => Ok(v),
        _ => Err(CommandError::Parse(format!(
            "Argument '{name}' is not a time value"
        ))),
    }
}

/// Gets a block position argument by name.
pub fn get_block_pos<S>(
    ctx: &CommandContext<S>,
    name: &str,
) -> Result<(i32, i32, i32), CommandError> {
    match ctx
        .arguments
        .get(name)
        .ok_or_else(|| CommandError::Parse(format!("No argument named '{name}'")))?
        .result
    {
        ArgumentResult::BlockPos(x, y, z) => Ok((x, y, z)),
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

/// Gets a vec3 argument by name.
pub fn get_vec3<S>(ctx: &CommandContext<S>, name: &str) -> Result<(f64, f64, f64), CommandError> {
    match ctx
        .arguments
        .get(name)
        .ok_or_else(|| CommandError::Parse(format!("No argument named '{name}'")))?
        .result
    {
        ArgumentResult::Vec3(x, y, z) => Ok((x, y, z)),
        _ => Err(CommandError::Parse(format!(
            "Argument '{name}' is not a vec3"
        ))),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn quotable_phrase_handles_escape_sequences() {
        let input = r#""hello \"world\"""#;
        let mut reader = StringReader::new(input, 0);
        let result = reader.read_string(StringKind::QuotablePhrase);
        assert_eq!(result, r#"hello "world""#);
    }

    #[test]
    fn quotable_phrase_handles_escaped_backslash() {
        let input = r#""path\\to\\file""#;
        let mut reader = StringReader::new(input, 0);
        let result = reader.read_string(StringKind::QuotablePhrase);
        assert_eq!(result, r"path\to\file");
    }

    #[test]
    fn quotable_phrase_no_escapes() {
        let input = r#""simple text""#;
        let mut reader = StringReader::new(input, 0);
        let result = reader.read_string(StringKind::QuotablePhrase);
        assert_eq!(result, "simple text");
    }

    #[test]
    fn time_overflow_returns_error() {
        let mut reader = StringReader::new("89479d", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Time { min: 0 });
        assert!(
            result.is_err(),
            "should reject time values that overflow i32"
        );
    }

    #[test]
    fn time_valid_days() {
        let mut reader = StringReader::new("1d", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Time { min: 0 });
        assert_eq!(result.unwrap(), ArgumentResult::Time(24000));
    }

    #[test]
    fn time_valid_seconds() {
        let mut reader = StringReader::new("5s", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Time { min: 0 });
        assert_eq!(result.unwrap(), ArgumentResult::Time(100));
    }
}
