//! Command parsing context, parsed arguments, and string reader.

use crate::commands::CommandError;
use crate::commands::arguments::{ArgumentType, StringKind};
use crate::commands::nodes::CommandFn;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;
use oxidized_protocol::types::game_type::GameType;
use std::collections::HashMap;
use std::fmt::Display;

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

    /// Reads and parses a numeric value from the next word.
    fn read_numeric<T: std::str::FromStr>(&mut self, type_name: &str) -> Result<T, CommandError> {
        let word = self.read_word();
        word.parse::<T>()
            .map_err(|_| CommandError::Parse(format!("Expected {type_name}, got '{word}'")))
    }

    /// Reads an integer.
    pub fn read_integer(&mut self) -> Result<i32, CommandError> {
        self.read_numeric("integer")
    }

    /// Reads a long.
    pub fn read_long(&mut self) -> Result<i64, CommandError> {
        self.read_numeric("long")
    }

    /// Reads a float.
    pub fn read_float(&mut self) -> Result<f32, CommandError> {
        self.read_numeric("float")
    }

    /// Reads a double.
    pub fn read_double(&mut self) -> Result<f64, CommandError> {
        self.read_numeric("double")
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

// ── Range validation ────────────────────────────────────────────────

/// Validates that a value is within optional min/max bounds.
fn validate_range<T: PartialOrd + Display>(
    value: T,
    min: Option<&T>,
    max: Option<&T>,
    type_name: &str,
) -> Result<T, CommandError> {
    if let Some(lo) = min {
        if value < *lo {
            return Err(CommandError::Parse(format!(
                "{type_name} must not be less than {lo}, found {value}"
            )));
        }
    }
    if let Some(hi) = max {
        if value > *hi {
            return Err(CommandError::Parse(format!(
                "{type_name} must not be more than {hi}, found {value}"
            )));
        }
    }
    Ok(value)
}

// ── Coordinate parsing helpers ──────────────────────────────────────

/// Parses three whitespace-separated integers (x y z).
fn parse_int3_coordinates(reader: &mut StringReader<'_>) -> Result<(i32, i32, i32), CommandError> {
    let x = reader.read_integer()?;
    reader.skip_whitespace();
    let y = reader.read_integer()?;
    reader.skip_whitespace();
    let z = reader.read_integer()?;
    Ok((x, y, z))
}

/// Parses three whitespace-separated doubles (x y z).
fn parse_double3_coordinates(
    reader: &mut StringReader<'_>,
) -> Result<(f64, f64, f64), CommandError> {
    let x = reader.read_double()?;
    reader.skip_whitespace();
    let y = reader.read_double()?;
    reader.skip_whitespace();
    let z = reader.read_double()?;
    Ok((x, y, z))
}

/// Parses two whitespace-separated doubles (x z).
fn parse_double2_coordinates(reader: &mut StringReader<'_>) -> Result<(f64, f64), CommandError> {
    let x = reader.read_double()?;
    reader.skip_whitespace();
    let z = reader.read_double()?;
    Ok((x, z))
}

// ── Per-type argument parsers ───────────────────────────────────────

fn parse_bool_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    Ok(ArgumentResult::Bool(reader.read_bool()?))
}

fn parse_integer_arg(
    reader: &mut StringReader<'_>,
    min: &Option<i32>,
    max: &Option<i32>,
) -> Result<ArgumentResult, CommandError> {
    let v = reader.read_integer()?;
    let v = validate_range(v, min.as_ref(), max.as_ref(), "Integer")?;
    Ok(ArgumentResult::Integer(v))
}

fn parse_long_arg(
    reader: &mut StringReader<'_>,
    min: &Option<i64>,
    max: &Option<i64>,
) -> Result<ArgumentResult, CommandError> {
    let v = reader.read_long()?;
    let v = validate_range(v, min.as_ref(), max.as_ref(), "Long")?;
    Ok(ArgumentResult::Long(v))
}

