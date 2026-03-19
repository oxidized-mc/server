//! Minecraft text component system.
//!
//! A [`Component`] is a tree of styled text nodes that the client renders
//! with inheritance-based style resolution. This module provides construction
//! (builder API), JSON serialization (for status/config), and NBT wire
//! encoding (for play-state packets).
//!
//! See [ADR-028](../../../docs/adr/adr-028-chat-components.md) for design rationale.

use std::fmt;

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::formatting::ChatFormatting;
use super::style::{ClickEvent, HoverEvent, Style, TextColor};
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

// ── Builder API ──────────────────────────────────────────────────────

impl Component {
    /// Create a plain text component.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidized_protocol::chat::Component;
    /// let c = Component::text("hello");
    /// assert_eq!(c.to_json(), r#"{"text":"hello"}"#);
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
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("component JSON serialization is infallible")
    }

    /// Parse a legacy §-code string into a component tree.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidized_protocol::chat::Component;
    /// let c = Component::from_legacy("§cRed text");
    /// let json = c.to_json();
    /// assert!(json.contains("red"), "got: {json}");
    /// ```
    pub fn from_legacy(s: &str) -> Self {
        let mut root = Component::text("");
        let mut current_text = String::new();
        let mut current_style = Style::default();
        let mut chars = s.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '\u{00A7}' {
                if let Some(&code_char) = chars.peek() {
                    if let Some(fmt) = ChatFormatting::from_code(code_char) {
                        // Flush current text
                        if !current_text.is_empty() {
                            let child =
                                Component::text(std::mem::take(&mut current_text))
                                    .with_style(current_style.clone());
                            root.children.push(child);
                        }

                        if fmt == ChatFormatting::Reset {
                            current_style = Style::default();
                        } else if fmt.is_color() {
                            // Color resets formatting
                            current_style = Style {
                                color: Some(TextColor::Named(fmt)),
                                ..Default::default()
                            };
                        } else {
                            // Apply formatting modifier
                            match fmt {
                                ChatFormatting::Bold => current_style.bold = Some(true),
                                ChatFormatting::Italic => current_style.italic = Some(true),
                                ChatFormatting::Underline => {
                                    current_style.underlined = Some(true);
                                }
                                ChatFormatting::Strikethrough => {
                                    current_style.strikethrough = Some(true);
                                }
                                ChatFormatting::Obfuscated => {
                                    current_style.obfuscated = Some(true);
                                }
                                _ => {}
                            }
                        }
                        chars.next(); // consume code char
                        continue;
                    }
                }
            }
            current_text.push(ch);
        }

        // Flush remaining text
        if !current_text.is_empty() {
            let child = Component::text(current_text).with_style(current_style);
            root.children.push(child);
        }

        // Simplify: if root has no content and exactly one child, return child
        if root.children.len() == 1 {
            if let ComponentContent::Text(ref t) = root.content {
                if t.is_empty() && root.style.is_empty() {
                    return root.children.into_iter().next().expect("checked len==1");
                }
            }
        }

        root
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
        if let Some(ref color) = self.style.color {
            if let TextColor::Named(cf) = color {
                out.push('\u{00A7}');
                out.push(cf.code());
            }
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
            }
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

// ── JSON Serialization (manual per ADR-028) ──────────────────────────

impl Serialize for Component {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        // Simple text with no style and no children → {"text":"..."}
        let has_style = !self.style.is_empty();
        let has_children = !self.children.is_empty();

        // Count fields: content-specific + style fields + extra
        let mut count = 1; // content type field
        if has_style {
            count += count_style_fields(&self.style);
        }
        if has_children {
            count += 1; // "extra"
        }
        // Add content-specific extra fields
        count += count_content_extra_fields(&self.content);

        let mut map = s.serialize_map(Some(count))?;

        // Content-type-specific fields
        match &self.content {
            ComponentContent::Text(t) => {
                map.serialize_entry("text", t)?;
            }
            ComponentContent::Translatable {
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
            }
            ComponentContent::Selector { pattern, separator } => {
                map.serialize_entry("selector", pattern)?;
                if let Some(sep) = separator {
                    map.serialize_entry("separator", sep)?;
                }
            }
            ComponentContent::Score { name, objective } => {
                #[derive(Serialize)]
                struct ScoreValue<'a> {
                    name: &'a str,
                    objective: &'a str,
                }
                map.serialize_entry("score", &ScoreValue { name, objective })?;
            }
            ComponentContent::Keybind(k) => {
                map.serialize_entry("keybind", k)?;
            }
            ComponentContent::Nbt {
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
                    }
                }
            }
        }

        // Style fields (flattened into the same map)
        if has_style {
            serialize_style_fields(&self.style, &mut map)?;
        }

        // Children
        if has_children {
            map.serialize_entry("extra", &self.children)?;
        }

        map.end()
    }
}

