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
    /// An (x, y, z) integer position, possibly relative.
    BlockPos(i32, i32, i32),
    /// An (x, y, z) double-precision position.
    Vec3(f64, f64, f64),
    /// World coordinates supporting `~` relative and `^` local syntax.
    Coordinates(Coordinates),
    /// A game mode.
    Gamemode(GameType),
    /// A resource location string.
    ResourceLocation(String),
    /// A UUID.
    Uuid(uuid::Uuid),
    /// A time value in ticks.
    Time(i32),
    /// A named color.
    Color(NamedColor),
    /// An angle value (possibly relative).
    Angle {
        /// The angle in degrees.
        value: f32,
        /// Whether relative to the source's current angle.
        relative: bool,
    },
    /// An entity anchor point.
    EntityAnchor(EntityAnchorKind),
    /// A set of axes (e.g. `xy`, `xz`, `xyz`).
    Swizzle(u8),
    /// A min..max integer range.
    IntRange {
        /// Inclusive minimum (if any).
        min: Option<i32>,
        /// Inclusive maximum (if any).
        max: Option<i32>,
    },
    /// A min..max float range.
    FloatRange {
        /// Inclusive minimum (if any).
        min: Option<f64>,
        /// Inclusive maximum (if any).
        max: Option<f64>,
    },
}

/// A single coordinate component that may be absolute, relative (`~`), or
/// local (`^`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldCoordinate {
    /// The numeric value (offset if relative/local, absolute otherwise).
    pub value: f64,
    /// Whether this coordinate is relative to the source position.
    pub relative: bool,
}

/// The kind of coordinate system used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateKind {
    /// World coordinates — each axis is absolute or `~` relative.
    World,
    /// Local coordinates — all axes use `^` (relative to facing direction).
    Local,
}

/// Three-component coordinates parsed from command input.
///
/// Vanilla supports two forms:
/// - **World coordinates**: `100 64 -200`, `~10 ~ ~-5` (absolute/relative per axis)
/// - **Local coordinates**: `^1 ^0 ^2` (left/up/forwards relative to facing)
///
/// The two forms cannot be mixed (all `^` or none `^`).
#[derive(Debug, Clone, PartialEq)]
pub struct Coordinates {
    /// X component (or "left" in local mode).
    pub x: WorldCoordinate,
    /// Y component (or "up" in local mode).
    pub y: WorldCoordinate,
    /// Z component (or "forwards" in local mode).
    pub z: WorldCoordinate,
    /// Whether these are world or local coordinates.
    pub kind: CoordinateKind,
}

impl Coordinates {
    /// Resolves these coordinates to absolute (x, y, z) using the given
    /// source position and rotation.
    ///
    /// For world coordinates, relative axes (`~`) add to the source position.
    /// For local coordinates (`^`), the offsets are rotated by the source's
    /// yaw and pitch.
    pub fn resolve(&self, position: (f64, f64, f64), rotation: (f32, f32)) -> (f64, f64, f64) {
        match self.kind {
            CoordinateKind::World => {
                let x = if self.x.relative {
                    position.0 + self.x.value
                } else {
                    self.x.value
                };
                let y = if self.y.relative {
                    position.1 + self.y.value
                } else {
                    self.y.value
                };
                let z = if self.z.relative {
                    position.2 + self.z.value
                } else {
                    self.z.value
                };
                (x, y, z)
            },
            CoordinateKind::Local => {
                let (yaw, pitch) = rotation;
                let yaw_rad = (yaw as f64).to_radians();
                let pitch_rad = (pitch as f64).to_radians();

                let (sin_yaw, cos_yaw) = yaw_rad.sin_cos();
                let (sin_pitch, cos_pitch) = pitch_rad.sin_cos();

                // Vanilla's local coordinate system:
                //   left  = x component (perpendicular to facing, horizontal)
                //   up    = y component (perpendicular to facing, vertical plane)
                //   fwd   = z component (in facing direction)
                let left = self.x.value;
                let up = self.y.value;
                let fwd = self.z.value;

                // Forward vector (from yaw/pitch)
                let fwd_x = -sin_yaw * cos_pitch;
                let fwd_y = -sin_pitch;
                let fwd_z = cos_yaw * cos_pitch;

                // Up vector (perpendicular to forward in vertical plane)
                let up_x = -sin_yaw * (-sin_pitch);
                let up_y = cos_pitch;
                let up_z = cos_yaw * (-sin_pitch);

                // Left vector (cross product of up and forward, simplified)
                let left_x = cos_yaw;
                let left_y = 0.0;
                let left_z = sin_yaw;

                let x = position.0 + left * left_x + up * up_x + fwd * fwd_x;
                let y = position.1 + left * left_y + up * up_y + fwd * fwd_y;
                let z = position.2 + left * left_z + up * up_z + fwd * fwd_z;
                (x, y, z)
            },
        }
    }

