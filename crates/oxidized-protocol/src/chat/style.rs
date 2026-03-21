//! Text styling: colors, formatting flags, click/hover events.
//!
//! Style uses `Option<T>` for every field — `None` means "inherit from parent".
//! The client resolves inheritance by walking up the component tree.

use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use oxidized_nbt::{NbtCompound, NbtTag};

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
    /// Show a dialog (new in 26.1).
    ShowDialog(String),
    /// Custom click action (new in 26.1).
    Custom(String),
}

impl ClickEvent {
    /// Returns the action name for this click event.
    pub fn action_name(&self) -> &str {
        match self {
            Self::OpenUrl(_) => "open_url",
            Self::RunCommand(_) => "run_command",
            Self::SuggestCommand(_) => "suggest_command",
            Self::CopyToClipboard(_) => "copy_to_clipboard",
            Self::ChangePage(_) => "change_page",
            Self::ShowDialog(_) => "show_dialog",
            Self::Custom(_) => "custom",
        }
    }

    /// Returns the `(field_name, string_value)` pair for this action.
    ///
    /// In vanilla 26.1, each action uses a different field name:
    /// - `open_url` → `"url"`
    /// - `run_command` / `suggest_command` → `"command"`
    /// - `copy_to_clipboard` → `"value"`
    /// - `change_page` → `"page"` (as string here; callers may need int)
    /// - `show_dialog` → `"dialog"`
    /// - `custom` → `"value"`
    pub fn field_name_and_value(&self) -> (&str, &str) {
        match self {
            Self::OpenUrl(v) => ("url", v),
            Self::RunCommand(v) | Self::SuggestCommand(v) => ("command", v),
            Self::CopyToClipboard(v) => ("value", v),
            Self::ChangePage(v) => ("page", v),
            Self::ShowDialog(v) => ("dialog", v),
            Self::Custom(v) => ("value", v),
        }
    }

    /// Writes this click event to an NBT compound.
    pub fn to_nbt(&self) -> NbtCompound {
        let mut compound = NbtCompound::new();
        compound.put_string("action", self.action_name());
        match self {
            Self::ChangePage(v) => {
                let page = v.parse::<i32>().unwrap_or(1).max(1);
                compound.put_int("page", page);
            },
            _ => {
                let (field, value) = self.field_name_and_value();
                compound.put_string(field, value);
            },
        }
        compound
    }

    /// Decode a click event from an NBT compound.
    ///
    /// Returns `None` for unrecognized actions.
    pub fn from_nbt(compound: &NbtCompound) -> Option<Self> {
        let action = compound.get_string("action")?;
        match action {
            "open_url" => compound.get_string("url").map(|v| Self::OpenUrl(v.to_string())),
            "run_command" => compound
                .get_string("command")
                .map(|v| Self::RunCommand(v.to_string())),
            "suggest_command" => compound
                .get_string("command")
                .map(|v| Self::SuggestCommand(v.to_string())),
            "copy_to_clipboard" => compound
                .get_string("value")
                .map(|v| Self::CopyToClipboard(v.to_string())),
            "change_page" => {
                let page = compound
                    .get_int("page")
                    .map(|p| p.to_string())
                    .or_else(|| compound.get_string("page").map(|s| s.to_string()))?;
                Some(Self::ChangePage(page))
            },
            "show_dialog" => compound
                .get_string("dialog")
                .map(|v| Self::ShowDialog(v.to_string())),
            "custom" => compound
                .get_string("value")
                .map(|v| Self::Custom(v.to_string())),
            _ => None,
        }
    }
}

