//! Legacy §-code formatting codes.
//!
//! Maps the 16 named colors (`§0`–`§f`) and 6 formatting modifiers
//! (`§k`–`§r`) used by Minecraft's legacy text system.

use std::fmt;

/// Legacy §-code formatting for system messages and plain-text fallbacks.
///
/// Each variant corresponds to a `§X` code where `X` is the code character.
/// Color variants also carry an RGB value; formatting-only variants do not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChatFormatting {
    // Colors (§0–§f)
    /// §0 — Black (#000000)
    Black,
    /// §1 — Dark Blue (#0000AA)
    DarkBlue,
    /// §2 — Dark Green (#00AA00)
    DarkGreen,
    /// §3 — Dark Aqua (#00AAAA)
    DarkAqua,
    /// §4 — Dark Red (#AA0000)
    DarkRed,
    /// §5 — Dark Purple (#AA00AA)
    DarkPurple,
    /// §6 — Gold (#FFAA00)
    Gold,
    /// §7 — Gray (#AAAAAA)
    Gray,
    /// §8 — Dark Gray (#555555)
    DarkGray,
    /// §9 — Blue (#5555FF)
    Blue,
    /// §a — Green (#55FF55)
    Green,
    /// §b — Aqua (#55FFFF)
    Aqua,
    /// §c — Red (#FF5555)
    Red,
    /// §d — Light Purple (#FF55FF)
    LightPurple,
    /// §e — Yellow (#FFFF55)
    Yellow,
    /// §f — White (#FFFFFF)
    White,
    // Formatting modifiers (§k–§r)
    /// §k — Obfuscated (random characters)
    Obfuscated,
    /// §l — Bold
    Bold,
    /// §m — Strikethrough
    Strikethrough,
    /// §n — Underline
    Underline,
    /// §o — Italic
    Italic,
    /// §r — Reset all formatting
    Reset,
}

impl ChatFormatting {
    /// All formatting variants in order.
    pub const ALL: &[ChatFormatting] = &[
        Self::Black,
        Self::DarkBlue,
        Self::DarkGreen,
        Self::DarkAqua,
        Self::DarkRed,
        Self::DarkPurple,
        Self::Gold,
        Self::Gray,
        Self::DarkGray,
        Self::Blue,
        Self::Green,
        Self::Aqua,
        Self::Red,
        Self::LightPurple,
        Self::Yellow,
        Self::White,
        Self::Obfuscated,
        Self::Bold,
        Self::Strikethrough,
        Self::Underline,
        Self::Italic,
        Self::Reset,
    ];

    /// The `§`-code character for this formatting.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidized_protocol::chat::ChatFormatting;
    /// assert_eq!(ChatFormatting::Red.code(), 'c');
    /// assert_eq!(ChatFormatting::Bold.code(), 'l');
    /// ```
    pub fn code(self) -> char {
        match self {
            Self::Black => '0',
            Self::DarkBlue => '1',
            Self::DarkGreen => '2',
            Self::DarkAqua => '3',
            Self::DarkRed => '4',
            Self::DarkPurple => '5',
            Self::Gold => '6',
            Self::Gray => '7',
            Self::DarkGray => '8',
            Self::Blue => '9',
            Self::Green => 'a',
            Self::Aqua => 'b',
            Self::Red => 'c',
            Self::LightPurple => 'd',
            Self::Yellow => 'e',
            Self::White => 'f',
            Self::Obfuscated => 'k',
            Self::Bold => 'l',
            Self::Strikethrough => 'm',
            Self::Underline => 'n',
            Self::Italic => 'o',
            Self::Reset => 'r',
        }
    }

    /// The full `§X` prefix string for this formatting.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidized_protocol::chat::ChatFormatting;
    /// assert_eq!(ChatFormatting::Gold.prefix(), "§6");
    /// ```
    pub fn prefix(self) -> String {
        format!("\u{00A7}{}", self.code())
    }