fn parse_float_arg(
    reader: &mut StringReader<'_>,
    min: &Option<f32>,
    max: &Option<f32>,
) -> Result<ArgumentResult, CommandError> {
    let v = reader.read_float()?;
    let v = validate_range(v, min.as_ref(), max.as_ref(), "Float")?;
    Ok(ArgumentResult::Float(v))
}

fn parse_double_num_arg(
    reader: &mut StringReader<'_>,
    min: &Option<f64>,
    max: &Option<f64>,
) -> Result<ArgumentResult, CommandError> {
    let v = reader.read_double()?;
    let v = validate_range(v, min.as_ref(), max.as_ref(), "Double")?;
    Ok(ArgumentResult::Double(v))
}

fn parse_block_pos_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let (x, y, z) = parse_int3_coordinates(reader)?;
    Ok(ArgumentResult::BlockPos(x, y, z))
}

fn parse_vec3_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let (x, y, z) = parse_double3_coordinates(reader)?;
    Ok(ArgumentResult::Vec3(x, y, z))
}

fn parse_vec2_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let (x, z) = parse_double2_coordinates(reader)?;
    Ok(ArgumentResult::Vec3(x, 0.0, z))
}

fn parse_gamemode_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let word = reader.read_word();
    match GameType::by_name(word) {
        Some(gm) => Ok(ArgumentResult::Gamemode(gm)),
        None => Err(CommandError::Parse(format!("Unknown game mode: '{word}'"))),
    }
}

fn parse_time_arg(reader: &mut StringReader<'_>, min: i32) -> Result<ArgumentResult, CommandError> {
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
    if ticks < min {
        return Err(CommandError::Parse(format!(
            "Time must not be less than {min} ticks, found {ticks}"
        )));
    }
    Ok(ArgumentResult::Time(ticks))
}

fn parse_uuid_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let word = reader.read_word();
    let uuid = uuid::Uuid::parse_str(word)
        .map_err(|_| CommandError::Parse(format!("Invalid UUID: '{word}'")))?;
    Ok(ArgumentResult::Uuid(uuid))
}

/// Reads a single word as a string argument (fallback for unimplemented types).
fn parse_word_as_string(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    Ok(ArgumentResult::String(reader.read_word().to_string()))
}

// ── Main dispatch ───────────────────────────────────────────────────

/// Parses an argument value from a `StringReader` given the argument type.
pub fn parse_argument(
    reader: &mut StringReader<'_>,
    arg_type: &ArgumentType,
) -> Result<ArgumentResult, CommandError> {
    match arg_type {
        ArgumentType::Bool => parse_bool_arg(reader),
        ArgumentType::Integer { min, max } => parse_integer_arg(reader, min, max),
        ArgumentType::Long { min, max } => parse_long_arg(reader, min, max),
        ArgumentType::Float { min, max } => parse_float_arg(reader, min, max),
        ArgumentType::Double { min, max } => parse_double_num_arg(reader, min, max),
        ArgumentType::String(kind) => Ok(ArgumentResult::String(reader.read_string(*kind))),
        // Entity selectors / game profiles / messages are complex —
        // read a word as a raw string for now (full parsing in a later phase).
        ArgumentType::Entity { .. } | ArgumentType::GameProfile | ArgumentType::Message => {
            parse_word_as_string(reader)
        },
        ArgumentType::BlockPos => parse_block_pos_arg(reader),
        ArgumentType::Vec3 => parse_vec3_arg(reader),
        ArgumentType::Vec2 => parse_vec2_arg(reader),
        ArgumentType::Gamemode => parse_gamemode_arg(reader),
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
        | ArgumentType::Dialog => parse_word_as_string(reader),
        ArgumentType::Time { min } => parse_time_arg(reader, *min),
        ArgumentType::Uuid => parse_uuid_arg(reader),
        // All remaining types: parse as a single word string for now.
        _ => parse_word_as_string(reader),
    }
}

