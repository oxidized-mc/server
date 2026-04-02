//! Minecraft text component data types, builder API, and legacy conversion.
//!
//! A [`Component`] is a tree of styled text nodes that the client renders
//! with inheritance-based style resolution.
//!
//! Serialization lives in sibling modules:
//! - [`super::component_json`] — JSON (status/config packets)
//! - [`super::component_nbt`] — NBT (play-state packets)
//!
//! See ADR-028 (Chat Components) for design rationale.

use std::fmt;

use oxidized_nbt::{NbtCompound, NbtList, NbtTag};
use serde::Serialize;
use serde::ser::SerializeMap;

use super::click_event::ClickEvent;
use super::formatting::ChatFormatting;
use super::hover_event::HoverEvent;
use super::style::Style;
use super::text_color::TextColor;
use crate::types::ResourceLocation;

/// A Minecraft text component.
///
/// Components form a tree: each component has content, an optional style,
/// and zero or more children. The client renders children after the parent,
/// inheriting unset style fields.
///
/// # Examples
///
/// ```
/// use oxidized_protocol::chat::{Component, Style, TextColor, ChatFormatting};
///
/// // Simple text
/// let msg = Component::text("Hello!");
///
/// // Styled text
/// let msg = Component::text("Warning!")
///     .bold()
///     .color(TextColor::Named(ChatFormatting::Red));
///
/// // Nested
/// let msg = Component::text("Player ")
///     .append(Component::text("Steve").bold())
///     .append(Component::text(" joined the game"));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    /// The content of this component node.
    pub content: ComponentContent,
    /// Style applied to this node (fields set to `None` inherit from parent).
    pub style: Style,
    /// Child components rendered after this node.
    pub children: Vec<Component>,
}

/// The content type of a component node.
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentContent {
    /// Plain text.
    Text(String),
    /// Translatable text with format arguments.
    Translatable {
        /// Translation key (e.g. `"chat.type.text"`).
        key: String,
        /// Fallback text if translation key is unknown.
        fallback: Option<String>,
        /// Arguments substituted into `%s` / `%1$s` placeholders.
        args: Vec<Component>,
    },
    /// Entity selector (e.g. `"@a[distance=..5]"`).
    Selector {
        /// The selector pattern.
        pattern: String,
        /// Optional separator between matched entities.
        separator: Option<Box<Component>>,
    },
    /// Scoreboard value.
    Score {
        /// Entity selector or player name.
        name: String,
        /// Scoreboard objective name.
        objective: String,
    },
    /// Client keybinding (e.g. `"key.jump"`).
    Keybind(String),
    /// NBT value from an entity, block, or storage.
    Nbt {
        /// NBT path expression.
        path: String,
        /// Whether to parse the result as a component.
        interpret: bool,
        /// Optional separator between multiple values.
        separator: Option<Box<Component>>,
        /// Source of the NBT data.
        source: NbtSource,
    },
}

/// Source for NBT component content.
#[derive(Debug, Clone, PartialEq)]
pub enum NbtSource {
    /// NBT from an entity (selector string).
    Entity(String),
    /// NBT from a block entity (position string).
    Block(String),
    /// NBT from command storage.
    Storage(ResourceLocation),
}

impl ComponentContent {
    /// Count additional JSON fields beyond the primary content field.
    pub(crate) fn count_extra_json_fields(&self) -> usize {
        match self {
            Self::Text(_) => 0,
            Self::Translatable { fallback, args, .. } => {
                let mut n = 0;
                if fallback.is_some() {
                    n += 1;
                }
                if !args.is_empty() {
                    n += 1;
                }
                n
            },
            Self::Selector { separator, .. } => usize::from(separator.is_some()),
            Self::Score { .. } | Self::Keybind(_) => 0,
            Self::Nbt {
                interpret,
                separator,
                ..
            } => {
                let mut n = 1; // source field
                if *interpret {
                    n += 1;
                }
                if separator.is_some() {
                    n += 1;
                }
                n
            },
        }
    }

