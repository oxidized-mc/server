//! Text styling: colors, formatting flags, click/hover events.
//!
//! Style uses `Option<T>` for every field — `None` means "inherit from parent".
//! The client resolves inheritance by walking up the component tree.

use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::ChatFormatting;
use crate::types::ResourceLocation;

/// Text color: either a named legacy color or a 24-bit hex RGB value.
#[derive(Debug, Clone, PartialEq)]
pub enum TextColor {
    /// One of the 16 legacy `§`-code colors.
    Named(ChatFormatting),
    /// 24-bit RGB hex color (`0x00RRGGBB`).
    Hex(u32),
}

impl TextColor {
    /// Returns the RGB value regardless of variant.
    pub fn rgb(&self) -> u32 {
        match self {
            Self::Named(cf) => cf.color().unwrap_or(0xFFFFFF),
            Self::Hex(v) => *v & 0xFFFFFF,
        }
    }
}

impl Serialize for TextColor {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Named(cf) => s.serialize_str(cf.name()),
            Self::Hex(v) => s.serialize_str(&format!("#{:06X}", v & 0xFFFFFF)),
        }
    }
}

impl<'de> Deserialize<'de> for TextColor {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if let Some(hex_str) = s.strip_prefix('#') {
            let v = u32::from_str_radix(hex_str, 16).map_err(de::Error::custom)?;
            Ok(Self::Hex(v))
        } else {
            ChatFormatting::from_name(&s)
                .map(Self::Named)
                .ok_or_else(|| de::Error::custom(format!("unknown color: {s}")))
        }
    }
}

/// Click event action triggered when the player clicks on a component.
#[derive(Debug, Clone, PartialEq)]
pub enum ClickEvent {
    /// Open a URL in the player's browser.
    OpenUrl(String),
    /// Execute a chat command (must start with `/`).
    RunCommand(String),
    /// Insert text into the chat input box.
    SuggestCommand(String),
    /// Copy text to the clipboard.
    CopyToClipboard(String),
    /// Change the page in a book (string representation of page number).
    ChangePage(String),
}

impl Serialize for ClickEvent {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(2))?;
        let (action, value) = match self {
            Self::OpenUrl(v) => ("open_url", v),
            Self::RunCommand(v) => ("run_command", v),
            Self::SuggestCommand(v) => ("suggest_command", v),
            Self::CopyToClipboard(v) => ("copy_to_clipboard", v),
            Self::ChangePage(v) => ("change_page", v),
        };
        map.serialize_entry("action", action)?;
        map.serialize_entry("value", value)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for ClickEvent {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            action: String,
            value: String,
        }
        let raw = Raw::deserialize(d)?;
        match raw.action.as_str() {
            "open_url" => Ok(Self::OpenUrl(raw.value)),
            "run_command" => Ok(Self::RunCommand(raw.value)),
            "suggest_command" => Ok(Self::SuggestCommand(raw.value)),
            "copy_to_clipboard" => Ok(Self::CopyToClipboard(raw.value)),
            "change_page" => Ok(Self::ChangePage(raw.value)),
            other => Err(de::Error::custom(format!(
                "unknown click event action: {other}"
            ))),
        }
    }
}

/// Item data shown in a hover tooltip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HoverItem {
    /// Item identifier (e.g. `"minecraft:diamond_sword"`).
    pub id: String,
    /// Stack count.
    #[serde(default = "default_count")]
    pub count: i32,
    /// Optional NBT/data component tag (SNBT string).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<serde_json::Value>,
}

fn default_count() -> i32 {
    1
}

/// Entity data shown in a hover tooltip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HoverEntity {
    /// Entity type (e.g. `"minecraft:player"`).
    #[serde(rename = "type")]
    pub entity_type: String,
    /// Entity UUID as string.
    pub id: String,
    /// Optional display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Box<super::Component>>,
}

/// Hover event shown when the player hovers over a component.
#[derive(Debug, Clone, PartialEq)]
pub enum HoverEvent {
    /// Show a text tooltip.
    ShowText(Box<super::Component>),
    /// Show an item tooltip.
    ShowItem(HoverItem),
    /// Show an entity tooltip.
    ShowEntity(HoverEntity),
}

