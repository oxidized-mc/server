//! NBT wire encoding and decoding for [`Component`].
//!
//! Play-state packets encode components as NBT (not JSON strings).
//! Status and configuration packets use JSON instead (see
//! [`super::component_json`]).

use oxidized_nbt::{NbtCompound, NbtList, NbtTag};

use super::component::{Component, ComponentContent, NbtSource};
use super::style::Style;
use crate::types::ResourceLocation;

impl Component {
    /// Encode this component as an NBT tag for wire transmission.
    ///
    /// Play-state packets encode components as NBT (not JSON strings).
    pub fn to_nbt(&self) -> NbtTag {
        let mut compound = NbtCompound::new();

        self.content.write_nbt_fields(&mut compound);
        self.style.write_nbt_fields(&mut compound);

        if !self.children.is_empty() {
            let mut list = NbtList::new(10);
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
            },
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
                name: score.get_string("name").unwrap_or_default().to_string(),
                objective: score
                    .get_string("objective")
                    .unwrap_or_default()
                    .to_string(),
            }
        } else if let Some(k) = compound.get_string("keybind") {
            ComponentContent::Keybind(k.to_string())
        } else if let Some(path) = compound.get_string("nbt") {
            let interpret = compound.get_byte("interpret").is_some_and(|b| b != 0);
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

        let style = Style::read_nbt_fields(compound)?;

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
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::chat::formatting::ChatFormatting;
    use crate::chat::style::TextColor;

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
}
