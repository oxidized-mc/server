//! SNBT (Stringified NBT) parser and formatter.
//!
//! SNBT is the human-readable text format for NBT data used in Minecraft
//! commands. This module provides [`parse_snbt`] for parsing SNBT strings
//! into [`NbtTag`] values, and [`format_snbt`] / [`format_snbt_pretty`] for
//! producing SNBT text from tags.

use std::fmt::Write;

use crate::compound::NbtCompound;
use crate::error::NbtError;
use crate::list::NbtList;
use crate::tag::NbtTag;

/// Parses an SNBT string into an [`NbtTag`].
///
/// # Examples
///
/// ```
/// # use oxidized_nbt::parse_snbt;
/// let tag = parse_snbt("{Health: 20.0f, Name: \"Steve\"}").unwrap();
/// ```
///
/// # Errors
///
/// Returns [`NbtError::SnbtParse`] if the input is not valid SNBT.
pub fn parse_snbt(input: &str) -> Result<NbtTag, NbtError> {
    let mut parser = SnbtParser::new(input);
    let tag = parser.parse_value(0)?;
    parser.skip_whitespace();
    if parser.pos < parser.input.len() {
        return Err(parser.error("unexpected trailing characters"));
    }
    Ok(tag)
}

/// Formats an [`NbtTag`] as compact SNBT (no extra whitespace).
///
/// # Examples
///
/// ```
/// # use oxidized_nbt::{NbtTag, format_snbt};
/// assert_eq!(format_snbt(&NbtTag::Int(42)), "42");
/// assert_eq!(format_snbt(&NbtTag::Byte(1)), "1b");
/// ```
pub fn format_snbt(tag: &NbtTag) -> String {
    let mut out = String::new();
    write_snbt(&mut out, tag, 0);
    out
}

/// Formats an [`NbtTag`] as pretty-printed SNBT with indentation.
///
/// Each level of nesting is indented by `indent` spaces. Compounds and
/// lists containing compounds are printed with one entry per line.
///
/// # Examples
///
/// ```
/// # use oxidized_nbt::{NbtTag, NbtCompound, format_snbt_pretty};
/// let mut c = NbtCompound::new();
/// c.put_int("x", 1);
/// let s = format_snbt_pretty(&NbtTag::Compound(c), 2);
/// assert!(s.contains('\n'));
/// ```
pub fn format_snbt_pretty(tag: &NbtTag, indent: usize) -> String {
    let mut out = String::new();
    write_snbt_pretty(&mut out, tag, 0, indent);
    out
}

// ── Parser ──────────────────────────────────────────────────────────────