    /// Resolves to integer block position (floors after resolving).
    pub fn resolve_block_pos(
        &self,
        position: (f64, f64, f64),
        rotation: (f32, f32),
    ) -> (i32, i32, i32) {
        let (x, y, z) = self.resolve(position, rotation);
        (x.floor() as i32, y.floor() as i32, z.floor() as i32)
    }
}

/// Named chat colors (vanilla's 16 formatting colors).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedColor {
    /// `black` (§0)
    Black,
    /// `dark_blue` (§1)
    DarkBlue,
    /// `dark_green` (§2)
    DarkGreen,
    /// `dark_aqua` (§3)
    DarkAqua,
    /// `dark_red` (§4)
    DarkRed,
    /// `dark_purple` (§5)
    DarkPurple,
    /// `gold` (§6)
    Gold,
    /// `gray` (§7)
    Gray,
    /// `dark_gray` (§8)
    DarkGray,
    /// `blue` (§9)
    Blue,
    /// `green` (§a)
    Green,
    /// `aqua` (§b)
    Aqua,
    /// `red` (§c)
    Red,
    /// `light_purple` (§d)
    LightPurple,
    /// `yellow` (§e)
    Yellow,
    /// `white` (§f)
    White,
}

impl NamedColor {
    /// Parses a color name (case-insensitive).
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "black" => Self::Black,
            "dark_blue" => Self::DarkBlue,
            "dark_green" => Self::DarkGreen,
            "dark_aqua" => Self::DarkAqua,
            "dark_red" => Self::DarkRed,
            "dark_purple" => Self::DarkPurple,
            "gold" => Self::Gold,
            "gray" => Self::Gray,
            "dark_gray" => Self::DarkGray,
            "blue" => Self::Blue,
            "green" => Self::Green,
            "aqua" => Self::Aqua,
            "red" => Self::Red,
            "light_purple" => Self::LightPurple,
            "yellow" => Self::Yellow,
            "white" => Self::White,
            _ => return None,
        })
    }
}

/// Entity anchor points for `/tp facing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityAnchorKind {
    /// At the entity's feet.
    Feet,
    /// At the entity's eyes.
    Eyes,
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

/// Parses a single coordinate component that may be absolute, relative (`~`),
/// or local (`^`). Returns `(WorldCoordinate, is_local)`.
fn parse_single_coordinate(
    reader: &mut StringReader<'_>,
) -> Result<(WorldCoordinate, bool), CommandError> {
    let remaining = reader.remaining();
    if remaining.starts_with('^') {
        reader.cursor += 1;
        // ^<number> or just ^ (meaning ^0)
        let value = if reader.can_read() && reader.peek() != Some(' ') {
            reader.read_double()?
        } else {
            0.0
        };
        Ok((WorldCoordinate { value, relative: true }, true))
    } else if remaining.starts_with('~') {
        reader.cursor += 1;
        // ~<number> or just ~ (meaning ~0)
        let value = if reader.can_read() && reader.peek() != Some(' ') {
            reader.read_double()?
        } else {
            0.0
        };
        Ok((WorldCoordinate { value, relative: true }, false))
    } else {
        let value = reader.read_double()?;
        Ok((WorldCoordinate { value, relative: false }, false))
    }
}

