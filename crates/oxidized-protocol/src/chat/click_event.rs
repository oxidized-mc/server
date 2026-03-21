//! Click event actions triggered when the player clicks on a text component.

use serde::de;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use oxidized_nbt::NbtCompound;

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
            "open_url" => compound
                .get_string("url")
                .map(|v| Self::OpenUrl(v.to_string())),
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
                    .and_then(|v| {
                        v.as_i64()
                            .map(|i| i.to_string())
                            .or_else(|| v.as_str().map(|s| s.to_string()))
                    })
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