/// Recursive-descent SNBT parser with depth tracking.
struct SnbtParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> SnbtParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    /// Returns an error if the given depth exceeds MAX_DEPTH.
    fn check_depth(depth: usize) -> Result<(), NbtError> {
        if depth > crate::error::MAX_DEPTH {
            Err(NbtError::DepthLimit {
                depth,
                max: crate::error::MAX_DEPTH,
            })
        } else {
            Ok(())
        }
    }

    /// Creates an [`NbtError::SnbtParse`] at the current position.
    fn error(&self, message: &str) -> NbtError {
        NbtError::SnbtParse {
            pos: self.pos,
            message: message.to_owned(),
        }
    }

    /// Returns the next byte without advancing, or `None` at EOF.
    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    /// Consumes the expected character or returns an error.
    fn expect(&mut self, ch: u8) -> Result<(), NbtError> {
        self.skip_whitespace();
        match self.peek() {
            Some(b) if b == ch => {
                self.pos += 1;
                Ok(())
            },
            Some(b) => {
                Err(self.error(&format!("expected '{}', found '{}'", ch as char, b as char)))
            },
            None => Err(self.error(&format!("expected '{}', found end of input", ch as char))),
        }
    }

    /// Skips ASCII whitespace.
    fn skip_whitespace(&mut self) {
        let bytes = self.input.as_bytes();
        while self.pos < bytes.len() && bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    /// Parses a single SNBT value at the given nesting depth.
    fn parse_value(&mut self, depth: usize) -> Result<NbtTag, NbtError> {
        self.skip_whitespace();
        match self.peek() {
            None => Err(self.error("unexpected end of input")),
            Some(b'{') => self.parse_compound(depth),
            Some(b'[') => self.parse_list_or_array(depth),
            Some(b'"') => {
                let s = self.parse_quoted_string(b'"')?;
                Ok(NbtTag::String(s))
            },
            Some(b'\'') => {
                let s = self.parse_quoted_string(b'\'')?;
                Ok(NbtTag::String(s))
            },
            _ => self.parse_primitive(),
        }
    }

    /// Parses a compound tag: `{key: value, ...}`.
    fn parse_compound(&mut self, depth: usize) -> Result<NbtTag, NbtError> {
        let next_depth = depth + 1;
        Self::check_depth(next_depth)?;
        self.expect(b'{')?;
        let mut compound = NbtCompound::new();
        self.skip_whitespace();

        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(NbtTag::Compound(compound));
        }

        loop {
            self.skip_whitespace();
            let key = self.parse_key()?;
            self.expect(b':')?;
            let value = self.parse_value(next_depth)?;
            compound.put(key, value);

            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                },
                Some(b'}') => {
                    self.pos += 1;
                    break;
                },
                _ => return Err(self.error("expected ',' or '}' in compound")),
            }
        }

        Ok(NbtTag::Compound(compound))
    }

    /// Parses a compound key (quoted or unquoted).
    fn parse_key(&mut self) -> Result<String, NbtError> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'"') => self.parse_quoted_string(b'"'),
            Some(b'\'') => self.parse_quoted_string(b'\''),
            _ => self.parse_unquoted_string(),
        }
    }

    /// Parses a `[` that starts a list or typed array.
    fn parse_list_or_array(&mut self, depth: usize) -> Result<NbtTag, NbtError> {
        self.expect(b'[')?;
        self.skip_whitespace();

        // Check for typed array prefix: [B; ...], [I; ...], [L; ...]
        if self.pos + 1 < self.input.len() {
            let prefix = self.input.as_bytes()[self.pos];
            let after = self.input.as_bytes()[self.pos + 1];
            if (prefix == b'B' || prefix == b'I' || prefix == b'L')
                && (after == b';' || after.is_ascii_whitespace())
            {
                // Check that `;` follows (possibly after whitespace)
                let saved = self.pos;
                self.pos += 1;
                self.skip_whitespace();
                if self.peek() == Some(b';') {
                    self.pos += 1; // skip `;`
                    return self.parse_typed_array(prefix);
                }
                // Not a typed array prefix — rewind
                self.pos = saved;
            }
        }

        self.parse_list(depth)
    }

    /// Parses a typed array after the `[X;` prefix has been consumed.
    fn parse_typed_array(&mut self, prefix: u8) -> Result<NbtTag, NbtError> {
        self.skip_whitespace();

        match prefix {
            b'B' => {
                let values = self.parse_array_elements(|p| {
                    let tag = p.parse_primitive()?;
                    match tag {
                        NbtTag::Byte(v) => Ok(v),
                        _ => Err(p.error("expected byte value in byte array")),
                    }
                })?;
                Ok(NbtTag::ByteArray(values))
            },
            b'I' => {
                let values = self.parse_array_elements(|p| {
                    let tag = p.parse_primitive()?;
                    match tag {
                        NbtTag::Int(v) => Ok(v),
                        _ => Err(p.error("expected int value in int array")),
                    }
                })?;
                Ok(NbtTag::IntArray(values))
            },
            b'L' => {
                let values = self.parse_array_elements(|p| {
                    let tag = p.parse_primitive()?;
                    match tag {
                        NbtTag::Long(v) => Ok(v),
                        _ => Err(p.error("expected long value in long array")),
                    }
                })?;
                Ok(NbtTag::LongArray(values))
            },
            _ => Err(self.error("invalid typed array prefix")),
        }
    }

    /// Parses comma-separated array elements until `]`.
    fn parse_array_elements<T>(
        &mut self,
        mut parse_one: impl FnMut(&mut Self) -> Result<T, NbtError>,
    ) -> Result<Vec<T>, NbtError> {
        let mut values = Vec::new();
        self.skip_whitespace();

        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(values);
        }

        loop {
            self.skip_whitespace();
            values.push(parse_one(self)?);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                },
                Some(b']') => {
                    self.pos += 1;
                    break;
                },
                _ => return Err(self.error("expected ',' or ']' in array")),
            }
        }
        Ok(values)
    }

    /// Parses a generic list `[v1, v2, ...]`.
    fn parse_list(&mut self, depth: usize) -> Result<NbtTag, NbtError> {
        let next_depth = depth + 1;
        Self::check_depth(next_depth)?;
        let mut list = NbtList::empty();
        self.skip_whitespace();

        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(NbtTag::List(list));
        }

        loop {
            self.skip_whitespace();
            let value = self.parse_value(next_depth)?;
            list.push(value).map_err(|e| NbtError::SnbtParse {
                pos: self.pos,
                message: e.to_string(),
            })?;

            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                },
                Some(b']') => {
                    self.pos += 1;
                    break;
                },
                _ => return Err(self.error("expected ',' or ']' in list")),
            }
        }

        Ok(NbtTag::List(list))
    }

    /// Parses a primitive: number, boolean, or unquoted string.
    fn parse_primitive(&mut self) -> Result<NbtTag, NbtError> {
        self.skip_whitespace();

        let start = self.pos;
        let bytes = self.input.as_bytes();

        if start >= bytes.len() {
            return Err(self.error("unexpected end of input"));
        }

        // Check for `true` / `false` keywords
        if self.input[self.pos..].starts_with("true") {
            let end = self.pos + 4;
            if end >= bytes.len() || !is_unquoted_char(bytes[end]) {
                self.pos = end;
                return Ok(NbtTag::Byte(1));
            }
        }
        if self.input[self.pos..].starts_with("false") {
            let end = self.pos + 5;
            if end >= bytes.len() || !is_unquoted_char(bytes[end]) {
                self.pos = end;
                return Ok(NbtTag::Byte(0));
            }
        }

        // Try number parse
        let first = bytes[start];
        if first == b'-' || first == b'+' || first == b'.' || first.is_ascii_digit() {
            if let Some(tag) = self.try_parse_number() {
                return Ok(tag);
            }
            // Reset if number parse failed — fall through to unquoted string
            self.pos = start;
        }

        // Unquoted string
        let s = self.parse_unquoted_string()?;
        if s.is_empty() {
            return Err(self.error("expected a value"));
        }
        Ok(NbtTag::String(s))
    }

    /// Attempts to parse a number with optional suffix. Returns `None` if
    /// the token is not a valid number (the position may have advanced).
    fn try_parse_number(&mut self) -> Option<NbtTag> {
        let start = self.pos;
        let bytes = self.input.as_bytes();

        // Consume optional sign
        if self.pos < bytes.len() && (bytes[self.pos] == b'-' || bytes[self.pos] == b'+') {
            self.pos += 1;
        }

        let mut has_digits = false;
        let mut has_dot = false;

        // Consume digits before decimal point
        while self.pos < bytes.len() && bytes[self.pos].is_ascii_digit() {
            has_digits = true;
            self.pos += 1;
        }

        // Decimal point
        if self.pos < bytes.len() && bytes[self.pos] == b'.' {
            has_dot = true;
            self.pos += 1;
            // Consume digits after decimal point
            while self.pos < bytes.len() && bytes[self.pos].is_ascii_digit() {
                has_digits = true;
                self.pos += 1;
            }
        }

        if !has_digits {
            self.pos = start;
            return None;
        }

        let number_end = self.pos;
        let number_str = &self.input[start..number_end];

        // Check for type suffix
        if self.pos < bytes.len() {
            let suffix = bytes[self.pos];
            match suffix {
                b'b' | b'B' => {
                    self.pos += 1;
                    let v: i8 = number_str.parse().ok()?;
                    return Some(NbtTag::Byte(v));
                },
                b's' | b'S' => {
                    self.pos += 1;
                    let v: i16 = number_str.parse().ok()?;
                    return Some(NbtTag::Short(v));
                },
                b'l' | b'L' => {
                    self.pos += 1;
                    let v: i64 = number_str.parse().ok()?;
                    return Some(NbtTag::Long(v));
                },
                b'f' | b'F' => {
                    self.pos += 1;
                    let v: f32 = number_str.parse().ok()?;
                    return Some(NbtTag::Float(v));
                },
                b'd' | b'D' => {
                    self.pos += 1;
                    let v: f64 = number_str.parse().ok()?;
                    return Some(NbtTag::Double(v));
                },
                _ => {},
            }
        }

        // No suffix
        if has_dot {
            let v: f64 = number_str.parse().ok()?;
            Some(NbtTag::Double(v))
        } else {
            let v: i32 = number_str.parse().ok()?;
            Some(NbtTag::Int(v))
        }
    }

    /// Parses a quoted string (double or single quotes), handling escape
    /// sequences `\"`, `\\`, `\'`.
    fn parse_quoted_string(&mut self, quote: u8) -> Result<String, NbtError> {
        let open_pos = self.pos;
        self.pos += 1; // skip opening quote
        let mut result = String::new();
        let bytes = self.input.as_bytes();

        loop {
            if self.pos >= bytes.len() {
                return Err(NbtError::SnbtParse {
                    pos: open_pos,
                    message: "unterminated string".to_owned(),
                });
            }

            let b = bytes[self.pos];
            if b == quote {
                self.pos += 1;
                return Ok(result);
            }

            if b == b'\\' {
                self.pos += 1;
                if self.pos >= bytes.len() {
                    return Err(NbtError::SnbtParse {
                        pos: open_pos,
                        message: "unterminated string escape".to_owned(),
                    });
                }
                let escaped = bytes[self.pos];
                match escaped {
                    b'\\' => result.push('\\'),
                    b'"' => result.push('"'),
                    b'\'' => result.push('\''),
                    b'n' => result.push('\n'),
                    b't' => result.push('\t'),
                    b'r' => result.push('\r'),
                    other => {
                        result.push('\\');
                        result.push(other as char);
                    },
                }
                self.pos += 1;
            } else {
                result.push(b as char);
                self.pos += 1;
            }
        }
    }

    /// Parses an unquoted string (alphanumeric, `_`, `-`, `.`, `+`).
    fn parse_unquoted_string(&mut self) -> Result<String, NbtError> {
        let start = self.pos;
        let bytes = self.input.as_bytes();
        while self.pos < bytes.len() && is_unquoted_char(bytes[self.pos]) {
            self.pos += 1;
        }
        if self.pos == start {
            return Err(self.error("expected an unquoted string"));
        }
        Ok(self.input[start..self.pos].to_owned())
    }
}

