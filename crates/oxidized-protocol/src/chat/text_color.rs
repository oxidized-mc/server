//! Text color representation: named legacy colors or 24-bit hex RGB.

use serde::de;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::ChatFormatting;

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