    /// Write content-specific fields into a JSON serialize map.
    pub(crate) fn write_json_fields<S: SerializeMap>(&self, map: &mut S) -> Result<(), S::Error> {
        match self {
            Self::Text(t) => {
                map.serialize_entry("text", t)?;
            },
            Self::Translatable {
                key,
                fallback,
                args,
            } => {
                map.serialize_entry("translate", key)?;
                if let Some(fb) = fallback {
                    map.serialize_entry("fallback", fb)?;
                }
                if !args.is_empty() {
                    map.serialize_entry("with", args)?;
                }
            },
            Self::Selector { pattern, separator } => {
                map.serialize_entry("selector", pattern)?;
                if let Some(sep) = separator {
                    map.serialize_entry("separator", sep)?;
                }
            },
            Self::Score { name, objective } => {
                #[derive(Serialize)]
                struct ScoreValue<'a> {
                    name: &'a str,
                    objective: &'a str,
                }
                map.serialize_entry("score", &ScoreValue { name, objective })?;
            },
            Self::Keybind(k) => {
                map.serialize_entry("keybind", k)?;
            },
            Self::Nbt {
                path,
                interpret,
                separator,
                source,
            } => {
                map.serialize_entry("nbt", path)?;
                if *interpret {
                    map.serialize_entry("interpret", &true)?;
                }
                if let Some(sep) = separator {
                    map.serialize_entry("separator", sep)?;
                }
                match source {
                    NbtSource::Entity(sel) => map.serialize_entry("entity", sel)?,
                    NbtSource::Block(pos) => map.serialize_entry("block", pos)?,
                    NbtSource::Storage(rl) => {
                        map.serialize_entry("storage", &rl.to_string())?;
                    },
                }
            },
        }
        Ok(())
    }

    /// Write content-specific fields into an NBT compound.
    pub(crate) fn write_nbt_fields(&self, compound: &mut NbtCompound) {
        match self {
            Self::Text(t) => {
                compound.put_string("text", t);
            },
            Self::Translatable {
                key,
                fallback,
                args,
            } => {
                compound.put_string("translate", key);
                if let Some(fb) = fallback {
                    compound.put_string("fallback", fb);
                }
                if !args.is_empty() {
                    let mut list = NbtList::new(10);
                    for arg in args {
                        let _ = list.push(arg.to_nbt());
                    }
                    compound.put("with", NbtTag::List(list));
                }
            },
            Self::Selector { pattern, separator } => {
                compound.put_string("selector", pattern);
                if let Some(sep) = separator {
                    compound.put("separator", sep.to_nbt());
                }
            },
            Self::Score { name, objective } => {
                let mut score = NbtCompound::new();
                score.put_string("name", name);
                score.put_string("objective", objective);
                compound.put("score", NbtTag::Compound(score));
            },
            Self::Keybind(k) => {
                compound.put_string("keybind", k);
            },
            Self::Nbt {
                path,
                interpret,
                separator,
                source,
            } => {
                compound.put_string("nbt", path);
                if *interpret {
                    compound.put_byte("interpret", 1);
                }
                if let Some(sep) = separator {
                    compound.put("separator", sep.to_nbt());
                }
                match source {
                    NbtSource::Entity(sel) => {
                        compound.put_string("entity", sel);
                    },
                    NbtSource::Block(pos) => {
                        compound.put_string("block", pos);
                    },
                    NbtSource::Storage(rl) => {
                        compound.put_string("storage", rl.to_string());
                    },
                }
            },
        }
    }
}

// ── Builder API ──────────────────────────────────────────────────────

impl Component {
    /// Create a plain text component.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidized_protocol::chat::Component;
    /// let c = Component::text("hello");
    /// assert_eq!(c.to_json().unwrap(), r#"{"text":"hello"}"#);
    /// ```
    pub fn text(s: impl Into<String>) -> Self {
        Self {
            content: ComponentContent::Text(s.into()),
            style: Style::default(),
            children: Vec::new(),
        }
    }

    /// Create a translatable component.
    pub fn translatable(key: impl Into<String>, args: Vec<Component>) -> Self {
        Self {
            content: ComponentContent::Translatable {
                key: key.into(),
                fallback: None,
                args,
            },
            style: Style::default(),
            children: Vec::new(),
        }
    }

    /// Create a translatable component with a fallback string.
    pub fn translatable_with_fallback(
        key: impl Into<String>,
        fallback: impl Into<String>,
        args: Vec<Component>,
    ) -> Self {
        Self {
            content: ComponentContent::Translatable {
                key: key.into(),
                fallback: Some(fallback.into()),
                args,
            },
            style: Style::default(),
            children: Vec::new(),
        }
    }