impl Serialize for ClickEvent {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(2))?;
        map.serialize_entry("action", self.action_name())?;
        let (field, value) = self.field_name_and_value();
        map.serialize_entry(field, value)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for ClickEvent {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map: std::collections::HashMap<String, serde_json::Value> =
            std::collections::HashMap::deserialize(d)?;
        let action = map
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| de::Error::missing_field("action"))?;
        match action {
            "open_url" => {
                let url = map
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| de::Error::missing_field("url"))?;
                Ok(Self::OpenUrl(url.to_string()))
            },
            "run_command" => {
                let cmd = map
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| de::Error::missing_field("command"))?;
                Ok(Self::RunCommand(cmd.to_string()))
            },
            "suggest_command" => {
                let cmd = map
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| de::Error::missing_field("command"))?;
                Ok(Self::SuggestCommand(cmd.to_string()))
            },
            "copy_to_clipboard" => {
                let val = map
                    .get("value")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| de::Error::missing_field("value"))?;
                Ok(Self::CopyToClipboard(val.to_string()))
            },
            "change_page" => {
                let page = map
                    .get("page")
                    .and_then(|v| v.as_i64().map(|i| i.to_string()).or_else(|| v.as_str().map(|s| s.to_string())))
                    .ok_or_else(|| de::Error::missing_field("page"))?;
                Ok(Self::ChangePage(page))
            },
            "show_dialog" => {
                let dialog = map
                    .get("dialog")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| de::Error::missing_field("dialog"))?;
                Ok(Self::ShowDialog(dialog.to_string()))
            },
            "custom" => {
                let val = map
                    .get("value")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| de::Error::missing_field("value"))?;
                Ok(Self::Custom(val.to_string()))
            },
            _ => Err(de::Error::custom(format!(
                "unknown click event action: {action}"
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
    #[serde(rename = "id")]
    pub entity_type: String,
    /// Entity UUID as string.
    #[serde(rename = "uuid")]
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

impl HoverEvent {
    /// Encode this hover event to an NBT compound.
    pub fn to_nbt(&self) -> NbtCompound {
        let mut compound = NbtCompound::new();
        match self {
            Self::ShowText(c) => {
                compound.put_string("action", "show_text");
                compound.put("value", c.to_nbt());
            },
            Self::ShowItem(item) => {
                compound.put_string("action", "show_item");
                compound.put_string("id", &item.id);
                compound.put_int("count", item.count);
            },
            Self::ShowEntity(entity) => {
                compound.put_string("action", "show_entity");
                compound.put_string("id", &entity.entity_type);
                // Store UUID as int array (vanilla UUIDUtil.LENIENT_CODEC)
                compound.put_string("uuid", &entity.id);
                if let Some(ref name) = entity.name {
                    compound.put("name", name.to_nbt());
                }
            },
        }
        compound
    }

    /// Decode a hover event from an NBT compound.
    ///
    /// Returns `Ok(None)` for unrecognized actions.
    pub fn from_nbt(compound: &NbtCompound) -> Result<Option<Self>, String> {
        let action = match compound.get_string("action") {
            Some(a) => a,
            None => return Ok(None),
        };
        match action {
            "show_text" => {
                let component = compound
                    .get("value")
                    .map(super::Component::from_nbt)
                    .transpose()?;
                Ok(component.map(|c| Self::ShowText(Box::new(c))))
            },
            "show_item" => {
                let id = compound.get_string("id").unwrap_or_default().to_string();
                let count = compound.get_int("count").unwrap_or(1);
                Ok(Some(Self::ShowItem(HoverItem { id, count, components: None })))
            },
            "show_entity" => {
                let entity_type = compound.get_string("id").unwrap_or_default().to_string();
                let id = compound.get_string("uuid").unwrap_or_default().to_string();
                let name = compound
                    .get("name")
                    .map(super::Component::from_nbt)
                    .transpose()?
                    .map(Box::new);
                Ok(Some(Self::ShowEntity(HoverEntity {
                    entity_type,
                    id,
                    name,
                })))
            },
            _ => Ok(None),
        }
    }
}

impl Serialize for HoverEvent {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::ShowText(c) => {
                let mut map = s.serialize_map(Some(2))?;
                map.serialize_entry("action", "show_text")?;
                map.serialize_entry("value", c)?;
                map.end()
            },
            Self::ShowItem(item) => {
                let mut map = s.serialize_map(Some(3))?;
                map.serialize_entry("action", "show_item")?;
                map.serialize_entry("id", &item.id)?;
                map.serialize_entry("count", &item.count)?;
                map.end()
            },
            Self::ShowEntity(entity) => {
                let n = 3 + usize::from(entity.name.is_some());
                let mut map = s.serialize_map(Some(n))?;
                map.serialize_entry("action", "show_entity")?;
                map.serialize_entry("id", &entity.entity_type)?;
                map.serialize_entry("uuid", &entity.id)?;
                if let Some(ref name) = entity.name {
                    map.serialize_entry("name", name)?;
                }
                map.end()
            },
        }
    }
}

impl<'de> Deserialize<'de> for HoverEvent {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw: serde_json::Value = serde_json::Value::deserialize(d)?;
        let obj = raw.as_object().ok_or_else(|| de::Error::custom("expected object"))?;
        let action = obj
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| de::Error::missing_field("action"))?;
        match action {
            "show_text" => {
                let value = obj
                    .get("value")
                    .ok_or_else(|| de::Error::missing_field("value"))?;
                let c = serde_json::from_value(value.clone()).map_err(de::Error::custom)?;
                Ok(Self::ShowText(Box::new(c)))
            },
            "show_item" => {
                let id = obj
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let count = obj
                    .get("count")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(1) as i32;
                Ok(Self::ShowItem(HoverItem { id, count, components: None }))
            },
            "show_entity" => {
                let entity_type = obj
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let id = obj
                    .get("uuid")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let name = obj
                    .get("name")
                    .map(|v| serde_json::from_value(v.clone()))
                    .transpose()
                    .map_err(de::Error::custom)?
                    .map(Box::new);
                Ok(Self::ShowEntity(HoverEntity {
                    entity_type,
                    id,
                    name,
                }))
            },
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
            insertion: self.insertion.clone().or_else(|| parent.insertion.clone()),
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

    /// Count non-`None` fields for JSON map sizing hints.
    pub(crate) fn count_fields(&self) -> usize {
        let mut n = 0;
        if self.color.is_some() {
            n += 1;
        }
        if self.bold.is_some() {
            n += 1;
        }
        if self.italic.is_some() {
            n += 1;
        }
        if self.underlined.is_some() {
            n += 1;
        }
        if self.strikethrough.is_some() {
            n += 1;
        }
        if self.obfuscated.is_some() {
            n += 1;
        }
        if self.insertion.is_some() {
            n += 1;
        }
        if self.click_event.is_some() {
            n += 1;
        }
        if self.hover_event.is_some() {
            n += 1;
        }
        if self.font.is_some() {
            n += 1;
        }
        n
    }

    /// Write style fields into an existing JSON serialize map.
    pub(crate) fn write_json_fields<S: SerializeMap>(&self, map: &mut S) -> Result<(), S::Error> {
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
            map.serialize_entry("click_event", ce)?;
        }
        if let Some(ref he) = self.hover_event {
            map.serialize_entry("hover_event", he)?;
        }
        if let Some(ref f) = self.font {
            map.serialize_entry("font", &f.to_string())?;
        }
        Ok(())
    }

    /// Write style fields into an NBT compound.
    pub(crate) fn write_nbt_fields(&self, compound: &mut NbtCompound) {
        if let Some(ref color) = self.color {
            match color {
                TextColor::Named(cf) => {
                    compound.put_string("color", cf.name());
                },
                TextColor::Hex(v) => {
                    compound.put_string("color", format!("#{:06X}", v & 0xFFFFFF));
                },
            }
        }
        if let Some(b) = self.bold {
            compound.put_byte("bold", if b { 1 } else { 0 });
        }
        if let Some(i) = self.italic {
            compound.put_byte("italic", if i { 1 } else { 0 });
        }
        if let Some(u) = self.underlined {
            compound.put_byte("underlined", if u { 1 } else { 0 });
        }
        if let Some(st) = self.strikethrough {
            compound.put_byte("strikethrough", if st { 1 } else { 0 });
        }
        if let Some(o) = self.obfuscated {
            compound.put_byte("obfuscated", if o { 1 } else { 0 });
        }
        if let Some(ref ins) = self.insertion {
            compound.put_string("insertion", ins);
        }
        if let Some(ref ce) = self.click_event {
            compound.put("click_event", NbtTag::Compound(ce.to_nbt()));
        }
        if let Some(ref he) = self.hover_event {
            compound.put("hover_event", NbtTag::Compound(he.to_nbt()));
        }
        if let Some(ref f) = self.font {
            compound.put_string("font", f.to_string());
        }
    }

    /// Read style fields from an NBT compound.
    pub(crate) fn read_nbt_fields(compound: &NbtCompound) -> Result<Self, String> {
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
        if let Some(NbtTag::Compound(ce)) = compound.get("click_event") {
            style.click_event = ClickEvent::from_nbt(ce);
        }
        if let Some(NbtTag::Compound(he)) = compound.get("hover_event") {
            style.hover_event = HoverEvent::from_nbt(he)?;
        }
        if let Some(f) = compound.get_string("font") {
            style.font = ResourceLocation::from_string(f).ok();
        }
        Ok(style)
    }
}