// ── Typed argument getters ──────────────────────────────────────────

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
pub fn get_integer<S>(ctx: &CommandContext<S>, name: &str) -> Result<i32, CommandError> {
    get_typed(ctx, name, "an integer", |r| match r {
        ArgumentResult::Integer(v) => Some(*v),
        _ => None,
    })
}

/// Gets a long argument by name.
pub fn get_long<S>(ctx: &CommandContext<S>, name: &str) -> Result<i64, CommandError> {
    get_typed(ctx, name, "a long", |r| match r {
        ArgumentResult::Long(v) => Some(*v),
        _ => None,
    })
}

/// Gets a float argument by name.
pub fn get_float<S>(ctx: &CommandContext<S>, name: &str) -> Result<f32, CommandError> {
    get_typed(ctx, name, "a float", |r| match r {
        ArgumentResult::Float(v) => Some(*v),
        _ => None,
    })
}

/// Gets a double argument by name.
pub fn get_double<S>(ctx: &CommandContext<S>, name: &str) -> Result<f64, CommandError> {
    get_typed(ctx, name, "a double", |r| match r {
        ArgumentResult::Double(v) => Some(*v),
        _ => None,
    })
}

/// Gets a boolean argument by name.
pub fn get_bool<S>(ctx: &CommandContext<S>, name: &str) -> Result<bool, CommandError> {
    get_typed(ctx, name, "a boolean", |r| match r {
        ArgumentResult::Bool(v) => Some(*v),
        _ => None,
    })
}

/// Gets a string argument by name.
pub fn get_string<'a, S>(ctx: &'a CommandContext<S>, name: &str) -> Result<&'a str, CommandError> {
    match get_arg_result(ctx, name)? {
        ArgumentResult::String(v) => Ok(v.as_str()),
        _ => Err(CommandError::Parse(format!(
            "Argument '{name}' is not a string"
        ))),
    }
}

/// Gets a gamemode argument by name.
pub fn get_gamemode<S>(ctx: &CommandContext<S>, name: &str) -> Result<GameType, CommandError> {
    get_typed(ctx, name, "a game mode", |r| match r {
        ArgumentResult::Gamemode(gm) => Some(*gm),
        _ => None,
    })
}