impl Serialize for HoverEvent {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(2))?;
        match self {
            Self::ShowText(c) => {
                map.serialize_entry("action", "show_text")?;
                map.serialize_entry("contents", c)?;
            }
            Self::ShowItem(item) => {
                map.serialize_entry("action", "show_item")?;
                map.serialize_entry("contents", item)?;
            }
            Self::ShowEntity(entity) => {
                map.serialize_entry("action", "show_entity")?;
                map.serialize_entry("contents", entity)?;
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for HoverEvent {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            action: String,
            contents: serde_json::Value,
        }
        let raw = Raw::deserialize(d)?;
        match raw.action.as_str() {
            "show_text" => {
                let c = serde_json::from_value(raw.contents).map_err(de::Error::custom)?;
                Ok(Self::ShowText(Box::new(c)))
            }
            "show_item" => {
                let item = serde_json::from_value(raw.contents).map_err(de::Error::custom)?;
                Ok(Self::ShowItem(item))
            }
            "show_entity" => {
                let entity = serde_json::from_value(raw.contents).map_err(de::Error::custom)?;
                Ok(Self::ShowEntity(entity))
            }
            other => Err(de::Error::custom(format!(
                "unknown hover event action: {other}"
            ))),
        }
    }
}

/// Rendering style for a text component.
///
/// All fields are `Option<T>` — `None` means "inherit from parent component".
/// The client resolves inheritance by walking up the component tree.
///
/// # Examples
///
/// ```
/// use oxidized_protocol::chat::{Style, TextColor, ChatFormatting};
///
/// let style = Style::builder()
///     .color(TextColor::Named(ChatFormatting::Red))
///     .bold(true)
///     .build();
/// assert_eq!(style.bold, Some(true));
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Style {
    /// Text color.
    pub color: Option<TextColor>,
    /// Bold formatting.
    pub bold: Option<bool>,
    /// Italic formatting.
    pub italic: Option<bool>,
    /// Underlined formatting.
    pub underlined: Option<bool>,
    /// Strikethrough formatting.
    pub strikethrough: Option<bool>,
    /// Obfuscated (random characters) formatting.
    pub obfuscated: Option<bool>,
    /// Text inserted into chat on shift-click.
    pub insertion: Option<String>,
    /// Click interaction.
    pub click_event: Option<ClickEvent>,
    /// Hover tooltip.
    pub hover_event: Option<HoverEvent>,
    /// Font resource location (default: `"minecraft:default"`).
    pub font: Option<ResourceLocation>,
}

impl Style {
    /// An empty style that inherits everything from its parent.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Create a [`StyleBuilder`].
    pub fn builder() -> StyleBuilder {
        StyleBuilder::new()
    }

    /// Returns `true` if all fields are `None`.
    pub fn is_empty(&self) -> bool {
        self.color.is_none()
            && self.bold.is_none()
            && self.italic.is_none()
            && self.underlined.is_none()
            && self.strikethrough.is_none()
            && self.obfuscated.is_none()
            && self.insertion.is_none()
            && self.click_event.is_none()
            && self.hover_event.is_none()
            && self.font.is_none()
    }

    /// Merge `other` into `self`, keeping `self`'s values where set.
    pub fn apply_to(&self, parent: &Style) -> Style {
        Style {
            color: self.color.clone().or_else(|| parent.color.clone()),
            bold: self.bold.or(parent.bold),
            italic: self.italic.or(parent.italic),
            underlined: self.underlined.or(parent.underlined),
            strikethrough: self.strikethrough.or(parent.strikethrough),
            obfuscated: self.obfuscated.or(parent.obfuscated),
            insertion: self
                .insertion
                .clone()
                .or_else(|| parent.insertion.clone()),
            click_event: self
                .click_event
                .clone()
                .or_else(|| parent.click_event.clone()),
            hover_event: self
                .hover_event
                .clone()
                .or_else(|| parent.hover_event.clone()),
            font: self.font.clone().or_else(|| parent.font.clone()),
        }
    }
}

impl Serialize for Style {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        // Count non-None fields for capacity hint
        let mut count = 0;
        if self.color.is_some() {
            count += 1;
        }
        if self.bold.is_some() {
            count += 1;
        }
        if self.italic.is_some() {
            count += 1;
        }
        if self.underlined.is_some() {
            count += 1;
        }
        if self.strikethrough.is_some() {
            count += 1;
        }
        if self.obfuscated.is_some() {
            count += 1;
        }
        if self.insertion.is_some() {
            count += 1;
        }
        if self.click_event.is_some() {
            count += 1;
        }
        if self.hover_event.is_some() {
            count += 1;
        }
        if self.font.is_some() {
            count += 1;
        }