impl Serialize for Style {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.count_fields()))?;
        self.write_json_fields(&mut map)?;
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
                        "click_event" => style.click_event = Some(map.next_value()?),
                        "hover_event" => style.hover_event = Some(map.next_value()?),
                        "font" => {
                            let s: String = map.next_value()?;
                            style.font =
                                Some(ResourceLocation::from_string(&s).map_err(de::Error::custom)?);
                        },
                        _ => {
                            // Skip unknown fields for forward-compat
                            let _: serde_json::Value = map.next_value()?;
                        },
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
        assert_eq!(json["command"], "/home");
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
        assert_eq!(json["value"]["text"], "tooltip");
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
        assert_eq!(json["id"], "minecraft:diamond");
        assert_eq!(json["count"], 64);
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
        assert_eq!(resolved.color, Some(TextColor::Named(ChatFormatting::Blue)));
    }

    #[test]
    fn test_style_json_uses_snake_case() {
        let style = Style {
            click_event: Some(ClickEvent::RunCommand("/test".into())),
            hover_event: Some(HoverEvent::ShowText(Box::new(
                super::super::Component::text("hi"),
            ))),
            ..Default::default()
        };
        let json = serde_json::to_string(&style).unwrap();
        assert!(json.contains("click_event"), "got: {json}");
        assert!(json.contains("hover_event"), "got: {json}");
    }
}