    /// Create a keybind component.
    pub fn keybind(key: impl Into<String>) -> Self {
        Self {
            content: ComponentContent::Keybind(key.into()),
            style: Style::default(),
            children: Vec::new(),
        }
    }

    /// Create a score component.
    pub fn score(name: impl Into<String>, objective: impl Into<String>) -> Self {
        Self {
            content: ComponentContent::Score {
                name: name.into(),
                objective: objective.into(),
            },
            style: Style::default(),
            children: Vec::new(),
        }
    }

    /// Create a selector component.
    pub fn selector(pattern: impl Into<String>) -> Self {
        Self {
            content: ComponentContent::Selector {
                pattern: pattern.into(),
                separator: None,
            },
            style: Style::default(),
            children: Vec::new(),
        }
    }

    /// Create an empty text component (used as a container for children).
    pub fn empty() -> Self {
        Self::text("")
    }

    /// Set the style of this component.
    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Append a child component.
    pub fn append(mut self, child: Component) -> Self {
        self.children.push(child);
        self
    }

    /// Set the text color.
    pub fn color(mut self, color: TextColor) -> Self {
        self.style.color = Some(color);
        self
    }

    /// Set bold formatting.
    pub fn bold(mut self) -> Self {
        self.style.bold = Some(true);
        self
    }

    /// Set italic formatting.
    pub fn italic(mut self) -> Self {
        self.style.italic = Some(true);
        self
    }

    /// Set underlined formatting.
    pub fn underlined(mut self) -> Self {
        self.style.underlined = Some(true);
        self
    }

    /// Set strikethrough formatting.
    pub fn strikethrough(mut self) -> Self {
        self.style.strikethrough = Some(true);
        self
    }

    /// Set obfuscated formatting.
    pub fn obfuscated(mut self) -> Self {
        self.style.obfuscated = Some(true);
        self
    }

    /// Set a click event.
    pub fn click(mut self, event: ClickEvent) -> Self {
        self.style.click_event = Some(event);
        self
    }

    /// Set a hover event.
    pub fn hover(mut self, event: HoverEvent) -> Self {
        self.style.hover_event = Some(event);
        self
    }

    /// Serialize to JSON string.
    ///
    /// # Errors
    ///
    /// Returns `serde_json::Error` if serialization fails (should not happen
    /// for well-formed components).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parse a legacy §-code string into a component tree.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidized_protocol::chat::Component;
    /// let c = Component::from_legacy("§cRed text");
    /// let json = c.to_json().unwrap();
    /// assert!(json.contains("red"), "got: {json}");
    /// ```
    pub fn from_legacy(s: &str) -> Self {
        Self::from_legacy_with_char(s, '\u{00A7}')
    }

    /// Parse a legacy color-coded string into a component tree, recognizing
    /// both the standard `§` character and a custom `color_char` prefix.
    ///
    /// This is the shared color parsing logic used for MOTD, chat messages,
    /// tab list, scoreboard, bossbar, name tags, and anywhere else color
    /// codes need to be resolved.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidized_protocol::chat::Component;
    /// // Using '&' as the color prefix
    /// let c = Component::from_legacy_with_char("&cRed &lBold", '&');
    /// let json = c.to_json().unwrap();
    /// assert!(json.contains("red"), "got: {json}");
    /// ```
    pub fn from_legacy_with_char(s: &str, color_char: char) -> Self {
        let mut parser = LegacyParser::new();
        let mut chars = s.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '\u{00A7}' || ch == color_char {
                if let Some(&code_char) = chars.peek() {
                    if let Some(fmt) = ChatFormatting::from_code(code_char) {
                        parser.apply_format(fmt);
                        chars.next(); // consume code char
                        continue;
                    }
                }
            }
            parser.push_char(ch);
        }

