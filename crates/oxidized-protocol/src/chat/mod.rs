//! Chat and text component system.
//!
//! Implements Minecraft's text component tree: styled, interactive,
//! translatable text used by chat messages, item names, signs, etc.
//! See ADR-028 for design rationale.

pub mod click_event;
pub mod component;
pub mod component_json;
pub mod component_nbt;
pub mod formatting;
pub mod hover_event;
pub mod style;
pub mod text_color;

pub use click_event::ClickEvent;
pub use component::{Component, ComponentContent, NbtSource};
pub use formatting::ChatFormatting;
pub use hover_event::{HoverEntity, HoverEvent, HoverItem};
pub use style::{Style, StyleBuilder};
pub use text_color::TextColor;