        let mut map = s.serialize_map(Some(count))?;
        if let Some(ref c) = self.color {
            map.serialize_entry("color", c)?;
        }
        if let Some(b) = self.bold {
            map.serialize_entry("bold", &b)?;
        }
        if let Some(i) = self.italic {
            map.serialize_entry("italic", &i)?;
        }
        if let Some(u) = self.underlined {
            map.serialize_entry("underlined", &u)?;
        }
        if let Some(st) = self.strikethrough {
            map.serialize_entry("strikethrough", &st)?;
        }
        if let Some(o) = self.obfuscated {
            map.serialize_entry("obfuscated", &o)?;
        }
        if let Some(ref ins) = self.insertion {
            map.serialize_entry("insertion", ins)?;
        }
        if let Some(ref ce) = self.click_event {
            map.serialize_entry("clickEvent", ce)?;
        }
        if let Some(ref he) = self.hover_event {
            map.serialize_entry("hoverEvent", he)?;
        }
        if let Some(ref f) = self.font {
            map.serialize_entry("font", &f.to_string())?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Style {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct StyleVisitor;

        impl<'de> Visitor<'de> for StyleVisitor {
            type Value = Style;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a style object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Style, A::Error> {
                let mut style = Style::default();
                while let Some(key) = map.next_key::<&str>()? {
                    match key {
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
                            style.font = Some(
                                ResourceLocation::from_string(&s).map_err(de::Error::custom)?,
                            );
                        }
                        _ => {
                            // Skip unknown fields for forward-compat
                            let _: serde_json::Value = map.next_value()?;
                        }
                    }
                }
                Ok(style)
            }
        }

        d.deserialize_map(StyleVisitor)
    }
}

/// Builder for [`Style`].
pub struct StyleBuilder(Style);

impl StyleBuilder {
    /// Create a new builder with all fields set to `None`.
    pub fn new() -> Self {
        Self(Style::default())
    }

    /// Set the text color.
    pub fn color(mut self, color: TextColor) -> Self {
        self.0.color = Some(color);
        self
    }

    /// Set bold formatting.
    pub fn bold(mut self, bold: bool) -> Self {
        self.0.bold = Some(bold);
        self
    }

    /// Set italic formatting.
    pub fn italic(mut self, italic: bool) -> Self {
        self.0.italic = Some(italic);
        self
    }

    /// Set underlined formatting.
    pub fn underlined(mut self, underlined: bool) -> Self {
        self.0.underlined = Some(underlined);
        self
    }

    /// Set strikethrough formatting.
    pub fn strikethrough(mut self, strikethrough: bool) -> Self {
        self.0.strikethrough = Some(strikethrough);
        self
    }

    /// Set obfuscated formatting.
    pub fn obfuscated(mut self, obfuscated: bool) -> Self {
        self.0.obfuscated = Some(obfuscated);
        self
    }

    /// Set insertion text.
    pub fn insertion(mut self, insertion: impl Into<String>) -> Self {
        self.0.insertion = Some(insertion.into());
        self
    }

    /// Set click event.
    pub fn click_event(mut self, event: ClickEvent) -> Self {
        self.0.click_event = Some(event);
        self
    }

    /// Set hover event.
    pub fn hover_event(mut self, event: HoverEvent) -> Self {
        self.0.hover_event = Some(event);
        self
    }

    /// Set font.
    pub fn font(mut self, font: ResourceLocation) -> Self {
        self.0.font = Some(font);
        self
    }

    /// Build the style.
    pub fn build(self) -> Style {
        self.0
    }
}

impl Default for StyleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_text_color_named_serialize() {
        let color = TextColor::Named(ChatFormatting::Red);
        let json = serde_json::to_string(&color).unwrap();
        assert_eq!(json, r#""red""#);
    }