        parser.finish()
    }

    /// Convert to a lossy legacy §-code string.
    ///
    /// Drops interactive elements (click/hover events), translations,
    /// selectors, and other non-text content.
    pub fn to_legacy_string(&self) -> String {
        let mut out = String::new();
        self.append_legacy(&mut out);
        out
    }

    fn append_legacy(&self, out: &mut String) {
        // Apply style
        if let Some(TextColor::Named(cf)) = &self.style.color {
            out.push('\u{00A7}');
            out.push(cf.code());
        }
        if self.style.bold == Some(true) {
            out.push_str(&ChatFormatting::Bold.prefix());
        }
        if self.style.italic == Some(true) {
            out.push_str(&ChatFormatting::Italic.prefix());
        }
        if self.style.underlined == Some(true) {
            out.push_str(&ChatFormatting::Underline.prefix());
        }
        if self.style.strikethrough == Some(true) {
            out.push_str(&ChatFormatting::Strikethrough.prefix());
        }
        if self.style.obfuscated == Some(true) {
            out.push_str(&ChatFormatting::Obfuscated.prefix());
        }

        // Emit text content
        match &self.content {
            ComponentContent::Text(t) => out.push_str(t),
            ComponentContent::Translatable { key, .. } => out.push_str(key),
            ComponentContent::Keybind(k) => out.push_str(k),
            ComponentContent::Score { name, objective } => {
                out.push_str(name);
                out.push(':');
                out.push_str(objective);
            },
            ComponentContent::Selector { pattern, .. } => out.push_str(pattern),
            ComponentContent::Nbt { path, .. } => out.push_str(path),
        }

        // Recurse into children
        for child in &self.children {
            child.append_legacy(out);
        }
    }
}

impl fmt::Display for Component {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Plain text extraction (no formatting)
        match &self.content {
            ComponentContent::Text(t) => f.write_str(t)?,
            ComponentContent::Translatable { key, .. } => f.write_str(key)?,
            ComponentContent::Keybind(k) => f.write_str(k)?,
            ComponentContent::Score { name, .. } => f.write_str(name)?,
            ComponentContent::Selector { pattern, .. } => f.write_str(pattern)?,
            ComponentContent::Nbt { path, .. } => f.write_str(path)?,
        }
        for child in &self.children {
            write!(f, "{child}")?;
        }
        Ok(())
    }
}

impl From<&str> for Component {
    fn from(s: &str) -> Self {
        Self::text(s)
    }
}

impl From<String> for Component {
    fn from(s: String) -> Self {
        Self::text(s)
    }
}

// ── Legacy §-code parser ─────────────────────────────────────────────

/// Incremental parser for legacy §-code formatted strings.
///
/// Accumulates text and style state, flushing styled segments as children
/// of a root component each time a formatting code is encountered.
struct LegacyParser {
    root: Component,
    current_text: String,
    current_style: Style,
}

impl LegacyParser {
    fn new() -> Self {
        Self {
            root: Component::text(""),
            current_text: String::new(),
            current_style: Style::default(),
        }
    }

    /// Flush accumulated text as a styled child, then apply the format code.
    fn apply_format(&mut self, fmt: ChatFormatting) {
        self.flush();
        if fmt == ChatFormatting::Reset {
            self.current_style = Style::default();
        } else if fmt.is_color() {
            // Color resets formatting (vanilla behavior)
            self.current_style = Style {
                color: Some(TextColor::Named(fmt)),
                ..Default::default()
            };
        } else {
            match fmt {
                ChatFormatting::Bold => self.current_style.bold = Some(true),
                ChatFormatting::Italic => self.current_style.italic = Some(true),
                ChatFormatting::Underline => self.current_style.underlined = Some(true),
                ChatFormatting::Strikethrough => self.current_style.strikethrough = Some(true),
                ChatFormatting::Obfuscated => self.current_style.obfuscated = Some(true),
                _ => {},
            }
        }
    }

    fn push_char(&mut self, ch: char) {
        self.current_text.push(ch);
    }

    /// Flush accumulated text into a styled child component.
    fn flush(&mut self) {
        if !self.current_text.is_empty() {
            let child = Component::text(std::mem::take(&mut self.current_text))
                .with_style(self.current_style.clone());
            self.root.children.push(child);
        }
    }