/// Gets a time argument by name (in ticks).
pub fn get_time<S>(ctx: &CommandContext<S>, name: &str) -> Result<i32, CommandError> {
    match get_arg_result(ctx, name)? {
        // Accept both Time and raw Integer as ticks
        ArgumentResult::Time(v) | ArgumentResult::Integer(v) => Ok(*v),
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
    get_typed(ctx, name, "a block position", |r| match r {
        ArgumentResult::BlockPos(x, y, z) => Some((*x, *y, *z)),
        _ => None,
    })
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
    get_typed(ctx, name, "a vec3", |r| match r {
        ArgumentResult::Vec3(x, y, z) => Some((*x, *y, *z)),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    // ── StringReader tests ──────────────────────────────────────────

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

    // ── validate_range tests ────────────────────────────────────────

    #[test]
    fn validate_range_in_bounds() {
        assert_eq!(validate_range(5, Some(&1), Some(&10), "Int").unwrap(), 5);
    }

    #[test]
    fn validate_range_below_min() {
        let err = validate_range(0, Some(&1), Some(&10), "Int").unwrap_err();
        assert!(err.to_string().contains("must not be less than 1"));
    }

    #[test]
    fn validate_range_above_max() {
        let err = validate_range(11, Some(&1), Some(&10), "Int").unwrap_err();
        assert!(err.to_string().contains("must not be more than 10"));
    }

    #[test]
    fn validate_range_no_bounds() {
        assert_eq!(validate_range(42, None, None, "Int").unwrap(), 42);
    }

    #[test]
    fn validate_range_float_bounds() {
        assert!(validate_range(0.5_f64, Some(&0.0), Some(&1.0), "Double").is_ok());
        assert!(validate_range(-0.1_f64, Some(&0.0), Some(&1.0), "Double").is_err());
    }

    // ── Coordinate parsing tests ────────────────────────────────────

    #[test]
    fn parse_int3_coordinates_valid() {
        let mut reader = StringReader::new("10 20 30", 0);
        let (x, y, z) = parse_int3_coordinates(&mut reader).unwrap();
        assert_eq!((x, y, z), (10, 20, 30));
    }

    #[test]
    fn parse_int3_coordinates_negative() {
        let mut reader = StringReader::new("-5 0 -10", 0);
        let (x, y, z) = parse_int3_coordinates(&mut reader).unwrap();
        assert_eq!((x, y, z), (-5, 0, -10));
    }

    #[test]
    fn parse_double3_coordinates_valid() {
        let mut reader = StringReader::new("1.5 2.5 3.5", 0);
        let (x, y, z) = parse_double3_coordinates(&mut reader).unwrap();
        assert!((x - 1.5).abs() < f64::EPSILON);
        assert!((y - 2.5).abs() < f64::EPSILON);
        assert!((z - 3.5).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_double2_coordinates_valid() {
        let mut reader = StringReader::new("1.0 3.0", 0);
        let (x, z) = parse_double2_coordinates(&mut reader).unwrap();
        assert!((x - 1.0).abs() < f64::EPSILON);
        assert!((z - 3.0).abs() < f64::EPSILON);
    }

    // ── parse_argument tests ────────────────────────────────────────

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

    #[test]
    fn parse_integer_with_range() {
        let mut reader = StringReader::new("5", 0);
        let result = parse_argument(
            &mut reader,
            &ArgumentType::Integer {
                min: Some(1),
                max: Some(10),
            },
        );
        assert_eq!(result.unwrap(), ArgumentResult::Integer(5));
    }

    #[test]
    fn parse_integer_below_min() {
        let mut reader = StringReader::new("0", 0);
        let result = parse_argument(
            &mut reader,
            &ArgumentType::Integer {
                min: Some(1),
                max: None,
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn parse_vec2_returns_vec3_with_zero_y() {
        let mut reader = StringReader::new("1.0 3.0", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Vec2);
        assert_eq!(result.unwrap(), ArgumentResult::Vec3(1.0, 0.0, 3.0));
    }

    #[test]
    fn parse_block_pos_three_ints() {
        let mut reader = StringReader::new("10 64 -30", 0);
        let result = parse_argument(&mut reader, &ArgumentType::BlockPos);
        assert_eq!(result.unwrap(), ArgumentResult::BlockPos(10, 64, -30));
    }

    #[test]
    fn parse_uuid_valid() {
        let mut reader = StringReader::new("550e8400-e29b-41d4-a716-446655440000", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Uuid);
        assert!(matches!(result.unwrap(), ArgumentResult::Uuid(_)));
    }

    // ── Typed getter tests ──────────────────────────────────────────

    #[test]
    fn get_typed_missing_argument() {
        let ctx: CommandContext<()> = CommandContext {
            source: (),
            input: String::new(),
            arguments: HashMap::new(),
            command: None,
        };
        let err = get_integer(&ctx, "missing").unwrap_err();
        assert!(err.to_string().contains("No argument named 'missing'"));
    }

    #[test]
    fn get_typed_wrong_type() {
        let mut args = HashMap::new();
        args.insert(
            "val".to_string(),
            ParsedArgument {
                range: StringRange::new(0, 1),
                result: ArgumentResult::Bool(true),
            },
        );
        let ctx: CommandContext<()> = CommandContext {
            source: (),
            input: String::new(),
            arguments: args,
            command: None,
        };
        let err = get_integer(&ctx, "val").unwrap_err();
        assert!(err.to_string().contains("is not an integer"));
    }
}
