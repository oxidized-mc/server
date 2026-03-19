//! Chat and text component system.
//!
//! Implements Minecraft's text component tree: styled, interactive,
//! translatable text used by chat messages, item names, signs, etc.
//! See ADR-028 for design rationale.

pub mod component;
pub mod component_json;
pub mod component_nbt;
pub mod formatting;
pub mod style;

pub use component::{Component, ComponentContent, NbtSource};
pub use formatting::ChatFormatting;
pub use style::{ClickEvent, HoverEntity, HoverEvent, HoverItem, Style, TextColor};