/// Parses a single integer coordinate that may be relative (`~`).
fn parse_single_int_coordinate(
    reader: &mut StringReader<'_>,
) -> Result<(WorldCoordinate, bool), CommandError> {
    let remaining = reader.remaining();
    if remaining.starts_with('^') {
        reader.cursor += 1;
        let value = if reader.can_read() && reader.peek() != Some(' ') {
            reader.read_integer()? as f64
        } else {
            0.0
        };
        Ok((WorldCoordinate { value, relative: true }, true))
    } else if remaining.starts_with('~') {
        reader.cursor += 1;
        let value = if reader.can_read() && reader.peek() != Some(' ') {
            reader.read_integer()? as f64
        } else {
            0.0
        };
        Ok((WorldCoordinate { value, relative: true }, false))
    } else {
        let value = reader.read_integer()? as f64;
        Ok((WorldCoordinate { value, relative: false }, false))
    }
}

/// Parses three whitespace-separated coordinates supporting `~`/`^` syntax.
fn parse_coordinates3(reader: &mut StringReader<'_>) -> Result<Coordinates, CommandError> {
    let (x, x_local) = parse_single_coordinate(reader)?;
    reader.skip_whitespace();
    let (y, y_local) = parse_single_coordinate(reader)?;
    reader.skip_whitespace();
    let (z, z_local) = parse_single_coordinate(reader)?;

    // Cannot mix local (^) with non-local coordinates.
    if x_local != y_local || y_local != z_local {
        return Err(CommandError::Parse(
            "Cannot mix world and local coordinates (^ and ~)".to_string(),
        ));
    }

    let kind = if x_local {
        CoordinateKind::Local
    } else {
        CoordinateKind::World
    };

    Ok(Coordinates { x, y, z, kind })
}

/// Parses three whitespace-separated integer coordinates supporting `~`/`^`.
fn parse_int_coordinates3(reader: &mut StringReader<'_>) -> Result<Coordinates, CommandError> {
    let (x, x_local) = parse_single_int_coordinate(reader)?;
    reader.skip_whitespace();
    let (y, y_local) = parse_single_int_coordinate(reader)?;
    reader.skip_whitespace();
    let (z, z_local) = parse_single_int_coordinate(reader)?;

    if x_local != y_local || y_local != z_local {
        return Err(CommandError::Parse(
            "Cannot mix world and local coordinates (^ and ~)".to_string(),
        ));
    }

    let kind = if x_local {
        CoordinateKind::Local
    } else {
        CoordinateKind::World
    };

    Ok(Coordinates { x, y, z, kind })
}

/// Parses two whitespace-separated coordinates (x z) for Vec2.
fn parse_coordinates2(reader: &mut StringReader<'_>) -> Result<Coordinates, CommandError> {
    let (x, x_local) = parse_single_coordinate(reader)?;
    reader.skip_whitespace();
    let (z, z_local) = parse_single_coordinate(reader)?;

    if x_local != z_local {
        return Err(CommandError::Parse(
            "Cannot mix world and local coordinates (^ and ~)".to_string(),
        ));
    }

    let kind = if x_local {
        CoordinateKind::Local
    } else {
        CoordinateKind::World
    };

    Ok(Coordinates {
        x,
        y: WorldCoordinate {
            value: 0.0,
            relative: false,
        },
        z,
        kind,
    })
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
    let coords = parse_int_coordinates3(reader)?;
    // If all absolute, return legacy BlockPos for backwards compat.
    if coords.kind == CoordinateKind::World
        && !coords.x.relative
        && !coords.y.relative
        && !coords.z.relative
    {
        return Ok(ArgumentResult::BlockPos(
            coords.x.value as i32,
            coords.y.value as i32,
            coords.z.value as i32,
        ));
    }
    Ok(ArgumentResult::Coordinates(coords))
}

fn parse_vec3_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let coords = parse_coordinates3(reader)?;
    // If all absolute, return legacy Vec3 for backwards compat.
    if coords.kind == CoordinateKind::World
        && !coords.x.relative
        && !coords.y.relative
        && !coords.z.relative
    {
        return Ok(ArgumentResult::Vec3(
            coords.x.value,
            coords.y.value,
            coords.z.value,
        ));
    }
    Ok(ArgumentResult::Coordinates(coords))
}

fn parse_vec2_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let coords = parse_coordinates2(reader)?;
    if coords.kind == CoordinateKind::World && !coords.x.relative && !coords.z.relative {
        return Ok(ArgumentResult::Vec3(coords.x.value, 0.0, coords.z.value));
    }
    Ok(ArgumentResult::Coordinates(coords))
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