fn count_style_fields(style: &Style) -> usize {
    let mut n = 0;
    if style.color.is_some() {
        n += 1;
    }
    if style.bold.is_some() {
        n += 1;
    }
    if style.italic.is_some() {
        n += 1;
    }
    if style.underlined.is_some() {
        n += 1;
    }
    if style.strikethrough.is_some() {
        n += 1;
    }
    if style.obfuscated.is_some() {
        n += 1;
    }
    if style.insertion.is_some() {
        n += 1;
    }
    if style.click_event.is_some() {
        n += 1;
    }
    if style.hover_event.is_some() {
        n += 1;
    }
    if style.font.is_some() {
        n += 1;
    }
    n
}

fn count_content_extra_fields(content: &ComponentContent) -> usize {
    match content {
        ComponentContent::Text(_) => 0,
        ComponentContent::Translatable {
            fallback, args, ..
        } => {
            let mut n = 0;
            if fallback.is_some() {
                n += 1;
            }
            if !args.is_empty() {
                n += 1;
            }
            n
        }
        ComponentContent::Selector { separator, .. } => {
            if separator.is_some() {
                1
            } else {
                0
            }
        }
        ComponentContent::Score { .. } => 0,
        ComponentContent::Keybind(_) => 0,
        ComponentContent::Nbt {
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
        }
    }
}

fn serialize_style_fields<S: SerializeMap>(
    style: &Style,
    map: &mut S,
) -> Result<(), S::Error> {
    if let Some(ref c) = style.color {
        map.serialize_entry("color", c)?;
    }
    if let Some(b) = style.bold {
        map.serialize_entry("bold", &b)?;
    }
    if let Some(i) = style.italic {
        map.serialize_entry("italic", &i)?;
    }
    if let Some(u) = style.underlined {
        map.serialize_entry("underlined", &u)?;
    }
    if let Some(st) = style.strikethrough {
        map.serialize_entry("strikethrough", &st)?;
    }
    if let Some(o) = style.obfuscated {
        map.serialize_entry("obfuscated", &o)?;
    }
    if let Some(ref ins) = style.insertion {
        map.serialize_entry("insertion", ins)?;
    }
    if let Some(ref ce) = style.click_event {
        map.serialize_entry("clickEvent", ce)?;
    }
    if let Some(ref he) = style.hover_event {
        map.serialize_entry("hoverEvent", he)?;
    }
    if let Some(ref f) = style.font {
        map.serialize_entry("font", &f.to_string())?;
    }
    Ok(())
}

impl<'de> Deserialize<'de> for Component {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ComponentVisitor;

        impl<'de> Visitor<'de> for ComponentVisitor {
            type Value = Component;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a component (string, array, or object)")
            }

            // Plain string → text component
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Component, E> {
                Ok(Component::text(v))
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<Component, E> {
                Ok(Component::text(v))
            }

            // Array → first element with rest as children
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Component, A::Error> {
                let first: Component = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::custom("empty component array"))?;
                let mut component = first;
                while let Some(child) = seq.next_element()? {
                    component.children.push(child);
                }
                Ok(component)
            }