    /// RGB color value for color codes, or `None` for formatting-only codes.
    ///
    /// # Examples
    ///
    /// ```
    /// use oxidized_protocol::chat::ChatFormatting;
    /// assert_eq!(ChatFormatting::Red.color(), Some(0xFF5555));
    /// assert_eq!(ChatFormatting::Bold.color(), None);
    /// ```
    pub fn color(self) -> Option<u32> {
        match self {
            Self::Black => Some(0x000000),
            Self::DarkBlue => Some(0x0000AA),
            Self::DarkGreen => Some(0x00AA00),
            Self::DarkAqua => Some(0x00AAAA),
            Self::DarkRed => Some(0xAA0000),
            Self::DarkPurple => Some(0xAA00AA),
            Self::Gold => Some(0xFFAA00),
            Self::Gray => Some(0xAAAAAA),
            Self::DarkGray => Some(0x555555),
            Self::Blue => Some(0x5555FF),
            Self::Green => Some(0x55FF55),
            Self::Aqua => Some(0x55FFFF),
            Self::Red => Some(0xFF5555),
            Self::LightPurple => Some(0xFF55FF),
            Self::Yellow => Some(0xFFFF55),
            Self::White => Some(0xFFFFFF),
            _ => None,
        }
    }

    /// Returns `true` if this is a color code (not a formatting modifier).
    pub fn is_color(self) -> bool {
        self.color().is_some()
    }

    /// Returns the numeric ID used by vanilla (0–15 for colors).
    pub fn id(self) -> Option<u8> {
        match self {
            Self::Black => Some(0),
            Self::DarkBlue => Some(1),
            Self::DarkGreen => Some(2),
            Self::DarkAqua => Some(3),
            Self::DarkRed => Some(4),
            Self::DarkPurple => Some(5),
            Self::Gold => Some(6),
            Self::Gray => Some(7),
            Self::DarkGray => Some(8),
            Self::Blue => Some(9),
            Self::Green => Some(10),
            Self::Aqua => Some(11),
            Self::Red => Some(12),
            Self::LightPurple => Some(13),
            Self::Yellow => Some(14),
            Self::White => Some(15),
            _ => None,
        }
    }

    /// Parse a `§`-code character to a formatting variant.
    ///
    /// Returns `None` for unrecognized characters.
    pub fn from_code(c: char) -> Option<Self> {
        match c.to_ascii_lowercase() {
            '0' => Some(Self::Black),
            '1' => Some(Self::DarkBlue),
            '2' => Some(Self::DarkGreen),
            '3' => Some(Self::DarkAqua),
            '4' => Some(Self::DarkRed),
            '5' => Some(Self::DarkPurple),
            '6' => Some(Self::Gold),
            '7' => Some(Self::Gray),
            '8' => Some(Self::DarkGray),
            '9' => Some(Self::Blue),
            'a' => Some(Self::Green),
            'b' => Some(Self::Aqua),
            'c' => Some(Self::Red),
            'd' => Some(Self::LightPurple),
            'e' => Some(Self::Yellow),
            'f' => Some(Self::White),
            'k' => Some(Self::Obfuscated),
            'l' => Some(Self::Bold),
            'm' => Some(Self::Strikethrough),
            'n' => Some(Self::Underline),
            'o' => Some(Self::Italic),
            'r' => Some(Self::Reset),
            _ => None,
        }
    }

    /// The snake_case name used in JSON serialization (e.g. `"dark_blue"`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Black => "black",
            Self::DarkBlue => "dark_blue",
            Self::DarkGreen => "dark_green",
            Self::DarkAqua => "dark_aqua",
            Self::DarkRed => "dark_red",
            Self::DarkPurple => "dark_purple",
            Self::Gold => "gold",
            Self::Gray => "gray",
            Self::DarkGray => "dark_gray",
            Self::Blue => "blue",
            Self::Green => "green",
            Self::Aqua => "aqua",
            Self::Red => "red",
            Self::LightPurple => "light_purple",
            Self::Yellow => "yellow",
            Self::White => "white",
            Self::Obfuscated => "obfuscated",
            Self::Bold => "bold",
            Self::Strikethrough => "strikethrough",
            Self::Underline => "underline",
            Self::Italic => "italic",
            Self::Reset => "reset",
        }
    }

    /// Parse a formatting variant from its snake_case name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "black" => Some(Self::Black),
            "dark_blue" => Some(Self::DarkBlue),
            "dark_green" => Some(Self::DarkGreen),
            "dark_aqua" => Some(Self::DarkAqua),
            "dark_red" => Some(Self::DarkRed),
            "dark_purple" => Some(Self::DarkPurple),
            "gold" => Some(Self::Gold),
            "gray" => Some(Self::Gray),
            "dark_gray" => Some(Self::DarkGray),
            "blue" => Some(Self::Blue),
            "green" => Some(Self::Green),
            "aqua" => Some(Self::Aqua),
            "red" => Some(Self::Red),
            "light_purple" => Some(Self::LightPurple),
            "yellow" => Some(Self::Yellow),
            "white" => Some(Self::White),
            "obfuscated" => Some(Self::Obfuscated),
            "bold" => Some(Self::Bold),
            "strikethrough" => Some(Self::Strikethrough),
            "underline" => Some(Self::Underline),
            "italic" => Some(Self::Italic),
            "reset" => Some(Self::Reset),
            _ => None,
        }
    }
}