/// Returns `true` if `b` is allowed in an unquoted SNBT string.
fn is_unquoted_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.' || b == b'+'
}

// ── Compact formatter ───────────────────────────────────────────────────

fn write_snbt(out: &mut String, tag: &NbtTag, depth: usize) {
    // Depth protection: if we exceed MAX_DEPTH, emit a placeholder instead
    // of risking stack overflow. This mirrors the parser's depth limit.
    if depth > crate::error::MAX_DEPTH {
        out.push_str("<too deep>");
        return;
    }
    match tag {
        NbtTag::Byte(v) => {
            let _ = write!(out, "{v}b");
        },
        NbtTag::Short(v) => {
            let _ = write!(out, "{v}s");
        },
        NbtTag::Int(v) => {
            let _ = write!(out, "{v}");
        },
        NbtTag::Long(v) => {
            let _ = write!(out, "{v}L");
        },
        NbtTag::Float(v) => {
            write_float32(out, *v);
        },
        NbtTag::Double(v) => {
            write_float64(out, *v);
        },
        NbtTag::ByteArray(arr) => format_typed_array(out, "[B;", arr, "b"),
        NbtTag::String(s) => write_quoted_string(out, s),
        NbtTag::List(list) => {
            out.push('[');
            for (i, elem) in list.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_snbt(out, elem, depth + 1);
            }
            out.push(']');
        },
        NbtTag::Compound(compound) => {
            out.push('{');
            for (i, (key, value)) in compound.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_key(out, key);
                out.push(':');
                write_snbt(out, value, depth + 1);
            }
            out.push('}');
        },
        NbtTag::IntArray(arr) => format_typed_array(out, "[I;", arr, ""),
        NbtTag::LongArray(arr) => format_typed_array(out, "[L;", arr, "L"),
    }
}