            // Object → full component
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Component, A::Error> {
                let mut text: Option<String> = None;
                let mut translate: Option<String> = None;
                let mut fallback: Option<String> = None;
                let mut with: Option<Vec<Component>> = None;
                let mut selector: Option<String> = None;
                let mut score_name: Option<String> = None;
                let mut score_objective: Option<String> = None;
                let mut keybind: Option<String> = None;
                let mut nbt_path: Option<String> = None;
                let mut interpret = false;
                let mut nbt_entity: Option<String> = None;
                let mut nbt_block: Option<String> = None;
                let mut nbt_storage: Option<String> = None;
                let mut separator: Option<Component> = None;
                let mut style = Style::default();
                let mut extra: Vec<Component> = Vec::new();

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "text" => text = Some(map.next_value()?),
                        "translate" => translate = Some(map.next_value()?),
                        "fallback" => fallback = Some(map.next_value()?),
                        "with" => with = Some(map.next_value()?),
                        "selector" => selector = Some(map.next_value()?),
                        "score" => {
                            #[derive(Deserialize)]
                            struct ScoreRaw {
                                name: String,
                                objective: String,
                            }
                            let s: ScoreRaw = map.next_value()?;
                            score_name = Some(s.name);
                            score_objective = Some(s.objective);
                        }
                        "keybind" => keybind = Some(map.next_value()?),
                        "nbt" => nbt_path = Some(map.next_value()?),
                        "interpret" => interpret = map.next_value()?,
                        "entity" => nbt_entity = Some(map.next_value()?),
                        "block" => nbt_block = Some(map.next_value()?),
                        "storage" => nbt_storage = Some(map.next_value()?),
                        "separator" => separator = Some(map.next_value()?),
                        "extra" => extra = map.next_value()?,
                        // Style fields
                        "color" => style.color = Some(map.next_value()?),
                        "bold" => style.bold = Some(map.next_value()?),
                        "italic" => style.italic = Some(map.next_value()?),
                        "underlined" => style.underlined = Some(map.next_value()?),
                        "strikethrough" => style.strikethrough = Some(map.next_value()?),
                        "obfuscated" => style.obfuscated = Some(map.next_value()?),
                        "insertion" => style.insertion = Some(map.next_value()?),
                        "clickEvent" => style.click_event = Some(map.next_value()?),
                        "hoverEvent" => style.hover_event = Some(map.next_value()?),
                        "font" => {
                            let s: String = map.next_value()?;
                            style.font =
                                Some(ResourceLocation::from_string(&s).map_err(
                                    |e| de::Error::custom(format!("invalid font: {e}")),
                                )?);
                        }
                        _ => {
                            let _: serde_json::Value = map.next_value()?;
                        }
                    }
                }

                // Determine content type by which field was present
                let content = if let Some(t) = text {
                    ComponentContent::Text(t)
                } else if let Some(key) = translate {
                    ComponentContent::Translatable {
                        key,
                        fallback,
                        args: with.unwrap_or_default(),
                    }
                } else if let Some(pattern) = selector {
                    ComponentContent::Selector {
                        pattern,
                        separator: separator.map(Box::new),
                    }
                } else if score_name.is_some() {
                    ComponentContent::Score {
                        name: score_name.unwrap_or_default(),
                        objective: score_objective.unwrap_or_default(),
                    }
                } else if let Some(k) = keybind {
                    ComponentContent::Keybind(k)
                } else if let Some(path) = nbt_path {
                    let source = if let Some(sel) = nbt_entity {
                        NbtSource::Entity(sel)
                    } else if let Some(pos) = nbt_block {
                        NbtSource::Block(pos)
                    } else if let Some(stor) = nbt_storage {
                        NbtSource::Storage(
                            ResourceLocation::from_string(&stor)
                                .map_err(de::Error::custom)?,
                        )
                    } else {
                        return Err(de::Error::custom("nbt component missing source"));
                    };
                    ComponentContent::Nbt {
                        path,
                        interpret,
                        separator: separator.map(Box::new),
                        source,
                    }
                } else {
                    // Default to empty text
                    ComponentContent::Text(String::new())
                };

                Ok(Component {
                    content,
                    style,
                    children: extra,
                })
            }
        }

        d.deserialize_any(ComponentVisitor)
    }
}

// ── NBT Wire Encoding ────────────────────────────────────────────────

use oxidized_nbt::{NbtCompound, NbtList, NbtTag};