    #[test]
    fn test_text_color_hex_serialize() {
        let color = TextColor::Hex(0xFF8000);
        let json = serde_json::to_string(&color).unwrap();
        assert_eq!(json, r##""#FF8000""##);
    }

    #[test]
    fn test_text_color_named_deserialize() {
        let color: TextColor = serde_json::from_str(r#""dark_blue""#).unwrap();
        assert_eq!(color, TextColor::Named(ChatFormatting::DarkBlue));
    }

    #[test]
    fn test_text_color_hex_deserialize() {
        let color: TextColor = serde_json::from_str(r##""#55AAFF""##).unwrap();
        assert_eq!(color, TextColor::Hex(0x55AAFF));
    }

    #[test]
    fn test_text_color_rgb() {
        assert_eq!(TextColor::Named(ChatFormatting::Red).rgb(), 0xFF5555);
        assert_eq!(TextColor::Hex(0x123456).rgb(), 0x123456);
    }

    #[test]
    fn test_style_empty_serializes_to_empty_map() {
        let style = Style::empty();
        let json = serde_json::to_string(&style).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_style_with_color_and_bold() {
        let style = Style::builder()
            .color(TextColor::Named(ChatFormatting::Red))
            .bold(true)
            .build();
        let json: serde_json::Value = serde_json::to_value(&style).unwrap();
        assert_eq!(json["color"], "red");
        assert_eq!(json["bold"], true);
    }

    #[test]
    fn test_style_roundtrip() {
        let style = Style {
            color: Some(TextColor::Hex(0xABCDEF)),
            bold: Some(true),
            italic: Some(false),
            underlined: None,
            strikethrough: Some(true),
            obfuscated: None,
            insertion: Some("test".into()),
            click_event: None,
            hover_event: None,
            font: None,
        };
        let json = serde_json::to_string(&style).unwrap();
        let deserialized: Style = serde_json::from_str(&json).unwrap();
        assert_eq!(style, deserialized);
    }

    #[test]
    fn test_click_event_run_command() {
        let ce = ClickEvent::RunCommand("/home".into());
        let json: serde_json::Value = serde_json::to_value(&ce).unwrap();
        assert_eq!(json["action"], "run_command");
        assert_eq!(json["value"], "/home");
    }

    #[test]
    fn test_click_event_roundtrip() {
        let ce = ClickEvent::SuggestCommand("/tell Alice ".into());
        let json = serde_json::to_string(&ce).unwrap();
        let deserialized: ClickEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(ce, deserialized);
    }

    #[test]
    fn test_hover_event_show_text() {
        let he = HoverEvent::ShowText(Box::new(super::super::Component::text("tooltip")));
        let json: serde_json::Value = serde_json::to_value(&he).unwrap();
        assert_eq!(json["action"], "show_text");
        assert_eq!(json["contents"]["text"], "tooltip");
    }

    #[test]
    fn test_hover_event_show_item() {
        let he = HoverEvent::ShowItem(HoverItem {
            id: "minecraft:diamond".into(),
            count: 64,
            components: None,
        });
        let json: serde_json::Value = serde_json::to_value(&he).unwrap();
        assert_eq!(json["action"], "show_item");
        assert_eq!(json["contents"]["id"], "minecraft:diamond");
        assert_eq!(json["contents"]["count"], 64);
    }

    #[test]
    fn test_style_is_empty() {
        assert!(Style::empty().is_empty());
        assert!(!Style::builder().bold(true).build().is_empty());
    }

    #[test]
    fn test_style_apply_to_inherits() {
        let parent = Style::builder()
            .color(TextColor::Named(ChatFormatting::Red))
            .bold(true)
            .build();
        let child = Style::builder().italic(true).build();
        let resolved = child.apply_to(&parent);
        assert_eq!(resolved.color, Some(TextColor::Named(ChatFormatting::Red)));
        assert_eq!(resolved.bold, Some(true));
        assert_eq!(resolved.italic, Some(true));
    }

    #[test]
    fn test_style_apply_to_overrides() {
        let parent = Style::builder()
            .color(TextColor::Named(ChatFormatting::Red))
            .build();
        let child = Style::builder()
            .color(TextColor::Named(ChatFormatting::Blue))
            .build();
        let resolved = child.apply_to(&parent);
        assert_eq!(
            resolved.color,
            Some(TextColor::Named(ChatFormatting::Blue))
        );
    }

    #[test]
    fn test_style_json_uses_camel_case() {
        let style = Style {
            click_event: Some(ClickEvent::RunCommand("/test".into())),
            hover_event: Some(HoverEvent::ShowText(Box::new(
                super::super::Component::text("hi"),
            ))),
            ..Default::default()
        };
        let json = serde_json::to_string(&style).unwrap();
        assert!(json.contains("clickEvent"), "got: {json}");
        assert!(json.contains("hoverEvent"), "got: {json}");
    }
}