fn parse_color_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let word = reader.read_word();
    NamedColor::from_name(word)
        .map(ArgumentResult::Color)
        .ok_or_else(|| CommandError::Parse(format!("Unknown color: '{word}'")))
}

fn parse_angle_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let remaining = reader.remaining();
    if remaining.starts_with('~') {
        reader.cursor += 1;
        let value = if reader.can_read() && reader.peek() != Some(' ') {
            reader.read_float()?
        } else {
            0.0
        };
        Ok(ArgumentResult::Angle {
            value,
            relative: true,
        })
    } else {
        let value = reader.read_float()?;
        Ok(ArgumentResult::Angle {
            value,
            relative: false,
        })
    }
}

fn parse_entity_anchor_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let word = reader.read_word();
    match word {
        "feet" => Ok(ArgumentResult::EntityAnchor(EntityAnchorKind::Feet)),
        "eyes" => Ok(ArgumentResult::EntityAnchor(EntityAnchorKind::Eyes)),
        _ => Err(CommandError::Parse(format!(
            "Invalid anchor: '{word}' (expected 'feet' or 'eyes')"
        ))),
    }
}

fn parse_swizzle_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let word = reader.read_word();
    let mut mask = 0u8;
    for ch in word.chars() {
        match ch {
            'x' => mask |= 0b001,
            'y' => mask |= 0b010,
            'z' => mask |= 0b100,
            _ => {
                return Err(CommandError::Parse(format!(
                    "Invalid axis in swizzle: '{ch}' (expected x, y, or z)"
                )));
            },
        }
    }
    if mask == 0 {
        return Err(CommandError::Parse(
            "Swizzle must contain at least one axis".to_string(),
        ));
    }
    Ok(ArgumentResult::Swizzle(mask))
}

fn parse_int_range_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let word = reader.read_word();
    if let Some((min_s, max_s)) = word.split_once("..") {
        let min = if min_s.is_empty() {
            None
        } else {
            Some(min_s.parse::<i32>().map_err(|_| {
                CommandError::Parse(format!("Invalid range minimum: '{min_s}'"))
            })?)
        };
        let max = if max_s.is_empty() {
            None
        } else {
            Some(max_s.parse::<i32>().map_err(|_| {
                CommandError::Parse(format!("Invalid range maximum: '{max_s}'"))
            })?)
        };
        Ok(ArgumentResult::IntRange { min, max })
    } else {
        let v = word.parse::<i32>().map_err(|_| {
            CommandError::Parse(format!("Invalid integer range: '{word}'"))
        })?;
        Ok(ArgumentResult::IntRange {
            min: Some(v),
            max: Some(v),
        })
    }
}

fn parse_float_range_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let word = reader.read_word();
    if let Some((min_s, max_s)) = word.split_once("..") {
        let min = if min_s.is_empty() {
            None
        } else {
            Some(min_s.parse::<f64>().map_err(|_| {
                CommandError::Parse(format!("Invalid range minimum: '{min_s}'"))
            })?)
        };
        let max = if max_s.is_empty() {
            None
        } else {
            Some(max_s.parse::<f64>().map_err(|_| {
                CommandError::Parse(format!("Invalid range maximum: '{max_s}'"))
            })?)
        };
        Ok(ArgumentResult::FloatRange { min, max })
    } else {
        let v = word.parse::<f64>().map_err(|_| {
            CommandError::Parse(format!("Invalid float range: '{word}'"))
        })?;
        Ok(ArgumentResult::FloatRange {
            min: Some(v),
            max: Some(v),
        })
    }
}

