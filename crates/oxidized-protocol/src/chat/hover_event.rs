//! Hover event actions shown when the player hovers over a text component.

use serde::de;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use oxidized_nbt::NbtCompound;

use super::Component;

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
    pub name: Option<Box<Component>>,
}

/// Hover event shown when the player hovers over a component.
#[derive(Debug, Clone, PartialEq)]
pub enum HoverEvent {
    /// Show a text tooltip.
    ShowText(Box<Component>),
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
    ///
    /// # Errors
    ///
    /// Returns a descriptive error string if a recognised action has
    /// malformed or missing fields.
    pub fn from_nbt(compound: &NbtCompound) -> Result<Option<Self>, String> {
        let action = match compound.get_string("action") {
            Some(a) => a,
            None => return Ok(None),
        };
        match action {
            "show_text" => {
                let component = compound.get("value").map(Component::from_nbt).transpose()?;
                Ok(component.map(|c| Self::ShowText(Box::new(c))))
            },
            "show_item" => {
                let id = compound.get_string("id").unwrap_or_default().to_string();
                let count = compound.get_int("count").unwrap_or(1);
                Ok(Some(Self::ShowItem(HoverItem {
                    id,
                    count,
                    components: None,
                })))
            },
            "show_entity" => {
                let entity_type = compound.get_string("id").unwrap_or_default().to_string();
                let id = compound.get_string("uuid").unwrap_or_default().to_string();
                let name = compound
                    .get("name")
                    .map(Component::from_nbt)
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
        let obj = raw
            .as_object()
            .ok_or_else(|| de::Error::custom("expected object"))?;
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
                let count = obj.get("count").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
                Ok(Self::ShowItem(HoverItem {
                    id,
                    count,
                    components: None,
                }))
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