/// Formats a typed NBT array (`[B;1b,2b]`, `[I;1,2]`, `[L;1L,2L]`).
fn format_typed_array<T: std::fmt::Display>(
    out: &mut String,
    prefix: &str,
    arr: &[T],
    suffix: &str,
) {
    out.push_str(prefix);
    for (i, v) in arr.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "{v}{suffix}");
    }
    out.push(']');
}

/// Writes a float with at least one decimal digit and the `f` suffix.
fn write_float32(out: &mut String, v: f32) {
    let s = format!("{v}");
    out.push_str(&s);
    if !s.contains('.') {
        out.push_str(".0");
    }
    out.push('f');
}

/// Writes a double with at least one decimal digit and the `d` suffix.
fn write_float64(out: &mut String, v: f64) {
    let s = format!("{v}");
    out.push_str(&s);
    if !s.contains('.') {
        out.push_str(".0");
    }
    out.push('d');
}

/// Writes a compound key, quoting it if it contains special characters.
fn write_key(out: &mut String, key: &str) {
    if key.is_empty() || !key.bytes().all(is_unquoted_char) {
        write_quoted_string(out, key);
    } else {
        out.push_str(key);
    }
}

/// Writes a quoted string with `"` delimiters, escaping `\` and `"`.
fn write_quoted_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(ch),
        }
    }
    out.push('"');
}