    /// Consume the parser and return the final component tree.
    fn finish(mut self) -> Component {
        self.flush();

        // Simplify: if root is empty text with exactly one child, unwrap it
        if self.root.children.len() == 1 {
            if let ComponentContent::Text(ref t) = self.root.content {
                if t.is_empty() && self.root.style.is_empty() {
                    return self.root.children.remove(0);
                }
            }
        }

        self.root
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    // ── Legacy §-code conversion ─────────────────────────────────────

    #[test]
    fn test_from_legacy_simple_color() {
        let c = Component::from_legacy("§cRed text");
        assert_eq!(c.style.color, Some(TextColor::Named(ChatFormatting::Red)));
        if let ComponentContent::Text(t) = &c.content {
            assert_eq!(t, "Red text");
        } else {
            panic!("expected text content");
        }
    }

    #[test]
    fn test_from_legacy_multiple_codes() {
        let c = Component::from_legacy("§cRed §lBold");
        // Should produce children with different styles
        assert!(!c.children.is_empty() || c.to_json().unwrap().contains("red"));
    }

    #[test]
    fn test_from_legacy_reset() {
        let c = Component::from_legacy("§c§lBold Red§r Normal");
        let legacy = c.to_legacy_string();
        assert!(legacy.contains("Bold Red"));
        assert!(legacy.contains("Normal"));
    }

    #[test]
    fn test_to_legacy_string() {
        let c = Component::text("Hello").color(TextColor::Named(ChatFormatting::Red));
        let legacy = c.to_legacy_string();
        assert!(legacy.contains("§c"), "got: {legacy}");
        assert!(legacy.contains("Hello"), "got: {legacy}");
    }

    // ── Display ──────────────────────────────────────────────────────

    #[test]
    fn test_display_extracts_plain_text() {
        let c = Component::text("Hello ").append(Component::text("World"));
        assert_eq!(format!("{c}"), "Hello World");
    }

    // ── From impls ───────────────────────────────────────────────────

    #[test]
    fn test_from_str() {
        let c: Component = "hello".into();
        assert_eq!(c.content, ComponentContent::Text("hello".into()));
    }

    // ── from_legacy_with_char ────────────────────────────────────────

    #[test]
    fn test_from_legacy_with_char_ampersand() {
        let c = Component::from_legacy_with_char("&cRed text", '&');
        let json = c.to_json().unwrap();
        assert!(json.contains("red"), "got: {json}");
        assert!(json.contains("Red text"), "got: {json}");
    }

    #[test]
    fn test_from_legacy_with_char_section_sign_still_works() {
        let c = Component::from_legacy_with_char("§cRed text", '&');
        let json = c.to_json().unwrap();
        assert!(json.contains("red"), "got: {json}");
    }

    #[test]
    fn test_from_legacy_with_char_mixed_prefixes() {
        let c = Component::from_legacy_with_char("§cRed &lBold", '&');
        let json = c.to_json().unwrap();
        assert!(json.contains("red"), "got: {json}");
        assert!(json.contains("bold"), "got: {json}");
    }

    #[test]
    fn test_from_legacy_with_char_bold_and_color() {
        let c = Component::from_legacy_with_char("&6&lGold Bold", '&');
        let json = c.to_json().unwrap();
        assert!(json.contains("gold"), "got: {json}");
        assert!(json.contains("bold"), "got: {json}");
    }

    #[test]
    fn test_from_legacy_with_char_reset() {
        let c = Component::from_legacy_with_char("&cRed &rNormal", '&');
        assert!(c.children.len() >= 2, "expected multiple children");
    }

    #[test]
    fn test_from_legacy_with_char_no_codes() {
        let c = Component::from_legacy_with_char("plain text", '&');
        assert_eq!(format!("{c}"), "plain text");
    }

    #[test]
    fn test_from_legacy_with_char_hash_prefix() {
        let c = Component::from_legacy_with_char("#cRed text", '#');
        let json = c.to_json().unwrap();
        assert!(json.contains("red"), "got: {json}");
    }

    #[test]
    fn test_from_legacy_with_char_trailing_prefix() {
        // Color char at end of string with no code after it
        let c = Component::from_legacy_with_char("text&", '&');
        assert_eq!(format!("{c}"), "text&");
    }

    #[test]
    fn test_from_legacy_with_char_invalid_code() {
        // &z is not a valid code — should be kept as literal
        let c = Component::from_legacy_with_char("&ztext", '&');
        assert_eq!(format!("{c}"), "&ztext");
    }
}