impl Component {
    /// Encode this component as an NBT tag for wire transmission.
    ///
    /// Play-state packets encode components as NBT (not JSON strings).
    pub fn to_nbt(&self) -> NbtTag {
        let mut compound = NbtCompound::new();

        // Content-type-specific fields
        match &self.content {
            ComponentContent::Text(t) => {
                compound.put_string("text", t);
            }
            ComponentContent::Translatable {
                key,
                fallback,
                args,
            } => {
                compound.put_string("translate", key);
                if let Some(fb) = fallback {
                    compound.put_string("fallback", fb);
                }
                if !args.is_empty() {
                    let mut list = NbtList::new(10); // compound list
                    for arg in args {
                        let _ = list.push(arg.to_nbt());
                    }
                    compound.put("with", NbtTag::List(list));
                }
            }
            ComponentContent::Selector { pattern, separator } => {
                compound.put_string("selector", pattern);
                if let Some(sep) = separator {
                    compound.put("separator", sep.to_nbt());
                }
            }
            ComponentContent::Score { name, objective } => {
                let mut score = NbtCompound::new();
                score.put_string("name", name);
                score.put_string("objective", objective);
                compound.put("score", NbtTag::Compound(score));
            }
            ComponentContent::Keybind(k) => {
                compound.put_string("keybind", k);
            }
            ComponentContent::Nbt {
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
                    }
                    NbtSource::Block(pos) => {
                        compound.put_string("block", pos);
                    }
                    NbtSource::Storage(rl) => {
                        compound.put_string("storage", &rl.to_string());
                    }
                }
            }
        }

        // Style fields
        if let Some(ref color) = self.style.color {
            match color {
                TextColor::Named(cf) => {
                    compound.put_string("color", cf.name());
                }
                TextColor::Hex(v) => {
                    compound.put_string("color", &format!("#{:06X}", v & 0xFFFFFF));
                }
            }
        }
        if let Some(b) = self.style.bold {
            compound.put_byte("bold", if b { 1 } else { 0 });
        }
        if let Some(i) = self.style.italic {
            compound.put_byte("italic", if i { 1 } else { 0 });
        }
        if let Some(u) = self.style.underlined {
            compound.put_byte("underlined", if u { 1 } else { 0 });
        }
        if let Some(st) = self.style.strikethrough {
            compound.put_byte("strikethrough", if st { 1 } else { 0 });
        }
        if let Some(o) = self.style.obfuscated {
            compound.put_byte("obfuscated", if o { 1 } else { 0 });
        }
        if let Some(ref ins) = self.style.insertion {
            compound.put_string("insertion", ins);
        }
        // Click/hover events as NBT compounds
        if let Some(ref ce) = self.style.click_event {
            let mut ce_nbt = NbtCompound::new();
            let (action, value) = match ce {
                ClickEvent::OpenUrl(v) => ("open_url", v.as_str()),
                ClickEvent::RunCommand(v) => ("run_command", v.as_str()),
                ClickEvent::SuggestCommand(v) => ("suggest_command", v.as_str()),
                ClickEvent::CopyToClipboard(v) => ("copy_to_clipboard", v.as_str()),
                ClickEvent::ChangePage(v) => ("change_page", v.as_str()),
            };
            ce_nbt.put_string("action", action);
            ce_nbt.put_string("value", value);
            compound.put("clickEvent", NbtTag::Compound(ce_nbt));
        }
        if let Some(ref he) = self.style.hover_event {
            let mut he_nbt = NbtCompound::new();
            match he {
                HoverEvent::ShowText(c) => {
                    he_nbt.put_string("action", "show_text");
                    he_nbt.put("contents", c.to_nbt());
                }
                HoverEvent::ShowItem(item) => {
                    he_nbt.put_string("action", "show_item");
                    let mut item_nbt = NbtCompound::new();
                    item_nbt.put_string("id", &item.id);
                    item_nbt.put_int("count", item.count);
                    he_nbt.put("contents", NbtTag::Compound(item_nbt));
                }
                HoverEvent::ShowEntity(entity) => {
                    he_nbt.put_string("action", "show_entity");
                    let mut ent_nbt = NbtCompound::new();
                    ent_nbt.put_string("type", &entity.entity_type);
                    ent_nbt.put_string("id", &entity.id);
                    if let Some(ref name) = entity.name {
                        ent_nbt.put("name", name.to_nbt());
                    }
                    he_nbt.put("contents", NbtTag::Compound(ent_nbt));
                }
            }
            compound.put("hoverEvent", NbtTag::Compound(he_nbt));
        }
        if let Some(ref f) = self.style.font {
            compound.put_string("font", &f.to_string());
        }

        // Children
        if !self.children.is_empty() {
            let mut list = NbtList::new(10); // compound list
            for child in &self.children {
                let _ = list.push(child.to_nbt());
            }
            compound.put("extra", NbtTag::List(list));
        }

        NbtTag::Compound(compound)
    }

    /// Decode a component from an NBT tag.
    pub fn from_nbt(tag: &NbtTag) -> Result<Self, String> {
        match tag {
            NbtTag::String(s) => Ok(Component::text(s.as_str())),
            NbtTag::Compound(compound) => Self::from_nbt_compound(compound),
            NbtTag::List(list) => {
                if list.is_empty() {
                    return Err("empty component list".into());
                }
                let mut iter = list.iter();
                let first = iter.next().ok_or("empty list")?;
                let mut component = Self::from_nbt(first)?;
                for child_tag in iter {
                    component.children.push(Self::from_nbt(child_tag)?);
                }
                Ok(component)
            }
            _ => Err(format!("unexpected NBT tag type for component: {:?}", tag)),
        }
    }

    fn from_nbt_compound(compound: &NbtCompound) -> Result<Self, String> {
        // Determine content type
        let content = if let Some(t) = compound.get_string("text") {
            ComponentContent::Text(t.to_string())
        } else if let Some(key) = compound.get_string("translate") {
            let fallback = compound.get_string("fallback").map(|s| s.to_string());
            let args = if let Some(NbtTag::List(list)) = compound.get("with") {
                list.iter()
                    .map(Self::from_nbt)
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                Vec::new()
            };
            ComponentContent::Translatable {
                key: key.to_string(),
                fallback,
                args,
            }
        } else if let Some(pattern) = compound.get_string("selector") {
            let separator = compound
                .get("separator")
                .map(Self::from_nbt)
                .transpose()?
                .map(Box::new);
            ComponentContent::Selector {
                pattern: pattern.to_string(),
                separator,
            }
        } else if let Some(NbtTag::Compound(score)) = compound.get("score") {
            ComponentContent::Score {
                name: score
                    .get_string("name")
                    .unwrap_or_default()
                    .to_string(),
                objective: score
                    .get_string("objective")
                    .unwrap_or_default()
                    .to_string(),
            }
        } else if let Some(k) = compound.get_string("keybind") {
            ComponentContent::Keybind(k.to_string())
        } else if let Some(path) = compound.get_string("nbt") {
            let interpret = compound
                .get_byte("interpret")
                .is_some_and(|b| b != 0);
            let separator = compound
                .get("separator")
                .map(Self::from_nbt)
                .transpose()?
                .map(Box::new);
            let source = if let Some(sel) = compound.get_string("entity") {
                NbtSource::Entity(sel.to_string())
            } else if let Some(pos) = compound.get_string("block") {
                NbtSource::Block(pos.to_string())
            } else if let Some(stor) = compound.get_string("storage") {
                NbtSource::Storage(
                    ResourceLocation::from_string(stor)
                        .map_err(|e| format!("invalid storage: {e}"))?,
                )
            } else {
                return Err("nbt component missing source".into());
            };
            ComponentContent::Nbt {
                path: path.to_string(),
                interpret,
                separator,
                source,
            }
        } else {
            ComponentContent::Text(String::new())
        };

        // Parse style
        let mut style = Style::default();
        if let Some(color_str) = compound.get_string("color") {
            if let Some(hex_str) = color_str.strip_prefix('#') {
                if let Ok(v) = u32::from_str_radix(hex_str, 16) {
                    style.color = Some(TextColor::Hex(v));
                }
            } else if let Some(cf) = ChatFormatting::from_name(color_str) {
                style.color = Some(TextColor::Named(cf));
            }
        }
        if let Some(b) = compound.get_byte("bold") {
            style.bold = Some(b != 0);
        }
        if let Some(i) = compound.get_byte("italic") {
            style.italic = Some(i != 0);
        }
        if let Some(u) = compound.get_byte("underlined") {
            style.underlined = Some(u != 0);
        }
        if let Some(st) = compound.get_byte("strikethrough") {
            style.strikethrough = Some(st != 0);
        }
        if let Some(o) = compound.get_byte("obfuscated") {
            style.obfuscated = Some(o != 0);
        }
        if let Some(ins) = compound.get_string("insertion") {
            style.insertion = Some(ins.to_string());
        }
        if let Some(NbtTag::Compound(ce)) = compound.get("clickEvent") {
            if let (Some(action), Some(value)) =
                (ce.get_string("action"), ce.get_string("value"))
            {
                style.click_event = match action {
                    "open_url" => Some(ClickEvent::OpenUrl(value.to_string())),
                    "run_command" => Some(ClickEvent::RunCommand(value.to_string())),
                    "suggest_command" => {
                        Some(ClickEvent::SuggestCommand(value.to_string()))
                    }
                    "copy_to_clipboard" => {
                        Some(ClickEvent::CopyToClipboard(value.to_string()))
                    }
                    "change_page" => Some(ClickEvent::ChangePage(value.to_string())),
                    _ => None,
                };
            }
        }
        if let Some(NbtTag::Compound(he)) = compound.get("hoverEvent") {
            if let Some(action) = he.get_string("action") {
                style.hover_event = match action {
                    "show_text" => he
                        .get("contents")
                        .map(Self::from_nbt)
                        .transpose()?
                        .map(|c| HoverEvent::ShowText(Box::new(c))),
                    "show_entity" => {
                        if let Some(NbtTag::Compound(ent)) = he.get("contents") {
                            let name = ent
                                .get("name")
                                .map(Self::from_nbt)
                                .transpose()?
                                .map(Box::new);
                            Some(HoverEvent::ShowEntity(
                                super::style::HoverEntity {
                                    entity_type: ent
                                        .get_string("type")
                                        .unwrap_or_default()
                                        .to_string(),
                                    id: ent
                                        .get_string("id")
                                        .unwrap_or_default()
                                        .to_string(),
                                    name,
                                },
                            ))
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
            }
        }
        if let Some(f) = compound.get_string("font") {
            style.font = ResourceLocation::from_string(f).ok();
        }

        // Parse children
        let children = if let Some(NbtTag::List(list)) = compound.get("extra") {
            list.iter()
                .map(Self::from_nbt)
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };

        Ok(Component {
            content,
            style,
            children,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ── JSON serialization ───────────────────────────────────────────

    #[test]
    fn test_text_component_serializes_to_json() {
        let c = Component::text("hello");
        assert_eq!(c.to_json(), r#"{"text":"hello"}"#);
    }

    #[test]
    fn test_styled_component_includes_color_field() {
        let c = Component::text("alert")
            .color(TextColor::Named(ChatFormatting::Red))
            .bold();
        let json: serde_json::Value = serde_json::from_str(&c.to_json()).unwrap();
        assert_eq!(json["color"], "red");
        assert_eq!(json["bold"], true);
        assert_eq!(json["text"], "alert");
    }

    #[test]
    fn test_hex_color_serializes_as_hash_string() {
        let c = Component::text("test").color(TextColor::Hex(0xFF8000));
        let json = c.to_json();
        assert!(json.contains("#FF8000"), "got: {json}");
    }

    #[test]
    fn test_translatable_with_args() {
        let c = Component::translatable(
            "chat.type.text",
            vec![Component::text("Alice"), Component::text("hello world")],
        );
        let json: serde_json::Value = serde_json::from_str(&c.to_json()).unwrap();
        assert_eq!(json["translate"], "chat.type.text");
        assert_eq!(json["with"][0]["text"], "Alice");
        assert_eq!(json["with"][1]["text"], "hello world");
    }

    #[test]
    fn test_click_event_in_component() {
        let c = Component::text("click me").click(ClickEvent::RunCommand("/home".into()));
        let json: serde_json::Value = serde_json::from_str(&c.to_json()).unwrap();
        assert_eq!(json["clickEvent"]["action"], "run_command");
        assert_eq!(json["clickEvent"]["value"], "/home");
    }

    #[test]
    fn test_hover_event_show_text() {
        let c = Component::text("hover me")
            .hover(HoverEvent::ShowText(Box::new(Component::text("tooltip"))));
        let json: serde_json::Value = serde_json::from_str(&c.to_json()).unwrap();
        assert_eq!(json["hoverEvent"]["action"], "show_text");
        assert_eq!(json["hoverEvent"]["contents"]["text"], "tooltip");
    }

    #[test]
    fn test_component_with_children() {
        let c = Component::text("Hello ")
            .append(Component::text("World").bold())
            .append(Component::text("!"));
        let json: serde_json::Value = serde_json::from_str(&c.to_json()).unwrap();
        assert_eq!(json["text"], "Hello ");
        assert_eq!(json["extra"][0]["text"], "World");
        assert_eq!(json["extra"][0]["bold"], true);
        assert_eq!(json["extra"][1]["text"], "!");
    }

    #[test]
    fn test_keybind_component() {
        let c = Component::keybind("key.jump");
        let json: serde_json::Value = serde_json::from_str(&c.to_json()).unwrap();
        assert_eq!(json["keybind"], "key.jump");
    }

    #[test]
    fn test_score_component() {
        let c = Component::score("@s", "kills");
        let json: serde_json::Value = serde_json::from_str(&c.to_json()).unwrap();
        assert_eq!(json["score"]["name"], "@s");
        assert_eq!(json["score"]["objective"], "kills");
    }

    // ── JSON deserialization ─────────────────────────────────────────

    #[test]
    fn test_deserialize_string() {
        let c: Component = serde_json::from_str(r#""hello""#).unwrap();
        assert_eq!(c.content, ComponentContent::Text("hello".into()));
    }

    #[test]
    fn test_deserialize_object() {
        let c: Component =
            serde_json::from_str(r#"{"text":"hello","bold":true,"color":"red"}"#).unwrap();
        assert_eq!(c.content, ComponentContent::Text("hello".into()));
        assert_eq!(c.style.bold, Some(true));
        assert_eq!(
            c.style.color,
            Some(TextColor::Named(ChatFormatting::Red))
        );
    }

    #[test]
    fn test_json_roundtrip_complex() {
        let original = Component::text("Hello ")
            .color(TextColor::Named(ChatFormatting::Gold))
            .bold()
            .append(
                Component::text("World")
                    .color(TextColor::Hex(0xFF6B35))
                    .click(ClickEvent::RunCommand("/help".into())),
            );
        let json = original.to_json();
        let deserialized: Component = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }

    // ── NBT encoding ─────────────────────────────────────────────────

    #[test]
    fn test_nbt_text_component() {
        let c = Component::text("hello");
        let tag = c.to_nbt();
        if let NbtTag::Compound(compound) = &tag {
            assert_eq!(compound.get_string("text"), Some("hello"));
        } else {
            panic!("expected compound, got: {tag:?}");
        }
    }

    #[test]
    fn test_nbt_styled_component() {
        let c = Component::text("alert")
            .color(TextColor::Named(ChatFormatting::Red))
            .bold();
        let tag = c.to_nbt();
        if let NbtTag::Compound(compound) = &tag {
            assert_eq!(compound.get_string("color"), Some("red"));
            assert_eq!(compound.get_byte("bold"), Some(1));
        } else {
            panic!("expected compound");
        }
    }

    #[test]
    fn test_nbt_roundtrip() {
        let original = Component::text("Hello ")
            .color(TextColor::Named(ChatFormatting::Gold))
            .bold()
            .append(Component::text("World").italic());
        let tag = original.to_nbt();
        let decoded = Component::from_nbt(&tag).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_nbt_translatable_roundtrip() {
        let original = Component::translatable(
            "chat.type.text",
            vec![Component::text("Alice"), Component::text("hi")],
        );
        let tag = original.to_nbt();
        let decoded = Component::from_nbt(&tag).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_nbt_string_shorthand() {
        let tag = NbtTag::String("simple".into());
        let c = Component::from_nbt(&tag).unwrap();
        assert_eq!(c.content, ComponentContent::Text("simple".into()));
    }

    // ── Legacy §-code conversion ─────────────────────────────────────

    #[test]
    fn test_from_legacy_simple_color() {
        let c = Component::from_legacy("§cRed text");
        assert_eq!(
            c.style.color,
            Some(TextColor::Named(ChatFormatting::Red))
        );
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
        assert!(!c.children.is_empty() || c.to_json().contains("red"));
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
        let c = Component::text("Hello")
            .color(TextColor::Named(ChatFormatting::Red));
        let legacy = c.to_legacy_string();
        assert!(legacy.contains("§c"), "got: {legacy}");
        assert!(legacy.contains("Hello"), "got: {legacy}");
    }

    // ── Display ──────────────────────────────────────────────────────

    #[test]
    fn test_display_extracts_plain_text() {
        let c = Component::text("Hello ")
            .append(Component::text("World"));
        assert_eq!(format!("{c}"), "Hello World");
    }

    // ── From impls ───────────────────────────────────────────────────

    #[test]
    fn test_from_str() {
        let c: Component = "hello".into();
        assert_eq!(c.content, ComponentContent::Text("hello".into()));
    }
}