/// Parses rotation (pitch yaw) with support for `~` relative syntax.
fn parse_rotation_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let coords = parse_coordinates2(reader)?;
    // Store rotation as Coordinates (x=pitch, z=yaw).
    Ok(ArgumentResult::Coordinates(coords))
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
        ArgumentType::Color => parse_color_arg(reader),
        ArgumentType::Angle => parse_angle_arg(reader),
        ArgumentType::EntityAnchor => parse_entity_anchor_arg(reader),
        ArgumentType::Swizzle => parse_swizzle_arg(reader),
        ArgumentType::IntRange => parse_int_range_arg(reader),
        ArgumentType::FloatRange => parse_float_range_arg(reader),
        ArgumentType::Rotation => parse_rotation_arg(reader),
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
    fn parse_block_pos_valid_integers() {
        let mut reader = StringReader::new("10 20 30", 0);
        let result = parse_argument(&mut reader, &ArgumentType::BlockPos).unwrap();
        assert_eq!(result, ArgumentResult::BlockPos(10, 20, 30));
    }

    #[test]
    fn parse_block_pos_negative_integers() {
        let mut reader = StringReader::new("-5 0 -10", 0);
        let result = parse_argument(&mut reader, &ArgumentType::BlockPos).unwrap();
        assert_eq!(result, ArgumentResult::BlockPos(-5, 0, -10));
    }

    #[test]
    fn parse_vec3_valid_doubles() {
        let mut reader = StringReader::new("1.5 2.5 3.5", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Vec3).unwrap();
        assert_eq!(result, ArgumentResult::Vec3(1.5, 2.5, 3.5));
    }

    #[test]
    fn parse_vec2_valid_doubles() {
        let mut reader = StringReader::new("1.0 3.0", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Vec2).unwrap();
        assert_eq!(result, ArgumentResult::Vec3(1.0, 0.0, 3.0));
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

    // ── Relative coordinate parsing tests ───────────────────────────

    #[test]
    fn parse_vec3_relative_tilde() {
        let mut reader = StringReader::new("~10 ~ ~-5", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Vec3).unwrap();
        match result {
            ArgumentResult::Coordinates(coords) => {
                assert_eq!(coords.kind, CoordinateKind::World);
                assert!(coords.x.relative);
                assert!((coords.x.value - 10.0).abs() < f64::EPSILON);
                assert!(coords.y.relative);
                assert!((coords.y.value - 0.0).abs() < f64::EPSILON);
                assert!(coords.z.relative);
                assert!((coords.z.value - -5.0).abs() < f64::EPSILON);
            },
            _ => panic!("Expected Coordinates, got {result:?}"),
        }
    }

    #[test]
    fn parse_vec3_absolute_returns_vec3() {
        let mut reader = StringReader::new("100.5 64.0 -200.5", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Vec3).unwrap();
        // All absolute → should return Vec3, not Coordinates
        assert_eq!(result, ArgumentResult::Vec3(100.5, 64.0, -200.5));
    }

    #[test]
    fn parse_vec3_local_caret() {
        let mut reader = StringReader::new("^1 ^0 ^2", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Vec3).unwrap();
        match result {
            ArgumentResult::Coordinates(coords) => {
                assert_eq!(coords.kind, CoordinateKind::Local);
                assert!(coords.x.relative);
                assert!((coords.x.value - 1.0).abs() < f64::EPSILON);
                assert!((coords.y.value - 0.0).abs() < f64::EPSILON);
                assert!((coords.z.value - 2.0).abs() < f64::EPSILON);
            },
            _ => panic!("Expected Coordinates, got {result:?}"),
        }
    }

    #[test]
    fn parse_vec3_mixed_tilde_caret_rejected() {
        let mut reader = StringReader::new("~1 ^0 ~2", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Vec3);
        assert!(result.is_err());
    }

    #[test]
    fn parse_vec3_bare_tilde() {
        let mut reader = StringReader::new("~ ~ ~", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Vec3).unwrap();
        match result {
            ArgumentResult::Coordinates(coords) => {
                assert_eq!(coords.kind, CoordinateKind::World);
                assert!(coords.x.relative);
                assert!((coords.x.value).abs() < f64::EPSILON);
                assert!(coords.y.relative);
                assert!(coords.z.relative);
            },
            _ => panic!("Expected Coordinates, got {result:?}"),
        }
    }

    #[test]
    fn parse_block_pos_relative() {
        let mut reader = StringReader::new("~5 ~0 ~-3", 0);
        let result = parse_argument(&mut reader, &ArgumentType::BlockPos).unwrap();
        match result {
            ArgumentResult::Coordinates(coords) => {
                assert_eq!(coords.kind, CoordinateKind::World);
                assert!(coords.x.relative);
                assert!((coords.x.value - 5.0).abs() < f64::EPSILON);
            },
            _ => panic!("Expected Coordinates, got {result:?}"),
        }
    }

    #[test]
    fn parse_block_pos_absolute_returns_block_pos() {
        let mut reader = StringReader::new("10 64 -30", 0);
        let result = parse_argument(&mut reader, &ArgumentType::BlockPos).unwrap();
        assert_eq!(result, ArgumentResult::BlockPos(10, 64, -30));
    }

    #[test]
    fn coordinates_resolve_absolute() {
        let coords = Coordinates {
            x: WorldCoordinate { value: 100.0, relative: false },
            y: WorldCoordinate { value: 64.0, relative: false },
            z: WorldCoordinate { value: -200.0, relative: false },
            kind: CoordinateKind::World,
        };
        let (x, y, z) = coords.resolve((0.0, 0.0, 0.0), (0.0, 0.0));
        assert!((x - 100.0).abs() < f64::EPSILON);
        assert!((y - 64.0).abs() < f64::EPSILON);
        assert!((z - -200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn coordinates_resolve_relative() {
        let coords = Coordinates {
            x: WorldCoordinate { value: 10.0, relative: true },
            y: WorldCoordinate { value: 0.0, relative: true },
            z: WorldCoordinate { value: -5.0, relative: true },
            kind: CoordinateKind::World,
        };
        let (x, y, z) = coords.resolve((50.0, 100.0, 200.0), (0.0, 0.0));
        assert!((x - 60.0).abs() < f64::EPSILON);
        assert!((y - 100.0).abs() < f64::EPSILON);
        assert!((z - 195.0).abs() < f64::EPSILON);
    }

    #[test]
    fn coordinates_resolve_mixed() {
        let coords = Coordinates {
            x: WorldCoordinate { value: 100.0, relative: false },
            y: WorldCoordinate { value: 5.0, relative: true },
            z: WorldCoordinate { value: -200.0, relative: false },
            kind: CoordinateKind::World,
        };
        let (x, y, z) = coords.resolve((50.0, 60.0, 200.0), (0.0, 0.0));
        assert!((x - 100.0).abs() < f64::EPSILON);
        assert!((y - 65.0).abs() < f64::EPSILON);
        assert!((z - -200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn coordinates_resolve_block_pos_floors() {
        let coords = Coordinates {
            x: WorldCoordinate { value: 10.7, relative: false },
            y: WorldCoordinate { value: -0.3, relative: false },
            z: WorldCoordinate { value: 5.9, relative: false },
            kind: CoordinateKind::World,
        };
        let (x, y, z) = coords.resolve_block_pos((0.0, 0.0, 0.0), (0.0, 0.0));
        assert_eq!((x, y, z), (10, -1, 5));
    }

    // ── New argument type parsing tests ─────────────────────────────

    #[test]
    fn parse_color_valid() {
        let mut reader = StringReader::new("red", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Color).unwrap();
        assert_eq!(result, ArgumentResult::Color(NamedColor::Red));
    }

    #[test]
    fn parse_color_dark_aqua() {
        let mut reader = StringReader::new("dark_aqua", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Color).unwrap();
        assert_eq!(result, ArgumentResult::Color(NamedColor::DarkAqua));
    }

    #[test]
    fn parse_color_invalid() {
        let mut reader = StringReader::new("pink", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Color);
        assert!(result.is_err());
    }

    #[test]
    fn parse_angle_absolute() {
        let mut reader = StringReader::new("45.5", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Angle).unwrap();
        assert_eq!(
            result,
            ArgumentResult::Angle {
                value: 45.5,
                relative: false
            }
        );
    }

    #[test]
    fn parse_angle_relative() {
        let mut reader = StringReader::new("~10", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Angle).unwrap();
        assert_eq!(
            result,
            ArgumentResult::Angle {
                value: 10.0,
                relative: true
            }
        );
    }

    #[test]
    fn parse_angle_bare_tilde() {
        let mut reader = StringReader::new("~", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Angle).unwrap();
        assert_eq!(
            result,
            ArgumentResult::Angle {
                value: 0.0,
                relative: true
            }
        );
    }

    #[test]
    fn parse_entity_anchor_feet() {
        let mut reader = StringReader::new("feet", 0);
        let result = parse_argument(&mut reader, &ArgumentType::EntityAnchor).unwrap();
        assert_eq!(
            result,
            ArgumentResult::EntityAnchor(EntityAnchorKind::Feet)
        );
    }

    #[test]
    fn parse_entity_anchor_eyes() {
        let mut reader = StringReader::new("eyes", 0);
        let result = parse_argument(&mut reader, &ArgumentType::EntityAnchor).unwrap();
        assert_eq!(
            result,
            ArgumentResult::EntityAnchor(EntityAnchorKind::Eyes)
        );
    }

    #[test]
    fn parse_entity_anchor_invalid() {
        let mut reader = StringReader::new("head", 0);
        let result = parse_argument(&mut reader, &ArgumentType::EntityAnchor);
        assert!(result.is_err());
    }

    #[test]
    fn parse_swizzle_xyz() {
        let mut reader = StringReader::new("xyz", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Swizzle).unwrap();
        assert_eq!(result, ArgumentResult::Swizzle(0b111));
    }

    #[test]
    fn parse_swizzle_xz() {
        let mut reader = StringReader::new("xz", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Swizzle).unwrap();
        assert_eq!(result, ArgumentResult::Swizzle(0b101));
    }

    #[test]
    fn parse_swizzle_y() {
        let mut reader = StringReader::new("y", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Swizzle).unwrap();
        assert_eq!(result, ArgumentResult::Swizzle(0b010));
    }

    #[test]
    fn parse_swizzle_invalid() {
        let mut reader = StringReader::new("w", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Swizzle);
        assert!(result.is_err());
    }

    #[test]
    fn parse_int_range_both_bounds() {
        let mut reader = StringReader::new("10..50", 0);
        let result = parse_argument(&mut reader, &ArgumentType::IntRange).unwrap();
        assert_eq!(
            result,
            ArgumentResult::IntRange {
                min: Some(10),
                max: Some(50)
            }
        );
    }

    #[test]
    fn parse_int_range_open_max() {
        let mut reader = StringReader::new("5..", 0);
        let result = parse_argument(&mut reader, &ArgumentType::IntRange).unwrap();
        assert_eq!(
            result,
            ArgumentResult::IntRange {
                min: Some(5),
                max: None
            }
        );
    }

    #[test]
    fn parse_int_range_open_min() {
        let mut reader = StringReader::new("..100", 0);
        let result = parse_argument(&mut reader, &ArgumentType::IntRange).unwrap();
        assert_eq!(
            result,
            ArgumentResult::IntRange {
                min: None,
                max: Some(100)
            }
        );
    }

    #[test]
    fn parse_int_range_exact() {
        let mut reader = StringReader::new("42", 0);
        let result = parse_argument(&mut reader, &ArgumentType::IntRange).unwrap();
        assert_eq!(
            result,
            ArgumentResult::IntRange {
                min: Some(42),
                max: Some(42)
            }
        );
    }

    #[test]
    fn parse_float_range_both_bounds() {
        let mut reader = StringReader::new("1.5..3.5", 0);
        let result = parse_argument(&mut reader, &ArgumentType::FloatRange).unwrap();
        assert_eq!(
            result,
            ArgumentResult::FloatRange {
                min: Some(1.5),
                max: Some(3.5)
            }
        );
    }

    #[test]
    fn parse_rotation_absolute() {
        let mut reader = StringReader::new("0 180", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Rotation).unwrap();
        match result {
            ArgumentResult::Vec3(x, _, z) => {
                assert!((x - 0.0).abs() < f64::EPSILON);
                assert!((z - 180.0).abs() < f64::EPSILON);
            },
            ArgumentResult::Coordinates(_) => {},
            _ => panic!("Expected Vec3 or Coordinates for rotation"),
        }
    }

    #[test]
    fn parse_rotation_relative() {
        let mut reader = StringReader::new("~10 ~0", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Rotation).unwrap();
        match result {
            ArgumentResult::Coordinates(coords) => {
                assert!(coords.x.relative);
                assert!((coords.x.value - 10.0).abs() < f64::EPSILON);
                assert!(coords.z.relative);
                assert!((coords.z.value - 0.0).abs() < f64::EPSILON);
            },
            _ => panic!("Expected Coordinates for relative rotation"),
        }
    }
}
