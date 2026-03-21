//! JSON serialization and deserialization for [`Component`].
//!
//! Components use JSON in status responses and configuration-state packets.
//! Play-state packets use NBT instead (see [`super::component_nbt`]).

use std::fmt;

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::component::{Component, ComponentContent, NbtSource};
use super::style::Style;
use crate::types::ResourceLocation;

// ── JSON Serialization (manual per ADR-028) ──────────────────────────

impl Serialize for Component {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let has_style = !self.style.is_empty();
        let has_children = !self.children.is_empty();

        let mut count = 1; // content type field
        if has_style {
            count += self.style.count_fields();
        }
        if has_children {
            count += 1;
        }
        count += self.content.count_extra_json_fields();

        let mut map = s.serialize_map(Some(count))?;

        self.content.write_json_fields(&mut map)?;

        if has_style {
            self.style.write_json_fields(&mut map)?;
        }

        if has_children {
            map.serialize_entry("extra", &self.children)?;
        }

        map.end()
    }
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
                        },
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
                        "click_event" => style.click_event = Some(map.next_value()?),
                        "hover_event" => style.hover_event = Some(map.next_value()?),
                        "font" => {
                            let s: String = map.next_value()?;
                            style.font =
                                Some(ResourceLocation::from_string(&s).map_err(|e| {
                                    de::Error::custom(format!("invalid font: {e}"))
                                })?);
                        },
                        _ => {
                            let _: serde_json::Value = map.next_value()?;
                        },
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
                            ResourceLocation::from_string(&stor).map_err(de::Error::custom)?,
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::chat::formatting::ChatFormatting;
    use crate::chat::style::{ClickEvent, HoverEvent, TextColor};

    // ── JSON serialization ───────────────────────────────────────────

    #[test]
    fn test_text_component_serializes_to_json() {
        let c = Component::text("hello");
        assert_eq!(c.to_json().unwrap(), r#"{"text":"hello"}"#);
    }

    #[test]
    fn test_styled_component_includes_color_field() {
        let c = Component::text("alert")
            .color(TextColor::Named(ChatFormatting::Red))
            .bold();
        let json: serde_json::Value = serde_json::from_str(&c.to_json().unwrap()).unwrap();
        assert_eq!(json["color"], "red");
        assert_eq!(json["bold"], true);
        assert_eq!(json["text"], "alert");
    }

    #[test]
    fn test_hex_color_serializes_as_hash_string() {
        let c = Component::text("test").color(TextColor::Hex(0xFF8000));
        let json = c.to_json().unwrap();
        assert!(json.contains("#FF8000"), "got: {json}");
    }

    #[test]
    fn test_translatable_with_args() {
        let c = Component::translatable(
            "chat.type.text",
            vec![Component::text("Alice"), Component::text("hello world")],
        );
        let json: serde_json::Value = serde_json::from_str(&c.to_json().unwrap()).unwrap();
        assert_eq!(json["translate"], "chat.type.text");
        assert_eq!(json["with"][0]["text"], "Alice");
        assert_eq!(json["with"][1]["text"], "hello world");
    }

    #[test]
    fn test_click_event_in_component() {
        let c = Component::text("click me").click(ClickEvent::RunCommand("/home".into()));
        let json: serde_json::Value = serde_json::from_str(&c.to_json().unwrap()).unwrap();
        assert_eq!(json["click_event"]["action"], "run_command");
        assert_eq!(json["click_event"]["value"], "/home");
    }

    #[test]
    fn test_hover_event_show_text() {
        let c = Component::text("hover me")
            .hover(HoverEvent::ShowText(Box::new(Component::text("tooltip"))));
        let json: serde_json::Value = serde_json::from_str(&c.to_json().unwrap()).unwrap();
        assert_eq!(json["hover_event"]["action"], "show_text");
        assert_eq!(json["hover_event"]["value"]["text"], "tooltip");
    }

    #[test]
    fn test_component_with_children() {
        let c = Component::text("Hello ")
            .append(Component::text("World").bold())
            .append(Component::text("!"));
        let json: serde_json::Value = serde_json::from_str(&c.to_json().unwrap()).unwrap();
        assert_eq!(json["text"], "Hello ");
        assert_eq!(json["extra"][0]["text"], "World");
        assert_eq!(json["extra"][0]["bold"], true);
        assert_eq!(json["extra"][1]["text"], "!");
    }

    #[test]
    fn test_keybind_component() {
        let c = Component::keybind("key.jump");
        let json: serde_json::Value = serde_json::from_str(&c.to_json().unwrap()).unwrap();
        assert_eq!(json["keybind"], "key.jump");
    }

    #[test]
    fn test_score_component() {
        let c = Component::score("@s", "kills");
        let json: serde_json::Value = serde_json::from_str(&c.to_json().unwrap()).unwrap();
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
        assert_eq!(c.style.color, Some(TextColor::Named(ChatFormatting::Red)));
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
        let json = original.to_json().unwrap();
        let deserialized: Component = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }
}