// ── Pretty formatter ────────────────────────────────────────────────────

fn write_snbt_pretty(out: &mut String, tag: &NbtTag, depth: usize, indent: usize) {
    match tag {
        NbtTag::Compound(compound) => {
            if compound.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push_str("{\n");
            let inner = depth + 1;
            for (i, (key, value)) in compound.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                push_indent(out, inner, indent);
                write_key(out, key);
                out.push_str(": ");
                write_snbt_pretty(out, value, inner, indent);
            }
            out.push('\n');
            push_indent(out, depth, indent);
            out.push('}');
        },
        NbtTag::List(list) if has_compounds(list) => {
            if list.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push_str("[\n");
            let inner = depth + 1;
            for (i, elem) in list.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                push_indent(out, inner, indent);
                write_snbt_pretty(out, elem, inner, indent);
            }
            out.push('\n');
            push_indent(out, depth, indent);
            out.push(']');
        },
        // Simple lists and all other types use compact format
        _ => write_snbt(out, tag, depth),
    }
}

/// Returns `true` if the list contains compound elements.
fn has_compounds(list: &NbtList) -> bool {
    use crate::error::TAG_COMPOUND;
    list.element_type() == TAG_COMPOUND
}

/// Pushes `depth * indent` spaces.
fn push_indent(out: &mut String, depth: usize, indent: usize) {
    for _ in 0..(depth * indent) {
        out.push(' ');
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    // ── Parse primitives ────────────────────────────────────────────────

    #[test]
    fn test_parse_byte() {
        assert_eq!(parse_snbt("42b").unwrap(), NbtTag::Byte(42));
        assert_eq!(parse_snbt("42B").unwrap(), NbtTag::Byte(42));
        assert_eq!(parse_snbt("-1b").unwrap(), NbtTag::Byte(-1));
    }

    #[test]
    fn test_parse_short() {
        assert_eq!(parse_snbt("42s").unwrap(), NbtTag::Short(42));
        assert_eq!(parse_snbt("42S").unwrap(), NbtTag::Short(42));
        assert_eq!(parse_snbt("-100s").unwrap(), NbtTag::Short(-100));
    }

    #[test]
    fn test_parse_int() {
        assert_eq!(parse_snbt("42").unwrap(), NbtTag::Int(42));
        assert_eq!(parse_snbt("-42").unwrap(), NbtTag::Int(-42));
        assert_eq!(parse_snbt("0").unwrap(), NbtTag::Int(0));
    }

    #[test]
    fn test_parse_long() {
        assert_eq!(parse_snbt("42L").unwrap(), NbtTag::Long(42));
        assert_eq!(parse_snbt("42l").unwrap(), NbtTag::Long(42));
        assert_eq!(
            parse_snbt("9999999999L").unwrap(),
            NbtTag::Long(9_999_999_999)
        );
    }

    #[test]
    fn test_parse_float() {
        assert_eq!(parse_snbt("42.0f").unwrap(), NbtTag::Float(42.0));
        assert_eq!(parse_snbt("42.0F").unwrap(), NbtTag::Float(42.0));
        assert_eq!(parse_snbt("-1.5f").unwrap(), NbtTag::Float(-1.5));
    }

    #[test]
    fn test_parse_double() {
        assert_eq!(parse_snbt("42.0d").unwrap(), NbtTag::Double(42.0));
        assert_eq!(parse_snbt("42.0D").unwrap(), NbtTag::Double(42.0));
        assert_eq!(parse_snbt("42.0").unwrap(), NbtTag::Double(42.0));
        assert_eq!(parse_snbt("-3.125").unwrap(), NbtTag::Double(-3.125));
    }

    #[test]
    fn test_parse_decimal_without_integer_part() {
        assert_eq!(parse_snbt(".5").unwrap(), NbtTag::Double(0.5));
        assert_eq!(parse_snbt(".5f").unwrap(), NbtTag::Float(0.5));
    }

    #[test]
    fn test_parse_true_false() {
        assert_eq!(parse_snbt("true").unwrap(), NbtTag::Byte(1));
        assert_eq!(parse_snbt("false").unwrap(), NbtTag::Byte(0));
    }

    #[test]
    fn test_parse_negative_number() {
        assert_eq!(parse_snbt("-42").unwrap(), NbtTag::Int(-42));
        assert_eq!(parse_snbt("-1.5d").unwrap(), NbtTag::Double(-1.5));
    }

    // ── Parse strings ───────────────────────────────────────────────────

    #[test]
    fn test_parse_double_quoted_string() {
        assert_eq!(
            parse_snbt("\"hello world\"").unwrap(),
            NbtTag::String("hello world".into())
        );
    }

    #[test]
    fn test_parse_single_quoted_string() {
        assert_eq!(
            parse_snbt("'hello world'").unwrap(),
            NbtTag::String("hello world".into())
        );
    }

    #[test]
    fn test_parse_quoted_string_with_escapes() {
        assert_eq!(
            parse_snbt(r#""line\"break""#).unwrap(),
            NbtTag::String("line\"break".into())
        );
        assert_eq!(
            parse_snbt(r#""back\\slash""#).unwrap(),
            NbtTag::String("back\\slash".into())
        );
    }

    #[test]
    fn test_parse_unquoted_string() {
        assert_eq!(parse_snbt("hello").unwrap(), NbtTag::String("hello".into()));
        assert_eq!(
            parse_snbt("my_key").unwrap(),
            NbtTag::String("my_key".into())
        );
    }

    // ── Parse compounds ─────────────────────────────────────────────────

    #[test]
    fn test_parse_empty_compound() {
        assert_eq!(
            parse_snbt("{}").unwrap(),
            NbtTag::Compound(NbtCompound::new())
        );
    }

    #[test]
    fn test_parse_compound() {
        let tag = parse_snbt("{key: 42, name: \"hello\"}").unwrap();
        let compound = tag.as_compound().unwrap();
        assert_eq!(compound.get_int("key"), Some(42));
        assert_eq!(compound.get_string("name"), Some("hello"));
    }

    #[test]
    fn test_parse_compound_quoted_keys() {
        let tag = parse_snbt("{\"key with spaces\": 1}").unwrap();
        let compound = tag.as_compound().unwrap();
        assert_eq!(compound.get_int("key with spaces"), Some(1));
    }

    // ── Parse lists ─────────────────────────────────────────────────────

    #[test]
    fn test_parse_empty_list() {
        let tag = parse_snbt("[]").unwrap();
        let list = tag.as_list().unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_parse_list() {
        let tag = parse_snbt("[1, 2, 3]").unwrap();
        let list = tag.as_list().unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list.get(0), Some(&NbtTag::Int(1)));
        assert_eq!(list.get(2), Some(&NbtTag::Int(3)));
    }

    // ── Parse typed arrays ──────────────────────────────────────────────

    #[test]
    fn test_parse_byte_array() {
        assert_eq!(
            parse_snbt("[B; 1b, 2b]").unwrap(),
            NbtTag::ByteArray(vec![1, 2])
        );
    }

    #[test]
    fn test_parse_int_array() {
        assert_eq!(
            parse_snbt("[I; 1, 2]").unwrap(),
            NbtTag::IntArray(vec![1, 2])
        );
    }

    #[test]
    fn test_parse_long_array() {
        assert_eq!(
            parse_snbt("[L; 1L, 2L]").unwrap(),
            NbtTag::LongArray(vec![1, 2])
        );
    }

    #[test]
    fn test_parse_empty_typed_arrays() {
        assert_eq!(parse_snbt("[B;]").unwrap(), NbtTag::ByteArray(vec![]));
        assert_eq!(parse_snbt("[I;]").unwrap(), NbtTag::IntArray(vec![]));
        assert_eq!(parse_snbt("[L;]").unwrap(), NbtTag::LongArray(vec![]));
    }

    // ── Parse nested ────────────────────────────────────────────────────

    #[test]
    fn test_parse_nested() {
        let tag = parse_snbt("{pos: {x: 1, y: 2}, items: [1, 2]}").unwrap();
        let compound = tag.as_compound().unwrap();
        let pos = compound.get_compound("pos").unwrap();
        assert_eq!(pos.get_int("x"), Some(1));
        assert_eq!(pos.get_int("y"), Some(2));
        let items = compound.get_list("items").unwrap();
        assert_eq!(items.len(), 2);
    }

    // ── Format ──────────────────────────────────────────────────────────

    #[test]
    fn test_format_byte() {
        assert_eq!(format_snbt(&NbtTag::Byte(42)), "42b");
        assert_eq!(format_snbt(&NbtTag::Byte(0)), "0b");
        assert_eq!(format_snbt(&NbtTag::Byte(1)), "1b");
    }

    #[test]
    fn test_format_short() {
        assert_eq!(format_snbt(&NbtTag::Short(42)), "42s");
    }

    #[test]
    fn test_format_int() {
        assert_eq!(format_snbt(&NbtTag::Int(42)), "42");
    }

    #[test]
    fn test_format_long() {
        assert_eq!(format_snbt(&NbtTag::Long(42)), "42L");
    }

    #[test]
    fn test_format_float() {
        assert_eq!(format_snbt(&NbtTag::Float(42.0)), "42.0f");
        assert_eq!(format_snbt(&NbtTag::Float(1.5)), "1.5f");
    }

    #[test]
    fn test_format_double() {
        assert_eq!(format_snbt(&NbtTag::Double(42.0)), "42.0d");
        assert_eq!(format_snbt(&NbtTag::Double(1.5)), "1.5d");
    }

    #[test]
    fn test_format_string() {
        assert_eq!(format_snbt(&NbtTag::String("hello".into())), "\"hello\"");
        assert_eq!(
            format_snbt(&NbtTag::String("say \"hi\"".into())),
            "\"say \\\"hi\\\"\""
        );
    }

    #[test]
    fn test_format_byte_array() {
        assert_eq!(
            format_snbt(&NbtTag::ByteArray(vec![1, 2, 3])),
            "[B;1b,2b,3b]"
        );
    }

    #[test]
    fn test_format_int_array() {
        assert_eq!(format_snbt(&NbtTag::IntArray(vec![1, 2])), "[I;1,2]");
    }

    #[test]
    fn test_format_long_array() {
        assert_eq!(format_snbt(&NbtTag::LongArray(vec![1, 2])), "[L;1L,2L]");
    }

    #[test]
    fn test_format_list() {
        let mut list = NbtList::empty();
        list.push(NbtTag::Int(1)).unwrap();
        list.push(NbtTag::Int(2)).unwrap();
        assert_eq!(format_snbt(&NbtTag::List(list)), "[1,2]");
    }

    #[test]
    fn test_format_compound() {
        let mut c = NbtCompound::new();
        c.put_int("x", 1);
        c.put_string("name", "hi");
        assert_eq!(format_snbt(&NbtTag::Compound(c)), "{x:1,name:\"hi\"}");
    }

    #[test]
    fn test_format_compound_quoted_key() {
        let mut c = NbtCompound::new();
        c.put_int("key with spaces", 1);
        let s = format_snbt(&NbtTag::Compound(c));
        assert!(s.starts_with("{\"key with spaces\":"));
    }

    // ── Roundtrip ───────────────────────────────────────────────────────

    #[test]
    fn test_roundtrip_primitives() {
        let cases = [
            NbtTag::Byte(42),
            NbtTag::Short(-100),
            NbtTag::Int(999),
            NbtTag::Long(123_456_789),
            NbtTag::Float(1.5),
            NbtTag::Double(3.125),
            NbtTag::String("hello".into()),
        ];
        for tag in &cases {
            let formatted = format_snbt(tag);
            let parsed = parse_snbt(&formatted).unwrap();
            assert_eq!(&parsed, tag, "roundtrip failed for {formatted}");
        }
    }

    #[test]
    fn test_roundtrip_compound() {
        let mut c = NbtCompound::new();
        c.put_int("x", 1);
        c.put_float("y", 2.5);
        c.put_string("name", "test");
        let tag = NbtTag::Compound(c);

        let formatted = format_snbt(&tag);
        let parsed = parse_snbt(&formatted).unwrap();
        assert_eq!(parsed, tag);
    }

    #[test]
    fn test_roundtrip_typed_arrays() {
        let cases = [
            NbtTag::ByteArray(vec![1, 2, 3]),
            NbtTag::IntArray(vec![10, 20]),
            NbtTag::LongArray(vec![100, 200]),
        ];
        for tag in &cases {
            let formatted = format_snbt(tag);
            let parsed = parse_snbt(&formatted).unwrap();
            assert_eq!(&parsed, tag, "roundtrip failed for {formatted}");
        }
    }

    // ── Pretty format ───────────────────────────────────────────────────

    #[test]
    fn test_pretty_format_compound() {
        let mut c = NbtCompound::new();
        c.put_int("x", 1);
        c.put_int("y", 2);
        let s = format_snbt_pretty(&NbtTag::Compound(c), 2);
        assert!(s.contains('\n'));
        assert!(s.contains("  x: 1"));
        assert!(s.contains("  y: 2"));
    }

    #[test]
    fn test_pretty_format_empty_compound() {
        assert_eq!(
            format_snbt_pretty(&NbtTag::Compound(NbtCompound::new()), 2),
            "{}"
        );
    }

    #[test]
    fn test_pretty_format_simple_values() {
        assert_eq!(format_snbt_pretty(&NbtTag::Int(42), 2), "42");
        assert_eq!(format_snbt_pretty(&NbtTag::Byte(1), 2), "1b");
    }

    // ── Error cases ─────────────────────────────────────────────────────

    #[test]
    fn test_error_unterminated_string() {
        assert!(parse_snbt("\"hello").is_err());
    }

    #[test]
    fn test_error_missing_closing_brace() {
        assert!(parse_snbt("{key: 42").is_err());
    }

    #[test]
    fn test_error_missing_closing_bracket() {
        assert!(parse_snbt("[1, 2").is_err());
    }

    #[test]
    fn test_error_trailing_characters() {
        assert!(parse_snbt("42 extra").is_err());
    }

    #[test]
    fn test_parse_whitespace_handling() {
        let tag = parse_snbt("  { key : 42 , name : \"hi\" }  ").unwrap();
        let compound = tag.as_compound().unwrap();
        assert_eq!(compound.get_int("key"), Some(42));
        assert_eq!(compound.get_string("name"), Some("hi"));
    }

    #[test]
    fn test_parse_depth_limit() {
        // Build an SNBT string with nesting deeper than MAX_DEPTH.
        let depth = crate::error::MAX_DEPTH + 10;
        let mut input = String::new();
        for _ in 0..depth {
            input.push_str("{x:");
        }
        input.push_str("1b");
        for _ in 0..depth {
            input.push('}');
        }
        let result = parse_snbt(&input);
        assert!(result.is_err(), "should reject deeply nested SNBT compound");
    }

    #[test]
    fn test_parse_list_depth_limit() {
        let depth = crate::error::MAX_DEPTH + 10;
        let mut input = String::new();
        for _ in 0..depth {
            input.push('[');
        }
        input.push_str("1b");
        for _ in 0..depth {
            input.push(']');
        }
        let result = parse_snbt(&input);
        assert!(result.is_err(), "should reject deeply nested SNBT list");
    }
}