impl fmt::Display for ChatFormatting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\u{00A7}{}", self.code())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_code_roundtrip_all_variants() {
        for &fmt in ChatFormatting::ALL {
            let code = fmt.code();
            let parsed = ChatFormatting::from_code(code)
                .unwrap_or_else(|| panic!("from_code failed for {:?} ('{}')", fmt, code));
            assert_eq!(parsed, fmt, "roundtrip failed for {:?}", fmt);
        }
    }

    #[test]
    fn test_name_roundtrip_all_variants() {
        for &fmt in ChatFormatting::ALL {
            let name = fmt.name();
            let parsed = ChatFormatting::from_name(name)
                .unwrap_or_else(|| panic!("from_name failed for {:?} ('{}')", fmt, name));
            assert_eq!(parsed, fmt, "name roundtrip failed for {:?}", fmt);
        }
    }

    #[test]
    fn test_specific_codes() {
        assert_eq!(ChatFormatting::Red.code(), 'c');
        assert_eq!(ChatFormatting::Bold.code(), 'l');
        assert_eq!(ChatFormatting::Reset.code(), 'r');
        assert_eq!(ChatFormatting::Black.code(), '0');
        assert_eq!(ChatFormatting::White.code(), 'f');
    }

    #[test]
    fn test_prefix_contains_section_sign() {
        let prefix = ChatFormatting::Gold.prefix();
        assert!(prefix.starts_with('\u{00A7}'));
        assert!(prefix.ends_with('6'));
        assert_eq!(prefix, "§6");
    }

    #[test]
    fn test_color_values() {
        assert_eq!(ChatFormatting::Black.color(), Some(0x000000));
        assert_eq!(ChatFormatting::Red.color(), Some(0xFF5555));
        assert_eq!(ChatFormatting::White.color(), Some(0xFFFFFF));
        assert_eq!(ChatFormatting::Gold.color(), Some(0xFFAA00));
    }

    #[test]
    fn test_formatting_codes_have_no_color() {
        assert_eq!(ChatFormatting::Bold.color(), None);
        assert_eq!(ChatFormatting::Italic.color(), None);
        assert_eq!(ChatFormatting::Obfuscated.color(), None);
        assert_eq!(ChatFormatting::Strikethrough.color(), None);
        assert_eq!(ChatFormatting::Underline.color(), None);
        assert_eq!(ChatFormatting::Reset.color(), None);
    }

    #[test]
    fn test_is_color() {
        assert!(ChatFormatting::Red.is_color());
        assert!(ChatFormatting::Black.is_color());
        assert!(!ChatFormatting::Bold.is_color());
        assert!(!ChatFormatting::Reset.is_color());
    }

    #[test]
    fn test_color_ids() {
        assert_eq!(ChatFormatting::Black.id(), Some(0));
        assert_eq!(ChatFormatting::White.id(), Some(15));
        assert_eq!(ChatFormatting::Bold.id(), None);
    }

    #[test]
    fn test_from_code_case_insensitive() {
        assert_eq!(ChatFormatting::from_code('A'), Some(ChatFormatting::Green));
        assert_eq!(ChatFormatting::from_code('a'), Some(ChatFormatting::Green));
        assert_eq!(ChatFormatting::from_code('L'), Some(ChatFormatting::Bold));
    }

    #[test]
    fn test_from_code_invalid() {
        assert_eq!(ChatFormatting::from_code('z'), None);
        assert_eq!(ChatFormatting::from_code('!'), None);
        assert_eq!(ChatFormatting::from_code(' '), None);
    }

    #[test]
    fn test_display_format() {
        assert_eq!(format!("{}", ChatFormatting::Red), "§c");
        assert_eq!(format!("{}", ChatFormatting::Bold), "§l");
    }

    #[test]
    fn test_all_count() {
        assert_eq!(ChatFormatting::ALL.len(), 22, "16 colors + 6 formatting");
    }
}
